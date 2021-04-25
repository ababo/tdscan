use wasm_bindgen::prelude::*;

use base::defs::Error;

pub fn err_jsval(error: Error) -> JsValue {
    JsValue::from_str(error.to_string().as_str())
}
