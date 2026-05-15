fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let config =
        cbindgen::Config::from_file("cbindgen.toml").expect("Unable to find cbindgen.toml");

    match cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
    {
        Ok(bindings) => {
            let generated_dir = format!("{crate_dir}/generated");
            std::fs::create_dir_all(&generated_dir).unwrap();
            bindings.write_to_file(format!("{generated_dir}/malbox_plugin.h"));
        }
        Err(e) => {
            println!("cargo::warning=cbindgen failed: {e}");
        }
    }
}
