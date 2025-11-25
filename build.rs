fn main() {
    println!("cargo:rerun-if-changed=src/copy_array.S");

    cc::Build::new()
        .file("src/copy_array.S") // Path to the assembly file
        .compile("copy_array");
}
