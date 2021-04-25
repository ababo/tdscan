use wasm_bindgen::prelude::*;

use crate::util::err_jsval;
use base::fm;
use base::model;

#[wasm_bindgen]
pub struct Viewer {}

#[wasm_bindgen]
impl Viewer {
    #[wasm_bindgen(js_name = fromModelBuffer)]
    pub fn from_model_buffer(
        buffer: &js_sys::ArrayBuffer,
    ) -> Result<Viewer, JsValue> {
        info!("Viewer::from_model_buffer");

        let data = js_sys::Uint8Array::new(buffer).to_vec();
        let model = fm::decode(data.as_slice()).map_err(err_jsval)?;

        Self::from_model(&model)
    }

    fn from_model(_model: &model::Model) -> Result<Viewer, JsValue> {
        Ok(Viewer {})
    }

    pub fn start(
        &mut self,
        _canvas: &web_sys::HtmlCanvasElement,
    ) -> Result<(), JsValue> {
        info!("Viewer::start");
        Ok(())
    }
}
