use anyhow::Result;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

mod helpers;

mod qapi_alternate;
mod qapi_bool;
mod qapi_branches;
mod qapi_command;
mod qapi_cond;
mod qapi_enum;
mod qapi_event;
mod qapi_features;
mod qapi_include;
mod qapi_members;
mod qapi_pragma;
mod qapi_schema;
mod qapi_string;
mod qapi_struct;
mod qapi_type_ref;
mod qapi_union;

pub use qapi_alternate::QapiAlternate;
pub use qapi_bool::QapiBool;
pub use qapi_branches::QapiBranches;
pub use qapi_command::QapiCommand;
pub use qapi_cond::QapiCond;
pub use qapi_enum::QapiEnum;
pub use qapi_event::QapiEvent;
pub use qapi_features::QapiFeatures;
pub use qapi_include::QapiInclude;
pub use qapi_members::QapiMembers;
pub use qapi_pragma::QapiPragma;
pub use qapi_schema::QapiSchema;
pub use qapi_string::QapiString;
pub use qapi_struct::QapiStruct;
pub use qapi_type_ref::QapiTypeRef;
pub use qapi_union::QapiUnion;

fn read_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

fn parse_file(input: &str) -> nom::IResult<&str, Vec<QapiSchema>> {
    QapiSchema::parse(input)
}

fn process_file(
    path: &Path,
    visited: &mut std::collections::HashSet<PathBuf>,
) -> Result<Vec<QapiSchema>> {
    if !visited.insert(path.to_path_buf()) {
        return Ok(vec![]);
    }

    let schema = read_file(path)?;
    let (l, tokens) = parse_file(&schema).unwrap();
    if l.len() != 0 {
        todo! {"Input did not full parse, but nothing errored"};
    }
    let mut result_tokens = vec![];

    for token in tokens {
        match token {
            QapiSchema::Include(ref include_path) => {
                let full_path = path.parent().unwrap().join(include_path.0 .0.clone());
                let mut included_tokens = process_file(&full_path, visited)?;
                result_tokens.append(&mut included_tokens);
            }
            _ => {
                result_tokens.push(token);
            }
        }
    }

    Ok(result_tokens)
}

fn main() -> Result<()> {
    let qemu_src_root = Path::new("/home/sam/repos/qemu");
    let schema_type = "storage-daemon/qapi";
    let schema_type = "qga";
    let schema_type = "qapi";
    let schema_file = qemu_src_root.join(schema_type).join("qapi-schema.json");
    let mut visited_files = std::collections::HashSet::new();
    let tokens = process_file(&schema_file, &mut visited_files)?;
    dbg![&tokens];
    Ok(())
}
