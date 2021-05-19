use std::io::Cursor;
use std::rc::Rc;

use js_sys::{ArrayBuffer, Promise};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::future_to_promise;
use web_sys::WebGlRenderingContext;

use crate::controller::{Controller, Time};
use crate::defs::IntoJsResult;
use crate::webgl_adapter::WebGlAdapter;
use base::fm;
use base::fm::Read as _;

// The async-syntax is avoided because of a known wasm-bindgen issue,
// see https://github.com/rustwasm/wasm-bindgen/issues/2195.

#[wasm_bindgen]
pub struct Viewer {
    controller: Rc<Controller<WebGlAdapter>>,
}

#[wasm_bindgen]
impl Viewer {
    pub fn create(context: WebGlRenderingContext) -> Promise {
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();

        future_to_promise(async move {
            let adapter = WebGlAdapter::create(context).await.into_result()?;
            let controller = Controller::new(adapter).await.into_result()?;
            Ok(Viewer { controller }.into())
        })
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

    #[wasm_bindgen(js_name = renderFrame)]
    pub fn render_frame(&self, time: Time) -> Promise {
        let controller = self.controller.clone();

        future_to_promise(async move {
            controller.render_frame(time).await.into_result()?;
            Ok(JsValue::NULL)
        })
    }
}
