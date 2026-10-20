mod annotations;
mod archive;
mod bundle;
mod includes;
mod resource_paths;
mod selection;

pub use bundle::{prepare_source, prepare_source_with_options, prepare_source_with_registry};
pub use resource_paths::resolve_resource;
pub(crate) use resource_paths::unique_resources;
pub use selection::{
    PrimaryDictionarySelection, select_primary_dictionary, select_primary_dictionary_with_registry,
};
