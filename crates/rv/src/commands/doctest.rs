use std::collections::BTreeMap;
use std::io::IsTerminal;

use crate::config::Config;
use crate::discovery;
use crate::{Error, GlobalArgs};
use clap::{Parser, ValueEnum};
use rayon::prelude::*;
use rv_doctest::{
    CheckStats, Failure, RbsChecker, RbsEnvironment, UnknownEntry, check_snippets, extract,
};
use rv_ruby_parser::ParsedFile;
use tabled::Table;
use tabled::settings::Style;

pub(crate) type DoctestArgs = CommandlineOpts;

#[derive(Parser)]
pub(crate) struct CommandlineOpts {
    /// Files or directories to check. Defaults to current directory.
    #[arg(default_value = ".")]
    pub include_paths: Vec<String>,

    /// Include files ignored by .gitignore.
    #[arg(long)]
    pub include_gitignored: bool,

    /// Check method call arity in fenced examples against RBS signatures.
    /// Omit the value (or pass `stat`) for a summary count; pass `verbose`
    /// to also list every unknown receiver, class, and method.
    #[arg(long, value_enum, num_args = 0..=1, default_missing_value = "stat")]
    pub rbs: Option<RbsMode>,

    /// Directories to search for .rbs files (comma-separated).
    #[arg(long, default_value = "sig", value_delimiter = ',')]
    pub rbs_dirs: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum RbsMode {
    /// Print a one-line coverage summary.
    Stat,
    /// Print the summary plus a table per unknown category.
    Verbose,
}

pub(crate) async fn doctest(global_args: &GlobalArgs, opts: DoctestArgs) -> Result<(), Error> {
    let files = discovery::discover_rb_files(&opts.include_paths, opts.include_gitignored);

    let parsed: Vec<(String, ParsedFile)> = files
        .par_iter()
        .map(|(path, source)| (path.to_string(), rv_ruby_parser::parse(source)))
        .collect();

    let total_snippets: usize = parsed
        .par_iter()
        .map(|(_, parsed_file)| extract(parsed_file).len())
        .sum();

    let ruby = Config::new(global_args, None)
        .ok()
        .and_then(|c| c.best_ruby());

    if ruby.is_none() && opts.rbs.is_none() {
        return Err(Error::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Ruby is not installed. Run `rv ruby install` or use --rbs for RBS-only checking.",
        )));
    }

    let mut failures: Vec<(String, rv_doctest::Failure)> = Vec::new();

    if let Some(ruby) = ruby {
        let mut set = tokio::task::JoinSet::new();
        for (path, parsed_file) in &parsed {
            let snippets = extract(parsed_file);
            if !snippets.is_empty() {
                let ruby = ruby.clone();
                let path = path.clone();
                set.spawn(async move {
                    let file_failures = check_snippets(&snippets, &ruby).await;
                    (path, file_failures)
                });
            }
        }
        while let Some(result) = set.join_next().await {
            let (path, file_failures) =
                result.map_err(|e| Error::IoError(std::io::Error::other(e.to_string())))?;
            for failure in file_failures {
                failures.push((path.clone(), failure));
            }
        }
    }

    let mut stats = CheckStats::default();

    if let Some(_mode) = opts.rbs {
        let rbs_dirs: Vec<&str> = opts.rbs_dirs.iter().map(String::as_str).collect();
        let env = RbsEnvironment::load(&rbs_dirs)
            .map_err(|e| Error::IoError(std::io::Error::other(e.to_string())))?;
        let checker = RbsChecker::new(env);
        for (path, parsed_file) in &parsed {
            for snippet in extract(parsed_file) {
                let report = checker.check(&snippet, path);
                stats.merge(&report.stats);
                for violation in report.violations {
                    failures.push((
                        path.clone(),
                        Failure {
                            snippet: snippet.clone(),
                            message: format!(
                                "RBS arity: {}#{}: {}",
                                violation.class_name, violation.method_name, violation.message
                            ),
                            line: violation.line,
                        },
                    ));
                }
            }
        }
    }

    if failures.is_empty() {
        let paths_str = opts.include_paths.join(", ");
        println!("All {total_snippets} of fenced Ruby codeblocks in {paths_str} pass all checks.");
    } else {
        for (path, failure) in &failures {
            println!("{}:{}: {}", path, failure.line, failure.message);
            println!("{}", failure.snippet.code);
        }
    }

    if let Some(mode) = opts.rbs {
        print_coverage(&stats, mode);
    }

    Ok(())
}

fn print_coverage(stats: &CheckStats, mode: RbsMode) {
    let skipped = stats.skipped();
    let receivers: u32 = stats.unknown_receivers.values().map(|e| e.count).sum();
    let classes: u32 = stats.unknown_classes.values().map(|e| e.count).sum();
    let methods: u32 = stats.unknown_methods.values().map(|e| e.count).sum();
    println!(
        "RBS coverage: {} checked, {skipped} skipped \
          ({receivers} unknown receiver, {classes} unknown class, {methods} unknown method)",
        stats.checked
    );

    if mode == RbsMode::Verbose {
        let linkify = std::io::stdout().is_terminal();
        print_table("Unknown receivers", &stats.unknown_receivers, linkify);
        print_table("Unknown classes", &stats.unknown_classes, linkify);
        print_table("Unknown methods", &stats.unknown_methods, linkify);
    }
}

/// Wrap a plain `path:line` string in an OSC 8 hyperlink pointing at the
/// absolute `file://` URI. The visible text stays plain `path:line`.
fn osc8_link(path_line: &str) -> String {
    let (path, _line) = path_line.rsplit_once(':').unwrap_or((path_line, ""));
    let absolute = std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string());
    format!("\x1b]8;;file://{absolute}\x1b\\{path_line}\x1b]8;;\x1b\\")
}

fn print_table(title: &str, entries: &BTreeMap<String, UnknownEntry>, linkify: bool) {
    if entries.is_empty() {
        return;
    }
    println!("{title}:");
    let mut rows: Vec<[String; 3]> = vec![[
        "Name".to_string(),
        "Count".to_string(),
        "Locations".to_string(),
    ]];
    rows.extend(entries.iter().map(|(name, entry)| {
        let locations = render_locations(&entry.locations, linkify);
        [name.clone(), entry.count.to_string(), locations]
    }));
    let table = Table::from_iter(rows).with(Style::rounded()).to_string();
    println!("{table}");
}

const MAX_LOCATIONS: usize = 3;

fn render_locations(locations: &[String], linkify: bool) -> String {
    let shown: Vec<String> = locations
        .iter()
        .take(MAX_LOCATIONS)
        .map(|loc| if linkify { osc8_link(loc) } else { loc.clone() })
        .collect();
    let extra = locations.len().saturating_sub(MAX_LOCATIONS);
    if extra > 0 {
        format!("{}\n\u{2026} (+{extra} more)", shown.join("\n"))
    } else {
        shown.join("\n")
    }
}
