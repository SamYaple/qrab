use anyhow::Result;
use std::path::Path;

mod helpers;

mod qapi_alternate;
mod qapi_bool;
mod qapi_branches;
mod qapi_command;
mod qapi_cond;
mod qapi_documentation;
mod qapi_enum;
mod qapi_event;
mod qapi_features;
mod qapi_include;
mod qapi_members;
mod qapi_pragma;
mod qapi_schema;
mod qapi_struct;
mod qapi_type_ref;
mod qapi_union;

pub use qapi_alternate::QapiAlternate;
pub use qapi_bool::QapiBool;
pub use qapi_branches::QapiBranches;
pub use qapi_command::QapiCommand;
pub use qapi_cond::QapiCond;
pub use qapi_documentation::{QapiDocumentation, QapiSectionDocumentation};
pub use qapi_enum::QapiEnum;
pub use qapi_event::QapiEvent;
pub use qapi_features::QapiFeatures;
pub use qapi_include::QapiInclude;
pub use qapi_members::QapiMembers;
pub use qapi_pragma::QapiPragma;
pub use qapi_schema::QapiSchema;
pub use qapi_struct::QapiStruct;
pub use qapi_type_ref::QapiTypeRef;
pub use qapi_union::QapiUnion;

fn main() -> Result<()> {
    let qemu_src_root = Path::new("/home/sam/repos/qemu");
    let schema_type = "storage-daemon/qapi";
    let schema_type = "qga";
    let schema_type = "qapi";
    let schema_file = qemu_src_root.join(schema_type).join("qapi-schema.json");

    let mut schema_raw_strs = std::collections::HashMap::new();
    helpers::walk_schemas(&schema_file, &mut schema_raw_strs)?;
    let schemas = helpers::process_schemas(&schema_raw_strs)?;
    //let schema = schemas.get(&schema_file).unwrap();
    //dbg![&schema_file, &schema];
    dbg![&schemas];

    Ok(())
}
