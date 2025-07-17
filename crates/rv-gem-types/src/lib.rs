pub mod dependency;
pub mod name_tuple;
pub mod platform;
pub mod requirement;
pub mod specification;
pub mod version;

pub use dependency::{Dependency, DependencyType};
pub use name_tuple::NameTuple;
pub use platform::{Platform, CPU};
pub use requirement::{ComparisonOperator, Requirement, VersionConstraint};
pub use specification::Specification;
pub use version::Version;
