use std::collections::BTreeMap;
/// RSpec-style doctest command for Ruby code in documentation comments.
///
/// This module provides the `rv doctest` command that validates Ruby code
/// embedded in documentation comments. It supports:
/// - Syntax validation via Ruby's `-c` check
/// - Minitest assertion execution
/// - RBS arity checking (optional)
///
/// # Example
///
/// ```ruby
/// #!/usr/bin/env ruby
/// def greet(name)
///   puts "Hello, \#{name}!"
/// end
///
/// ### Usage Example
/// greet("World")  # => "Hello, World!"
/// ```
///
/// The doctest command processes fenced ```ruby ``` blocks in documentation
/// and validates them for correctness.
use std::io::IsTerminal;

use crate::config::Config;
use crate::discovery;
use crate::{Error, GlobalArgs};
use clap::{Parser, ValueEnum};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use rv_doctest::{
    CheckStats, Failure, RbsChecker, RbsEnvironment, Snippet, UnknownEntry, check_snippets,
    extract_analyzed,
};
use rv_ruby_parser::Diagnostic;
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

/// Exit code reported when any discovered file fails a check. Distinct from the
/// `1` that `main` uses for rv's own errors, so callers can tell "your examples
/// are broken" from "rv could not run the checks".
pub(crate) const FAILURE_EXIT_CODE: i32 = 2;

/// Run the doctest checks, returning the process exit code to use: `0` when
/// everything passed, [`FAILURE_EXIT_CODE`] when any file could not be read or
/// any snippet failed a syntax, assertion, or RBS check.
pub(crate) async fn doctest(global_args: &GlobalArgs, opts: DoctestArgs) -> Result<i32, Error> {
    let expanded_paths = discovery::expand_paths(&opts.include_paths).map_err(|e| {
        Error::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Path not found: {}", e.path),
        ))
    })?;

    let discovery_result =
        discovery::discover_rb_files(&expanded_paths, opts.include_gitignored, None)?;

    let discovery_failures = discovery_result.failures;
    let total_files_count = discovery_result.files.len();

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

    struct ParsedSourceFile {
        path: String,
        diagnostics: Vec<Diagnostic>,
        snippets: Vec<(rv_doctest::Snippet, rv_doctest::SnippetAnalysis)>,
    }

    let parsed: Vec<ParsedSourceFile> = discovery_result
        .files
        .into_par_iter()
        .map(|(path, source)| {
            let parsed = rv_ruby_parser::parse(&source);
            // One parse per snippet, shared by the syntax, assertion, and RBS
            // checkers.
            let snippets = extract_analyzed(&parsed);
            ParsedSourceFile {
                path: path.to_string(),
                diagnostics: parsed.diagnostics,
                snippets,
            }
        })
        .collect();

    // Counted from the parse itself, so the totals are right whether or not
    // Ruby is available to run the snippets.
    let total_snippets: usize = parsed.iter().map(|pf| pf.snippets.len()).sum();
    let total_files_with_snippets = parsed.iter().filter(|pf| !pf.snippets.is_empty()).count();

    // Prism already reported any parse error in the file. Re-checking with
    // `ruby -c` over a lossy UTF-8 copy would spawn a process per file and
    // invent errors for sources that are not UTF-8 (a `# encoding:` magic
    // comment, binary literals) where the replacement character lands.
    let syntax_failures: Vec<(String, String)> = parsed
        .iter()
        .flat_map(|pf| {
            pf.diagnostics.iter().map(|diagnostic| {
                (
                    format!("{}:{}:{}", pf.path, diagnostic.line, diagnostic.column),
                    diagnostic.message.clone(),
                )
            })
        })
        .collect();

    let progress_clone = progress.clone();

    let failures: std::sync::Arc<std::sync::Mutex<Vec<(String, Failure)>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

    if let Some(ref ruby_val) = ruby {
        futures_util::stream::iter(&parsed)
            .map(|pf| {
                let ruby_check = ruby_val.clone();
                let progress_ref = progress_clone.clone();
                let failures = failures.clone();
                async move {
                    let file_failures: Vec<(String, Failure)> = if pf.snippets.is_empty() {
                        Vec::new()
                    } else {
                        check_snippets(&pf.snippets, &ruby_check)
                            .await
                            .into_iter()
                            .map(|f| (pf.path.clone(), f))
                            .collect()
                    };

                    progress_ref.inc(1);
                    failures.lock().unwrap().extend(file_failures);
                }
            })
            .buffer_unordered(MAX_CONCURRENT_CHECKS)
            .collect::<()>()
            .await;
    } else {
        // RBS-only: nothing to run, so there is no per-file work to await.
        progress_clone.inc(parsed.len() as u64);
    }

    progress.finish_and_clear();

    let mut failures = failures.lock().unwrap().clone();

    let mut stats = CheckStats::default();

    if opts.rbs.is_some() {
        let rbs_dirs: Vec<&str> = opts.rbs_dirs.iter().map(String::as_str).collect();
        let env = RbsEnvironment::load(&rbs_dirs)
            .map_err(|e| Error::IoError(std::io::Error::other(e.to_string())))?;
        let checker = RbsChecker::new(env);

        let all_snippets: Vec<_> = parsed
            .iter()
            .flat_map(|pf| {
                pf.snippets
                    .iter()
                    .map(|(snippet, analysis)| (pf.path.clone(), snippet, analysis))
            })
            .collect();

        let rbs_results: Vec<_> = all_snippets
            .into_par_iter()
            .map(|(path, snippet, analysis)| {
                let report = checker.check(snippet, &analysis.calls, &path);
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

    for failure in &discovery_failures {
        eprintln!("{}: could not be read: {}", failure.path, failure.error);
    }

    if syntax_failures.is_empty() && failures.is_empty() && discovery_failures.is_empty() {
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

    if let Some(rbs_mode) = opts.rbs {
        print_coverage(&stats, rbs_mode);
    }

    let failed =
        !syntax_failures.is_empty() || !failures.is_empty() || !discovery_failures.is_empty();

    Ok(if failed { FAILURE_EXIT_CODE } else { 0 })
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
