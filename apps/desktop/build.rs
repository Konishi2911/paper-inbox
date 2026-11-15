fn main() {
    let config = slint_build::CompilerConfiguration::new().with_debug_info(cfg!(debug_assertions));
    slint_build::compile_with_config("ui/main.slint", config).expect("Failed to compile Slint UI");
}
