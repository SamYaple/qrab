use anyhow::Result;
use qapi_spec_parser::{generate_qapi_code, versions::find_schema_entry};
use std::path::Path;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Please provide qemu source path as argument");
        std::process::exit(1);
    }
    let qemu_src_root = Path::new(&args[1]);
    let schema_file = find_schema_entry(qemu_src_root)
        .expect("Could not find QAPI schema entry point");

    let output = generate_qapi_code(qemu_src_root, &schema_file)?;
    print!("{}", output);
    Ok(())
}
