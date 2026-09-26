fn main() {
    // 只为这几个自定义命令生成权限：页面能调用的本地能力仅限于切换全屏、保存录像、打开录像文件夹
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(tauri_build::AppManifest::new().commands(&[
        "toggle_fullscreen",
        "is_fullscreen",
        "save_recording",
        "open_recordings_folder",
    ])))
    .expect("failed to run tauri-build");
}
