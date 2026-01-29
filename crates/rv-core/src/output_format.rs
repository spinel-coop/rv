#[derive(clap::ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}
