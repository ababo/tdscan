use std::io::Cursor;

use wasm_bindgen::prelude::wasm_bindgen;
use web_sys::HtmlCanvasElement;

use crate::controller::Controller;
use crate::defs::{IntoJsResult, JsResult};
use crate::webgl_adapter::WebGlAdapter;
use base::fm::Reader;

#[wasm_bindgen]
pub struct Viewer {
    controller: Controller<WebGlAdapter>,
}

#[wasm_bindgen]
impl Viewer {
    pub fn create(canvas: &HtmlCanvasElement) -> JsResult<Viewer> {
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();

        let adapter = WebGlAdapter::create(canvas).res()?;
        let controller = Controller::new(adapter);

        Ok(Viewer { controller })
    }

    #[wasm_bindgen(js_name = loadFmBuffer)]
    pub fn load_fm_buffer(
        &mut self,
        buffer: &js_sys::ArrayBuffer,
    ) -> JsResult<()> {
        self.controller.clear();

        let data = js_sys::Uint8Array::new(buffer).to_vec();
        let mut reader = Reader::from_reader(Cursor::new(data)).res()?;

        loop {
            match reader.read_record().res()? {
                Some(rec) => self.controller.add_record(rec).res()?,
                None => break,
            }
        }

        Ok(())
    }
}
