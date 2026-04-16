fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let config =
        cbindgen::Config::from_file("cbindgen.toml").expect("Unable to find cbindgen.toml");

    let _ = cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
        .map(|bindings| {
            let include_dir = format!("{crate_dir}/include");
            std::fs::create_dir_all(&include_dir).unwrap();
            bindings.write_to_file(format!("{include_dir}/malbox_plugin.h"));
        });
}
