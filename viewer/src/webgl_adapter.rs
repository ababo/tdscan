use std::cell::Cell;
use std::f32::consts::PI;
use std::mem::size_of;
use std::rc::Rc;
use std::slice::from_raw_parts;

use async_trait::async_trait;
use glam::{Mat4, Vec3};
use js_sys::{Uint16Array, Uint8Array};
use memoffset::offset_of;
use wasm_bindgen::JsCast;
use web_sys::{HtmlCanvasElement, WebGlProgram, WebGlRenderingContext};

use crate::controller::{Adapter, Face, MouseEvent, VertexData};
use crate::defs::IntoResult;
use crate::util::web;
use crate::util::webgl;
use base::defs::Result;
use base::fm;
use base::util::glam::point3_to_vec3;

pub struct WebGlAdapter {
    canvas: HtmlCanvasElement,
    context: WebGlRenderingContext,
    now_offset: Cell<fm::Time>,
    program: WebGlProgram,
}

impl WebGlAdapter {
    pub fn create(canvas: HtmlCanvasElement) -> Result<Rc<WebGlAdapter>> {
        let context = canvas.get_context("webgl").into_result()?.unwrap();
        let context = context.dyn_into::<WebGlRenderingContext>().unwrap();

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

        let max_num_textures = context
            .get_parameter(WebGlRenderingContext::MAX_TEXTURE_IMAGE_UNITS)
            .unwrap()
            .as_f64()
            .unwrap() as u32;

        let frag_shader = webgl::compile_shader(
            &context,
            WebGlRenderingContext::FRAGMENT_SHADER,
            &include_str!("shader/frag.glsl").replace(
                "MAX_TEXTURE_IMAGE_UNITS",
                &format!("{}", max_num_textures),
            ),
        )?;

        let program =
            webgl::link_program(&context, &vert_shader, &frag_shader)?;
        context.use_program(Some(&program));

        let buf = context.create_buffer().unwrap();
        context.bind_buffer(WebGlRenderingContext::ARRAY_BUFFER, Some(&buf));

        webgl::define_attribute::<u8>(
            &context,
            &program,
            "element",
            size_of::<u8>(),
            size_of::<VertexData>(),
            offset_of!(VertexData, element),
        )?;

        webgl::define_attribute::<f32>(
            &context,
            &program,
            "texture",
            size_of::<fm::Point2>(),
            size_of::<VertexData>(),
            offset_of!(VertexData, texture),
        )?;

        webgl::define_attribute::<f32>(
            &context,
            &program,
            "vertex",
            size_of::<fm::Point3>(),
            size_of::<VertexData>(),
            offset_of!(VertexData, vertex),
        )?;

        let adapter = Rc::new(Self {
            canvas,
            context,
            now_offset: Cell::new(0),
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
    WebGlRenderingContext::TEXTURE0 + index as u32
}

fn milliseconds_to_time(milliseconds: f64) -> fm::Time {
    (milliseconds * 1000000.0) as fm::Time
}

#[async_trait(?Send)]
impl Adapter for WebGlAdapter {
    type Subscription = web::Subscription;

    fn destroy(self: &Rc<Self>) {}

    async fn next_frame(self: &Rc<Self>) -> fm::Time {
        let now = web::next_frame().await;
        milliseconds_to_time(now) + self.now_offset.get()
    }

    fn render_frame(self: &Rc<Self>) -> Result<()> {
        let size = self.context.get_buffer_parameter(
            WebGlRenderingContext::ELEMENT_ARRAY_BUFFER,
            WebGlRenderingContext::BUFFER_SIZE,
        );

        let size = size.as_f64().unwrap() as usize / size_of::<u16>();

        self.context.draw_elements_with_i32(
            WebGlRenderingContext::TRIANGLES,
            size as i32,
            WebGlRenderingContext::UNSIGNED_SHORT,
            0,
        );

        Ok(())
    }

    fn set_faces(self: &Rc<Self>, faces: &[Face]) -> Result<()> {
        let buf = self.context.create_buffer().unwrap();
        self.context.bind_buffer(
            WebGlRenderingContext::ELEMENT_ARRAY_BUFFER,
            Some(&buf),
        );

        let indexes: &[u16] = unsafe {
            from_raw_parts(
                &faces[0] as *const Face as *const u16,
                faces.len() * size_of::<Face>() / size_of::<u16>(),
            )
        };

        self.context.buffer_data_with_array_buffer_view(
            WebGlRenderingContext::ELEMENT_ARRAY_BUFFER,
            &Uint16Array::from(indexes),
            WebGlRenderingContext::STATIC_DRAW,
        );

        Ok(())
    }

    async fn set_now(self: &Rc<Self>, now: fm::Time) {
        // Cannot use performance.now() here because of inconsistency
        // with requestAnimationFrame() timestamps on Chrome, see
        // https://stackoverflow.com/a/46121920/2772588.
        let jsnow = web::next_frame().await;
        self.now_offset.set(now - milliseconds_to_time(jsnow));
    }

    async fn set_texture(
        self: &Rc<Self>,
        index: usize,
        image: fm::Image,
    ) -> Result<()> {
        self.context.active_texture(texture_num(index));

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

        let location = webgl::get_uniform_location(
            &self.context,
            &self.program,
            &format!("textures[{}]", index),
        )?;
        self.context.uniform1i(Some(&location), index as i32);

        self.context
            .bind_texture(WebGlRenderingContext::TEXTURE_2D, Some(&texture));

        Ok(())
    }

    fn set_vertices(self: &Rc<Self>, vertices: &[VertexData]) -> Result<()> {
        let bytes: &[u8] = unsafe {
            from_raw_parts(
                &vertices[0] as *const VertexData as *const u8,
                vertices.len() * size_of::<VertexData>(),
            )
        };

        self.context.buffer_data_with_array_buffer_view(
            WebGlRenderingContext::ARRAY_BUFFER,
            &Uint8Array::from(bytes),
            WebGlRenderingContext::STATIC_DRAW,
        );

        Ok(())
    }

    fn set_eye_position(self: &Rc<Self>, eye: &fm::Point3) -> Result<()> {
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
