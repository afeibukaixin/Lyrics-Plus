fn main() {
    println!("cargo:rerun-if-changed=icons/icon.png");
    println!("cargo:rerun-if-changed=icons/icon.icns");
    println!("cargo:rerun-if-changed=src/player/spectrum_bridge.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let mut spectrum = cc::Build::new();
        spectrum
            .file("src/player/spectrum_bridge.m")
            .flag("-fobjc-arc")
            .flag("-fblocks");
        let compiler = spectrum.get_compiler();
        spectrum.compile("lyrics_plus_spectrum");

        // Objective-C 的 @available 检查依赖 Clang 运行库；Rust 链接器不会自动补上它。
        let runtime_name = "libclang_rt.osx.a";
        let runtime_output = compiler
            .to_command()
            .arg(format!("-print-file-name={runtime_name}"))
            .output()
            .expect("failed to locate the macOS Clang runtime");
        if !runtime_output.status.success() {
            panic!("failed to locate the macOS Clang runtime");
        }
        let runtime_path = std::path::PathBuf::from(
            String::from_utf8_lossy(&runtime_output.stdout)
                .trim()
                .to_owned(),
        );
        if !runtime_path.is_file() {
            panic!("macOS Clang runtime not found: {}", runtime_path.display());
        }
        let runtime_dir = runtime_path
            .parent()
            .expect("macOS Clang runtime path has no parent directory");
        println!("cargo:rustc-link-search=native={}", runtime_dir.display());
        println!("cargo:rustc-link-lib=static=clang_rt.osx");

        println!("cargo:rustc-link-lib=framework=CoreAudio");
        println!("cargo:rustc-link-lib=framework=CoreFoundation");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }
    tauri_build::build()
}
