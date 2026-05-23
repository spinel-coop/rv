#![deny(warnings, missing_copy_implementations)]

use clap::Parser;
use ignore::WalkBuilder;
use ignore::gitignore::GitignoreBuilder;
use regex::Regex;
use rubyfmt::init_logger;
use similar::TextDiff;
use std::ffi::OsStr;
use std::fs::{File, OpenOptions, read};
use std::io::{self, BufRead, BufReader, IsTerminal, Read, Write};
use std::path::Path;
use std::process::{Command, exit};
use std::sync::{Arc, LazyLock, Mutex};

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
    // Errors seen when performing IO s
    IOError(io::Error, String),
    // Errors seen when grepping for files
    FileSearchFailure(ignore::Error),
}

/// Rubyfmt CLI
#[derive(Debug, Parser)]
#[clap(long_about = None)]
pub(crate) struct CommandlineOpts {
    /// Turn on check mode. This outputs diffs of inputs to STDOUT. Will exit non-zero when differences are detected.
    #[clap(short, long)]
    check: bool,

    /// Turn on to ignore gitignored files. Gitignored files are not considered by rubyfmt by default.
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

    /// Write files back in place, do not write output to STDOUT.
    #[clap(short, long, name = "in-place")]
    in_place: bool,

    /// When reading from stdin, treat the input as if it were at this path.
    /// This allows .rubyfmtignore and .gitignore patterns to be applied to stdin input.
    #[clap(long, name = "stdin-filepath", conflicts_with_all = ["include-paths", "in-place"])]
    stdin_filepath: Option<String>,

    /// Paths for rubyfmt to analyze. By default the output will be printed to STDOUT. See `--in-place` to write files back in-place.
    /// Acceptable paths are:{n}
    /// - File paths (i.e lib/foo/bar.rb){n}
    /// - Directories (i.e. lib/foo/){n}
    /// - Input files (i.e. @/tmp/files.txt). These files must contain one file path or directory per line
    ///
    /// rubyfmt will use these as input.{n}
    #[clap(name = "include-paths")]
    include_paths: Vec<String>,
}

/******************************************************/
/* Error handling                                     */
/******************************************************/

fn handle_io_error(err: io::Error, source: &str, error_exit: ErrorExit) {
    let msg = format!("Rubyfmt experienced an IO error: {}", err);
    print_error(&msg, Some(source));

    if error_exit == ErrorExit::Exit {
        exit(rubyfmt::FormatError::IOError as i32);
    }
}

fn handle_ignore_error(err: ignore::Error, error_exit: ErrorExit) {
    let msg = format!("Rubyfmt experienced an error searching for files: {}", err);
    print_error(&msg, None);
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
            print_error(msg, Some(source));
            e();
        }
        IOError(ioe) => {
            let msg = format!("Rubyfmt experienced an IO error: {}", ioe);
            print_error(&msg, Some(source));
            e();
        }
    }
}

fn print_error(msg: &str, file_path: Option<&str>) {
    let mut first_line: String = "Error!".to_string();

    if let Some(line) = file_path {
        first_line = format!("Error! source: {}", line);
    }

    eprintln!("{}\n{}", first_line, msg);
}

pub(crate) fn handle_execution_error(opts: &CommandlineOpts, err: ExecutionError) {
    let mut exit_type = ErrorExit::NoExit;
    // If include_paths are empty, this is operating on STDIN which should always exit
    if opts.fail_fast || opts.include_paths.is_empty() {
        exit_type = ErrorExit::Exit;
    }

    match err {
        ExecutionError::RubyfmtError(e, path) => handle_rubyfmt_error(e, &path, exit_type),
        ExecutionError::IOError(e, path) => handle_io_error(e, &path, exit_type),
        ExecutionError::FileSearchFailure(e) => handle_ignore_error(e, exit_type),
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

/// Check if a path should be ignored based on .gitignore and .rubyfmtignore patterns.
/// The path should be relative to the current working directory.
fn is_path_ignored(path: &Path, include_gitignored: bool) -> bool {
    let cwd = std::env::current_dir().unwrap();
    let mut builder = GitignoreBuilder::new(&cwd);

    if !include_gitignored {
        builder.add(".gitignore");
    }
    builder.add(".rubyfmtignore");

    if let Ok(gitignore) = builder.build() {
        let is_dir = path.is_dir();
        gitignore
            .matched_path_or_any_parents(path, is_dir)
            .is_ignore()
    } else {
        false
    }
}

fn file_walker_builder(include_paths: Vec<&String>, include_gitignored: bool) -> WalkBuilder {
    // WalkBuilder does not have an API for adding multiple inputs.
    // Must pass the first input to the constructor, and the tail afterwards.
    // Safe to unwrap here.
    let (include_head, include_tail) = include_paths.split_first().unwrap();
    let mut builder = WalkBuilder::new(include_head);

    for path in include_tail {
        builder.add(path);
    }

    builder.git_ignore(!include_gitignored);
    builder.add_custom_ignore_filename(".rubyfmtignore");
    builder
}

// Parse command line arguments. Expand any input files.
fn get_command_line_options(opts: CommandlineOpts) -> CommandlineOpts {
    let mut expanded_paths: Vec<String> = Vec::new();

    for path in opts.include_paths {
        // Expand input files
        if let Some(file_name) = path.strip_prefix('@') {
            match File::open(file_name) {
                Ok(file) => {
                    let buf = BufReader::new(file);
                    expanded_paths.extend(buf.lines().map(|l| l.expect("Could not parse line")));
                }
                Err(e) => handle_io_error(e, &path, ErrorExit::Exit),
            }
        } else {
            expanded_paths.push(path);
        }
    }

    CommandlineOpts {
        include_paths: expanded_paths,
        ..opts
    }
}

fn iterate_input_files(opts: &CommandlineOpts, f: InputFunc) {
    if opts.include_paths.is_empty() {
        // If not include paths are present, assume user is passing via STDIN
        let mut buffer = Vec::new();

        if io::stdin().is_terminal() {
            // Call executable with `--help` args to print help statement
            let mut command = Command::new(std::env::current_exe().unwrap());
            command.arg("--help");
            command.spawn().unwrap().wait().unwrap();
            return;
        }

        io::stdin()
            .read_to_end(&mut buffer)
            .expect("reading from stdin to not fail");

        let path = if let Some(stdin_filepath) = &opts.stdin_filepath {
            let path = Path::new(stdin_filepath);
            if is_path_ignored(path, opts.include_gitignored) {
                // Print unchanged output for ignored files unless we're in check mode
                if !opts.check {
                    puts_stdout(&buffer);
                }
                return;
            }
            path
        } else {
            Path::new("stdin")
        };

        f((path, &buffer))
    } else {
        let mut file_paths = Vec::new();
        let mut dir_paths = Vec::new();
        for path in &opts.include_paths {
            if Path::new(&path).is_file() {
                file_paths.push(path)
            } else {
                dir_paths.push(path)
            }
        }

        if !file_paths.is_empty() {
            for result in file_walker_builder(file_paths, opts.include_gitignored).build() {
                match result {
                    Ok(pp) => {
                        let file_path = pp.path();
                        match read(file_path) {
                            Ok(buffer) => f((file_path, &buffer)),
                            Err(e) => handle_execution_error(
                                opts,
                                ExecutionError::IOError(e, file_path.display().to_string()),
                            ),
                        }
                    }
                    Err(e) => handle_execution_error(opts, ExecutionError::FileSearchFailure(e)),
                }
            }
        }

        if !dir_paths.is_empty() {
            for result in file_walker_builder(dir_paths, opts.include_gitignored).build() {
                match result {
                    Ok(pp) => {
                        let file_path = pp.path();

                        if file_path.is_file()
                            && file_path.extension().and_then(OsStr::to_str) == Some("rb")
                        {
                            match read(file_path) {
                                Ok(buffer) => f((file_path, &buffer)),
                                Err(e) => handle_execution_error(
                                    opts,
                                    ExecutionError::IOError(e, file_path.display().to_string()),
                                ),
                            }
                        }
                    }
                    Err(e) => handle_execution_error(opts, ExecutionError::FileSearchFailure(e)),
                }
            }
        }
    }
}

type InputFunc<'a> = &'a dyn Fn((&Path, &[u8]));
type FormattingFunc<'a> = &'a dyn Fn((&Path, &[u8], Option<Vec<u8>>));

pub(crate) fn iterate_formatted(opts: &CommandlineOpts, f: FormattingFunc) {
    iterate_input_files(
        opts,
        &|(file_path, before)| match rubyfmt_string(opts, before) {
            Ok(r) => f((file_path, before, r)),
            Err(e) => handle_execution_error(
                opts,
                ExecutionError::RubyfmtError(e, file_path.display().to_string()),
            ),
        },
    );
}

fn puts_stdout(input: &[u8]) {
    io::stdout()
        .write_all(input)
        .expect("Could not write to stdout");
    io::stdout().flush().expect("flush works");
}

pub(crate) fn main(opts: CommandlineOpts) {
    ctrlc::set_handler(move || {
        eprintln!("`rubyfmt` process was terminated. Exiting...");
        exit(1);
    })
    .expect("Error setting Ctrl-C handler");

    let opts = get_command_line_options(opts);
    init_logger();

    match opts {
        CommandlineOpts { check: true, .. } => {
            let text_diffs: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            let errors_count: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

            iterate_input_files(
                &opts,
                &|(file_path, before)| match rubyfmt_string(&opts, before) {
                    Ok(None) => {}
                    Ok(Some(fmtted)) => {
                        let diff = TextDiff::from_lines(before, &fmtted);
                        let path_string = file_path.to_str().unwrap();
                        text_diffs.lock().unwrap().push(format!(
                            "{}",
                            diff.unified_diff().header(path_string, path_string)
                        ));
                    }
                    Err(e) => {
                        handle_rubyfmt_error(
                            e,
                            &file_path.display().to_string(),
                            ErrorExit::NoExit,
                        );
                        *errors_count.lock().unwrap() += 1;
                    }
                },
            );

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

        CommandlineOpts { in_place: true, .. } => {
            iterate_formatted(&opts, &|(file_path, before, after)| match after {
                Some(fmtted) if fmtted.ne(before) => {
                    let file_write = OpenOptions::new()
                        .write(true)
                        .truncate(true)
                        .open(file_path)
                        .and_then(|mut file| file.write_all(&fmtted));

                    match file_write {
                        Ok(_) => {}
                        Err(e) => handle_execution_error(
                            &opts,
                            ExecutionError::IOError(e, file_path.display().to_string()),
                        ),
                    }
                }
                _ => {}
            })
        }

        _ => iterate_formatted(&opts, &|(_, before, after)| match after {
            Some(fmtted) => puts_stdout(&fmtted),
            None => puts_stdout(before),
        }),
    }
}
