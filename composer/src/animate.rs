use std::io::{stdin, stdout};
use std::path::PathBuf;

use rlua;
use structopt::StructOpt;

use base::defs::{Error, ErrorKind::*, Result};
use base::fm;
use base::model;
use base::util::fs;

#[derive(StructOpt)]
#[structopt(about = "Animate element using .lua script")]
pub struct AnimateParams {
    #[structopt(help = "Input .fm file (STDIN if omitted)")]
    in_path: Option<PathBuf>,
    #[structopt(help = "Input .lua animation script", short = "s")]
    script_path: PathBuf,
    #[structopt(help = "Timestep", short = "t")]
    timestep: i64,
    #[structopt(help = "Number of iterations", short = "n")]
    num_iters: i64,
    #[structopt(
        help = "Output .fm file (STDOUT if omitted)",
        long,
        short = "o"
    )]
    out_path: Option<PathBuf>,
    #[structopt(flatten)]
    fm_write_params: fm::WriterParams,
}

pub fn animate_with_params(params: &AnimateParams) -> Result<()> {
    let mut reader = if let Some(path) = &params.in_path {
        let reader = fm::Reader::new(fs::open_file(path)?)?;
        Box::new(reader) as Box<dyn fm::Read>
    } else {
        let reader = fm::Reader::new(stdin())?;
        Box::new(reader) as Box<dyn fm::Read>
    };

    let script = fs::read_file_to_string(&params.script_path)?;

    let mut writer = if let Some(path) = &params.out_path {
        let writer =
            fm::Writer::new(fs::create_file(path)?, &params.fm_write_params)?;
        Box::new(writer) as Box<dyn fm::Write>
    } else {
        let writer = fm::Writer::new(stdout(), &params.fm_write_params)?;
        Box::new(writer) as Box<dyn fm::Write>
    };

    animate(
        reader.as_mut(),
        &script,
        params.timestep,
        params.num_iters,
        writer.as_mut(),
    )
}

pub fn animate(
    reader: &mut dyn fm::Read,
    script: &str,
    timestep: i64,
    num_iters: i64,
    writer: &mut dyn fm::Write,
) -> Result<()> {
    let (view, state) = read_element(reader)?;

    let lua = rlua::Lua::new();
    lua.context(|ctx| {
        ctx.load(script).exec()?;
        set_view_state(&ctx, &state)
    })
    .map_err(lua_err_to_err)?;

    let mut time = state.time.clone();

    use model::record::Type::*;
    writer.write_record(&model::Record {
        r#type: Some(ElementView(view)),
    })?;
    writer.write_record(&model::Record {
        r#type: Some(ElementViewState(state)),
    })?;

    for _ in 0..num_iters {
        time += timestep;

        let state = lua
            .context(|ctx| {
                let update_view_state: rlua::Function =
                    ctx.globals().get("update_view_state")?;
                update_view_state.call(time)?;
                get_view_state(&ctx)
            })
            .map_err(lua_err_to_err)?;

        writer.write_record(&model::Record {
            r#type: Some(ElementViewState(state)),
        })?;
    }

    Ok(())
}

fn read_element(
    reader: &mut dyn fm::Read,
) -> Result<(model::ElementView, model::ElementViewState)> {
    let err = |desc| Error::new(InconsistentState, format!("{}", desc));

    let read_record = |reader: &mut dyn fm::Read, desc| {
        reader.read_record()?.ok_or(err(desc))
    };

    let desc = "no element view as a first record";
    let rec = read_record(reader, desc)?;

    use model::record::Type::*;
    let view = if let Some(ElementView(view)) = rec.r#type {
        Ok(view)
    } else {
        Err(err(desc))
    }?;

    let desc = "no element view state as a second record";
    let rec = read_record(reader, desc)?;

    let state = if let Some(ElementViewState(state)) = rec.r#type {
        Ok(state)
    } else {
        Err(err(desc))
    }?;

    if view.element != state.element {
        return Err(err("view state of unknown element"));
    }

    Ok((view, state))
}

fn lua_err_to_err(err: rlua::Error) -> Error {
    Error::with_source(LuaError, format!("failed to run script"), err)
}

fn set_view_state(
    ctx: &rlua::Context,
    state: &model::ElementViewState,
) -> rlua::Result<()> {
    let vertices = ctx.create_table()?;
    for (i, v) in state.vertices.iter().enumerate() {
        let point = create_point3_table(ctx, v)?;
        vertices.set(i + 1, point)?;
    }

    let normals = ctx.create_table()?;
    for (i, v) in state.normals.iter().enumerate() {
        let point = create_point3_table(ctx, v)?;
        normals.set(i + 1, point)?;
    }

    let view_state = ctx.create_table()?;
    view_state.set("element", state.element.as_str())?;
    view_state.set("time", state.time)?;
    view_state.set("vertices", vertices)?;
    view_state.set("normals", normals)?;

    ctx.globals().set("view_state", view_state)
}

fn create_point3_table<'a>(
    ctx: &rlua::Context<'a>,
    point: &model::Point3,
) -> rlua::Result<rlua::Table<'a>> {
    let table = ctx.create_table()?;
    table.set("x", point.x)?;
    table.set("y", point.y)?;
    table.set("z", point.z)?;
    Ok(table)
}

fn get_view_state(
    ctx: &rlua::Context,
) -> rlua::Result<model::ElementViewState> {
    let view_state: rlua::Table = ctx.globals().get("view_state")?;

    let view_state_vertices: rlua::Table = view_state.get("vertices")?;
    let num_vertices = view_state_vertices.len()? as usize;
    let mut vertices = Vec::with_capacity(num_vertices);
    for i in 0..num_vertices {
        let point_table: rlua::Table = view_state_vertices.get(i + 1)?;
        let vertex = get_point3(&point_table)?;
        vertices.push(vertex);
    }

    let view_state_normals: rlua::Table = view_state.get("normals")?;
    let num_normals = view_state_normals.len()? as usize;
    let mut normals = Vec::with_capacity(num_normals);
    for i in 0..num_normals {
        let point_table: rlua::Table = view_state_normals.get(i + 1)?;
        let normal = get_point3(&point_table)?;
        normals.push(normal);
    }

    Ok(model::ElementViewState {
        element: view_state.get("element")?,
        time: view_state.get("time")?,
        vertices,
        normals,
    })
}

fn get_point3(point: &rlua::Table) -> rlua::Result<model::Point3> {
    Ok(model::Point3 {
        x: point.get("x")?,
        y: point.get("y")?,
        z: point.get("z")?,
    })
}
