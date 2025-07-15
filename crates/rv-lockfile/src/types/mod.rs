pub mod dependency;
pub mod platform;
pub mod source;
pub mod spec;

pub use dependency::Dependency;
pub use platform::Platform;
pub use source::{GemSource, GitSource, PathSource, PluginSource, Source};
pub use spec::LazySpecification;
