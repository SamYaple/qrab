fn main() {
    println!("cargo:rerun-if-env-changed=QEMU_MIN_VERSION");
    println!("cargo:rerun-if-env-changed=QEMU_MAX_VERSION");
}
