pub mod assets;
mod error;
mod generator;
mod helpers;
mod hierarchy;
mod path_utils;
mod templates;

pub use error::ReportError;
pub use generator::ReportGenerator;
pub use hierarchy::{
    add_files_to_hierarchy, build_candidate_lookup, build_unified_hierarchy,
    create_file_groups_from_candidates,
};
pub use path_utils::{
    clean_directory_health_tree_paths, clean_entity_refs, clean_path_prefixes,
    clean_path_prefixes_in_file_groups, clean_path_string,
};
