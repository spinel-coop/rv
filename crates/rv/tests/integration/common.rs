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
        // Use the binary path from the workspace target directory
        let binary_path = std::env::var("CARGO_BIN_EXE_rv").unwrap_or_else(|_| {
            // Fallback to the expected location in the target directory
            let manifest_dir = env!("CARGO_MANIFEST_DIR");
            let workspace_root = std::path::Path::new(manifest_dir)
                .parent()
                .and_then(|p| p.parent())
                .expect("Failed to find workspace root");
            workspace_root
                .join("target")
                .join("debug")
                .join("rv")
                .to_string_lossy()
                .to_string()
        });

        let mut cmd = Command::new(binary_path);
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

        // Remove trailing whitespace and normalize line endings
        output.to_string()
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
