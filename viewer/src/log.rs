use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);

    #[wasm_bindgen(js_namespace = console)]
    pub fn debug(s: &str);

    #[wasm_bindgen(js_namespace = console)]
    pub fn info(s: &str);

    #[wasm_bindgen(js_namespace = console)]
    pub fn warn(s: &str);

    #[wasm_bindgen(js_namespace = console)]
    pub fn error(s: &str);
}

#[macro_export]
macro_rules! log {
    ($($t:tt)*) => (crate::log::log(&format_args!($($t)*).to_string()))
}

#[cfg(debug)]
#[macro_export]
macro_rules! debug {
    ($($t:tt)*) => (crate::log::debug(&format_args!($($t)*).to_string()))
}

#[cfg(not(debug))]
#[macro_export]
macro_rules! debug {
    ($($t:tt)*) => {};
}

#[macro_export]
macro_rules! info {
    ($($t:tt)*) => (crate::log::info(&format_args!($($t)*).to_string()))
}

#[macro_export]
macro_rules! warn {
    ($($t:tt)*) => (crate::log::warn(&format_args!($($t)*).to_string()))
}

#[macro_export]
macro_rules! error {
    ($($t:tt)*) => (crate::log::error(&format_args!($($t)*).to_string()))
}
