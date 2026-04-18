//! Path-addressable helpers for the `form_state` `serde_json::Value` tree.
//!
//! Form state uses wire-name keys so the assembled object is already a
//! valid `"arguments"` payload; no extra assembly pass is needed. These
//! helpers let field widgets mutate deeply-nested fields and arrays
//! without shuttling owned clones up and down the component tree.

use serde_json::{Map, Value};

/// One segment of a path into `form_state`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSeg {
    Field(String),
    Index(usize),
}

pub type FormPath = Vec<PathSeg>;

/// Borrow an immutable reference to the value at `path`, if any.
pub fn get<'a>(state: &'a Value, path: &[PathSeg]) -> Option<&'a Value> {
    let mut cur = state;
    for seg in path {
        cur = match (cur, seg) {
            (Value::Object(m), PathSeg::Field(k)) => m.get(k)?,
            (Value::Array(a), PathSeg::Index(i)) => a.get(*i)?,
            _ => return None,
        };
    }
    Some(cur)
}

/// Set `new` at `path`, creating intermediate objects/arrays as needed.
/// Missing array indices are padded with `Null` — callers are responsible
/// for only asking for positions that make sense for their data.
pub fn set(state: &mut Value, path: &[PathSeg], new: Value) {
    if path.is_empty() {
        *state = new;
        return;
    }
    let mut cur = state;
    for (i, seg) in path.iter().enumerate() {
        let is_last = i + 1 == path.len();
        match seg {
            PathSeg::Field(k) => {
                if !cur.is_object() {
                    *cur = Value::Object(Map::new());
                }
                let map = cur.as_object_mut().expect("just set");
                if is_last {
                    map.insert(k.clone(), new);
                    return;
                }
                cur = map.entry(k.clone()).or_insert(Value::Null);
            }
            PathSeg::Index(idx) => {
                if !cur.is_array() {
                    *cur = Value::Array(Vec::new());
                }
                let arr = cur.as_array_mut().expect("just set");
                while arr.len() <= *idx {
                    arr.push(Value::Null);
                }
                if is_last {
                    arr[*idx] = new;
                    return;
                }
                cur = &mut arr[*idx];
            }
        }
    }
}

/// Remove the value at `path`, leaving the parent otherwise intact. A
/// leaf-field removal deletes the object key; a leaf-index removal
/// shortens the array. Nothing is allocated for a missing path.
pub fn remove(state: &mut Value, path: &[PathSeg]) {
    if path.is_empty() {
        *state = Value::Null;
        return;
    }
    let (last, parents) = path.split_last().expect("non-empty checked");
    let mut cur = state;
    for seg in parents {
        cur = match (cur, seg) {
            (Value::Object(m), PathSeg::Field(k)) => match m.get_mut(k) {
                Some(v) => v,
                None => return,
            },
            (Value::Array(a), PathSeg::Index(i)) => match a.get_mut(*i) {
                Some(v) => v,
                None => return,
            },
            _ => return,
        };
    }
    match (cur, last) {
        (Value::Object(m), PathSeg::Field(k)) => {
            m.remove(k);
        }
        (Value::Array(a), PathSeg::Index(i)) if *i < a.len() => {
            a.remove(*i);
        }
        _ => {}
    }
}

/// Render a path as a forward-slash-joined string for error display.
pub fn path_to_string(prefix: &str, path: &[PathSeg]) -> String {
    let mut s = prefix.to_string();
    for seg in path {
        match seg {
            PathSeg::Field(k) => {
                s.push('/');
                s.push_str(k);
            }
            PathSeg::Index(i) => {
                s.push('/');
                s.push_str(&i.to_string());
            }
        }
    }
    s
}
