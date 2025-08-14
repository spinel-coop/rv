use std::process::Command;
use tempfile::TempDir;

pub struct RvTest {
    pub temp_dir: TempDir,
    pub test_root: String,
}

impl RvTest {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temporary directory");
        let test_root = temp_dir.path().to_string_lossy().to_string();

        Self {
            temp_dir,
            test_root,
        }
    }

    pub fn rv_command(&self) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_rv"));
        cmd.env("RV_ROOT_DIR", &self.test_root);
        // Set consistent arch/os for cross-platform testing
        cmd.env("RV_TEST_ARCH", "aarch64");
        cmd.env("RV_TEST_OS", "macos");
        cmd
    }

    pub fn ruby_list(&self, args: &[&str]) -> RvOutput {
        let mut cmd = self.rv_command();
        cmd.args(["ruby", "list"]);
        cmd.args(args);

        let output = cmd.output().expect("Failed to execute rv command");
        RvOutput::new(&self.test_root, output)
    }

    pub fn create_ruby_dir(&self, name: &str) -> std::path::PathBuf {
        let ruby_dir = self.temp_dir.path().join("opt").join("rubies").join(name);
        std::fs::create_dir_all(&ruby_dir).expect("Failed to create ruby directory");

        let bin_dir = ruby_dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("Failed to create bin directory");

        // Create a mock ruby executable that outputs the expected format for rv-ruby
        let ruby_exe = bin_dir.join("ruby");
        let mock_script = r#"#!/bin/bash
# Extract Ruby information from directory name
dir_name=$(basename $(dirname $(dirname $0)))

# Extract version from directory name: ruby-3.1.4 -> 3.1.4
version=$(echo "$dir_name" | sed 's/^[^-]*-//')

# Extract engine from directory name: ruby-3.1.4 -> ruby, jruby-9.4.0.0 -> jruby
engine=$(echo "$dir_name" | sed 's/-.*$//')

# If engine equals version, it means no engine prefix, so default to ruby
if [[ "$engine" == "$version" ]]; then
    engine="ruby"
fi

# Mock the Ruby script that rv-ruby expects
# The script should output:
# 1. RUBY_ENGINE (or 'ruby' if not defined)
# 2. RUBY_VERSION  
# 3. RUBY_PLATFORM (or 'unknown' if not defined)
# 4. host_cpu from RbConfig (or 'unknown' if not available)
# 5. host_os from RbConfig (or 'unknown' if not available)
# 6. GEM_ROOT export line (empty if rubygems not available)

if [[ "$1" == "-e" ]]; then
    case "$2" in
        *RUBY_ENGINE*RUBY_VERSION*RUBY_PLATFORM*RbConfig*host_cpu*host_os*rubygems*)
            # This is the full script from extract_ruby_info
            echo "$engine"
            echo "$version"
            echo "aarch64-darwin23"
            echo "aarch64"
            echo "darwin23"
            echo ""
            ;;
        *defined*RUBY_ENGINE*RUBY_VERSION*)
            # This is the simpler script from extract_ruby_info_from_executable
            echo "$engine-$version"
            ;;
        *)
            # Unknown script, return something reasonable
            echo "$engine-$version"
            ;;
    esac
else
    # If not -e, just output version info
    echo "$engine $version"
fi
"#
        .to_string();
        std::fs::write(&ruby_exe, mock_script).expect("Failed to create ruby executable");

        // Make it executable on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&ruby_exe).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&ruby_exe, perms).unwrap();
        }

        ruby_dir
    }
}

pub struct RvOutput {
    pub output: std::process::Output,
    pub test_root: String,
}

impl RvOutput {
    fn new(test_root: &str, output: std::process::Output) -> Self {
        Self {
            output,
            test_root: test_root.to_string(),
        }
    }

    pub fn success(&self) -> bool {
        self.output.status.success()
    }

    pub fn stdout(&self) -> String {
        String::from_utf8_lossy(&self.output.stdout).to_string()
    }

    #[allow(dead_code)]
    pub fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.output.stderr).to_string()
    }

    /// Normalize output for cross-platform snapshot testing
    pub fn normalized_stdout(&self) -> String {
        let mut output = self.stdout();

        // Replace Windows path separators with forward slashes
        if cfg!(windows) {
            output = output.replace('\\', "/");
        }

        // Remove test root from paths
        let mut full_test_root = self.test_root.clone();
        // On macOS, the test root might be prefixed with "/private"
        if cfg!(target_os = "macos") {
            full_test_root.insert_str(0, "/private");
        }
        output.replace(&full_test_root, "")
    }

    /// Normalize stderr for cross-platform snapshot testing
    #[allow(dead_code)]
    pub fn normalized_stderr(&self) -> String {
        let mut output = self.stderr();

        // Replace Windows path separators with forward slashes
        if cfg!(windows) {
            output = output.replace('\\', "/");
        }

        output.to_string()
    }
}
