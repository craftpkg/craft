pub mod pipes;

pub use pipes::{
    INSTALLED_LOCKFILE, InstallPipe, LinkerPipe, VIRTUAL_STORE_DIR, lockfile_from_graph,
    prune_node_modules,
};
