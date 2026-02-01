use crate::common::{RvOutput, RvTest};

impl RvTest {
    pub fn script_run(&self, script: &str, args: &[&str]) -> RvOutput {
        let mut run_args = vec!["run", script];
        if !args.is_empty() {
            run_args.extend(args);
        }
        self.rv(&run_args)
    }

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
}

#[test]
fn test_run_script_not_found() {
    let test = RvTest::new();
    let output = test.script_run("nonexistent.rb", &[]);

    output.assert_failure();
    assert!(output.stderr().contains("No such file or directory"));
}

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
    assert!(output.stderr().contains("NoMatchingRuby"));
}

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
