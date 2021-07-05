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

// The async-syntax is avoided because of a known wasm-bindgen issue,
// see https://github.com/rustwasm/wasm-bindgen/issues/2195.

#[wasm_bindgen]
pub struct Viewer {
    controller: Rc<Controller<WebGlAdapter>>,
}

#[wasm_bindgen]
impl Viewer {
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
        let controller = self.controller.clone();
        let buffer = Cursor::new(js_sys::Uint8Array::new(&buffer).to_vec());

        future_to_promise(async move {
            let mut reader = fm::Reader::new(buffer).into_result()?;
            controller.load(&mut reader).await.into_result()?;
            Ok(JsValue::NULL)
        })
    }

    #[wasm_bindgen(js_name = renderAll)]
    pub fn render_all(&self) -> Promise {
        let controller = self.controller.clone();

        future_to_promise(async move {
            controller.render_all().await.into_result()?;
            Ok(JsValue::NULL)
        })
    }

    #[wasm_bindgen(js_name = renderMoment)]
    pub fn render_moment(&self, at: f64) -> StdResult<(), JsValue> {
        let at = Self::seconds_to_time(at);
        self.controller.render_moment(at).into_result()
    }

    #[wasm_bindgen(js_name = renderPeriod)]
    pub fn render_period(&self, from: f64, to: f64) -> Promise {
        let controller = self.controller.clone();
        let from = Self::seconds_to_time(from);
        let to = Self::seconds_to_time(to);

        future_to_promise(async move {
            controller.render_period(from, to).await.into_result()?;
            Ok(JsValue::NULL)
        })
    }

    #[wasm_bindgen(js_name = resetEyePosition)]
    pub fn reset_eye_position(&self) -> StdResult<(), JsValue> {
        self.controller.reset_eye_position().into_result()
    }

    fn seconds_to_time(seconds: f64) -> fm::Time {
        (seconds * 1E9) as fm::Time
    }
}
