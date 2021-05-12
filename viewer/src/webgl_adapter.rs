use std::rc::Rc;

use async_trait::async_trait;
use web_sys::{WebGlProgram, WebGlRenderingContext};

use crate::controller::Adapter;
#[allow(unused_imports)]
use crate::util::js;
use crate::util::webgl;
use base::defs::Result;
use base::model;

pub struct WebGlAdapter {
    #[allow(dead_code)]
    context: WebGlRenderingContext,
    #[allow(dead_code)]
    program: WebGlProgram,
}

impl WebGlAdapter {
    pub async fn create(
        context: WebGlRenderingContext,
    ) -> Result<Rc<WebGlAdapter>> {
        context.clear(
            WebGlRenderingContext::COLOR_BUFFER_BIT
                | WebGlRenderingContext::DEPTH_BUFFER_BIT,
        );
        context.enable(WebGlRenderingContext::DEPTH_TEST);
        context.enable(WebGlRenderingContext::CULL_FACE);
        context.front_face(WebGlRenderingContext::CCW);
        context.cull_face(WebGlRenderingContext::BACK);

        let vert_shader = webgl::compile_shader(
            &context,
            WebGlRenderingContext::VERTEX_SHADER,
            include_str!("shader/vert.glsl"),
        )?;

        let frag_shader = webgl::compile_shader(
            &context,
            WebGlRenderingContext::FRAGMENT_SHADER,
            include_str!("shader/frag.glsl"),
        )?;

        let program =
            webgl::link_program(&context, &vert_shader, &frag_shader)?;

        Ok(Rc::new(Self { context, program }))
    }
}

#[async_trait(?Send)]
impl Adapter for WebGlAdapter {
    async fn set_texture(
        self: &Rc<Self>,
        _index: usize,
        _image: model::Image,
    ) -> Result<()> {
        info!("WebGlAdapter::set_texture");
        Ok(())
    }
}
