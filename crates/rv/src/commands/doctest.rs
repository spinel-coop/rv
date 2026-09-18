use std::collections::BTreeMap;
use std::io::IsTerminal;

use crate::config::Config;
use crate::discovery;
use crate::{Error, GlobalArgs};
use clap::{Parser, ValueEnum};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use rv_doctest::{
    CheckStats, Failure, RbsChecker, RbsEnvironment, UnknownEntry, check_file_syntax,
    check_snippets, extract,
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
    Stat,
    Verbose,
}

pub(crate) async fn doctest(global_args: &GlobalArgs, opts: DoctestArgs) -> Result<(), Error> {
    let files = discovery::discover_rb_files(&opts.include_paths, opts.include_gitignored);
    let total_files_count = files.len();

    let ruby = Config::new(global_args, None)
        .ok()
        .and_then(|c| c.best_ruby());

    if ruby.is_none() && opts.rbs.is_none() {
        return Err(Error::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Ruby is not installed. Run `rv ruby install` or use --rbs for RBS-only checking.",
        )));
    }

    let ruby = ruby.ok_or_else(|| {
        Error::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Ruby is not installed. Run `rv ruby install` or use --rbs for RBS-only checking.",
        ))
    })?;

    let progress = ProgressBar::new(total_files_count as u64);
    progress.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({eta} remaining)")
        .unwrap_or_else(|_| ProgressStyle::default_bar()));

    let parsed: Vec<(String, Vec<u8>, ParsedFile)> = files
        .par_iter()
        .map(|(path, source)| {
            let parsed = rv_ruby_parser::parse(source);
            (path.to_string(), source.clone(), parsed)
        })
        .collect();

    let mut failures: Vec<(String, rv_doctest::Failure)> = Vec::new();
    let mut syntax_failures: Vec<(String, String)> = Vec::new();
    let mut total_snippets: usize = 0;

    if let Some(ruby) = Some(ruby.clone()) {
        let ruby_clone = ruby.clone();
        for (path, source, parsed_file) in &parsed {
            let source_str = String::from_utf8_lossy(source);

            if let Err(e) = check_file_syntax(ruby_clone.clone(), &source_str).await {
                syntax_failures.push((path.clone(), e.to_string()));
            }

            let snippets = extract(parsed_file);
            total_snippets += snippets.len();

            if !snippets.is_empty() {
                let ruby_clone = ruby.clone();
                let snippet_failures = check_snippets(&snippets, &ruby_clone).await;
                for failure in snippet_failures {
                    failures.push((path.clone(), failure));
                }
            }

            progress.inc(1);
        }
    }

    progress.finish_and_clear();

    let total_files_with_snippets: usize = parsed
        .iter()
        .filter(|(_, _, pf)| !extract(pf).is_empty())
        .count();

    let mut stats = CheckStats::default();

    if let Some(_mode) = opts.rbs {
        let rbs_dirs: Vec<&str> = opts.rbs_dirs.iter().map(String::as_str).collect();
        let env = RbsEnvironment::load(&rbs_dirs)
            .map_err(|e| Error::IoError(std::io::Error::other(e.to_string())))?;
        let checker = RbsChecker::new(env);
        for (path, _, parsed_file) in &parsed {
            for snippet in extract(parsed_file) {
                let report = checker.check(&snippet, path);
                stats.merge(report.stats);
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

    if syntax_failures.is_empty() && failures.is_empty() {
        let file_word = if total_files_with_snippets == 1 {
            "file"
        } else {
            "files"
        };
        println!(
            "All {total_snippets} fenced Ruby codeblocks in {total_files_with_snippets} {file_word} pass all checks."
        );
    } else {
        for (path, msg) in &syntax_failures {
            println!("{}: syntax error: {}", path, msg);
        }
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
        "RBS coverage: {} checked, {skipped} \
          ({receivers} unknown receiver, {classes} unknown class, {methods} unknown method)",
        stats.checked_count()
    );

    if mode == RbsMode::Verbose {
        let linkify = std::io::stdout().is_terminal();
        print_table("[RBS] Unknown receivers", &stats.unknown_receivers, linkify);
        print_table("[RBS] Unknown classes", &stats.unknown_classes, linkify);
        print_table("[RBS] Unknown methods", &stats.unknown_methods, linkify);
    }
}

fn osc8_link(path_line: &str) -> String {
    let (path, _line) = path_line.rsplit_once(':').unwrap_or((path_line, ""));
    let absolute = std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string());
    format!("\x1b]8;;file://{absolute}\x1b\\{path_line}\x1b]8;;\x1b\\")
}

fn print_table(title: &str, entries: &BTreeMap<String, UnknownEntry>, linkify: bool) {
    let _ = linkify;
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
        format!("{} \u{2026} (+{extra} more)", shown.join("\n"))
    } else {
        shown.join("\n")
    }
}
