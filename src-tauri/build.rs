fn main() {
    println!("cargo:rerun-if-changed=icons/icon.ico");

    // 如果 ../dist 目录不存在，则在编译前自动创建它以及一个占位的 index.html，
    // 以防止 tauri::generate_context! 宏和 rust-embed 在开发环境下因目录不存在而编译报错。
    let dist_dir = std::path::Path::new("../dist");
    if !dist_dir.exists() {
        let _ = std::fs::create_dir_all(dist_dir);
        let _ = std::fs::write(dist_dir.join("index.html"), "<!-- placeholder -->");
    }

    tauri_build::build();
}
