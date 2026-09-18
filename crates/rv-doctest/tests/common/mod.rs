use std::path::PathBuf;

pub fn write_rbs(dir: &std::path::Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(format!("{name}.rbs"));
    std::fs::write(&path, content).unwrap();
    path
}
