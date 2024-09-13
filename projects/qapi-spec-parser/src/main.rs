use anyhow::Result;
use std::fs::File;
use std::io::Read;
use std::path::Path;

mod helpers;
mod qapi_type_ref;
use qapi_type_ref::QapiTypeRef;

fn read_file(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(contents)
}

fn main() -> Result<()> {
    let qemu_src_root = Path::new("/home/sam/repos/qemu");
    let schema_file = qemu_src_root
        .join("qapi")
        //.join("qga")
        //.join("storage-daemon/qapi")
        .join("qapi-schema.json");
    let schema = read_file(&schema_file)?;
    let (leftover, out) = nom::multi::many1(QapiTypeRef::parse)(&schema).unwrap();
    dbg![&leftover];
    dbg![&out];
    Ok(())
}
