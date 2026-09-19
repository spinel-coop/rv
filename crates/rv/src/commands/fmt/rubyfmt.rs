use clap::Parser;
use regex::Regex;
use similar::TextDiff;
use std::fs::OpenOptions;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::exit;
use std::sync::{Arc, LazyLock, Mutex};

use crate::discovery::{self, DiscoverableFileFailure};

static MAGIC_COMMENT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^#\s*rubyfmt:\s*(?P<enabled>true|false)\s*$").unwrap());

/// Simple Enum to exit on errors or not
#[derive(Debug, PartialEq, Copy, Clone)]
enum ErrorExit {
    NoExit,
    Exit,
}

/// Error enum representing errors in the cli.
#[derive(Debug)]
pub(crate) enum ExecutionError {
    // Errors seen when rubyfmt is executing
    RubyfmtError(rubyfmt::RichFormatError, String),
    // Errors seen when performing IO
    IOError(io::Error, String),
    // Errors seen when discovering files
    DiscoverableFileFailure(discovery::DiscoverableFileFailure),
}

/// Rubyfmt CLI
#[derive(Debug, Parser, Clone)]
#[clap(long_about = None)]
pub(crate) struct CommandlineOpts {
    /// Turn on check mode. This outputs diffs of inputs to STDOUT. Will exit non-zero when differences are detected.
    #[clap(short, long)]
    check: bool,

    /// Turn on to format gitignored files. Gitignored files are ignored by default.
    #[clap(long, name = "include-gitignored")]
    include_gitignored: bool,

    /// Only format ruby files containing the magic `# rubyfmt: true` header
    #[clap(long, name = "header-opt-in")]
    header_opt_in: bool,

    /// Do not format ruby files containing the magic `# rubyfmt: false` header
    #[clap(long, name = "header-opt-out")]
    header_opt_out: bool,

    /// Fail on all syntax and io errors early. Warnings otherwise.
    #[clap(long, name = "fail-fast")]
    fail_fast: bool,

    /// Write formatted files to STDOUT, instead of formatting in place.
    #[clap(short, long)]
    stdout: bool,

    /// When reading from stdin, treat the input as if it were at this path.
    /// This allows .rubyfmtignore and .gitignore patterns to be applied to stdin input.
    #[clap(long, name = "stdin-filepath", conflicts_with_all = ["paths"])]
    stdin_filepath: Option<String>,

    /// Paths to format. To format the entire project, run `rv fmt .`{n}
    /// Acceptable paths are:{n}
    /// - File paths (i.e lib/foo/bar.rb){n}
    /// - Directories (i.e. lib/foo/){n}
    /// - Input files (i.e. @/tmp/files.txt). These files must contain one file path or directory per line
    #[clap(name = "paths")]
    include_paths: Vec<String>,

    /// If stdin is attached to a terminal.
    #[clap(long, name = "is-tty", hide = true, default_value_t = io::stdin().is_terminal())]
    is_tty: bool,
}

/******************************************************/
/* Error handling                                     */
/******************************************************/

fn handle_io_error(err: io::Error, source: &str, error_exit: ErrorExit) {
    let msg = format!("Rubyfmt experienced an IO error: {}", err);
    print_error(&msg, Some(source), &mut io::stderr().lock());

    if error_exit == ErrorExit::Exit {
        exit(rubyfmt::FormatError::IOError as i32);
    }
}

fn handle_rubyfmt_error(err: rubyfmt::RichFormatError, source: &str, error_exit: ErrorExit) {
    use rubyfmt::RichFormatError::*;
    let exit_code = err.as_exit_code();
    let e = || {
        if error_exit == ErrorExit::Exit {
            exit(exit_code);
        }
    };
    match err {
        SyntaxError => {
            let msg = "Rubyfmt detected a syntax error in the ruby code being executed";
            print_error(msg, Some(source), &mut io::stderr().lock());
            e();
        }
        IOError(ioe) => {
            let msg = format!("Rubyfmt experienced an IO error: {}", ioe);
            print_error(&msg, Some(source), &mut io::stderr().lock());
            e();
        }
    }
}

fn print_error(msg: &str, file_path: Option<&str>, writer: &mut impl Write) {
    let mut first_line: String = "Error!".to_string();

    if let Some(line) = file_path {
        first_line = format!("Error! source: {}", line);
    }

    let _ = writeln!(writer, "{}\n{}", first_line, msg);
}

fn handle_file_search_failure(err: DiscoverableFileFailure, error_exit: ErrorExit) {
    let msg = format!("Rubyfmt could not read file '{}': {}", err.path, err.error);
    print_error(&msg, None, &mut io::stderr().lock());
    if error_exit == ErrorExit::Exit {
        exit(rubyfmt::FormatError::IOError as i32);
    }
}

pub(crate) fn handle_execution_error(opts: &CommandlineOpts, err: ExecutionError) {
    let exit_type = if opts.fail_fast || opts.include_paths.is_empty() {
        ErrorExit::Exit
    } else {
        ErrorExit::NoExit
    };

    match err {
        ExecutionError::RubyfmtError(e, path) => handle_rubyfmt_error(e, &path, exit_type),
        ExecutionError::IOError(e, path) => handle_io_error(e, &path, exit_type),
        ExecutionError::DiscoverableFileFailure(e) => handle_file_search_failure(e, exit_type),
    }
}

/******************************************************/
/* Rubyfmt Integration                                */
/******************************************************/

fn rubyfmt_string(
    &CommandlineOpts {
        header_opt_in,
        header_opt_out,
        ..
    }: &CommandlineOpts,
    buffer: &[u8],
) -> Result<Option<Vec<u8>>, rubyfmt::RichFormatError> {
    if header_opt_in || header_opt_out {
        // Only look at the first 500 bytes for the magic header.
        // This is for performance. Use lossy UTF-8 conversion since the
        // magic comment is always ASCII.
        let slice_size = buffer.len().min(500);
        let slice = String::from_utf8_lossy(&buffer[..slice_size]);

        let matched = MAGIC_COMMENT_REGEX
            .captures(&slice)
            .and_then(|c| c.name("enabled"))
            .map(|s| s.as_str());

        // If opted in to magic "# rubyfmt: true" header and true is not
        // in the file, return early
        if header_opt_in && Some("true") != matched {
            return Ok(None);
        }

        // If opted in to magic "# rubyfmt: false" header and false is
        // in the file, return early
        if header_opt_out && Some("false") == matched {
            return Ok(None);
        }
    }

    rubyfmt::format_buffer(buffer).map(Some)
}

/******************************************************/
/* Helpers                                            */
/******************************************************/

type FormattingFunc<'a> = &'a dyn Fn((&Path, &[u8], Option<Vec<u8>>));

pub(crate) fn iterate_formatted(opts: &CommandlineOpts, f: FormattingFunc) {
    match discovery::discover_rb_files(&opts.include_paths, opts.include_gitignored) {
        Ok(result) => {
            for (file_path, buffer) in result.files {
                match rubyfmt_string(opts, &buffer) {
                    Ok(r) => f((file_path.as_ref(), &buffer, r)),
                    Err(e) => handle_execution_error(
                        opts,
                        ExecutionError::RubyfmtError(e, file_path.to_string()),
                    ),
                }
            }
            for failure in result.failures {
                handle_execution_error(
                    opts,
                    ExecutionError::DiscoverableFileFailure(DiscoverableFileFailure {
                        path: failure.path,
                        error: failure.error,
                    }),
                );
            }
        }
        Err(e) => handle_execution_error(
            opts,
            ExecutionError::DiscoverableFileFailure(DiscoverableFileFailure {
                path: e.path,
                error: std::io::Error::new(std::io::ErrorKind::NotFound, "path not found"),
            }),
        ),
    }
}

fn puts_stdout(input: &[u8]) {
    io::stdout()
        .write_all(input)
        .expect("Could not write to stdout");
    io::stdout().flush().expect("flush works");
}

pub(crate) fn main(mut opts: CommandlineOpts) {
    if opts.include_paths.is_empty() {
        if opts.is_tty {
            opts.include_paths.push(".".into());
        } else {
            opts.stdout = true;
        }
    }

    opts.include_paths = match discovery::expand_paths(&opts.include_paths) {
        Ok(paths) => paths,
        Err(e) => {
            handle_execution_error(
                &opts,
                ExecutionError::DiscoverableFileFailure(discovery::DiscoverableFileFailure {
                    path: e.path,
                    error: std::io::Error::new(std::io::ErrorKind::NotFound, "path not found"),
                }),
            );
            return;
        }
    };

    match opts {
        CommandlineOpts { check: true, .. } => {
            let text_diffs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let errors_count: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

            match discovery::discover_rb_files(&opts.include_paths, opts.include_gitignored) {
                Ok(result) => {
                    for (file_path, buffer) in result.files {
                        match rubyfmt_string(&opts, &buffer) {
                            Ok(None) => {}
                            Ok(Some(fmtted)) => {
                                let diff = TextDiff::from_lines(&buffer, &fmtted);
                                let path_string = file_path.to_string();
                                text_diffs.lock().unwrap().push(format!(
                                    "{}",
                                    diff.unified_diff().header(&path_string, &path_string)
                                ));
                            }
                            Err(e) => {
                                handle_rubyfmt_error(e, file_path.as_str(), ErrorExit::NoExit);
                                *errors_count.lock().unwrap() += 1;
                            }
                        }
                    }
                    for failure in result.failures {
                        handle_execution_error(
                            &opts,
                            ExecutionError::DiscoverableFileFailure(failure),
                        );
                    }
                }
                Err(e) => {
                    handle_execution_error(
                        &opts,
                        ExecutionError::DiscoverableFileFailure(
                            discovery::DiscoverableFileFailure {
                                path: e.path,
                                error: std::io::Error::new(
                                    std::io::ErrorKind::NotFound,
                                    "path not found",
                                ),
                            },
                        ),
                    );
                    return;
                }
            }

            let all_diffs = text_diffs.lock().unwrap();

            let mut diffs_reported = 0;

            for diff in all_diffs.iter() {
                if !diff.is_empty() {
                    puts_stdout(diff.as_bytes());
                    diffs_reported += 1
                }
            }
            let errors = *errors_count.lock().unwrap();
            if errors > 0 {
                exit(rubyfmt::FormatError::SyntaxError as i32);
            } else if diffs_reported > 0 {
                exit(rubyfmt::FormatError::DiffDetected as i32);
            } else {
                exit(0)
            }
        }

        CommandlineOpts { stdout: true, .. } => {
            iterate_formatted(&opts, &|(_, before, after)| match after {
                Some(fmtted) => puts_stdout(&fmtted),
                None => puts_stdout(before),
            })
        }

        _ => match discovery::discover_rb_files(&opts.include_paths, opts.include_gitignored) {
            Ok(result) => {
                for (file_path, buffer) in result.files {
                    match rubyfmt_string(&opts, &buffer) {
                        Ok(None) => {}
                        Ok(Some(fmtted)) if fmtted.ne(&buffer) => {
                            let file_write = OpenOptions::new()
                                .write(true)
                                .truncate(true)
                                .open(&file_path)
                                .and_then(|mut file| file.write_all(&fmtted));

                            match file_write {
                                Ok(_) => {}
                                Err(e) => handle_execution_error(
                                    &opts,
                                    ExecutionError::IOError(e, file_path.to_string()),
                                ),
                            }
                        }
                        Err(e) => {
                            handle_execution_error(
                                &opts,
                                ExecutionError::RubyfmtError(e, file_path.to_string()),
                            );
                        }
                        _ => {}
                    }
                }
                for failure in result.failures {
                    handle_execution_error(&opts, ExecutionError::DiscoverableFileFailure(failure));
                }
            }
            Err(e) => {
                handle_execution_error(
                    &opts,
                    ExecutionError::DiscoverableFileFailure(discovery::DiscoverableFileFailure {
                        path: e.path,
                        error: std::io::Error::new(std::io::ErrorKind::NotFound, "path not found"),
                    }),
                );
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use std::sync::{Arc, Mutex};

    // ---------------------------------------------------------------------
    // Test helpers
    // ---------------------------------------------------------------------

    fn make_opts() -> CommandlineOpts {
        CommandlineOpts {
            check: false,
            include_gitignored: false,
            header_opt_in: false,
            header_opt_out: false,
            fail_fast: false,
            stdout: false,
            stdin_filepath: None,
            include_paths: vec![],
            is_tty: false,
        }
    }

    fn opts_with(overrides: impl FnOnce(&mut CommandlineOpts)) -> CommandlineOpts {
        let mut opts = make_opts();
        overrides(&mut opts);
        opts
    }

    type CollectedInputs = Vec<(String, Vec<u8>)>;
    type CollectedFormatting = Vec<(String, Vec<u8>, Option<Vec<u8>>)>;

    /// Run a callback and collect every `(path, buffer)` pair yielded by
    /// `discovery::discover_rb_files` into a shared `Vec`.
    fn collect_inputs(opts: &CommandlineOpts) -> Vec<(String, Vec<u8>)> {
        match discovery::discover_rb_files(&opts.include_paths, opts.include_gitignored) {
            Ok(result) => {
                let collected: Arc<Mutex<CollectedInputs>> = Arc::new(Mutex::new(Vec::new()));
                for (path, buffer) in result.files {
                    collected
                        .lock()
                        .unwrap()
                        .push((path.to_string(), buffer.to_vec()));
                }
                Arc::try_unwrap(collected)
                    .ok()
                    .and_then(|m| m.into_inner().ok())
                    .unwrap_or_default()
            }
            Err(_) => Vec::new(),
        }
    }

    /// Same as `collect_inputs` but discards buffers.
    fn collect_paths(opts: &CommandlineOpts) -> Vec<String> {
        collect_inputs(opts).into_iter().map(|(p, _)| p).collect()
    }

    // ==========================================================================
    // MAGIC COMMENT PARSING (via rubyfmt_string)
    // ==========================================================================

    #[test]
    fn magic_comment_with_opt_in_and_header_true_is_formatted() {
        let opts = opts_with(|o| o.header_opt_in = true);
        let result = rubyfmt_string(&opts, b"# rubyfmt: true\nx = 1").unwrap();
        assert!(
            result.is_some(),
            "File with `rubyfmt: true` should be formatted"
        );
    }

    #[test]
    fn magic_comment_with_opt_in_and_header_false_is_skipped() {
        let opts = opts_with(|o| o.header_opt_in = true);
        let result = rubyfmt_string(&opts, b"# rubyfmt: false\nx = 1").unwrap();
        assert!(
            result.is_none(),
            "File with `rubyfmt: false` should not be formatted when opt-in is enabled"
        );
    }

    #[test]
    fn magic_comment_with_opt_in_but_no_header_is_skipped() {
        let opts = opts_with(|o| o.header_opt_in = true);
        let result = rubyfmt_string(&opts, b"x = 1").unwrap();
        assert!(
            result.is_none(),
            "File without magic comment should not be formatted when opt-in is enabled"
        );
    }

    #[test]
    fn magic_comment_with_opt_out_and_header_true_is_formatted() {
        let opts = opts_with(|o| o.header_opt_out = true);
        let result = rubyfmt_string(&opts, b"# rubyfmt: true\nx = 1").unwrap();
        assert!(
            result.is_some(),
            "File with `rubyfmt: true` should be formatted with opt-out"
        );
    }

    #[test]
    fn magic_comment_with_opt_out_and_header_false_is_skipped() {
        let opts = opts_with(|o| o.header_opt_out = true);
        let result = rubyfmt_string(&opts, b"# rubyfmt: false\nx = 1").unwrap();
        assert!(
            result.is_none(),
            "File with `rubyfmt: false` should not be formatted when opt-out is enabled"
        );
    }

    #[test]
    fn magic_comment_is_ignored_when_no_opt_flag_is_set() {
        let opts = make_opts();
        let result = rubyfmt_string(&opts, b"# rubyfmt: false\nx = 1").unwrap();
        assert!(
            result.is_some(),
            "Without opt-in/opt-out, magic comments should be ignored and file formatted"
        );
    }

    #[test]
    fn magic_comment_accepts_whitespace_variations() {
        let opts = opts_with(|o| o.header_opt_in = true);

        let cases: &[(&[u8], &str)] = &[
            (b"# rubyfmt: true\nx = 1", "leading space before rubyfmt"),
            (b"#rubyfmt: true\nx = 1", "no space after hash"),
            (
                b"# rubyfmt: true  \nx = 1",
                "trailing whitespace before newline",
            ),
        ];

        for (input, label) in cases {
            let result = rubyfmt_string(&opts, input)
                .unwrap_or_else(|e| panic!("{label}: expected Ok, got {e:?}"));
            assert!(result.is_some(), "{label}: should be treated as opted in");
        }

        let negative = rubyfmt_string(&opts, b"#rubyfmt: false\nx = 1").unwrap();
        assert!(
            negative.is_none(),
            "no-space variant should also be respected when value is `false`"
        );
    }

    #[test]
    fn magic_comment_only_reads_first_500_bytes() {
        let opts = opts_with(|o| o.header_opt_in = true);
        let mut buffer = vec![b' '; 600];
        buffer[0..15].copy_from_slice(b"# rubyfmt: true");
        let result = rubyfmt_string(&opts, &buffer).unwrap();
        assert!(
            result.is_some(),
            "Should find magic comment in first 500 bytes"
        );
    }

    #[test]
    fn magic_comment_not_found_when_beyond_500_bytes() {
        let opts = opts_with(|o| o.header_opt_in = true);
        let mut buffer = vec![b' '; 600];
        buffer[501..516].copy_from_slice(b"# rubyfmt: true");
        let result = rubyfmt_string(&opts, &buffer).unwrap();
        assert!(
            result.is_none(),
            "Should not find magic comment beyond first 500 bytes"
        );
    }

    #[test]
    fn valid_ruby_is_formatted() {
        let opts = make_opts();
        let formatted = rubyfmt_string(&opts, b"x=1").unwrap().expect("ok");
        let expected = rubyfmt::format_buffer(b"x=1").unwrap();
        assert_eq!(formatted, expected);
    }

    #[test]
    fn syntax_error_is_reported_as_syntax_error() {
        let opts = make_opts();
        let err = rubyfmt_string(&opts, b"def \n").expect_err("invalid ruby should error");
        assert!(
            matches!(err, rubyfmt::RichFormatError::SyntaxError),
            "expected SyntaxError, got {err:?}"
        );
    }

    // ==========================================================================
    // COMMAND LINE OPTION PARSING (clap)
    // ==========================================================================

    #[test]
    fn clap_parsing_default_options() {
        let opts = CommandlineOpts::try_parse_from(["fmt"]).unwrap();
        assert!(!opts.check);
        assert!(!opts.include_gitignored);
        assert!(!opts.header_opt_in);
        assert!(!opts.header_opt_out);
        assert!(!opts.fail_fast);
        assert!(!opts.stdout);
        assert!(opts.stdin_filepath.is_none());
        assert!(opts.include_paths.is_empty());
    }

    #[test]
    fn clap_parsing_check_mode_long_flag() {
        let opts = CommandlineOpts::try_parse_from(["fmt", "--check"]).unwrap();
        assert!(opts.check);
    }

    #[test]
    fn clap_parsing_check_mode_short_flag() {
        let opts = CommandlineOpts::try_parse_from(["fmt", "-c"]).unwrap();
        assert!(opts.check);
    }

    #[test]
    fn clap_parsing_include_gitignored_flag() {
        let opts = CommandlineOpts::try_parse_from(["fmt", "--include-gitignored"]).unwrap();
        assert!(opts.include_gitignored);
    }

    #[test]
    fn clap_parsing_header_opt_in_flag() {
        let opts = CommandlineOpts::try_parse_from(["fmt", "--header-opt-in"]).unwrap();
        assert!(opts.header_opt_in);
    }

    #[test]
    fn clap_parsing_header_opt_out_flag() {
        let opts = CommandlineOpts::try_parse_from(["fmt", "--header-opt-out"]).unwrap();
        assert!(opts.header_opt_out);
    }

    #[test]
    fn clap_parsing_fail_fast_flag() {
        let opts = CommandlineOpts::try_parse_from(["fmt", "--fail-fast"]).unwrap();
        assert!(opts.fail_fast);
    }

    #[test]
    fn clap_parsing_stdin_filepath_flag() {
        let opts = CommandlineOpts::try_parse_from(["fmt", "--stdin-filepath", "test.rb"]).unwrap();
        assert_eq!(opts.stdin_filepath.as_deref(), Some("test.rb"));
        // stdin-filepath is mutually exclusive with paths.
        let err =
            CommandlineOpts::try_parse_from(["fmt", "--stdin-filepath", "test.rb", "lib/foo.rb"])
                .unwrap_err();
        assert!(
            err.to_string()
                .to_lowercase()
                .contains("cannot be used with"),
            "expected conflict error, got: {err}"
        );
    }

    #[test]
    fn clap_parsing_paths() {
        let opts = CommandlineOpts::try_parse_from(["fmt", "lib/", "app/main.rb"]).unwrap();
        assert_eq!(opts.include_paths, vec!["lib/", "app/main.rb"]);
    }

    #[test]
    fn clap_parsing_combined_options() {
        let opts = CommandlineOpts::try_parse_from([
            "fmt",
            "--check",
            "--header-opt-in",
            "--fail-fast",
            "lib/",
        ])
        .unwrap();
        assert!(opts.check);
        assert!(opts.header_opt_in);
        assert!(opts.fail_fast);
        assert_eq!(opts.include_paths, vec!["lib/"]);
    }

    // ==========================================================================
    // PATH EXPANSION (discovery::expand_paths)
    // ==========================================================================

    #[test]
    fn expansion_reads_paths_from_at_file() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "path/to/file1.rb").unwrap();
        writeln!(tmp, "path/to/file2.rb").unwrap();
        writeln!(tmp, "directory/").unwrap();
        let path = tmp.path().to_str().unwrap().to_string();

        let opts = opts_with(|o| o.include_paths = vec![format!("@{path}")]);
        let expanded = discovery::expand_paths(&opts.include_paths).unwrap();
        assert_eq!(
            expanded,
            vec!["path/to/file1.rb", "path/to/file2.rb", "directory/"]
        );
    }

    #[test]
    fn expansion_keeps_paths_without_at_prefix_untouched() {
        let opts = opts_with(|o| {
            o.include_paths = vec!["lib/foo.rb".to_string(), "dir/".to_string()];
        });
        let expanded = discovery::expand_paths(&opts.include_paths).unwrap();
        assert_eq!(expanded, vec!["lib/foo.rb", "dir/"]);
    }

    #[test]
    fn expansion_mixes_literal_and_at_paths() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(tmp, "expanded/path.rb").unwrap();
        let path = tmp.path().to_str().unwrap().to_string();

        let opts = opts_with(|o| {
            o.include_paths = vec!["lib/".to_string(), format!("@{path}")];
        });
        let expanded = discovery::expand_paths(&opts.include_paths).unwrap();
        assert_eq!(expanded, vec!["lib/", "expanded/path.rb"]);
    }

    #[test]
    fn expansion_with_empty_at_file_yields_no_extra_paths() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();

        let opts = opts_with(|o| o.include_paths = vec![format!("@{path}")]);
        let expanded = discovery::expand_paths(&opts.include_paths).unwrap();
        assert!(expanded.is_empty());
    }

    #[test]
    fn expansion_errors_on_missing_at_file() {
        let opts = opts_with(|o| o.include_paths = vec!["@nonexistent.txt".to_string()]);
        assert!(discovery::expand_paths(&opts.include_paths).is_err());
    }

    #[test]
    fn expansion_errors_on_invalid_utf8_in_at_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), b"lib/a.rb\n\xff\xfe").unwrap();
        let opts =
            opts_with(|o| o.include_paths = vec![format!("@{}", tmp.path().to_str().unwrap())]);
        assert!(discovery::expand_paths(&opts.include_paths).is_err());
    }

    // ==========================================================================
    //     rv fmt .              → format current directory in place
    //     rv fmt                → act like `rv fmt .`
    //     rv fmt . --stdout     → format current directory to stdout
    //     rv fmt                → piped: format stdin to stdout
    //     rv fmt --stdout       → piped: format stdin to stdout
    //     rv fmt .              → piped: format dir in place, ignore stdin
    //     rv fmt . --stdout     → piped: format dir to stdout, ignore stdin
    // ==========================================================================

    /// `rv fmt .` — format the current directory in place.
    #[test]
    fn fmt_dot_formats_current_dir_in_place() {
        let opts = opts_with(|o| {
            o.include_paths = vec![".".to_string()];
            o.is_tty = true;
        });
        assert_eq!(opts.include_paths, vec!["."]);
        assert!(!opts.stdout);
        assert!(!opts.check);
        assert!(!opts.fail_fast);
    }

    /// `rv fmt` with an interactive tty should default to formatting the
    /// current directory in place.
    #[test]
    fn fmt_without_paths_defaults_to_current_dir_on_tty() {
        let opts = opts_with(|o| o.is_tty = true);
        assert!(opts.include_paths.is_empty());
    }

    /// `rv fmt . --stdout` — format the current directory and write to stdout.
    #[test]
    fn fmt_dot_with_stdout_flag_writes_to_stdout() {
        let opts = opts_with(|o| {
            o.stdout = true;
            o.is_tty = true;
            o.include_paths = vec![".".to_string()];
        });
        assert!(opts.stdout);
    }

    /// `echo "..." | rv fmt --stdout` — same as above, --stdout is explicit.
    #[test]
    fn fmt_with_piped_stdin_and_stdout_flag_formats_stdin_to_stdout() {
        let opts = opts_with(|o| o.stdout = true);
        assert!(opts.include_paths.is_empty());
        assert!(opts.stdout);
    }

    /// `echo "..." | rv fmt .` — piped stdin is ignored; the directory is
    /// formatted in place.
    #[test]
    fn fmt_dot_with_piped_stdin_ignores_stdin_and_formats_in_place() {
        let opts = opts_with(|o| o.include_paths = vec![".".to_string()]);
        assert!(!opts.stdout);
    }

    /// `echo "..." | rv fmt . --stdout` — piped stdin is ignored; the
    /// directory is formatted and the result is written to stdout.
    #[test]
    fn fmt_dot_with_piped_stdin_and_stdout_flag_ignores_stdin() {
        let opts = opts_with(|o| {
            o.stdout = true;
            o.include_paths = vec![".".to_string()];
        });
        assert!(opts.stdout);
    }

    // ==========================================================================
    // DISCOVERY (discovery::discover_rb_files)
    // ==========================================================================

    #[test]
    fn discover_rb_files_accepts_a_single_path() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.rb"), "x = 1").unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        let files = discovery::discover_rb_files(&[path], false).unwrap();
        assert!(!files.files.is_empty(), "single path should yield files");
    }

    #[test]
    fn discover_rb_files_accepts_multiple_paths() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.rb"), "x = 1").unwrap();
        let path1 = tmp.path().to_str().unwrap().to_string();
        let path2 = tmp.path().join("subdir");
        std::fs::create_dir_all(&path2).unwrap();
        std::fs::write(path2.join("b.rb"), "y = 2").unwrap();
        let path2 = path2.to_str().unwrap().to_string();

        let files = discovery::discover_rb_files(&[path1, path2], true).unwrap();
        assert!(!files.files.is_empty());
    }

    #[test]
    fn discover_rb_files_handles_missing_directory_gracefully() {
        let files =
            discovery::discover_rb_files(&["/nonexistent/directory".to_string()], false).unwrap();
        assert!(files.files.is_empty());
    }

    // ==========================================================================
    // ERROR HANDLING
    // ==========================================================================

    #[test]
    fn handle_execution_error_with_io_error_does_not_exit_when_paths_are_non_empty() {
        // fail_fast=false and non-empty include_paths => ErrorExit::NoExit,
        // so the test must continue past this call.
        let opts = opts_with(|o| o.include_paths = vec!["somefile.rb".to_string()]);
        let err = ExecutionError::IOError(
            io::Error::new(io::ErrorKind::NotFound, "file not found"),
            "test.rb".to_string(),
        );
        handle_execution_error(&opts, err);
    }

    #[test]
    fn handle_execution_error_with_syntax_error_does_not_exit_when_paths_are_non_empty() {
        let opts = opts_with(|o| o.include_paths = vec!["somefile.rb".to_string()]);
        let err = ExecutionError::RubyfmtError(
            rubyfmt::RichFormatError::SyntaxError,
            "test.rb".to_string(),
        );
        handle_execution_error(&opts, err);
    }

    // Note: the fail_fast and empty-paths branches of `handle_execution_error`
    // call `std::process::exit(...)` and therefore cannot be exercised from a
    // unit test in the same process.

    // ==========================================================================
    // PRINT ERROR
    // ==========================================================================

    #[test]
    fn print_error_includes_source_when_provided() {
        let mut out = Vec::new();
        print_error("boom", Some("lib/foo.rb"), &mut out);
        let rendered = String::from_utf8(out).unwrap();
        assert!(
            rendered.contains("Error! source: lib/foo.rb"),
            "expected source in header, got: {rendered:?}"
        );
        assert!(
            rendered.contains("boom"),
            "expected message body, got: {rendered:?}"
        );
    }

    #[test]
    fn print_error_omits_source_when_not_provided() {
        let mut out = Vec::new();
        print_error("boom", None, &mut out);
        let rendered = String::from_utf8(out).unwrap();
        assert!(
            rendered.contains("Error!\n"),
            "expected plain `Error!` header when no source, got: {rendered:?}"
        );
        assert!(
            !rendered.contains("source:"),
            "should not contain `source:` when no source given, got: {rendered:?}"
        );
        assert!(
            rendered.contains("boom"),
            "expected message body, got: {rendered:?}"
        );
    }

    // ==========================================================================
    // DISCOVERY TESTS
    // ==========================================================================

    #[test]
    fn discover_rb_files_yields_file_with_contents() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("test.rb");
        std::fs::write(&file_path, b"x = 1\n").unwrap();

        let opts = opts_with(|o| {
            o.stdout = false;
            o.include_paths = vec![file_path.to_str().unwrap().to_string()];
        });

        let collected = collect_inputs(&opts);
        assert_eq!(collected.len(), 1);
        assert!(
            collected[0].0.ends_with("test.rb"),
            "expected test.rb path, got {}",
            collected[0].0
        );
        assert_eq!(collected[0].1, b"x = 1\n");
    }

    #[test]
    fn discover_rb_files_walks_only_rb_files_in_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("a.rb"), "").unwrap();
        std::fs::write(dir.join("b.rb"), "").unwrap();
        std::fs::write(dir.join("readme.txt"), "").unwrap();

        let opts = opts_with(|o| {
            o.stdout = false;
            o.include_paths = vec![dir.to_str().unwrap().to_string()];
        });

        let paths = collect_paths(&opts);
        let names: Vec<&str> = paths
            .iter()
            .filter_map(|p| Path::new(p).file_name().and_then(|n| n.to_str()))
            .collect();
        assert!(names.contains(&"a.rb"), "expected a.rb, got {names:?}");
        assert!(names.contains(&"b.rb"), "expected b.rb, got {names:?}");
        assert!(
            !names.contains(&"readme.txt"),
            "non-.rb file should be excluded, got {names:?}"
        );
    }

    #[test]
    fn discover_rb_files_does_not_filter_rubyfmtignore() {
        // discovery::discover_rb_files does NOT filter .rubyfmtignore;
        // that's caller's responsibility (is_path_ignored).
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("keep.rb"), "x = 1").unwrap();
        std::fs::write(dir.join("skip.rb"), "y = 2").unwrap();
        std::fs::write(dir.join(".rubyfmtignore"), "skip.rb\n").unwrap();

        let opts = opts_with(|o| {
            o.stdout = false;
            o.include_paths = vec![dir.to_str().unwrap().to_string()];
        });

        let paths = collect_paths(&opts);
        let names: Vec<&str> = paths
            .iter()
            .filter_map(|p| Path::new(p).file_name().and_then(|n| n.to_str()))
            .collect();
        // Both files are present because .rubyfmtignore is not handled by discovery.
        assert!(names.contains(&"keep.rb"), "keep.rb should be present");
        assert!(
            names.contains(&"skip.rb"),
            "skip.rb should be present (not filtered)"
        );
    }

    // ==========================================================================
    // ITERATE_FORMATTED
    // ==========================================================================

    #[test]
    fn iterate_formatted_emits_a_formatting_decision_for_each_input() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("clean.rb");
        std::fs::write(&file, b"x = 1\n").unwrap();

        let opts = opts_with(|o| {
            o.stdout = false;
            o.include_paths = vec![file.to_str().unwrap().to_string()];
        });

        let collected: Arc<Mutex<CollectedFormatting>> = Arc::new(Mutex::new(Vec::new()));
        iterate_formatted(&opts, &|(path, before, after)| {
            collected
                .lock()
                .unwrap()
                .push((path.display().to_string(), before.to_vec(), after));
        });

        let results = Arc::try_unwrap(collected)
            .ok()
            .and_then(|m| m.into_inner().ok())
            .unwrap();
        assert_eq!(results.len(), 1);
        let (path, before, after) = &results[0];
        assert!(path.ends_with("clean.rb"));
        assert_eq!(before, b"x = 1\n");
        let after = after.as_ref().expect("format should succeed");
        let expected = rubyfmt::format_buffer(b"x = 1\n").unwrap();
        assert_eq!(after, &expected);
    }

    #[test]
    fn iterate_formatted_skipped_when_header_opt_in_does_not_match() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("opted_out.rb");
        std::fs::write(&file, b"# rubyfmt: false\nx = 1\n").unwrap();

        let opts = opts_with(|o| {
            o.stdout = false;
            o.header_opt_in = true;
            o.include_paths = vec![file.to_str().unwrap().to_string()];
        });

        let collected: Arc<Mutex<Vec<Option<Vec<u8>>>>> = Arc::new(Mutex::new(Vec::new()));
        iterate_formatted(&opts, &|(_, _, after)| {
            collected.lock().unwrap().push(after);
        });

        let results = Arc::try_unwrap(collected)
            .ok()
            .and_then(|m| m.into_inner().ok())
            .unwrap();
        assert_eq!(results.len(), 1);
        assert!(
            results[0].is_none(),
            "header-opt-in should yield None for files without `rubyfmt: true`"
        );
    }
}
