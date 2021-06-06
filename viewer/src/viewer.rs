use std::io::Cursor;
use std::rc::Rc;
use std::result::Result as StdResult;

use js_sys::{ArrayBuffer, Promise};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::future_to_promise;
use web_sys::HtmlCanvasElement;

use crate::controller::Controller;
use crate::defs::IntoJsResult;
use crate::webgl_adapter::WebGlAdapter;
use base::fm;
use base::fm::Read as _;
use base::model;

// The async-syntax is avoided because of a known wasm-bindgen issue,
// see https://github.com/rustwasm/wasm-bindgen/issues/2195.

#[wasm_bindgen]
pub struct Viewer {
    controller: Rc<Controller<WebGlAdapter>>,
}

#[wasm_bindgen]
impl Viewer {
    fn seconds_to_time(seconds: f64) -> model::Time {
        (seconds * 1E9) as model::Time
    }

    #[wasm_bindgen(js_name = animateAll)]
    pub fn animate_all(&self) -> Promise {
        let controller = self.controller.clone();

        future_to_promise(async move {
            controller.animate_all().await.into_result()?;
            Ok(JsValue::NULL)
        })
    }

    #[wasm_bindgen(js_name = animateRange)]
    pub fn animate_range(&self, from: f64, to: f64) -> Promise {
        let from = Self::seconds_to_time(from);
        let to = Self::seconds_to_time(to);
        let controller = self.controller.clone();

        future_to_promise(async move {
            controller.animate_range(from, to).await.into_result()?;
            Ok(JsValue::NULL)
        })
    }

    pub fn create(canvas: HtmlCanvasElement) -> StdResult<Viewer, JsValue> {
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();

        let adapter = WebGlAdapter::create(canvas).into_result()?;
        let controller = Controller::create(adapter).into_result()?;
        Ok(Viewer { controller }).into()
    }

    pub fn destroy(&self) {
        self.controller.destroy();
    }

    #[wasm_bindgen(js_name = loadFmBuffer)]
    pub fn load_fm_buffer(&self, buffer: ArrayBuffer) -> Promise {
        let buffer = js_sys::Uint8Array::new(&buffer).to_vec();

        let controller = self.controller.clone();
        controller.clear();

        future_to_promise(async move {
            let mut reader =
                fm::Reader::new(Cursor::new(buffer)).into_result()?;

            loop {
                match reader.read_record().into_result()? {
                    Some(rec) => {
                        controller.add_record(rec).await.into_result()?
                    }
                    None => break,
                }
            }

            Ok(JsValue::NULL)
        })
    }

    #[wasm_bindgen(js_name = showScene)]
    pub fn show_scene(&self, time: f64) -> StdResult<(), JsValue> {
        let time = Self::seconds_to_time(time);
        self.controller.show_scene(time).into_result()
    }
}
