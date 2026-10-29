fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=resources/app.rc");
    println!("cargo:rerun-if-changed=resources/app.ico");

    #[cfg(target_os = "windows")]
    {
        // GPUI deeply-nested element trees in debug mode require more than the
        // default 1MB stack size on Windows. Increase main thread stack to 8MB
        // to match the default on Linux and macOS.
        #[cfg(target_env = "msvc")]
        println!("cargo:rustc-link-arg-bins=/STACK:8388608");
        #[cfg(target_env = "gnu")]
        println!("cargo:rustc-link-arg-bins=-Wl,--stack,8388608");

        let resource_dir = std::path::Path::new("resources");
        let rc_file = resource_dir.join("app.rc");
        let _ = embed_resource::compile(rc_file, embed_resource::ParamsIncludeDirs([resource_dir]));
    }
}
