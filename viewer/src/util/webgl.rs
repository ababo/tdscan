use std::any::type_name;
use std::mem::size_of;

use glam::Mat4;
use web_sys::{WebGlProgram, WebGlRenderingContext, WebGlShader};

use base::defs::{Error, ErrorKind::WebGlError, Result};

pub fn compile_shader(
    context: &WebGlRenderingContext,
    shader_type: u32,
    source: &str,
) -> Result<WebGlShader> {
    let shader = context.create_shader(shader_type).unwrap();

    context.shader_source(&shader, source);
    context.compile_shader(&shader);

    if !context
        .get_shader_parameter(&shader, WebGlRenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        let msg = context.get_shader_info_log(&shader).unwrap();
        let desc = format!("failed to create WebGL shader: {}", msg);
        return Err(Error::new(WebGlError, desc));
    }

    Ok(shader)
}

pub fn link_program(
    context: &WebGlRenderingContext,
    vert_shader: &WebGlShader,
    frag_shader: &WebGlShader,
) -> Result<WebGlProgram> {
    let program = context.create_program().unwrap();

    context.attach_shader(&program, vert_shader);
    context.attach_shader(&program, frag_shader);
    context.link_program(&program);

    if !context
        .get_program_parameter(&program, WebGlRenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        let msg = context.get_program_info_log(&program).unwrap();
        let desc = format!("failed to link WebGL program: {}", msg);
        return Err(Error::new(WebGlError, desc));
    }

    Ok(program)
}

pub fn define_attribute<T>(
    context: &WebGlRenderingContext,
    program: &WebGlProgram,
    name: &str,
    size: usize,
    stride: usize,
    offset: usize,
) -> Result<()> {
    let r#type = match type_name::<T>() {
        "f32" => WebGlRenderingContext::FLOAT,
        "i8" => WebGlRenderingContext::BYTE,
        "i16" => WebGlRenderingContext::SHORT,
        "u8" => WebGlRenderingContext::UNSIGNED_BYTE,
        "u16" => WebGlRenderingContext::UNSIGNED_SHORT,
        _ => panic!("unsupported WebGL attribute type"),
    };

    let location = context.get_attrib_location(&program, name);
    if location == -1 {
        let desc = format!("failed to find WebGL program attribute '{}'", name);
        return Err(Error::new(WebGlError, desc));
    }

    context.vertex_attrib_pointer_with_i32(
        location as u32,
        (size / size_of::<T>()) as i32,
        r#type,
        false,
        stride as i32,
        offset as i32,
    );

    context.enable_vertex_attrib_array(location as u32);

    Ok(())
}

pub fn set_uniform_mat4(
    context: &WebGlRenderingContext,
    program: &WebGlProgram,
    name: &str,
    matrix: &Mat4,
) -> Result<()> {
    let location =
        context.get_uniform_location(program, name).ok_or_else(|| {
            let desc = format!("failed to find WeGL uniform '{}'", name);
            Error::new(WebGlError, desc)
        })?;

    context.uniform_matrix4fv_with_f32_array(
        Some(&location),
        false,
        matrix.as_ref(),
    );

    Ok(())
}
