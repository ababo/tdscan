use wasm_bindgen::prelude::*;

use crate::console_log;
use base::model;

#[wasm_bindgen]
pub struct Viewer {}

#[wasm_bindgen]
impl Viewer {
    #[wasm_bindgen(js_name = fromModelBuffer)]
    pub fn from_model_buffer(
        _buffer: &js_sys::ArrayBuffer,
    ) -> Result<Viewer, JsValue> {
        console_log!("Viewer::from_model_buffer");
        let model = model::Model {
            ..Default::default()
        };
        Self::from_model(&model)
    }

    fn from_model(_model: &model::Model) -> Result<Viewer, JsValue> {
        Ok(Viewer {})
    }

    pub fn start(
        &mut self,
        _canvas: &web_sys::HtmlCanvasElement,
    ) -> Result<(), JsValue> {
        console_log!("Viewer::start");
        Ok(())
    }
}
