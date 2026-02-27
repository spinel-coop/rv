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

    /// Create CI config: "github", "gitlab", "circle"
    #[arg(long)]
    pub ci: Option<String>,

    /// Create an extension scaffold: "c", "rust", "go"
    #[arg(long)]
    pub ext: Option<String>,

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

pub(crate) fn gem(_global_args: &GlobalArgs, args: GemArgs) -> Result<()> {
    let name = args.name;
    validate_name(&name)?;

    static GEMSPEC_J2: &str = include_str!("gem/templates/gemspec.j2");
    static LIB_MAIN_J2: &str = include_str!("gem/templates/lib_main.j2");
    static BIN_EXEC_J2: &str = include_str!("gem/templates/bin_exec.j2");

    let mut env = Environment::new();

    // loader returns std::result::Result<Option<String>, minijinja::Error>
    env.set_loader(
        move |name| -> std::result::Result<Option<String>, minijinja::Error> {
            match name {
                "gemspec.j2" => Ok(Some(GEMSPEC_J2.to_string())),
                "lib_main.j2" => Ok(Some(LIB_MAIN_J2.to_string())),
                "bin_exec.j2" => Ok(Some(BIN_EXEC_J2.to_string())),
                _ => Ok(None),
            }
        },
    );

    // Prepare context data for templates
    let version = "0.1.0".to_string();
    let author = "Unknown".to_string();
    let email = git_config_email().unwrap_or_else(|| "unknown@example.com".to_string());
    let description = "A generated Ruby gem".to_string();
    let homepage = args
        .github_username
        .clone()
        .map(|u| format!("https://github.com/{}/{}", u, name))
        .unwrap_or_else(|| "https://example.com/".to_string());
    let license = "MIT".to_string();
    let ruby_version = ">= 3.0".to_string();
    let dependencies: Vec<std::collections::HashMap<&str, String>> = vec![];

    let context = context! {
        name => name.clone(),
        version => version.clone(),
        author => author.clone(),
        email => email.clone(),
        description => description.clone(),
        homepage => homepage.clone(),
        license => license.clone(),
        ruby_version => ruby_version.clone(),
        dependencies => dependencies.clone(),
    };

    let mut target: Utf8PathBuf = std::env::current_dir()?.try_into().unwrap();
    target = target.join(name.clone());

    if target.exists() {
        println!(
            "Target directory {} already exists, skipping creation.",
            target.as_str()
        );
    } else {
        fs::create_dir_all(&target)?;
    }

    let underscored = name.replace('-', "_");
    let namespaced_path = name.replace('-', "/");
    let lib_file_path = format!("lib/{}.rb", namespaced_path);
    let lib_folder_path = format!("lib/{}", namespaced_path);
    let version_file_path = format!("lib/{}/version.rb", namespaced_path);
    let gemspec_path = format!("{}.gemspec", name);

    // Ensure lib directory exists
    let lib_dir = target.join("lib").join(namespaced_path.clone());
    fs::create_dir_all(&lib_dir)?;

    // Write lib/<namespaced_path>.rb
    let lib_rb = target.join(&lib_file_path);
    let module_declaration = constant_name_from(&name);

    // Try to render `lib_main.j2` template; fall back to a basic file if template not found.
    let lib_rendered = match env.get_template("lib_main.j2") {
        Ok(t) => {
            let lib_ctx = context! {
                name => name.clone(),
                version => version.clone(),
                module_decl => module_declaration.clone(),
                namespaced_path => namespaced_path.clone(),
            };
            t.render(lib_ctx).map_err(|e| {
                Error::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?
        }
        Err(_) => format!(
            "require_relative \"{}/version\"\n\nmodule {module_decl}\n  # Your code goes here...\nend\n",
            namespaced_path,
            module_decl = module_declaration
        ),
    };
    fs::write(&lib_rb, lib_rendered.as_bytes())?;

    // Write lib/<namespaced_path>/version.rb
    let version_rb = target.join(&version_file_path);
    // Ensure directory for version.rb exists
    if let Some(version_dir) = version_rb.parent() {
        fs::create_dir_all(version_dir)?;
    }
    let version_contents = format!(
        "module {module_decl}\n  VERSION = \"{version}\"\nend\n",
        module_decl = module_declaration,
        version = version
    );
    fs::write(&version_rb, version_contents.as_bytes())?;

    // Render gemspec using minijinja template and write to disk
    let gemspec_file = target.join(&gemspec_path);
    let rendered_gemspec = match env.get_template("gemspec.j2") {
        Ok(t) => t.render(context.clone()).map_err(|e| {
            Error::IoError(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?,
        Err(e) => {
            return Err(Error::IoError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("template load error: {}", e),
            )));
        }
    };
    fs::write(&gemspec_file, rendered_gemspec.as_bytes())?;

    // Write README.md
    let readme = target.join("README.md");
    let readme_contents = format!("# {name}\n\nTODO: Describe the gem.\n", name = name);
    fs::write(&readme, readme_contents.as_bytes())?;

    // Optionally create exe
    if args.exe {
        let exe_dir = target.join("exe");
        fs::create_dir_all(&exe_dir)?;
        let exe_path = exe_dir.join(&name);

        // Try to render `bin_exec.j2` template; fall back to a basic executable if template not found.
        let exe_rendered = match env.get_template("bin_exec.j2") {
            Ok(t) => {
                let bin_ctx = context! {
                    name => name.clone(),
                    namespaced_path => namespaced_path.clone(),
                    module_decl => module_declaration.clone(),
                };
                t.render(bin_ctx).map_err(|e| {
                    Error::IoError(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        e.to_string(),
                    ))
                })?
            }
            Err(_) => format!(
                "#!/usr/bin/env ruby\nputs \"This is the {name} executable.\"",
                name = name
            ),
        };
        fs::write(&exe_path, exe_rendered.as_bytes())?;
        // try to make executable; ignore errors on non-unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&exe_path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&exe_path, perms).unwrap();
        }
    }

    // Optionally add basic test scaffolding
    if let Some(test) = args.test {
        match test.as_str() {
            "rspec" => {
                let spec_dir = target.join("spec");
                fs::create_dir_all(&spec_dir)?;
                let spec_helper_path = spec_dir.join("spec_helper.rb");
                fs::write(&spec_helper_path, b"RSpec.configure do |c|\nend\n")?;
                let spec_path =
                    spec_dir.join(format!("{}_spec.rb", namespaced_path.replace('/', "_")));
                fs::write(
                    &spec_path,
                    format!(
                        "require_relative '../lib/{}'\n\nRSpec.describe {} do\nend\n",
                        namespaced_path, module_declaration
                    )
                    .as_bytes(),
                )?;
            }
            "minitest" => {
                let test_dir = target.join("test");
                fs::create_dir_all(&test_dir)?;
                let helper = test_dir.join("test_helper.rb");
                fs::write(&helper, b"require 'minitest/autorun'\n")?;
                let test_path = test_dir.join(format!("test_{}.rb", underscored.replace('/', "_")));
                fs::write(
                    &test_path,
                    format!(
                        "require_relative '../lib/{}'\n\nclass Test{} < Minitest::Test\nend\n",
                        namespaced_path,
                        sanitize_const_last(&module_declaration)
                    )
                    .as_bytes(),
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

    // Optionally initialize git
    if args.git {
        println!("Initializing git repo in {}", target.as_str());
        // Try to run `git init <target>`
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
    }

    // Optionally run bundle install
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

    // Optionally open editor
    if let Some(editor) = args.edit {
        let gemspec_file = target.join(format!("{}.gemspec", name));
        let _ = Command::new(editor).arg(gemspec_file.as_str()).spawn();
    }

    println!("\nGem '{}' was created at {}\n", name, target.as_str());

    Ok(())
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

/// Convert "foo-bar/baz" or "foo-bar" into a Ruby constant like "Foo::Bar" or "Foo::Bar::Baz"
fn constant_name_from(name: &str) -> String {
    // First convert to parts by '/' and '-'
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
    // Return last constant for use in class name: Foo::Bar => Bar
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
