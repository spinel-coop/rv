use std::collections::HashMap;

use crate::common::{RvOutput, RvTest};

/// One entry of `rv doctor --format json`. The JSON shape is the stable
/// interface here; the text rendering is free to change.
#[derive(Debug, serde::Deserialize)]
struct Check {
    name: String,
    status: String,
    message: String,
    fix: Option<Fix>,
}

#[derive(Debug, serde::Deserialize)]
struct Fix {
    detail: String,
    command: Option<String>,
}

struct Report(Vec<Check>);

impl Report {
    fn of(output: &RvOutput) -> Self {
        Self(serde_json::from_str(&output.stdout()).expect("doctor should emit valid JSON"))
    }

    #[track_caller]
    fn check(&self, name: &str) -> &Check {
        self.0
            .iter()
            .find(|check| check.name == name)
            .unwrap_or_else(|| panic!("no {name:?} check in {:#?}", self.0))
    }

    fn names(&self) -> Vec<&str> {
        self.0.iter().map(|check| check.name.as_str()).collect()
    }

    fn failures(&self) -> Vec<&str> {
        self.0
            .iter()
            .filter(|check| check.status == "fail")
            .map(|check| check.name.as_str())
            .collect()
    }
}

/// The environment rv's shell integration would export, read straight out of
/// `rv shell env` so that these tests can't drift from the real hook.
///
/// The nushell renderer emits JSON, which saves parsing shell quoting.
fn activated_env(test: &RvTest) -> HashMap<String, String> {
    let output = test.rv(&["shell", "env", "nu"]);
    output.assert_success();

    let changes: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(output.stdout().trim()).expect("`shell env nu` should emit JSON");

    // An empty object means "unset this", which is already the state under test.
    changes
        .into_iter()
        .filter_map(|(var, value)| Some((var, value.as_str()?.to_owned())))
        .collect()
}

#[test]
fn test_reports_a_healthy_environment() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-3.4.1");
    test.write_ruby_version_file("3.4.1");
    let activated = activated_env(&test);
    test.env.extend(activated);

    let output = test.rv(&["doctor", "--format", "json"]);
    output.assert_success();

    let report = Report::of(&output);
    assert!(
        report.failures().is_empty(),
        "expected no failures, got {:?} in {:#?}",
        report.failures(),
        report.0
    );

    assert_eq!(report.check("ruby").status, "ok");
    assert!(
        report.check("ruby").message.contains("3.4.1"),
        "{:?}",
        report.check("ruby")
    );
    assert_eq!(report.check("ruby on PATH").status, "ok");
}

#[test]
fn test_reports_a_missing_shell_integration() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.4.1");

    // No `activated_env`: the environment is exactly what a shell without rv's
    // hook would hand us.
    let output = test.rv(&["doctor", "--format", "json"]);
    output.assert_failure();

    let report = Report::of(&output);
    assert_eq!(report.failures(), ["activation"]);

    let activation = report.check("activation");
    assert_eq!(activation.message, "no Ruby active in this environment");

    // The remaining environment checks are suppressed rather than repeating the
    // same failure once per variable.
    assert!(
        !report.names().contains(&"RUBY_ROOT"),
        "{:?}",
        report.names()
    );
}

#[test]
fn test_reports_a_ruby_that_was_uninstalled_underneath_the_shell() {
    let mut test = RvTest::new();
    let ruby_dir = test.create_ruby_dir("ruby-3.4.1");
    let activated = activated_env(&test);
    test.env.extend(activated);

    // The shell is still holding a RUBY_ROOT that no longer exists.
    fs_err::remove_dir_all(&ruby_dir).unwrap();
    test.create_ruby_dir("ruby-3.3.0");

    let output = test.rv(&["doctor", "--format", "json"]);
    output.assert_failure();

    let report = Report::of(&output);
    assert_eq!(report.failures(), ["activation"]);
    assert!(
        report.check("activation").message.contains("has no Ruby"),
        "{:?}",
        report.check("activation")
    );
}

#[test]
fn test_reports_a_pinned_ruby_that_is_not_installed() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.4.1");
    test.write_ruby_version_file("3.3.0");

    let output = test.rv(&["doctor", "--format", "json"]);
    output.assert_failure();

    let report = Report::of(&output);
    let ruby = report.check("ruby");

    assert_eq!(ruby.status, "fail");
    assert!(ruby.message.contains("3.3.0"), "{ruby:?}");
    assert_eq!(
        ruby.fix.as_ref().and_then(|fix| fix.command.as_deref()),
        Some("rv ruby install ruby-3.3.0")
    );
}

#[test]
fn test_reports_no_rubies_at_all() {
    let test = RvTest::new();

    let output = test.rv(&["doctor", "--format", "json"]);
    output.assert_failure();

    let report = Report::of(&output);
    let ruby = report.check("ruby");

    assert_eq!(ruby.status, "fail");
    assert_eq!(ruby.message, "no Ruby installations found");
    assert_eq!(
        ruby.fix.as_ref().and_then(|fix| fix.command.as_deref()),
        Some("rv ruby install")
    );

    // Nothing to activate, so activation is reported as inapplicable rather than
    // as a second failure.
    assert_eq!(report.check("activation").status, "skipped");
}

#[test]
fn test_reports_a_shim_shadowing_ruby() {
    let mut test = RvTest::new();
    test.create_ruby_dir("ruby-3.4.1");
    let activated = activated_env(&test);
    test.env.extend(activated);

    // Put an rbenv-shaped shim ahead of everything rv put on PATH.
    let shims = test.temp_root().join(".rbenv/shims");
    fs_err::create_dir_all(&shims).unwrap();
    let shim = shims.join(test.ruby_executable_name());
    fs_err::write(&shim, "#!/bin/sh\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs_err::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let path = test.env.get("PATH").cloned().unwrap_or_default();
    test.env
        .insert("PATH".into(), format!("{shims}{PATH_SEPARATOR}{path}"));

    let output = test.rv(&["doctor", "--format", "json"]);
    output.assert_failure();

    let report = Report::of(&output);
    let shadowed = report.check("ruby on PATH");

    assert_eq!(shadowed.status, "fail");
    assert!(shadowed.message.contains(".rbenv"), "{shadowed:?}");
    assert!(
        shadowed
            .fix
            .as_ref()
            .is_some_and(|fix| fix.detail.contains("rbenv is ahead of rv")),
        "{shadowed:?}"
    );
}

#[test]
fn test_reports_conflicting_version_files() {
    let test = RvTest::new();
    test.create_ruby_dir("ruby-3.4.1");
    test.write_ruby_version_file("3.4.1");
    write_lockfile_pinning(&test, "3.3.0p0");

    let output = test.rv(&["doctor", "--format", "json"]);

    let report = Report::of(&output);
    let files = report.check("version files");

    assert_eq!(files.status, "warn");
    assert!(files.message.starts_with(".ruby-version pins"), "{files:?}");
    assert!(files.message.contains("Gemfile.lock says"), "{files:?}");
}

#[test]
fn test_exit_zero_keeps_a_failing_report_from_failing_the_command() {
    let test = RvTest::new();

    test.rv(&["doctor"]).assert_failure();
    test.rv(&["doctor", "--exit-zero"]).assert_success();
}

/// A minimal `Gemfile.lock` whose `RUBY VERSION` section names `version`.
fn write_lockfile_pinning(test: &RvTest, version: &str) {
    fs_err::write(
        test.current_dir().join("Gemfile.lock"),
        format!(
            "GEM\n  remote: https://rubygems.org/\n  specs:\n\n\
             PLATFORMS\n  ruby\n\n\
             DEPENDENCIES\n\n\
             RUBY VERSION\n   ruby {version}\n\n\
             BUNDLED WITH\n   2.5.3\n"
        ),
    )
    .unwrap();
}

#[cfg(unix)]
const PATH_SEPARATOR: &str = ":";
#[cfg(windows)]
const PATH_SEPARATOR: &str = ";";
