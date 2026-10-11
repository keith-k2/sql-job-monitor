fn main() {
    slint_build::compile("ui/main.slint").expect("failed to compile Slint UI");

    println!("cargo:rerun-if-changed=assets/app.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/app.ico")
            .compile()
            .expect("failed to embed Windows icon resource");
    }
}
