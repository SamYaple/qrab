This crate takes a qemu repo as an argument when run and outputs the ./qapi/src/lib.rs file to stdout

Tested up to qemu tag v9.1.0

cargo run -p qapi-spec-parser -- vendor/qemu
