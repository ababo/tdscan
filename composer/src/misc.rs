use std::collections::HashMap;
use std::hash::Hash;

use petgraph::unionfind::UnionFind;
use rand::Rng;
use rlua::Value as LuaValue;
use serde_json::Value as JsonValue;

use base::defs::{Error, ErrorKind::*};
use base::fm;

pub fn lua_err_to_err(err: rlua::Error) -> Error {
    Error::with_source(
        LuaError,
        "failed to evaluate expression".to_string(),
        err,
    )
}

pub fn lua_table_from_record<'a>(
    ctx: rlua::Context<'a>,
    record: &fm::Record,
    truncate_len: Option<usize>,
) -> rlua::Result<rlua::Table<'a>> {
    let mut val = serde_json::to_value(record).unwrap();
    if let Some(max_len) = truncate_len {
        truncate_json_value(&mut val, max_len);
    }
    lua_table_from_json_val(ctx, &val).map(|v| {
        if let rlua::Value::Table(t) = v {
            t
        } else {
            unreachable!()
        }
    })
}

fn lua_table_from_json_val<'a>(
    ctx: rlua::Context<'a>,
    val: &serde_json::Value,
) -> rlua::Result<rlua::Value<'a>> {
    match val {
        JsonValue::Null => Ok(LuaValue::Nil),
        JsonValue::Bool(b) => Ok(LuaValue::Boolean(*b)),
        JsonValue::Number(n) => Ok(LuaValue::Number(n.as_f64().unwrap())),
        JsonValue::String(s) => ctx.create_string(s).map(LuaValue::String),
        JsonValue::Array(a) => {
            let mut vec = Vec::with_capacity(a.len());
            for e in a {
                let item = lua_table_from_json_val(ctx, e)?;
                vec.push(item);
            }
            ctx.create_sequence_from(vec).map(LuaValue::Table)
        }
        JsonValue::Object(o) => {
            let mut map = Vec::with_capacity(o.len());
            for (k, v) in o {
                let key = ctx.create_string(k)?;
                let val = lua_table_from_json_val(ctx, v)?;
                map.push((key, val));
            }
            ctx.create_table_from(map).map(LuaValue::Table)
        }
    }
}

pub fn select_random<T, R: Rng>(
    items: &mut Vec<T>,
    max_num: usize,
    rng: &mut R,
) {
    if items.len() > max_num {
        for i in 0..max_num {
            let j = rng.gen_range(i..items.len());
            items.swap(i, j);
        }
        items.truncate(max_num);
    }
}

pub fn truncate_json_value(value: &mut JsonValue, max_len: usize) {
    match value {
        JsonValue::String(r#str) => {
            r#str.truncate(max_len);
        }
        JsonValue::Array(arr) => {
            arr.truncate(max_len);
            for e in arr {
                truncate_json_value(e, max_len);
            }
        }
        JsonValue::Object(obj) => {
            let keys: Vec<_> = obj.keys().skip(max_len).cloned().collect();
            for k in keys {
                obj.remove(k.as_str());
            }

            for (_, v) in obj {
                truncate_json_value(v, max_len);
            }
        }
        _ => {}
    };
}

pub fn extract_biggest_partition_component(
    partition: UnionFind<usize>,
) -> Vec<usize> {
    let family = vec_inv_many(&partition.into_labeling());
    let biggest_idx =
        family.iter().map(|(&i, j)| (j.len(), i)).max().unwrap().1;
    family[&biggest_idx].clone()
}

pub fn vec_inv<T>(v: &[T]) -> HashMap<T, usize>
where
    T: Copy + Eq + Hash
{
    HashMap::from_iter(v.iter().enumerate().map(|(i, &j)| (j, i)))
}

pub fn vec_inv_many<T>(labeling: &[T]) -> HashMap<T, Vec<usize>>
where
    T: Copy + Eq + Hash
{
    let mut family = HashMap::<T, Vec<usize>>::new();
    for (i, &j) in labeling.iter().enumerate() {
        family.entry(j).or_insert_with(Vec::new);
        family.get_mut(&j).unwrap().push(i);
    }
    family
}

