use anyhow::Result;
use std::fs::File;
use std::io::Read;
use std::path::Path;

mod helpers;

mod qapi_alternate;
//mod qapi_branches;
//mod qapi_command;
mod qapi_cond;
//mod qapi_enum;
//mod qapi_event;
mod qapi_features;
mod qapi_include;
mod qapi_members;
//mod qapi_pragma;
mod qapi_string;
mod qapi_struct;
mod qapi_type_ref;
//mod qapi_union;

pub use qapi_alternate::QapiAlternate;
//pub use qapi_branches::QapiBranches;
//pub use qapi_command::QapiCommand;
pub use qapi_cond::QapiCond;
//pub use qapi_enum::QapiEnum;
//pub use qapi_event::QapiEvent;
pub use qapi_features::QapiFeatures;
pub use qapi_include::QapiInclude;
pub use qapi_members::QapiMembers;
//pub use qapi_pragma::QapiPragma;
pub use qapi_string::QapiString;
pub use qapi_struct::QapiStruct;
pub use qapi_type_ref::QapiTypeRef;
//pub use qapi_union::QapiUnion;

fn read_file(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

fn main() -> Result<()> {
    let qemu_src_root = Path::new("/home/sam/repos/qemu");
    let schema_type = "qapi"; // choose ["qapi", "qga", "storage-daemon/qapi"]
    let schema_file = qemu_src_root.join(schema_type).join("qapi-schema.json");
    let schema = read_file(&schema_file)?;
    let (leftover, out) = nom::multi::many1(QapiAlternate::parse)(&schema).unwrap();
    dbg![&leftover];
    dbg![&out];
    Ok(())
}
