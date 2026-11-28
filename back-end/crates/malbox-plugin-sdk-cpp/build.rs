//! Build script that regenerates the C header `generated/malbox_plugin.h` from
//! this crate's `extern "C"` API using cbindgen.

fn main() {
    let crate_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("cargo always sets CARGO_MANIFEST_DIR");
    let config =
        cbindgen::Config::from_file("cbindgen.toml").expect("Unable to find cbindgen.toml");

    match cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
    {
        Ok(bindings) => {
            let generated_dir = format!("{crate_dir}/generated");
            std::fs::create_dir_all(&generated_dir)
                .expect("failed to create the generated/ header directory");
            bindings.write_to_file(format!("{generated_dir}/malbox_plugin.h"));
        }
        Err(e) => {
            println!("cargo::warning=cbindgen failed: {e}");
        }
    }
}
