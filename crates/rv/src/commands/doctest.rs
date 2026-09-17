use crate::config::Config;
use crate::discovery;
use crate::{Error, GlobalArgs};
use clap::Parser;
use rayon::prelude::*;
use rv_doctest::{CommandRubyChecker, check_snippets, extract};
use rv_ruby_parser::ParsedFile;

pub(crate) type DoctestArgs = CommandlineOpts;

#[derive(Parser)]
pub(crate) struct CommandlineOpts {
    /// Files or directories to check. Defaults to current directory.
    #[arg(default_value = ".")]
    pub include_paths: Vec<String>,

    /// Include files ignored by .gitignore.
    #[arg(long)]
    pub include_gitignored: bool,
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

    let mut failures: Vec<(String, rv_doctest::Failure)> = Vec::new();

    if let Some(ruby) = ruby {
        let mut set = tokio::task::JoinSet::new();
        for (path, parsed_file) in &parsed {
            let snippets = extract(parsed_file);
            if !snippets.is_empty() {
                let ruby = ruby.clone();
                let path = path.clone();
                set.spawn(async move {
                    let mut checker = CommandRubyChecker::new(ruby);
                    let file_failures = check_snippets(&snippets, &mut checker).await;
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

    if failures.is_empty() {
        let paths_str = opts.include_paths.join(", ");
        println!(
            "All {total_snippets} of fenced Ruby codeblocks in {paths_str} pass syntax checks."
        );
    } else {
        for (path, failure) in &failures {
            println!(
                "{}:{}: {}",
                path, failure.snippet.start_line, failure.message
            );
            println!("{}", failure.snippet.code);
        }
    }

    Ok(())
}
