pub mod dependency;
pub mod name_tuple;
pub mod platform;
pub mod requirement;
pub mod specification;
pub mod version;

pub use dependency::{Dependency, DependencyError, DependencyType};
pub use name_tuple::{NameTuple, NameTupleError};
pub use platform::Platform;
pub use requirement::{ComparisonOperator, Requirement, VersionConstraint};
pub use specification::{Specification, SpecificationError};
pub use version::{Version, VersionError};
