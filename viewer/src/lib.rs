#[macro_use]
mod log;
mod controller;
mod defs;
mod util;
mod viewer;
mod webgl_adapter;

#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;
