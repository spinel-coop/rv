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

    /// Make an executable in `exe/` with the gem name
    #[arg(long)]
    pub exe: bool,

    /// Run `bundle install` in the new gem directory (if bundler is available)
    #[arg(long)]
    pub bundle: bool,

    /// Open the generated gemspec with this editor (editor command)
    #[arg(long)]
    pub edit: Option<String>,

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
    static BIN_EXEC_J2: &str = include_str!("gem/templates/bin_exec.j2");
    static BIN_CONSOLE_J2: &str = include_str!("gem/templates/bin_console.j2");
    static GEMFILE_J2: &str = include_str!("gem/templates/gemfile.j2");
    static README_J2: &str = include_str!("gem/templates/readme.j2");

    let mut env = Environment::new();

    env.set_loader(
        move |tpl_name| -> std::result::Result<Option<String>, minijinja::Error> {
            match tpl_name {
                "gemspec.j2" => Ok(Some(GEMSPEC_J2.to_string())),
                "lib_main.j2" => Ok(Some(LIB_MAIN_J2.to_string())),
                "bin_exec.j2" => Ok(Some(BIN_EXEC_J2.to_string())),
                "bin_console.j2" => Ok(Some(BIN_CONSOLE_J2.to_string())),
                "gemfile.j2" => Ok(Some(GEMFILE_J2.to_string())),
                "readme.j2" => Ok(Some(README_J2.to_string())),
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
        name => name.clone(),
        title => title.clone(),
        version => version.clone(),
        author => author.clone(),
        email => email.clone(),
        homepage => homepage.clone(),
        namespaced_path => namespaced_path.clone(),
        module_decl => module_declaration.clone(),
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
            version => version.clone(),
            module_decl => module_declaration.clone(),
            namespaced_path => namespaced_path.clone(),
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
    let rendered_readme = render_template(&env, "readme.j2", context.clone())?;
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
            namespaced_path => namespaced_path.clone(),
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

    // --- Optional: exe/<name> ---
    if args.exe {
        let exe_rendered = render_template(
            &env,
            "bin_exec.j2",
            context! {
                name => name.clone(),
                namespaced_path => namespaced_path.clone(),
                module_decl => module_declaration.clone(),
            },
        )?;
        let exe_rel = format!("exe/{}", name);
        let exe_path = target.join(&exe_rel);
        create_file(&exe_path, &dp(&exe_rel), exe_rendered.as_bytes())?;
        make_executable(&exe_path);
    }

    // --- Optional: test scaffold ---
    if let Some(ref test) = args.test {
        match test.as_str() {
            "rspec" => {
                let spec_dir = target.join("spec");
                fs::create_dir_all(&spec_dir)?;

                let spec_helper =
                    "# frozen_string_literal: true\n\nRSpec.configure do |config|\nend\n";
                create_file(
                    &spec_dir.join("spec_helper.rb"),
                    &dp("spec/spec_helper.rb"),
                    spec_helper.as_bytes(),
                )?;

                let spec_rel = format!("spec/{}_spec.rb", underscored.replace('/', "_"));
                let spec_content = format!(
                    "# frozen_string_literal: true\n\nrequire \"spec_helper\"\n\nRSpec.describe {} do\nend\n",
                    module_declaration
                );
                create_file(
                    &target.join(&spec_rel),
                    &dp(&spec_rel),
                    spec_content.as_bytes(),
                )?;
            }
            "minitest" => {
                let test_dir = target.join("test");
                fs::create_dir_all(&test_dir)?;

                let helper = "# frozen_string_literal: true\n\n$LOAD_PATH.unshift File.expand_path(\"../lib\", __dir__)\n\nrequire \"minitest/autorun\"\n";
                create_file(
                    &test_dir.join("test_helper.rb"),
                    &dp("test/test_helper.rb"),
                    helper.as_bytes(),
                )?;

                let test_rel = format!("test/test_{}.rb", underscored.replace('/', "_"));
                let test_content = format!(
                    "# frozen_string_literal: true\n\nrequire \"test_helper\"\n\nclass Test{} < Minitest::Test\n  def test_that_it_has_a_version_number\n    refute_nil ::{}::VERSION\n  end\nend\n",
                    sanitize_const_last(&module_declaration),
                    module_declaration
                );
                create_file(
                    &target.join(&test_rel),
                    &dp(&test_rel),
                    test_content.as_bytes(),
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

    // --- Optional: bundle install ---
    if args.bundle {
        println!("Running `bundle install` in the new gem directory.");
        if let Ok(mut cmd) = Command::new("bundle")
            .arg("install")
            .current_dir(target.as_std_path())
            .spawn()
        {
            let _ = cmd.wait();
        } else {
            println!("Could not run `bundle install` - is Bundler installed?");
        }
    }

    // --- Optional: open editor ---
    if let Some(editor) = args.edit {
        let gemspec_file = target.join(format!("{}.gemspec", name));
        let _ = Command::new(editor).arg(gemspec_file.as_str()).spawn();
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
            Error::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("template '{}' error: {}", template_name, e),
            ))
        })
}

fn make_executable(path: &Utf8PathBuf) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path.as_std_path()) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = std::fs::set_permissions(path.as_std_path(), perms);
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
    if let Ok(out) = Command::new("git").args(&["config", "user.email"]).output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}

fn git_config_name() -> Option<String> {
    if let Ok(out) = Command::new("git").args(&["config", "user.name"]).output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}
