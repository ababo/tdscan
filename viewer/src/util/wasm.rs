use js_sys::{Uint8Array, WebAssembly};
use wasm_bindgen::JsCast;

#[allow(dead_code)]
pub fn new_uint8_array(slice: &[u8]) -> Uint8Array {
    let buf = wasm_bindgen::memory()
        .dyn_into::<WebAssembly::Memory>()
        .unwrap()
        .buffer();
    let loc = slice.as_ptr() as u32 / 2;
    Uint8Array::new(&buf).subarray(loc, loc + slice.len() as u32)
}
