use js_sys::Error as JsError;
use wasm_bindgen::{JsCast, JsValue};

use base::defs::{Error, ErrorKind, Result};

pub type JsResult<T> = std::result::Result<T, JsValue>;

pub fn jsval_to_err(value: JsValue) -> Error {
    let desc = if let Some(err) = value.dyn_ref::<JsError>() {
        let msg: String = err.message().into();
        format!("{}", msg)
    } else if let Some(r#str) = value.as_string() {
        let msg: String = r#str.into();
        format!("{}", msg)
    } else {
        format!("{:?}", value)
    };
    Error::new(ErrorKind::JsError, format!("{:?}", desc))
}

pub trait IntoResult<T> {
    fn res(self) -> Result<T>;
}

impl<T> IntoResult<T> for JsResult<T> {
    fn res(self) -> Result<T> {
        self.map_err(jsval_to_err)
    }
}

pub fn err_to_jsval(error: Error) -> JsValue {
    JsValue::from_str(error.to_string().as_str())
}

pub trait IntoJsResult<T> {
    fn res(self) -> JsResult<T>;
}

impl<T> IntoJsResult<T> for Result<T> {
    fn res(self) -> JsResult<T> {
        self.map_err(err_to_jsval)
    }
}
