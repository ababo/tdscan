use std::f32::consts::PI;
use std::mem::size_of;
use std::rc::Rc;
use std::slice::from_raw_parts;

use async_trait::async_trait;
use glam::{Mat4, Vec3};
use js_sys::{Uint16Array, Uint8Array};
use memoffset::offset_of;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext, WebGlProgram};

use crate::controller::{Adapter, Face, MouseEvent, Vertex};
use crate::defs::IntoResult;
use crate::util::glam::point3_to_vec3;
use crate::util::web;
use crate::util::webgl;
use base::defs::Result;
use base::model;

pub struct WebGlAdapter {
    canvas: HtmlCanvasElement,
    context: WebGl2RenderingContext,
    program: WebGlProgram,
}

impl WebGlAdapter {
    pub fn create(canvas: HtmlCanvasElement) -> Result<Rc<WebGlAdapter>> {
        let context = canvas.get_context("webgl2").into_result()?.unwrap();
        let context = context.dyn_into::<WebGl2RenderingContext>().unwrap();

        context.clear(
            WebGl2RenderingContext::COLOR_BUFFER_BIT
                | WebGl2RenderingContext::DEPTH_BUFFER_BIT,
        );
        context.enable(WebGl2RenderingContext::DEPTH_TEST);
        context.enable(WebGl2RenderingContext::CULL_FACE);
        context.front_face(WebGl2RenderingContext::CCW);
        context.cull_face(WebGl2RenderingContext::BACK);

        let vert_shader = webgl::compile_shader(
            &context,
            WebGl2RenderingContext::VERTEX_SHADER,
            include_str!("shader/vert.glsl"),
        )?;

        let max_num_textures = context
            .get_parameter(WebGl2RenderingContext::MAX_TEXTURE_IMAGE_UNITS)
            .unwrap()
            .as_f64()
            .unwrap() as u32;

        let frag_shader = webgl::compile_shader(
            &context,
            WebGl2RenderingContext::FRAGMENT_SHADER,
            &include_str!("shader/frag.glsl").replace(
                "MAX_TEXTURE_IMAGE_UNITS",
                &format!("{}", max_num_textures),
            ),
        )?;

        let program =
            webgl::link_program(&context, &vert_shader, &frag_shader)?;
        context.use_program(Some(&program));

        let buf = context.create_buffer().unwrap();
        context.bind_buffer(WebGl2RenderingContext::ARRAY_BUFFER, Some(&buf));

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

        let adapter = Rc::new(Self {
            canvas,
            context,
            program,
        });

        adapter.set_projection()?;

        Ok(adapter)
    }

    fn set_projection(self: &Rc<Self>) -> Result<()> {
        let width = self.canvas.client_width() as f32;
        let height = self.canvas.client_height() as f32;

        let projection = Mat4::perspective_rh_gl(
            45.0 * PI / 180.0,
            width / height,
            0.1,
            1000.0,
        );

        webgl::set_uniform_mat4(
            &self.context,
            &self.program,
            "projection",
            &projection,
        )
    }
}

fn texture_num(index: usize) -> u32 {
    WebGl2RenderingContext::TEXTURE0 + index as u32
}

#[async_trait(?Send)]
impl Adapter for WebGlAdapter {
    type Subscription = web::Subscription;

    fn destroy(self: &Rc<Self>) {}

    fn render_frame(self: &Rc<Self>) -> Result<()> {
        let size = self.context.get_buffer_parameter(
            WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER,
            WebGl2RenderingContext::BUFFER_SIZE,
        );

        let size = size.as_f64().unwrap() as usize / size_of::<u16>();

        self.context.draw_elements_with_i32(
            WebGl2RenderingContext::TRIANGLES,
            size as i32,
            WebGl2RenderingContext::UNSIGNED_SHORT,
            0,
        );

        Ok(())
    }

    fn set_faces(self: &Rc<Self>, faces: &[Face]) -> Result<()> {
        let buf = self.context.create_buffer().unwrap();
        self.context.bind_buffer(
            WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER,
            Some(&buf),
        );

        let indexes: &[u16] = unsafe {
            from_raw_parts(
                &faces[0] as *const Face as *const u16,
                faces.len() * size_of::<Face>() / size_of::<u16>(),
            )
        };

        self.context.buffer_data_with_array_buffer_view(
            WebGl2RenderingContext::ELEMENT_ARRAY_BUFFER,
            &Uint16Array::from(indexes),
            WebGl2RenderingContext::STATIC_DRAW,
        );

        Ok(())
    }

    async fn set_texture(
        self: &Rc<Self>,
        index: usize,
        image: model::Image,
    ) -> Result<()> {
        self.context.active_texture(texture_num(index));

        let texture = self.context.create_texture().unwrap();
        self.context
            .bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture));

        self.context.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_WRAP_S,
            WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
        );
        self.context.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_WRAP_T,
            WebGl2RenderingContext::CLAMP_TO_EDGE as i32,
        );
        self.context.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MIN_FILTER,
            WebGl2RenderingContext::LINEAR as i32,
        );
        self.context.tex_parameteri(
            WebGl2RenderingContext::TEXTURE_2D,
            WebGl2RenderingContext::TEXTURE_MAG_FILTER,
            WebGl2RenderingContext::LINEAR as i32,
        );

        self.context
            .tex_image_2d_with_u32_and_u32_and_html_image_element(
                WebGl2RenderingContext::TEXTURE_2D,
                0,
                WebGl2RenderingContext::RGBA as i32,
                WebGl2RenderingContext::RGBA,
                WebGl2RenderingContext::UNSIGNED_BYTE,
                &web::decode_image(&image).await?,
            )
            .into_result()?;

        let location = webgl::get_uniform_location(
            &self.context,
            &self.program,
            &format!("textures[{}]", index),
        )?;
        self.context.uniform1i(Some(&location), index as i32);

        self.context
            .bind_texture(WebGl2RenderingContext::TEXTURE_2D, Some(&texture));

        Ok(())
    }

    fn set_texture_index(self: &Rc<Self>, index: &[u16]) -> Result<()> {
        let index: Vec<i32> = index.iter().map(|v| *v as i32).collect();
        webgl::set_uniform_i32_array(
            &self.context,
            &self.program,
            "texture_index",
            &index,
        )
    }

    fn set_vertices(self: &Rc<Self>, vertices: &[Vertex]) -> Result<()> {
        let bytes: &[u8] = unsafe {
            from_raw_parts(
                &vertices[0] as *const Vertex as *const u8,
                vertices.len() * size_of::<Vertex>(),
            )
        };

        self.context.buffer_data_with_array_buffer_view(
            WebGl2RenderingContext::ARRAY_BUFFER,
            &Uint8Array::from(bytes),
            WebGl2RenderingContext::STATIC_DRAW,
        );

        Ok(())
    }

    fn set_eye_position(self: &Rc<Self>, eye: &model::Point3) -> Result<()> {
        let eye = point3_to_vec3(eye);
        let center = Vec3::new(0.0, 0.0, 0.0);
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_at_rh(eye, center, up);
        webgl::set_uniform_mat4(&self.context, &self.program, "view", &view)
    }

    fn subscribe_to_mouse_move<F: Fn(&MouseEvent) + 'static>(
        self: &Rc<Self>,
        handler: F,
    ) -> Result<Self::Subscription> {
        let sub = web::subscribe(&self.canvas, "mousemove", move |e| {
            let event = web_sys::MouseEvent::unchecked_from_js_ref(e.as_ref());
            handler(&MouseEvent {
                dx: event.movement_x() as f32,
                dy: event.movement_y() as f32,
                primary_button: event.buttons() & 1 != 0,
            });
        })?;
        Ok(sub)
    }

    fn subscribe_to_mouse_wheel<F: Fn(&MouseEvent) + 'static>(
        self: &Rc<Self>,
        handler: F,
    ) -> Result<Self::Subscription> {
        let sub = web::subscribe(&self.canvas, "wheel", move |e| {
            let event = web_sys::WheelEvent::unchecked_from_js_ref(e.as_ref());
            handler(&MouseEvent {
                dy: event.delta_y() as f32,
                ..Default::default()
            });
        })?;
        Ok(sub)
    }
}
