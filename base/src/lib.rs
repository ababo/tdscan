pub mod defs;
pub mod fm;
#[macro_use]
pub mod util;

pub mod model {
    include!(concat!(env!("OUT_DIR"), "/base.model.rs"));
}
