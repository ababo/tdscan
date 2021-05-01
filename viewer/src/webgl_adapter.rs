use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, WebGlRenderingContext};

use crate::controller::Adapter;
use crate::defs::IntoResult;
use crate::util::webgl::{compile_shader, link_program};
use base::defs::{Error, ErrorKind::JsError, Result};

pub struct WebGlAdapter {
    context: WebGlRenderingContext,
}

impl WebGlAdapter {
    pub fn create(canvas: &HtmlCanvasElement) -> Result<WebGlAdapter> {
        let context = canvas.get_context("webgl").res()?;
        if context.is_none() {
            let desc = format!("failed to get WebGL context from canvas");
            return Err(Error::new(JsError, desc));
        }

        let context = context
            .unwrap()
            .dyn_into::<WebGlRenderingContext>()
            .unwrap();

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

        Ok(WebGlAdapter { context })
    }
}

impl Adapter for WebGlAdapter {}
