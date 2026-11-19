fn main() {
    let config = slint_build::CompilerConfiguration::new().with_debug_info(cfg!(debug_assertions));
    slint_build::compile_with_config("ui/main.slint", config).expect("Failed to compile Slint UI");

    // The host decides the resource format, so this runs only when building on Windows.
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icons/icon.ico");
        resource.set("ProductName", "Paper Inbox");
        resource.set("FileDescription", "Paper Inbox");
        if let Err(error) = resource.compile() {
            println!("cargo:warning=could not embed the Windows icon: {error}");
        }
    }
}
