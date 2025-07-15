pub mod dependency;
pub mod platform;
pub mod source;
pub mod spec;

pub use dependency::Dependency;
pub use platform::Platform;
pub use source::{Source, GitSource, GemSource, PathSource, PluginSource};
pub use spec::LazySpecification;