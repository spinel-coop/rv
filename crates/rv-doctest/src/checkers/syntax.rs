use async_trait::async_trait;
use rv_ruby::Ruby;
use std::path::PathBuf;

use crate::{CheckError, RubyChecker};

pub struct SyntaxChecker {
    ruby: Ruby,
}

impl SyntaxChecker {
    pub fn new(ruby: Ruby) -> Self {
        Self { ruby }
    }
}

#[async_trait]
impl RubyChecker for SyntaxChecker {
    async fn check(&mut self, code: &str) -> Result<(), CheckError> {
        use tokio::io::AsyncWriteExt;

        let tmp_path: PathBuf = tempfile::Builder::new()
            .suffix(".rb")
            .tempfile()?
            .into_temp_path()
            .to_path_buf();

        {
            let mut file = tokio::fs::File::create(&tmp_path).await?;
            file.write_all(code.as_bytes()).await?;
        }

        let output = tokio::process::Command::new(self.ruby.executable_path())
            .args(["-c"])
            .arg(&tmp_path)
            .output()
            .await?;

        if output.status.success() {
            Ok(())
        } else {
            Err(CheckError::Syntax {
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            })
        }
    }
}
