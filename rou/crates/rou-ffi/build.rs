fn main() {
    let root = std::env::var("CARGO_MANIFEST_DIR").expect("crate directory");
    println!("cargo:rerun-if-changed=src/lib.rs");
    cbindgen::Builder::new()
        .with_crate(&root)
        .with_config(cbindgen::Config {
            usize_is_size_t: true,
            ..Default::default()
        })
        .with_language(cbindgen::Language::C)
        .with_include_guard("ROU_H")
        .with_documentation(true)
        .generate()
        .expect("generate rou.h")
        .write_to_file(std::path::Path::new(&root).join("../../include/rou.h"));
}
