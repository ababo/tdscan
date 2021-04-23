mod util;
mod viewer;

use wasm_bindgen::prelude::*;

use base::defs::Error;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

pub fn err_to_jsvalue(error: Error) -> JsValue {
    JsValue::from_str(error.to_string().as_str())
}
