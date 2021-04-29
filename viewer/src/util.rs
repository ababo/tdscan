use wasm_bindgen::prelude::*;

use base::defs::{Error, Result};

pub type JsResult<T> = std::result::Result<T, JsValue>;

pub fn err_jsval(error: Error) -> JsValue {
    JsValue::from_str(error.to_string().as_str())
}

pub trait IntoJsResult<T> {
    fn res(self) -> JsResult<T>;
}

impl<T> IntoJsResult<T> for Result<T> {
    fn res(self) -> JsResult<T> {
        self.map_err(err_jsval)
    }
}
