//! `rv doctor` — inspect the current environment and report anything that would
//! stop rv from working, together with the command that fixes it.
//!
//! The checks are deliberately offline and read-only. Most of them work by
//! comparing the environment rv *would* produce — see [`Config::env_for`] — with
//! the one this process actually inherited, so the report stays honest as rv's
//! environment handling changes rather than drifting into a separate guess at it.

pub mod configuration;
pub mod environment;
pub mod installation;

use std::collections::BTreeMap;
use std::fmt;

use anstream::println;
use camino::Utf8PathBuf;
use clap::Args;
use owo_colors::OwoColorize;
use serde::Serialize;

use crate::GlobalArgs;
use crate::config::Config;
use crate::output_format::OutputFormat;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    ConfigError(#[from] crate::config::Error),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("Could not serialize JSON: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("{failed} of {total} checks failed")]
    #[diagnostic(help("Run the suggested commands above, then try `rv doctor` again."))]
    Unhealthy { failed: usize, total: usize },
}

type Result<T> = miette::Result<T, Error>;

#[derive(Args)]
pub struct DoctorArgs {
    /// Output format for the report
    #[arg(long, value_enum, default_value = "text")]
    pub format: OutputFormat,

    /// Exit successfully even when checks fail
    #[arg(long)]
    pub exit_zero: bool,
}

/// How a single check turned out, ordered least to most severe so that
/// [`Iterator::max`] over a report yields its overall status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Not applicable here — an earlier failure made this check meaningless.
    Skipped,
    Ok,
    /// Works today, but will bite. Does not fail the command.
    Warn,
    Fail,
}

impl Status {
    fn symbol(self) -> &'static str {
        match self {
            Self::Skipped => "-",
            Self::Ok => "✔",
            Self::Warn => "!",
            Self::Fail => "✗",
        }
    }

    fn render(self) -> String {
        let symbol = self.symbol();
        match self {
            Self::Skipped => symbol.dimmed().to_string(),
            Self::Ok => symbol.green().to_string(),
            Self::Warn => symbol.yellow().to_string(),
            Self::Fail => symbol.red().to_string(),
        }
    }
}

/// Checks are grouped for display in declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Section {
    Environment,
    Installation,
    Configuration,
}

impl fmt::Display for Section {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Environment => write!(f, "Environment"),
            Self::Installation => write!(f, "Installation"),
            Self::Configuration => write!(f, "Configuration"),
        }
    }
}

/// What the user should do about a check that didn't pass.
#[derive(Debug, Serialize)]
pub struct Fix {
    /// Why it went wrong, and what to do about it.
    pub detail: String,
    /// A command to copy and run, when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Check {
    pub section: Section,
    pub name: &'static str,
    pub status: Status,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<Fix>,
}

impl Check {
    pub fn ok(section: Section, name: &'static str, message: impl Into<String>) -> Self {
        Self::new(section, name, Status::Ok, message)
    }

    pub fn warn(section: Section, name: &'static str, message: impl Into<String>) -> Self {
        Self::new(section, name, Status::Warn, message)
    }

    pub fn fail(section: Section, name: &'static str, message: impl Into<String>) -> Self {
        Self::new(section, name, Status::Fail, message)
    }

    pub fn skipped(section: Section, name: &'static str, message: impl Into<String>) -> Self {
        Self::new(section, name, Status::Skipped, message)
    }

    fn new(
        section: Section,
        name: &'static str,
        status: Status,
        message: impl Into<String>,
    ) -> Self {
        Self {
            section,
            name,
            status,
            message: message.into(),
            fix: None,
        }
    }

    /// Attach advice with no single command behind it.
    #[must_use]
    pub fn suggest(self, detail: impl Into<String>) -> Self {
        Self {
            fix: Some(Fix {
                detail: detail.into(),
                command: None,
            }),
            ..self
        }
    }

    /// Attach advice along with the command that carries it out.
    #[must_use]
    pub fn suggest_command(self, detail: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            fix: Some(Fix {
                detail: detail.into(),
                command: Some(command.into()),
            }),
            ..self
        }
    }
}

#[derive(Debug, Default, Serialize)]
pub struct Report {
    checks: Vec<Check>,
}

impl Report {
    fn count(&self, status: Status) -> usize {
        self.checks.iter().filter(|c| c.status == status).count()
    }

    fn render(&self, format: &OutputFormat) -> Result<()> {
        match format {
            OutputFormat::Json => {
                serde_json::to_writer_pretty(std::io::stdout(), &self.checks)?;
                println!();
            }
            OutputFormat::Text => self.render_text(),
        }

        Ok(())
    }

    fn render_text(&self) {
        // Align the message column across every section, not just within one.
        let width = self
            .checks
            .iter()
            .map(|c| c.name.len())
            .max()
            .unwrap_or_default();
        // Line up wrapped advice under the message it belongs to: two spaces of
        // indent, the status symbol and its space, the name column, two spaces.
        let hang = " ".repeat(2 + 2 + width + 2);

        let mut sections: BTreeMap<Section, Vec<&Check>> = BTreeMap::new();
        for check in &self.checks {
            sections.entry(check.section).or_default().push(check);
        }

        for (section, checks) in sections {
            println!("\n{}", section.to_string().green().bold());

            for check in checks {
                println!(
                    "  {} {:width$}  {}",
                    check.status.render(),
                    check.name,
                    check.message
                );

                let Some(fix) = &check.fix else { continue };

                println!("{hang}{}", fix.detail.dimmed());
                if let Some(command) = &fix.command {
                    println!("{hang}{} {}", "→".dimmed(), command.cyan());
                }
            }
        }

        println!("\n{}", self.summary());
    }

    fn summary(&self) -> String {
        let failed = self.count(Status::Fail);
        let warned = self.count(Status::Warn);

        let mut parts = Vec::new();
        if failed > 0 {
            parts.push(
                format!("{} failed", plural(failed, "check"))
                    .red()
                    .to_string(),
            );
        }
        if warned > 0 {
            parts.push(plural(warned, "warning").yellow().to_string());
        }

        if parts.is_empty() {
            return "Everything looks good.".green().to_string();
        }

        format!("{}.", parts.join(", "))
    }
}

/// `1 check`, `2 checks`.
fn plural(count: usize, noun: &str) -> String {
    let suffix = if count == 1 { "" } else { "s" };
    format!("{count} {noun}{suffix}")
}

/// A snapshot of the environment variables rv was started with.
///
/// Checks read from this rather than [`std::env`] so that tests can describe an
/// arbitrary environment without mutating global process state.
#[derive(Debug, Default, Clone)]
pub struct ProcessEnv {
    vars: BTreeMap<String, String>,
}

impl ProcessEnv {
    pub fn from_process() -> Self {
        std::env::vars().collect()
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(String::as_str)
    }

    /// The entries of `PATH` in order, skipping any that aren't valid UTF-8.
    pub fn path(&self) -> Vec<Utf8PathBuf> {
        let Some(path) = self.get("PATH") else {
            return Vec::new();
        };

        std::env::split_paths(path)
            .filter_map(|p| Utf8PathBuf::try_from(p).ok())
            .collect()
    }
}

impl<K: Into<String>, V: Into<String>> FromIterator<(K, V)> for ProcessEnv {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        Self {
            vars: iter
                .into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        }
    }
}

pub(crate) fn doctor(global_args: &GlobalArgs, args: DoctorArgs) -> Result<()> {
    let env = ProcessEnv::from_process();

    // Deliberately `Config::new` rather than `Config::with_settings`: this is the
    // config `rv shell env` builds, so it describes the environment the shell
    // integration actually exports. Anything the settings files would change is
    // reported separately by `configuration`, which can also surface the parse
    // errors that `with_settings` swallows.
    let config = Config::new(global_args, None)?;

    let mut checks = environment::check(&config, &env)?;
    checks.extend(installation::check(&config, &env));
    checks.extend(configuration::check(global_args, &config));

    let report = Report { checks };
    report.render(&args.format)?;

    let failed = report.count(Status::Fail);
    if failed > 0 && !args.exit_zero {
        return Err(Error::Unhealthy {
            failed,
            total: report.checks.len(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_orders_by_severity() {
        let mut statuses = [Status::Fail, Status::Ok, Status::Warn, Status::Skipped];
        statuses.sort();

        assert_eq!(
            statuses,
            [Status::Skipped, Status::Ok, Status::Warn, Status::Fail]
        );
    }

    #[test]
    fn process_env_splits_path() {
        let env = ProcessEnv::from_iter([(
            "PATH",
            std::env::join_paths(["/a/bin", "/b/bin"])
                .unwrap()
                .to_str()
                .unwrap(),
        )]);

        assert_eq!(env.path(), ["/a/bin", "/b/bin"]);
    }

    #[test]
    fn process_env_without_path_is_empty() {
        assert!(ProcessEnv::default().path().is_empty());
    }

    /// `owo_colors` colors unconditionally; anstream strips at the stream. Tests
    /// assert on what a plain terminal would show.
    fn plain(text: String) -> String {
        anstream::adapter::strip_str(&text).to_string()
    }

    #[test]
    fn summary_counts_failures_and_warnings() {
        let report = Report {
            checks: vec![
                Check::fail(Section::Environment, "a", ""),
                Check::fail(Section::Environment, "b", ""),
                Check::warn(Section::Environment, "c", ""),
                Check::ok(Section::Environment, "d", ""),
            ],
        };

        assert_eq!(plain(report.summary()), "2 checks failed, 1 warning.");
    }

    #[test]
    fn summary_is_positive_when_healthy() {
        let report = Report {
            checks: vec![Check::ok(Section::Environment, "a", "")],
        };

        assert_eq!(plain(report.summary()), "Everything looks good.");
    }
}
