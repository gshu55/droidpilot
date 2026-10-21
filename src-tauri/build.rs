fn main() {
    println!("cargo::rustc-check-cfg=cfg(has_embedded_adb)");
    println!("cargo::rustc-check-cfg=cfg(has_embedded_scrcpy_server)");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let adb_paths: Vec<&str> = match target_os.as_str() {
        "windows" => vec![
            "binaries/windows/adb.exe",
            "binaries/windows/AdbWinApi.dll",
            "binaries/windows/AdbWinUsbApi.dll",
        ],
        "linux" => vec!["binaries/linux/adb"],
        "macos" => vec!["binaries/macos/adb"],
        _ => Vec::new(),
    };

    let all_present = adb_paths
        .iter()
        .all(|path| std::path::Path::new(path).exists());

    if all_present {
        println!("cargo:rustc-cfg=has_embedded_adb");
    } else {
        println!("cargo:warning=嵌入 adb 已禁用：目标平台二进制不完整，将回退到系统 PATH 中的 adb");
    }

    for path in adb_paths {
        if std::path::Path::new(path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
    }

    let scrcpy_server_path = "binaries/scrcpy/scrcpy-server-v4.1";
    if std::path::Path::new(scrcpy_server_path).exists() {
        println!("cargo:rustc-cfg=has_embedded_scrcpy_server");
        println!("cargo:rerun-if-changed={scrcpy_server_path}");
    } else {
        println!(
            "cargo:warning=嵌入 scrcpy server 已禁用：缺少 binaries/scrcpy/scrcpy-server-v4.1"
        );
    }

    tauri_build::build()
}
