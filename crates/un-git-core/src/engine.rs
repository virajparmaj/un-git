//! Local, fail-closed Git inspection and candidate preparation. No remote writes.
mod fixtures;
mod limits;
mod model;
pub(crate) mod runner;
mod scan;
mod transaction;
mod validate;

pub use fixtures::create_demo;
pub use limits::{
    MAX_PREPARE_COMMITS, MAX_REACHABLE_OBJECTS, MAX_SCAN_COMMITS, MAX_SOURCE_OBJECT_BYTES,
};
pub use model::*;
pub use scan::{doctor, plan, scan};
pub use transaction::{prepare, prepare_with_progress};
pub use validate::verify;
