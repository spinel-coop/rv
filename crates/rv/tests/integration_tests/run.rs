use crate::common::{RvOutput, RvTest};
use std::fs;

impl RvTest {
    pub fn script_run(&self, script: &str, args: &[&str]) -> RvOutput {
        let mut run_args = vec!["run", script];
        if !args.is_empty() {
            run_args.extend(args);
        }
        self.rv(&run_args)
    }

    #[cfg(unix)]
    pub fn script_run_with_ruby(&self, ruby: &str, script: &str, args: &[&str]) -> RvOutput {
        let mut run_args = vec!["run", "--ruby", ruby, script];
        if !args.is_empty() {
            run_args.extend(args);
        }
        self.rv(&run_args)
    }

    pub fn script_run_no_install(&self, script: &str) -> RvOutput {
        self.rv(&["run", "--no-install", script])
    }

    pub fn write_script(&self, name: &str, content: &str) -> String {
        let script_path = self.cwd.join(name);
        std::fs::write(&script_path, content).expect("Failed to write script");
        script_path.to_string()
    }

    pub fn run_ruby(&self, version: Option<&str>, no_install: bool, args: &[&str]) -> RvOutput {
        let mut run_args = vec!["run"];

        if no_install {
            run_args.push("--no-install");
        }
        if let Some(version) = version {
            run_args.push("--ruby");
            run_args.push(version);
        }

        self.rv(&[&run_args, ["ruby"].as_ref(), args].concat())
    }
}

#[test]
fn test_run_ruby_simple() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");
    let output = test.run_ruby(
        Some("3.3.5"),
        Default::default(),
        &["-e", "'puts \"Hello, World\"'"],
    );

    output.assert_success();
    assert!(output.stderr().is_empty());
    assert_eq!(
        output.normalized_stdout(),
        "ruby\n3.3.5\naarch64-darwin23\naarch64\ndarwin23\n\n"
    );
}

#[test]
fn test_run_ruby_interpreter_cache() {
    let mut test = RvTest::new();

    test.create_ruby_dir("ruby-3.3.5");
    test.create_ruby_dir("ruby-3.4.1");

    let cache_dir = test.enable_cache();

    let output = test.run_ruby(
        Some("3.4.1"),
        Default::default(),
        &["-e", "'puts \"Hello, World\"'"],
    );

    output.assert_success();

    let interpreters_dir = cache_dir.join("ruby-v0").join("interpreters");
    assert!(interpreters_dir.exists());

    // it should cache a single version, not both versions
    assert_eq!(
        fs::read_dir(&interpreters_dir)
            .unwrap()
            .collect::<Vec<_>>()
            .len(),
        1
    )
}

#[test]
fn test_run_ruby_default() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");
    let output = test.run_ruby(None, Default::default(), &["-e", "'puts \"Hello, World\"'"]);

    output.assert_success();
    assert!(output.stderr().is_empty());
    assert_eq!(
        output.normalized_stdout(),
        "ruby\n3.3.5\naarch64-darwin23\naarch64\ndarwin23\n\n"
    );
}

#[test]
fn test_run_ruby_default_skips_prereleases() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.4.8");
    test.create_ruby_dir("ruby-4.0.0-preview3");
    let output = test.run_ruby(None, Default::default(), &["-e", "'puts \"Hello, World\"'"]);

    output.assert_success();
    assert!(output.stderr().is_empty());
    assert_eq!(
        output.normalized_stdout(),
        "ruby\n3.4.8\naarch64-darwin23\naarch64\ndarwin23\n\n"
    );
}

#[test]
fn test_run_ruby_simple_no_install() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");

    let no_install = true;
    // This should pass because we already installed 3.3.5
    let output = test.run_ruby(
        Some("3.3.5"),
        no_install,
        &["-e", "'puts \"Hello, World\"'"],
    );

    output.assert_success();
    assert!(output.stderr().is_empty());
    assert_eq!(
        output.normalized_stdout(),
        "ruby\n3.3.5\naarch64-darwin23\naarch64\ndarwin23\n\n"
    );
}

#[test]
fn test_run_ruby_invalid_version() {
    let test = RvTest::new();
    let output = test.run_ruby(
        Some("3.4.5.6.7"),
        Default::default(),
        &["-e", "'puts \"Hello, World\"'"],
    );

    output.assert_failure();
    assert_eq!(
        output.normalized_stderr(),
        "error: invalid value '3.4.5.6.7' for '--ruby <RUBY>': Could not parse version 3.4.5.6.7, no more than 4 numbers are allowed\n\nFor more information, try '--help'.\n",
    );
}

#[test]
fn test_run_script_not_found() {
    let test = RvTest::new();

    test.create_ruby_dir("ruby-4.0.1");

    let output = test.script_run("nonexistent.rb", &[]);

    output.assert_failure();

    #[cfg(windows)]
    let expected_err = "program not found";
    #[cfg(unix)]
    let expected_err = "No such file or directory";

    output.assert_stderr_contains(expected_err);
}

#[cfg(unix)]
#[test]
fn test_run_script_with_metadata() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.4.1");

    let script = test.write_script(
        "test.rb",
        r#"# /// script
# requires-ruby = "3.4"
# ///
puts RUBY_VERSION
"#,
    );

    let output = test.script_run(&script, &[]);

    output.assert_success();
    assert_eq!(
        output.normalized_stdout(),
        "ruby\n3.4.1\naarch64-darwin23\naarch64\ndarwin23\n\n"
    );
}

#[cfg(unix)]
#[test]
fn test_run_script_with_shebang_and_metadata() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");

    let script = test.write_script(
        "test.rb",
        r#"#!/usr/bin/env rv run
# /// script
# requires-ruby = "3.3"
# ///
puts RUBY_VERSION
"#,
    );

    let output = test.script_run(&script, &[]);

    output.assert_success();
    assert_eq!(
        output.normalized_stdout(),
        "ruby\n3.3.5\naarch64-darwin23\naarch64\ndarwin23\n\n"
    );
}

#[cfg(unix)]
#[test]
fn test_run_script_without_metadata() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.4.1");

    let script = test.write_script(
        "test.rb",
        r#"puts RUBY_VERSION
"#,
    );

    let output = test.script_run(&script, &[]);

    output.assert_success();
    assert_eq!(
        output.normalized_stdout(),
        "ruby\n3.4.1\naarch64-darwin23\naarch64\ndarwin23\n\n"
    );
}

#[cfg(unix)]
#[test]
fn test_run_ruby_flag_overrides_metadata() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");
    test.create_ruby_dir("ruby-3.4.1");

    let script = test.write_script(
        "test.rb",
        r#"# /// script
# requires-ruby = "3.4"
# ///
puts RUBY_VERSION
"#,
    );

    let output = test.script_run_with_ruby("3.3", &script, &[]);

    output.assert_success();
    assert_eq!(
        output.normalized_stdout(),
        "ruby\n3.3.5\naarch64-darwin23\naarch64\ndarwin23\n\n"
    );
}

#[cfg(unix)]
#[test]
fn test_run_with_dot_ruby_version() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");
    test.create_ruby_dir("ruby-3.4.1");

    std::fs::write(test.temp_root().join(".ruby-version"), "3.3.5\n").unwrap();

    let script = test.write_script(
        "test.rb",
        r#"puts RUBY_VERSION
"#,
    );

    let output = test.script_run(&script, &[]);

    output.assert_success();
    assert_eq!(
        output.normalized_stdout(),
        "ruby\n3.3.5\naarch64-darwin23\naarch64\ndarwin23\n\n"
    );
}

#[cfg(unix)]
#[test]
fn test_run_metadata_overrides_dot_ruby_version() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");
    test.create_ruby_dir("ruby-3.4.1");

    std::fs::write(test.temp_root().join(".ruby-version"), "3.3.5\n").unwrap();

    let script = test.write_script(
        "test.rb",
        r#"# /// script
# requires-ruby = "3.4"
# ///
puts RUBY_VERSION
"#,
    );

    let output = test.script_run(&script, &[]);

    output.assert_success();
    assert_eq!(
        output.normalized_stdout(),
        "ruby\n3.4.1\naarch64-darwin23\naarch64\ndarwin23\n\n"
    );
}

#[cfg(unix)]
#[test]
fn test_run_passes_arguments_to_script() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.4.1");

    let script = test.write_script(
        "test.rb",
        r#"puts RUBY_VERSION
"#,
    );

    let output = test.script_run(&script, &["-e", "puts 'hello'"]);

    output.assert_success();
}

/// Regression test for https://github.com/spinel-coop/rv/issues/542
/// On Windows, `rv run irb` failed because Rust's Command doesn't consult
/// PATHEXT to find .cmd/.bat files. The fix resolves tool names against PATH
/// with standard Windows extensions before spawning.
#[test]
fn test_run_tool_in_path() {
    let test = RvTest::new();
    let ruby_dir = test.create_ruby_dir("ruby-4.0.1");
    test.create_tool_in_ruby_dir(&ruby_dir, "irb");

    let output = test.script_run("irb", &[]);

    output.assert_success();
    output.assert_stdout_contains("irb running");
}

#[test]
fn test_run_no_install_with_missing_ruby() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.3.5");

    let script = test.write_script(
        "test.rb",
        r#"# /// script
# requires-ruby = "3.4"
# ///
puts RUBY_VERSION
"#,
    );

    let output = test.script_run_no_install(&script);

    output.assert_failure();
    output.assert_stderr_contains("NoMatchingRuby");
}

#[cfg(unix)]
#[test]
fn test_run_jruby_metadata() {
    let test = RvTest::new();
    test.create_ruby_dir("jruby-9.4.8.0");

    let script = test.write_script(
        "test.rb",
        r#"# /// script
# requires-ruby = "jruby-9.4"
# ///
puts RUBY_VERSION
"#,
    );

    let output = test.script_run(&script, &[]);

    output.assert_success();
    assert_eq!(
        output.normalized_stdout(),
        "jruby\n9.4.8.0\naarch64-darwin23\naarch64\ndarwin23\n\n"
    );
}
