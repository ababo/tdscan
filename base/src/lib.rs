pub mod defs;
pub mod util;

pub mod model {
    include!(concat!(env!("OUT_DIR"), "/base.model.rs"));
}
