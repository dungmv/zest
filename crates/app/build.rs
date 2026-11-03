fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=resources/app.rc");
    println!("cargo:rerun-if-changed=resources/app.ico");
    println!("cargo:rerun-if-env-changed=ZEST_APP_VERSION");
    println!("cargo:rerun-if-env-changed=GITHUB_REF_NAME");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    println!("cargo:rerun-if-env-changed=ZEST_COMMIT_SHA");

    // Resolve app version from GitHub Actions build or git tags, falling back to Cargo.toml
    let raw_version = std::env::var("ZEST_APP_VERSION")
        .or_else(|_| std::env::var("GITHUB_REF_NAME"))
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::process::Command::new("git")
                .args(["describe", "--tags", "--abbrev=0"])
                .output()
                .ok()
                .filter(|out| out.status.success())
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| {
            std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".to_string())
        });

    let version = raw_version.trim().trim_start_matches(['v', 'V']);
    println!("cargo:rustc-env=ZEST_APP_VERSION={version}");

    // Resolve git commit SHA
    let commit_hash = std::env::var("ZEST_COMMIT_SHA")
        .or_else(|_| std::env::var("GITHUB_SHA"))
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(|s| {
            let s = s.trim();
            if s.len() >= 7 {
                s[..7].to_string()
            } else {
                s.to_string()
            }
        })
        .or_else(|| {
            std::process::Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .filter(|out| out.status.success())
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=ZEST_COMMIT_HASH={commit_hash}");

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
