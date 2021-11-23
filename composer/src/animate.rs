use std::path::PathBuf;

use structopt::StructOpt;

use base::defs::{Error, ErrorKind::*, Result};
use base::fm;
use base::util::fs;
use base::util::cli;

#[derive(StructOpt)]
#[structopt(about = "Animate element using .lua script")]
pub struct AnimateCommand {
    #[structopt(flatten)]
    input: cli::FmInput,

    #[structopt(flatten)]
    output: cli::FmOutput,

    #[structopt(help = "Input .lua animation script", short = "s")]
    script_path: PathBuf,

    #[structopt(help = "Timestep", short = "t")]
    timestep: i64,

    #[structopt(help = "Number of iterations", short = "n")]
    num_iters: i64,
}

impl AnimateCommand {
    pub fn run(&self) -> Result<()> {
        let mut reader = self.input.get()?;
        let mut writer = self.output.get()?;

        let script = fs::read_file_to_string(&self.script_path)?;

        animate(
            reader.as_mut(),
            writer.as_mut(),
            &script,
            self.timestep,
            self.num_iters,
        )
    }
}

pub fn animate(
    reader: &mut dyn fm::Read,
    writer: &mut dyn fm::Write,
    script: &str,
    timestep: i64,
    num_iters: i64,
) -> Result<()> {
    let (view, state) = read_element(reader)?;

    let lua = rlua::Lua::new();
    lua.context(|ctx| {
        ctx.load(script).exec()?;
        set_view_state(&ctx, &state)
    })
    .map_err(lua_err_to_err)?;

    let mut time = state.time;

    use fm::record::Type::*;
    writer.write_record(&fm::Record {
        r#type: Some(ElementView(view)),
    })?;
    writer.write_record(&fm::Record {
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

        writer.write_record(&fm::Record {
            r#type: Some(ElementViewState(state)),
        })?;
    }

    Ok(())
}

fn read_element(
    reader: &mut dyn fm::Read,
) -> Result<(fm::ElementView, fm::ElementViewState)> {
    let err = |desc: &str| Error::new(InconsistentState, desc.to_string());

    let read_record = |reader: &mut dyn fm::Read, desc| {
        reader.read_record()?.ok_or_else(|| err(desc))
    };

    let desc = "no element view as a first record";
    let rec = read_record(reader, desc)?;

    use fm::record::Type::*;
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
    Error::with_source(LuaError, "failed to run script".to_string(), err)
}

fn set_view_state(
    ctx: &rlua::Context,
    state: &fm::ElementViewState,
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
    point: &fm::Point3,
) -> rlua::Result<rlua::Table<'a>> {
    let table = ctx.create_table()?;
    table.set("x", point.x)?;
    table.set("y", point.y)?;
    table.set("z", point.z)?;
    Ok(table)
}

fn get_view_state(ctx: &rlua::Context) -> rlua::Result<fm::ElementViewState> {
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

    Ok(fm::ElementViewState {
        element: view_state.get("element")?,
        time: view_state.get("time")?,
        vertices,
        normals,
    })
}

fn get_point3(point: &rlua::Table) -> rlua::Result<fm::Point3> {
    Ok(fm::Point3 {
        x: point.get("x")?,
        y: point.get("y")?,
        z: point.get("z")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::fm::Read as _;
    use base::util::test::*;
    use base::{assert_eq_point3, record_variant};
    use fm::record::Type::*;

    const EMPTY_SCRIPT: &str = r#"
        function update_view_state(time)
        end
    "#;

    #[test]
    fn test_animate_no_view() {
        let mut reader = create_reader_with_records(&vec![]);
        let mut writer = create_writer();
        let err =
            animate(&mut reader, &mut writer, EMPTY_SCRIPT, 1, 1).unwrap_err();
        assert_eq!(
            err.description.as_str(),
            "no element view as a first record"
        );
    }

    #[test]
    fn test_animate_no_view_state() {
        let view = new_element_view_rec(fm::ElementView {
            element: format!("abc"),
            ..Default::default()
        });
        let mut reader = create_reader_with_records(&vec![view]);
        let mut writer = create_writer();
        let err =
            animate(&mut reader, &mut writer, EMPTY_SCRIPT, 1, 1).unwrap_err();
        assert_eq!(
            err.description.as_str(),
            "no element view state as a second record"
        );
    }

    #[test]
    fn test_animate_unknown_element() {
        let view = new_element_view_rec(fm::ElementView {
            element: format!("abc"),
            ..Default::default()
        });
        let state = new_element_view_state_rec(fm::ElementViewState {
            element: format!("bcd"),
            time: 123,
            vertices: vec![],
            normals: vec![],
        });
        let mut reader = create_reader_with_records(&vec![view, state]);
        let mut writer = create_writer();
        let err =
            animate(&mut reader, &mut writer, EMPTY_SCRIPT, 1, 1).unwrap_err();
        assert_eq!(err.description.as_str(), "view state of unknown element");
    }

    #[test]
    fn test_animate_valid_element() {
        let view = new_element_view_rec(fm::ElementView {
            element: format!("abc"),
            ..Default::default()
        });
        let state = new_element_view_state_rec(fm::ElementViewState {
            element: format!("abc"),
            time: 123,
            vertices: vec![
                new_point3(0.12, 0.23, 0.34),
                new_point3(0.45, 0.56, 0.67),
                new_point3(0.78, 0.89, 0.90),
            ],
            normals: vec![
                new_point3(0.21, 0.32, 0.43),
                new_point3(0.54, 0.65, 0.76),
                new_point3(0.87, 0.98, 0.09),
            ],
        });
        let mut reader =
            create_reader_with_records(&vec![view.clone(), state.clone()]);

        let script = r#"
            function update_view_state(time)
                view_state.time = time
                for i, v in ipairs(view_state.vertices) do
                    view_state.vertices[i] = {
                        x = v.x + 0.01,
                        y = v.y + 0.02,
                        z = v.z + 0.03,
                    }
                end
                for i, v in ipairs(view_state.normals) do
                    view_state.normals[i] = {
                        x = v.x + 0.04,
                        y = v.y + 0.05,
                        z = v.z + 0.06,
                    }
                end
            end
        "#;

        let mut writer = create_writer();

        animate(&mut reader, &mut writer, script, 12, 2).unwrap();

        let mut reader = writer_to_reader(writer);

        assert_eq!(reader.read_record().unwrap().unwrap(), view);
        assert_eq!(reader.read_record().unwrap().unwrap(), state);

        let rec = reader.read_record().unwrap().unwrap();
        let state = record_variant!(ElementViewState, rec);
        assert_eq!(state.element.as_str(), "abc");
        assert_eq!(state.time, 135);
        assert_eq!(state.vertices.len(), 3);
        assert_eq_point3!(state.vertices[0], new_point3(0.13, 0.25, 0.37));
        assert_eq_point3!(state.vertices[1], new_point3(0.46, 0.58, 0.70));
        assert_eq_point3!(state.vertices[2], new_point3(0.79, 0.91, 0.93));
        assert_eq!(state.normals.len(), 3);
        assert_eq_point3!(state.normals[0], new_point3(0.25, 0.37, 0.49));
        assert_eq_point3!(state.normals[1], new_point3(0.58, 0.70, 0.82));
        assert_eq_point3!(state.normals[2], new_point3(0.91, 1.03, 0.15));

        let rec = reader.read_record().unwrap().unwrap();
        let state = record_variant!(ElementViewState, rec);
        assert_eq!(state.element.as_str(), "abc");
        assert_eq!(state.time, 147);
        assert_eq!(state.vertices.len(), 3);
        assert_eq_point3!(state.vertices[0], new_point3(0.14, 0.27, 0.40));
        assert_eq_point3!(state.vertices[1], new_point3(0.47, 0.60, 0.73));
        assert_eq_point3!(state.vertices[2], new_point3(0.80, 0.93, 0.96));
        assert_eq!(state.normals.len(), 3);
        assert_eq_point3!(state.normals[0], new_point3(0.29, 0.42, 0.55));
        assert_eq_point3!(state.normals[1], new_point3(0.62, 0.75, 0.88));
        assert_eq_point3!(state.normals[2], new_point3(0.95, 1.08, 0.21));

        assert!(reader.read_record().unwrap().is_none());
    }
}
