// This file makes `composer` into a rust library crate.

// It is useful for debugging.
// It allows `composer` to be loaded into the evcxr jupyter kernel.

// The file `main.rs` still exists to make `composer` into an executable.

pub mod build_view;
pub mod combine;
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

// re-exporting these so evcxr gets the same package versions
/*pub use nalgebra;
pub use petgraph;
pub use simplelog;
pub use structopt;
pub use log;*/
