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
        RvOutput::new(output)
    }

    pub fn create_ruby_dir(&self, name: &str) -> std::path::PathBuf {
        let ruby_dir = self.temp_dir.path().join("opt").join("rubies").join(name);
        std::fs::create_dir_all(&ruby_dir).expect("Failed to create ruby directory");

        let bin_dir = ruby_dir.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("Failed to create bin directory");

        // Create a mock ruby executable
        let ruby_exe = bin_dir.join("ruby");
        std::fs::write(&ruby_exe, "#!/bin/bash\necho 'mock ruby'")
            .expect("Failed to create ruby executable");

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
}

impl RvOutput {
    fn new(output: std::process::Output) -> Self {
        Self { output }
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

        // Use a simple replacement to normalize the temporary directory path
        let lines: Vec<String> = output
            .lines()
            .map(|line| {
                if line.contains("/opt/rubies/") {
                    // For JSON output, handle quoted paths
                    if line.contains("\"path\":") {
                        // JSON path normalization
                        if let Some(path_start) = line.find("\"/") {
                            let before_path = &line[..path_start + 1];
                            if let Some(opt_pos) = line.find("/opt/rubies/") {
                                let after_opt = &line[opt_pos..];
                                format!("{before_path}{after_opt}")
                            } else {
                                line.to_string()
                            }
                        } else {
                            line.to_string()
                        }
                    } else {
                        // Text output normalization
                        if let Some(opt_pos) = line.find("/opt/rubies/") {
                            let after_opt = &line[opt_pos..];
                            // Find the last space before the path to preserve formatting
                            if let Some(space_pos) = line.rfind(' ') {
                                let prefix = &line[..space_pos + 1];
                                // Check if there's a color code right after the space
                                let after_space = &line[space_pos + 1..opt_pos];
                                if after_space.starts_with('\x1b') {
                                    // Find the end of the color code (m)
                                    if let Some(color_end) = after_space.find('m') {
                                        let color_code = &after_space[..color_end + 1];
                                        format!("{prefix}{color_code}{after_opt}")
                                    } else {
                                        format!("{prefix}{after_opt}")
                                    }
                                } else {
                                    format!("{prefix}{after_opt}")
                                }
                            } else {
                                after_opt.to_string()
                            }
                        } else {
                            line.to_string()
                        }
                    }
                } else {
                    line.to_string()
                }
            })
            .collect();

        lines.join("\n")
    }

    /// Normalize stderr for cross-platform snapshot testing
    #[allow(dead_code)]
    pub fn normalized_stderr(&self) -> String {
        let mut output = self.stderr();

        // Replace Windows path separators with forward slashes
        if cfg!(windows) {
            output = output.replace('\\', "/");
        }

        // Remove trailing whitespace and normalize line endings
        output.to_string()
    }
}
