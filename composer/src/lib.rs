// This file makes `composer` into a rust library crate.

// It is useful for debugging.
// It allows `composer` to be loaded into the evcxr jupyter kernel.

// The file `main.rs` still exists to make `composer` into an executable.

pub mod build_view;
pub mod combine;
pub mod export_obj;
pub mod export_to_json;
pub mod import_obj;
pub mod mesh;
pub mod misc;
pub mod optimize_scan_geometry;
pub mod point_cloud;
mod poisson;
pub mod scan;
pub mod select;

pub mod texture;

pub use base;
pub use base::fm;

/*
The following changes are necessary as long as `lib.rs` exists:
 * write `pub use composer;` in main.rs (remove `pub`?)
 * put the following in `build_view.rs`:
     + `use indexmap::map::IndexMap;`
     + `pub params: BuildViewParams`
     + `dbg_*` functions
 * run cargo and jupyter-lab with `env -u CXXSTDLIB`
*/ 