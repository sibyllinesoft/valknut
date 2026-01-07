pub mod assets;
mod error;
mod generator;
mod helpers;
mod hierarchy;
mod templates;

pub use error::ReportError;
pub use generator::ReportGenerator;
pub use hierarchy::{
    add_files_to_hierarchy, build_candidate_lookup, build_unified_hierarchy,
    create_file_groups_from_candidates,
};
