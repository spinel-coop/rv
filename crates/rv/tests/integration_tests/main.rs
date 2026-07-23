mod clean_install;
mod common;
mod ruby;
mod run;
mod shell;
mod tool;

use crate::common::RvTest;
use regex::Regex;

#[test]
fn test_no_command() {
    let test = RvTest::new();
    let result = test.rv(&["--color=always"]);
    result.assert_failure();
    let stderr = result.stderr();
    let error_re = Regex::new(r"(?s)\n  \[subcommands:.*");
    let msg = error_re.unwrap().replace(&stderr, "");
    // On Windows, clap reports the exe name as 'rv.exe' instead of 'rv'
    let msg = msg.replace("'rv.exe'", "'rv'");
    assert_eq!(
        msg,
        "error: 'rv' requires a subcommand but one was not provided",
    );
}

#[test]
fn test_global_flags() {
    let test = RvTest::new();
    let result = test.rv(&["--help"]);
    result.assert_success();
    let stdout = result.stdout();
    assert!(
        !stdout.contains("--gemfile"),
        "--gemfile should not be a global flag"
    );
}

// The `help` subcommand exists only at the top level: `rv help [NAME]` prints the
// help of the targeted command. The per-command `rv NAME help` forms are removed
// because having many ways to ask for help is confusing (see issue #431).

#[test]
fn test_help_subcommand_shows_long_help() {
    // The `help` subcommand renders *long* help, whereas `-h`/`--help` render short help.
    // Long help expands enum variant descriptions; short help only lists the bare values.
    let test = RvTest::new();

    let long = test.rv(&["help"]);
    long.assert_success();
    long.assert_stdout_contains("Use color output if the output supports it");

    let short = test.rv(&["-h"]);
    short.assert_success();
    assert!(
        !short
            .stdout()
            .contains("Use color output if the output supports it"),
        "short help (`-h`) should not expand enum variant descriptions"
    );
}

#[test]
fn test_help_subcommand_shows_nested_command_help() {
    let test = RvTest::new();
    let result = test.rv(&["help", "ruby", "pin"]);
    result.assert_success();
    result.assert_stdout_contains("Show or set the Ruby version for the current project");
    result.assert_stdout_contains("Usage: rv ruby pin");
}

#[test]
fn test_help_subcommand_removed_from_command_groups() {
    let test = RvTest::new();
    for group in ["ruby", "tool", "cache", "self"] {
        let result = test.rv(&[group, "help"]);
        result.assert_failure();
        result.assert_stderr_contains("unrecognized subcommand 'help'");
    }
}

#[test]
fn test_help_subcommand_removed_from_shell() {
    // `rv shell` takes a positional shell name (e.g. `zsh`), so `help` is rejected as
    // an invalid value rather than an unrecognized subcommand. Either way, the `help`
    // subcommand is gone and the command fails instead of printing help.
    let test = RvTest::new();
    let result = test.rv(&["shell", "help"]);
    result.assert_failure();
}

#[test]
fn test_help_not_advertised_as_command_group_subcommand() {
    let test = RvTest::new();
    for group in ["ruby", "tool", "cache", "self"] {
        let result = test.rv(&[group, "-h"]);
        result.assert_success();
        assert!(
            !result
                .stdout()
                .contains("Print this message or the help of the given subcommand(s)"),
            "`help` should not be listed as a subcommand of `rv {group}`"
        );
    }
}
