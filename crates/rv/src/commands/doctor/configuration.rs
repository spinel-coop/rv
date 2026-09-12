//! Checks on the files that configure rv: `rv.kdl`, bundler's config, and the
//! version files that decide which Ruby a directory gets.

use rv_ruby::request::Source;

use super::{Check, Section};
use crate::GlobalArgs;
use crate::config::{Config, bundler_settings::BundlerSettings, rv_settings::RvSettings};

const SECTION: Section = Section::Configuration;

pub(super) fn check(global_args: &GlobalArgs, config: &Config) -> Vec<Check> {
    let mut checks = vec![rv_settings(global_args, config), bundler_settings(config)];
    checks.extend(overruled_pins(config));
    checks
}

/// `Config::with_settings` treats a broken `rv.kdl` as fatal, so most commands
/// die before they can explain it. Report it as a check instead.
fn rv_settings(global_args: &GlobalArgs, config: &Config) -> Check {
    let home_dir = rv_dirs::home_dir();

    let loaded = RvSettings::new(global_args, &home_dir, &config.project_root)
        .and_then(|settings| settings.validate().map(|()| settings));

    match loaded {
        Ok(settings) => Check::ok(
            SECTION,
            "rv settings",
            match settings.install_path {
                Some(path) => format!("install-path {path}"),
                None => "defaults".to_string(),
            },
        ),
        Err(err) => Check::fail(SECTION, "rv settings", err.to_string())
            .suggest("Correct your rv.kdl. Until then, every rv command will fail."),
    }
}

/// The opposite problem: `Config::with_settings` *swallows* bundler errors and
/// falls back to defaults, so a typo silently changes where gems get installed.
fn bundler_settings(config: &Config) -> Check {
    let home_dir = rv_dirs::home_dir();

    match BundlerSettings::new(&home_dir, &config.project_root) {
        Ok(settings) => Check::ok(
            SECTION,
            "bundler settings",
            match settings.path() {
                Some(path) => format!("path {}", rv_dirs::unexpand(&path)),
                None => "defaults".to_string(),
            },
        ),
        Err(err) => Check::warn(SECTION, "bundler settings", err.to_string()).suggest(
            "rv is falling back to its defaults, so gems may not land where \
             bundler expects them.",
        ),
    }
}

/// rv takes the first version file it finds and never mentions the rest. A
/// `.ruby-version` quietly outranking a newer `Gemfile.lock` is worth saying out
/// loud.
fn overruled_pins(config: &Config) -> Option<Check> {
    let pins: Vec<_> = crate::config::directory_ruby_pins(&config.project_root)
        .filter_map(Result::ok)
        .collect();

    let ((winner, winning_source), rest) = pins.split_first()?;

    let overruled: Vec<_> = rest
        .iter()
        .filter(|(request, _)| request.to_string() != winner.to_string())
        .map(|(request, source)| format!("{} says {request}", describe(source)))
        .collect();

    if overruled.is_empty() {
        return None;
    }

    let message = format!(
        "{} pins {winner}, but {} disagree",
        describe(winning_source),
        overruled.join(", ")
    );

    Some(
        Check::warn(SECTION, "version files", message).suggest_command(
            "rv uses the first one and ignores the rest.",
            format!("rv ruby pin {winner}"),
        ),
    )
}

fn describe(source: &Source) -> &'static str {
    match source {
        Source::DotRubyVersion(_) => ".ruby-version",
        Source::DotToolVersions(_) => ".tool-versions",
        Source::GemfileLock(_) => "Gemfile.lock",
    }
}
