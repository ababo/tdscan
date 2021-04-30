use std::collections::HashMap;
use std::io::Cursor;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::WebGlRenderingContext;

use crate::defs::{IntoJsResult, JsResult};
use crate::util::webgl::{compile_shader, link_program};
use base::fm::Reader;
use base::model;

#[wasm_bindgen]
pub struct Viewer {
    gl_context: WebGlRenderingContext,
    element_views: HashMap<String, model::ElementView>,
    element_view_states: HashMap<String, Vec<model::ElementViewState>>,
}

#[wasm_bindgen]
impl Viewer {
    pub fn create(canvas: &web_sys::HtmlCanvasElement) -> JsResult<Viewer> {
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();

        let context = canvas.get_context("webgl")?.unwrap();
        let context = context.dyn_into::<WebGlRenderingContext>()?;

        context.clear(
            WebGlRenderingContext::COLOR_BUFFER_BIT
                | WebGlRenderingContext::DEPTH_BUFFER_BIT,
        );
        context.enable(WebGlRenderingContext::DEPTH_TEST);
        context.enable(WebGlRenderingContext::CULL_FACE);
        context.front_face(WebGlRenderingContext::CCW);
        context.cull_face(WebGlRenderingContext::BACK);

        let vert_shader = compile_shader(
            &context,
            WebGlRenderingContext::VERTEX_SHADER,
            include_str!("shader/vert.glsl"),
        )?;

        let frag_shader = compile_shader(
            &context,
            WebGlRenderingContext::FRAGMENT_SHADER,
            include_str!("shader/frag.glsl"),
        )?;

        let _program = link_program(&context, &vert_shader, &frag_shader)?;

        Ok(Viewer {
            gl_context: context,
            element_views: HashMap::new(),
            element_view_states: HashMap::new(),
        })
    }

    #[wasm_bindgen(js_name = loadFmBuffer)]
    pub fn load_fm_buffer(
        &mut self,
        buffer: &js_sys::ArrayBuffer,
    ) -> JsResult<()> {
        let data = js_sys::Uint8Array::new(buffer).to_vec();
        let mut reader = Reader::from_reader(Cursor::new(data)).res()?;

        loop {
            match reader.read_record().res()? {
                Some(rec) => self.add_fm_record(rec),
                None => break,
            }
        }

        Ok(())
    }

    fn add_fm_record(&mut self, record: model::Record) {
        use model::record::Type::*;
        match record.r#type {
            Some(ElementView(v)) => {
                self.element_views.insert(v.element.clone(), v);
            }
            Some(ElementViewState(s)) => {
                match self.element_view_states.get_mut(&s.element) {
                    Some(states) => states.push(s),
                    None => {
                        self.element_view_states
                            .insert(s.element.clone(), vec![s]);
                    }
                };
            }
            _ => (),
        }
    }

    pub fn seek(&mut self, _to: &js_sys::Number) -> JsResult<()> {
        let _ = &self.gl_context;
        Ok(())
    }
}
