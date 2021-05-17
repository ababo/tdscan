use std::mem::size_of;
use std::rc::Rc;

use async_trait::async_trait;
use memoffset::offset_of;
use web_sys::{WebGlProgram, WebGlRenderingContext};
use zerocopy::AsBytes as _;

use crate::controller::{Adapter, Face, Vertex};
use crate::defs::IntoResult;
use crate::util::wasm;
use crate::util::web;
use crate::util::webgl;
use base::defs::Result;
use base::model;

pub struct WebGlAdapter {
    context: WebGlRenderingContext,
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
        context.use_program(Some(&program));

        let buf = context.create_buffer().unwrap();
        context.bind_buffer(WebGlRenderingContext::ARRAY_BUFFER, Some(&buf));
        webgl::define_attribute::<f32>(
            &context,
            &program,
            "texture",
            size_of::<model::Point2>(),
            size_of::<Vertex>(),
            offset_of!(Vertex, texture),
        )?;
        webgl::define_attribute::<f32>(
            &context,
            &program,
            "position",
            size_of::<model::Point3>(),
            size_of::<Vertex>(),
            offset_of!(Vertex, position),
        )?;
        webgl::define_attribute::<f32>(
            &context,
            &program,
            "normal",
            size_of::<model::Point3>(),
            size_of::<Vertex>(),
            offset_of!(Vertex, normal),
        )?;

        Ok(Rc::new(Self { context, program }))
    }
}

fn texture_num(index: usize) -> u32 {
    WebGlRenderingContext::TEXTURE0 + index as u32
}

#[async_trait(?Send)]
impl Adapter for WebGlAdapter {
    async fn set_faces(self: &Rc<Self>, faces: &[Face]) -> Result<()> {
        let buf = self.context.create_buffer().unwrap();
        self.context.bind_buffer(
            WebGlRenderingContext::ELEMENT_ARRAY_BUFFER,
            Some(&buf),
        );

        self.context.buffer_data_with_array_buffer_view(
            WebGlRenderingContext::ELEMENT_ARRAY_BUFFER,
            &wasm::new_uint8_array(faces.as_bytes()),
            WebGlRenderingContext::STATIC_DRAW,
        );

        Ok(())
    }

    async fn set_texture(
        self: &Rc<Self>,
        index: usize,
        image: model::Image,
    ) -> Result<()> {
        let texture = self.context.create_texture().unwrap();
        self.context
            .bind_texture(WebGlRenderingContext::TEXTURE_2D, Some(&texture));
        self.context.tex_parameteri(
            WebGlRenderingContext::TEXTURE_2D,
            WebGlRenderingContext::TEXTURE_WRAP_S,
            WebGlRenderingContext::CLAMP_TO_EDGE as i32,
        );
        self.context.tex_parameteri(
            WebGlRenderingContext::TEXTURE_2D,
            WebGlRenderingContext::TEXTURE_WRAP_T,
            WebGlRenderingContext::CLAMP_TO_EDGE as i32,
        );
        self.context.tex_parameteri(
            WebGlRenderingContext::TEXTURE_2D,
            WebGlRenderingContext::TEXTURE_MIN_FILTER,
            WebGlRenderingContext::LINEAR as i32,
        );
        self.context.tex_parameteri(
            WebGlRenderingContext::TEXTURE_2D,
            WebGlRenderingContext::TEXTURE_MAG_FILTER,
            WebGlRenderingContext::LINEAR as i32,
        );

        self.context
            .tex_image_2d_with_u32_and_u32_and_image(
                WebGlRenderingContext::TEXTURE_2D,
                0,
                WebGlRenderingContext::RGBA as i32,
                WebGlRenderingContext::RGBA,
                WebGlRenderingContext::UNSIGNED_BYTE,
                &web::decode_image(&image).await?,
            )
            .into_result()?;

        let location = self.context.get_uniform_location(
            &self.program,
            &format!("textures[{}]", index),
        );
        self.context.uniform1i(location.as_ref(), index as i32);
        self.context.active_texture(texture_num(index));
        self.context
            .bind_texture(WebGlRenderingContext::TEXTURE_2D, Some(&texture));

        Ok(())
    }
}
