use wasm_bindgen::prelude::*;
use web_sys::{WebGlProgram, WebGlRenderingContext, WebGlShader};

use crate::defs::JsResult;

pub fn compile_shader(
    context: &WebGlRenderingContext,
    shader_type: u32,
    source: &str,
) -> JsResult<WebGlShader> {
    let shader = context.create_shader(shader_type).unwrap();

    context.shader_source(&shader, source);
    context.compile_shader(&shader);

    if !context
        .get_shader_parameter(&shader, WebGlRenderingContext::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        let err_str = context.get_shader_info_log(&shader).unwrap();
        return Err(JsValue::from_str(
            format!("failed to create shader: {}", err_str).as_str(),
        ));
    }

    Ok(shader)
}

pub fn link_program(
    context: &WebGlRenderingContext,
    vert_shader: &WebGlShader,
    frag_shader: &WebGlShader,
) -> JsResult<WebGlProgram> {
    let program = context.create_program().unwrap();

    context.attach_shader(&program, vert_shader);
    context.attach_shader(&program, frag_shader);
    context.link_program(&program);

    if !context
        .get_program_parameter(&program, WebGlRenderingContext::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        let err_str = context.get_program_info_log(&program).unwrap();
        return Err(JsValue::from_str(
            format!("failed to link program: {}", err_str).as_str(),
        ));
    }

    Ok(program)
}
