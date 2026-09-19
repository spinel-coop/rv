use std::collections::BTreeMap;
use std::io::IsTerminal;
use std::sync::atomic::{Ordering, Ordering as OrderingType};

use crate::config::Config;
use crate::discovery;
use crate::{Error, GlobalArgs};
use clap::{Parser, ValueEnum};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use rv_doctest::{
    CheckStats, Failure, RbsChecker, RbsEnvironment, Snippet, UnknownEntry, check_snippets,
    extract, syntax_check,
};
use rv_ruby_parser::ParsedFile;
use tabled::Table;
use tabled::settings::Style;

const MAX_CONCURRENT_CHECKS: usize = 20;

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

    let progress = ProgressBar::new(total_files_count as u64);
    progress.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} files ({eta} remaining)")
        .unwrap_or_else(|_| ProgressStyle::default_bar()));

    #[derive(Clone)]
    struct ParsedSourceFile {
        path: String,
        bytes: Vec<u8>,
        parsed: ParsedFile,
    }

    let parsed: Vec<ParsedSourceFile> = files
        .into_par_iter()
        .map(|(path, source)| {
            let parsed = rv_ruby_parser::parse(&source);
            ParsedSourceFile {
                path: path.to_string(),
                bytes: source,
                parsed,
            }
        })
        .collect();

    let syntax_checked_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let progress_clone = progress.clone();

    let failures: std::sync::Arc<std::sync::Mutex<Vec<(String, Failure)>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let syntax_failures: std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let total_snippets = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    if let Some(ref ruby_val) = ruby {
        futures_util::stream::iter(parsed.clone())
            .map(|pf| {
                let ParsedSourceFile {
                    path,
                    bytes: source,
                    parsed: parsed_file,
                } = pf;
                let ruby_check = ruby_val.clone();
                let progress_ref = progress_clone.clone();
                let syntax_count = syntax_checked_count.clone();
                let failures = failures.clone();
                let syntax_failures = syntax_failures.clone();
                let snippets_count = total_snippets.clone();
                async move {
                    let source_str = String::from_utf8_lossy(&source);
                    let mut file_syntax_failures: Vec<(String, String)> = Vec::new();
                    let mut file_snippet_failures: Vec<(String, Failure)> = Vec::new();

                    if let Err(e) = syntax_check(ruby_check.clone(), &source_str).await {
                        file_syntax_failures.push((path.clone(), e.to_string()));
                    }

                    let snippets = extract(&parsed_file);

                    if !snippets.is_empty() {
                        file_snippet_failures = check_snippets(&snippets, &ruby_check)
                            .await
                            .into_iter()
                            .map(|f| (path.clone(), f))
                            .collect();
                    }

                    syntax_count.fetch_add(1, OrderingType::SeqCst);
                    progress_ref.inc(1);
                    snippets_count.fetch_add(snippets.len(), OrderingType::SeqCst);

                    failures.lock().unwrap().extend(file_snippet_failures);
                    syntax_failures.lock().unwrap().extend(file_syntax_failures);
                }
            })
            .buffer_unordered(MAX_CONCURRENT_CHECKS)
            .collect::<()>()
            .await;
    } else {
        futures_util::stream::iter(parsed.clone())
            .map(|pf| {
                let ParsedSourceFile {
                    parsed: parsed_file,
                    ..
                } = pf;
                let progress_ref = progress_clone.clone();
                let syntax_count = syntax_checked_count.clone();
                let snippets_count = total_snippets.clone();
                async move {
                    let snippets = extract(&parsed_file);
                    syntax_count.fetch_add(1, OrderingType::SeqCst);
                    progress_ref.inc(1);
                    snippets_count.fetch_add(snippets.len(), OrderingType::SeqCst);
                }
            })
            .buffer_unordered(MAX_CONCURRENT_CHECKS)
            .collect::<()>()
            .await;
    }

    let mut failures = failures.lock().unwrap().clone();
    let syntax_failures = syntax_failures.lock().unwrap().clone();
    let total_snippets = total_snippets.load(Ordering::SeqCst);
    let mut stats = CheckStats::default();

    if opts.rbs.is_some() {
        let rbs_dirs: Vec<&str> = opts.rbs_dirs.iter().map(String::as_str).collect();
        let env = RbsEnvironment::load(&rbs_dirs)
            .map_err(|e| Error::IoError(std::io::Error::other(e.to_string())))?;
        let checker = RbsChecker::new(env);

        let all_snippets: Vec<_> = parsed
            .iter()
            .flat_map(|pf| {
                let ParsedSourceFile {
                    path,
                    parsed: parsed_file,
                    ..
                } = pf;
                extract(parsed_file)
                    .into_iter()
                    .map(move |snippet| (path.clone(), snippet))
            })
            .collect();

        let rbs_results: Vec<_> = all_snippets
            .into_par_iter()
            .map(|(path, snippet)| {
                let report = checker.check(&snippet, &path);
                (path, snippet, report)
            })
            .collect();

        for (path, snippet, report) in rbs_results {
            stats.merge(report.stats);
            for violation in report.violations {
                let rbs_failure = Failure {
                    snippet: Snippet {
                        item_name: format!("{}.{}", violation.class_name, violation.method_name),
                        item_kind: rv_ruby_parser::ItemKind::Def,
                        parent_path: violation.class_name.clone(),
                        start_line: violation.line,
                        code: snippet.code.clone(),
                    },
                    message: violation.message.clone(),
                    line: violation.line,
                };
                failures.push((path.clone(), rbs_failure));
            }
        }
    }

    let total_files_with_snippets: usize = parsed
        .iter()
        .filter(|ParsedSourceFile { parsed, .. }| !extract(parsed).is_empty())
        .count();

    if syntax_failures.is_empty() && failures.is_empty() {
        let file_word = if total_files_with_snippets == 1 {
            "file"
        } else {
            "files"
        };
        println!(
            "All {total_snippets} fenced Ruby codeblocks in {total_files_with_snippets} {file_word} pass checks."
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

    let syntax_checked = syntax_checked_count.load(Ordering::SeqCst);
    println!(
        "Syntax-checked {} file{}.",
        syntax_checked,
        if syntax_checked == 1 { "" } else { "s" }
    );

    if let Some(mode) = opts.rbs {
        print_coverage(&stats, mode);
    }

    Ok(())
}

fn print_coverage(stats: &CheckStats, mode: RbsMode) {
    let _skipped = stats.skipped();
    let receivers: u32 = stats.unknown_receivers.values().map(|e| e.count).sum();
    let classes: u32 = stats.unknown_classes.values().map(|e| e.count).sum();
    let methods: u32 = stats.unknown_methods.values().map(|e| e.count).sum();
    println!(
        "RBS coverage: {} checked, \
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

fn osc8_link(path_line: &str) -> String {
    let (path, _line) = path_line.rsplit_once(':').unwrap_or((path_line, ""));
    let absolute = std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string());
    format!("\x1b]8;;file://{absolute}\x1b\\{path_line}\x1b]8;;\x1b\\")
}
