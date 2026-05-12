use anstream::println;
use camino::Utf8PathBuf;
use clap::Args;
use fs_err as fs;
use minijinja::{Environment, context};
use std::process::Command;
use std::str;
use tracing::debug;

use crate::GlobalArgs;

#[derive(Args)]
pub struct GemArgs {
    /// Name of the gem to create (e.g. my_gem or my-gem)
    pub name: String,

    /// Create a git repository in the new gem directory
    #[arg(long)]
    pub git: bool,

    /// Create a basic test scaffold: "rspec" or "minitest"
    #[arg(long)]
    pub test: Option<String>,

    /// GitHub username to use for generated URLs (optional)
    #[arg(long)]
    pub github_username: Option<String>,
}

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error("A file already exists at the target path `{0}`")]
    TargetExists(String),

    #[error("Invalid gem name `{0}`")]
    InvalidName(String),
}

type Result<T> = miette::Result<T, Error>;

fn report_create(display_path: &str) {
    println!("      \x1b[32mcreate\x1b[0m  {}", display_path);
}

fn create_file(full_path: &Utf8PathBuf, display_path: &str, content: &[u8]) -> Result<()> {
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(full_path, content)?;
    report_create(display_path);
    Ok(())
}

pub(crate) fn gem(_global_args: &GlobalArgs, args: GemArgs) -> Result<()> {
    let name = args.name;
    validate_name(&name)?;

    static GEMSPEC_J2: &str = include_str!("gem/templates/gemspec.j2");
    static LIB_MAIN_J2: &str = include_str!("gem/templates/lib_main.j2");
    static BIN_CONSOLE_J2: &str = include_str!("gem/templates/bin_console.j2");
    static GEMFILE_J2: &str = include_str!("gem/templates/gemfile.j2");
    static README_J2: &str = include_str!("gem/templates/readme.j2");
    static MINITEST_TEST_HELPER_J2: &str = include_str!("gem/templates/minitest_test_helper.j2");
    static MINITEST_TEST_J2: &str = include_str!("gem/templates/minitest_test.j2");
    static RSPEC_SPEC_HELPER_J2: &str = include_str!("gem/templates/rspec_spec_helper.j2");
    static RSPEC_SPEC_J2: &str = include_str!("gem/templates/rspec_spec.j2");

    let mut env = Environment::new();

    env.set_loader(
        move |tpl_name| -> std::result::Result<Option<String>, minijinja::Error> {
            match tpl_name {
                "gemspec.j2" => Ok(Some(GEMSPEC_J2.to_string())),
                "lib_main.j2" => Ok(Some(LIB_MAIN_J2.to_string())),
                "bin_console.j2" => Ok(Some(BIN_CONSOLE_J2.to_string())),
                "gemfile.j2" => Ok(Some(GEMFILE_J2.to_string())),
                "readme.j2" => Ok(Some(README_J2.to_string())),
                "minitest_test_helper.j2" => Ok(Some(MINITEST_TEST_HELPER_J2.to_string())),
                "minitest_test.j2" => Ok(Some(MINITEST_TEST_J2.to_string())),
                "rspec_spec_helper.j2" => Ok(Some(RSPEC_SPEC_HELPER_J2.to_string())),
                "rspec_spec.j2" => Ok(Some(RSPEC_SPEC_J2.to_string())),
                _ => Ok(None),
            }
        },
    );

    // Derive naming helpers
    let underscored = name.replace('-', "_");
    let namespaced_path = name.replace('-', "/");
    let module_declaration = constant_name_from(&name);

    let title = {
        let mut chars = name.chars();
        match chars.next() {
            Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            None => name.clone(),
        }
    };

    // Prepare context data
    let version = "0.1.0".to_string();
    let author = git_config_name().unwrap_or_else(|| "TODO: Write your name".to_string());
    let email = git_config_email().unwrap_or_else(|| "TODO: Write your email address".to_string());
    let homepage = args
        .github_username
        .clone()
        .map(|u| format!("https://github.com/{}/{}", u, name))
        .unwrap_or_else(|| "TODO: Put your gem's website or public repo URL here.".to_string());

    let context = context! {
        name => name,
        title => title,
        version => version,
        author => author,
        email => email,
        homepage => homepage,
        namespaced_path => namespaced_path,
        module_decl => module_declaration,
    };

    let mut target: Utf8PathBuf = std::env::current_dir()?.try_into().unwrap();
    target = target.join(name.clone());

    println!("Creating gem '{}'...", name);

    if target.exists() {
        println!(
            "Target directory {} already exists, skipping creation.",
            target.as_str()
        );
    } else {
        fs::create_dir_all(&target)?;
    }

    // Helper: build a display path like "my_gem/lib/my_gem.rb"
    let dp = |rel: &str| format!("{}/{}", name, rel);

    // --- git init ---
    if args.git {
        println!("Initializing git repo in {}", target.as_str());
        match Command::new("git")
            .arg("init")
            .arg(target.as_str())
            .output()
        {
            Ok(out) => {
                debug!("git init stdout: {}", String::from_utf8_lossy(&out.stdout));
                debug!("git init stderr: {}", String::from_utf8_lossy(&out.stderr));
            }
            Err(e) => {
                debug!("Failed to run git init: {}", e);
            }
        }

        // --- .gitignore ---
        let gitignore =
            "/.bundle/\n/.yardoc\n/_yardoc/\n/coverage/\n/doc/\n/pkg/\n/spec/reports/\n/tmp/\n";
        create_file(
            &target.join(".gitignore"),
            &dp(".gitignore"),
            gitignore.as_bytes(),
        )?;
    }

    // --- Gemfile ---
    let rendered_gemfile = render_template(&env, "gemfile.j2", context.clone())?;
    create_file(
        &target.join("Gemfile"),
        &dp("Gemfile"),
        rendered_gemfile.as_bytes(),
    )?;

    // --- lib/<namespaced_path>.rb ---
    let lib_rendered = render_template(
        &env,
        "lib_main.j2",
        context! {
            name => name.clone(),
            version => version,
            module_decl => module_declaration,
            namespaced_path => namespaced_path,
        },
    )?;
    let lib_rb_path = format!("lib/{}.rb", namespaced_path);
    create_file(
        &target.join(&lib_rb_path),
        &dp(&lib_rb_path),
        lib_rendered.as_bytes(),
    )?;

    // --- lib/<namespaced_path>/version.rb ---
    let version_file_path = format!("lib/{}/version.rb", namespaced_path);
    let version_contents = format!(
        "# frozen_string_literal: true\n\nmodule {module_decl}\n  VERSION = \"{version}\"\nend\n",
        module_decl = module_declaration,
        version = version
    );
    create_file(
        &target.join(&version_file_path),
        &dp(&version_file_path),
        version_contents.as_bytes(),
    )?;

    // --- <name>.gemspec ---
    let rendered_gemspec = render_template(&env, "gemspec.j2", context.clone())?;
    let gemspec_rel = format!("{}.gemspec", name);
    create_file(
        &target.join(&gemspec_rel),
        &dp(&gemspec_rel),
        rendered_gemspec.as_bytes(),
    )?;

    // --- Rakefile ---
    let rakefile = "# frozen_string_literal: true\n\nrequire \"bundler/gem_tasks\"\nrequire \"rake/testtask\"\n\nRake::TestTask.new(:test) do |t|\n  t.libs << \"test\"\n  t.libs << \"lib\"\n  t.test_files = FileList[\"test/**/test_*.rb\"]\nend\n\ntask default: :test\n";
    create_file(
        &target.join("Rakefile"),
        &dp("Rakefile"),
        rakefile.as_bytes(),
    )?;

    // --- README.md ---
    let rendered_readme = render_template(&env, "readme.j2", context)?;
    create_file(
        &target.join("README.md"),
        &dp("README.md"),
        rendered_readme.as_bytes(),
    )?;

    // --- bin/console ---
    let console_rendered = render_template(
        &env,
        "bin_console.j2",
        context! {
            namespaced_path => namespaced_path,
        },
    )?;
    let bin_console_path = target.join("bin").join("console");
    create_file(
        &bin_console_path,
        &dp("bin/console"),
        console_rendered.as_bytes(),
    )?;
    make_executable(&bin_console_path);

    // --- bin/setup ---
    let bin_setup_content = "#!/usr/bin/env bash\nset -euo pipefail\nIFS=$'\\n\\t'\nset -vx\n\nbundle install\n\n# Do any other automated setup that you need to do here\n";
    let bin_setup_path = target.join("bin").join("setup");
    create_file(
        &bin_setup_path,
        &dp("bin/setup"),
        bin_setup_content.as_bytes(),
    )?;
    make_executable(&bin_setup_path);

    // --- Optional: test scaffold ---
    if let Some(ref test) = args.test {
        match test.as_str() {
            "rspec" => {
                // .rspec config file
                create_file(
                    &target.join(".rspec"),
                    &dp(".rspec"),
                    b"--require spec_helper\n",
                )?;

                let test_ctx = context! {
                    namespaced_path => namespaced_path,
                    module_decl => module_declaration,
                };

                let spec_helper_rendered =
                    render_template(&env, "rspec_spec_helper.j2", test_ctx.clone())?;
                create_file(
                    &target.join("spec").join("spec_helper.rb"),
                    &dp("spec/spec_helper.rb"),
                    spec_helper_rendered.as_bytes(),
                )?;

                let spec_rel = format!("spec/{}_spec.rb", underscored.replace('/', "_"));
                let spec_rendered = render_template(&env, "rspec_spec.j2", test_ctx)?;
                create_file(
                    &target.join(&spec_rel),
                    &dp(&spec_rel),
                    spec_rendered.as_bytes(),
                )?;
            }
            "minitest" => {
                let test_ctx = context! {
                    namespaced_path => namespaced_path,
                    module_decl => module_declaration,
                    last_const => sanitize_const_last(&module_declaration),
                };

                let helper_rendered =
                    render_template(&env, "minitest_test_helper.j2", test_ctx.clone())?;
                create_file(
                    &target.join("test").join("test_helper.rb"),
                    &dp("test/test_helper.rb"),
                    helper_rendered.as_bytes(),
                )?;

                let test_rel = format!("test/test_{}.rb", underscored.replace('/', "_"));
                let test_rendered = render_template(&env, "minitest_test.j2", test_ctx)?;
                create_file(
                    &target.join(&test_rel),
                    &dp(&test_rel),
                    test_rendered.as_bytes(),
                )?;
            }
            other => {
                println!(
                    "Unknown test framework '{}', skipping test scaffolding.",
                    other
                );
            }
        }
    }

    println!(
        "\nGem '{}' was successfully created. For more information on making a RubyGem visit https://bundler.io/guides/creating_gem.html\n",
        name
    );

    Ok(())
}

fn render_template(
    env: &Environment,
    template_name: &str,
    ctx: minijinja::Value,
) -> Result<String> {
    env.get_template(template_name)
        .and_then(|t| t.render(ctx))
        .map_err(|e| {
            Error::IoError(std::io::Error::other(format!(
                "template '{}' error: {}",
                template_name, e
            )))
        })
}

fn make_executable(_path: &Utf8PathBuf) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(_path.as_std_path()) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(_path.as_std_path(), perms);
        }
    }
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::InvalidName(name.to_string()));
    }
    if name.chars().next().unwrap().is_ascii_digit() {
        return Err(Error::InvalidName(name.to_string()));
    }
    Ok(())
}

/// Convert "foo-bar" or "foo-bar-baz" into a Ruby constant like "Foo::Bar" or "Foo::Bar::Baz".
fn constant_name_from(name: &str) -> String {
    let parts: Vec<String> = name
        .split('/')
        .flat_map(|s| s.split('-'))
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut c = s.chars();
            match c.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + c.as_str(),
                None => "".to_string(),
            }
        })
        .collect();
    parts.join("::")
}

fn sanitize_const_last(module_decl: &str) -> String {
    module_decl.split("::").last().unwrap_or("Test").to_string()
}

fn git_config_email() -> Option<String> {
    if let Ok(out) = Command::new("git").args(["config", "user.email"]).output()
        && out.status.success()
        && let s = String::from_utf8_lossy(&out.stdout).trim().to_string()
        && !s.is_empty()
    {
        return Some(s);
    }

    None
}

fn git_config_name() -> Option<String> {
    if let Ok(out) = Command::new("git").args(["config", "user.name"]).output()
        && out.status.success()
        && let s = String::from_utf8_lossy(&out.stdout).trim().to_string()
        && !s.is_empty()
    {
        return Some(s);
    }
    None
}
