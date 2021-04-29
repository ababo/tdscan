use std::io::Cursor;

use wasm_bindgen::prelude::*;

use crate::util::{IntoJsResult, JsResult};
use base::fm::Reader;

#[wasm_bindgen]
pub struct Viewer {}

#[wasm_bindgen]
impl Viewer {
    pub fn create(_canvas: &web_sys::HtmlCanvasElement) -> JsResult<Viewer> {
        info!("Viewer::create");
        Ok(Viewer {})
    }

    #[wasm_bindgen(js_name = loadFmBuffer)]
    pub fn load_fm_buffer(
        &mut self,
        buffer: &js_sys::ArrayBuffer,
    ) -> JsResult<()> {
        info!("Viewer::load_fm_buffer");

        let data = js_sys::Uint8Array::new(buffer).to_vec();
        let mut reader = Reader::from_reader(Cursor::new(data)).res()?;
        loop {
            let rec = reader.read_record().res()?;
            match rec {
                Some(_) => info!("record read"),
                None => break,
            }
        }

        Ok(())
    }
}
