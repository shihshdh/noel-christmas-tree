// Noël · 圣诞树的 Windows 客户端。页面、three.js、手势模型全部打包在客户端里，从本地加载，不走浏览器、不依赖外网。
// 和 Pixel Reconstruction 客户端同一套做法：Tauri 2 + 系统自带的 WebView2。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use tauri::{ipc::InvokeBody, webview::NewWindowResponse, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;
use url::Url;

/// 留在窗口里的地址：打包进来的本地页面、页面内生成的 blob/data。
fn stays_inside(url: &Url) -> bool {
    match url.scheme() {
        "tauri" | "about" | "blob" | "data" => true,
        "http" | "https" => url.host_str() == Some("tauri.localhost"),
        _ => false,
    }
}

/// 手势要用摄像头：客户端里直接放行，不再每次弹权限框。
/// 只放行摄像头；麦克风、定位等其余权限仍按 WebView2 默认处理。
#[cfg(windows)]
fn allow_camera(window: &tauri::WebviewWindow) {
    let _ = window.with_webview(|webview| unsafe {
        use webview2_com::{Microsoft::Web::WebView2::Win32::*, PermissionRequestedEventHandler};
        let Ok(core) = webview.controller().CoreWebView2() else { return };
        let mut token = 0i64;
        let _ = core.add_PermissionRequested(
            &PermissionRequestedEventHandler::create(Box::new(|_, args| {
                let Some(args) = args else { return Ok(()) };
                let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
                args.PermissionKind(&mut kind)?;
                if kind == COREWEBVIEW2_PERMISSION_KIND_CAMERA {
                    args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
                }
                Ok(())
            })),
            &mut token,
        );
    });
}

// ---- 录像保存：页面只能往“录像”文件夹里写 .mp4 / .webm，别处一概不行 ----

/// 录像保存位置，按顺序选第一个可写的：D:\Noel\录像 → 系统“视频”文件夹\Noel → “文档”\Noel 录像。
/// 卸载客户端不会删除这些文件。
fn recordings_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    if std::path::Path::new(r"D:\").is_dir() {
        candidates.push(PathBuf::from(r"D:\Noel\录像"));
    }
    if let Ok(videos) = app.path().video_dir() {
        candidates.push(videos.join("Noel"));
    }
    for dir in candidates {
        if std::fs::create_dir_all(&dir).is_ok() && writable(&dir) {
            return Ok(dir);
        }
    }
    let dir = app.path().document_dir().map_err(|e| e.to_string())?.join("Noel 录像");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn writable(dir: &PathBuf) -> bool {
    let probe = dir.join(".write-test");
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(probe);
    ok
}

/// 只接受简单的文件名：字母、数字、点、横线、下划线，不能以点开头，防止路径穿越。
fn safe_name(value: &str) -> Option<&str> {
    let ok = !value.is_empty()
        && value.len() <= 120
        && !value.starts_with('.')
        && value.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    ok.then_some(value)
}

fn header<'a>(request: &'a tauri::ipc::Request<'_>, name: &str) -> Result<&'a str, String> {
    request.headers().get(name).and_then(|v| v.to_str().ok()).ok_or_else(|| format!("缺少 {name}"))
}

/// 把一段录像写进录像文件夹。大文件由网页分块发送（每块不超过 8MB）：offset 为 0 的块新建临时文件，
/// 之后按顺序追加，最后一块写完再改名，中断时不会留下半个正式文件。返回完整路径。
#[tauri::command]
fn save_recording(app: tauri::AppHandle, request: tauri::ipc::Request<'_>) -> Result<String, String> {
    use std::io::Write;
    let name = safe_name(header(&request, "x-file-name")?).ok_or("文件名不合法")?;
    let lower = name.to_ascii_lowercase();
    if !(lower.ends_with(".mp4") || lower.ends_with(".webm")) {
        return Err("只能保存 mp4 / webm 录像".into());
    }
    let offset: u64 = header(&request, "x-chunk-offset").unwrap_or("0").parse().map_err(|_| "分块位置不合法")?;
    let last = header(&request, "x-chunk-final").unwrap_or("1") == "1";
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("录像内容缺失".into());
    };
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("单块过大".into());
    }
    let dir = recordings_root(&app)?;
    let target = dir.join(name);
    let temp = dir.join(format!(".{name}.part"));
    let mut file = if offset == 0 {
        std::fs::File::create(&temp).map_err(|e| e.to_string())?
    } else {
        // 追加前核对已写长度，乱序或重复的块直接拒绝
        let written = std::fs::metadata(&temp).map_err(|_| "缺少前面的分块")?.len();
        if written != offset {
            return Err("分块顺序不一致".into());
        }
        std::fs::OpenOptions::new().append(true).open(&temp).map_err(|e| e.to_string())?
    };
    file.write_all(bytes).map_err(|e| e.to_string())?;
    drop(file);
    if last {
        std::fs::rename(&temp, &target).map_err(|e| e.to_string())?;
    }
    Ok(target.to_string_lossy().into_owned())
}

/// 在资源管理器里打开录像文件夹。
#[tauri::command]
fn open_recordings_folder(app: tauri::AppHandle) -> Result<String, String> {
    let dir = recordings_root(&app)?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())?;
    Ok(dir.to_string_lossy().into_owned())
}

/// 页面右上角的全屏按钮 / F 键：切换整个客户端窗口的全屏，返回切换后的状态。
#[tauri::command]
fn toggle_fullscreen(window: tauri::WebviewWindow) -> Result<bool, String> {
    let next = !window.is_fullscreen().map_err(|e| e.to_string())?;
    window.set_fullscreen(next).map_err(|e| e.to_string())?;
    Ok(next)
}

#[tauri::command]
fn is_fullscreen(window: tauri::WebviewWindow) -> bool {
    window.is_fullscreen().unwrap_or(false)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 重复打开时把已有窗口带到前面，而不是再开一个
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![toggle_fullscreen, is_fullscreen, save_recording, open_recordings_folder])
        .setup(|app| {
            let handle = app.handle().clone();
            let popup_handle = app.handle().clone();
            // 硬件加速：双显卡时强制用独显，驱动在黑名单里也不退回软件渲染，光栅化走 GPU。
            // 前一段是 wry 的默认参数，覆盖时必须保留。设置了 NOEL_DEBUG_PORT 时额外开调试端口（仅供排查）。
            let mut browser_args = String::from(concat!(
                "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection",
                " --force_high_performance_gpu --ignore-gpu-blocklist",
                " --enable-gpu-rasterization --enable-zero-copy"
            ));
            if let Some(port) = std::env::var("NOEL_DEBUG_PORT").ok().filter(|p| p.chars().all(|c| c.is_ascii_digit())) {
                browser_args.push_str(&format!(" --remote-debugging-port={port}"));
            }
            let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()));
            // 网页数据（照片墙里的照片、设置）放在 D:\Noel\数据；没有 D 盘时用系统默认位置
            if std::path::Path::new(r"D:\").is_dir() {
                let dir = PathBuf::from(r"D:\Noel\数据");
                if std::fs::create_dir_all(&dir).is_ok() && writable(&dir) {
                    builder = builder.data_directory(dir);
                }
            }
            let window = builder
                .title("Noël · Grand Luxury Tree")
                .inner_size(1440.0, 900.0)
                .min_inner_size(960.0, 600.0)
                .center()
                // 一打开就铺满整块屏幕（你的是 2560×1600），画面按屏幕原生像素渲染
                .fullscreen(true)
                .background_color(tauri::window::Color(3, 6, 12, 255))
                // 让网页自己处理拖进来的照片（否则窗口层面会先截走文件拖放）
                .disable_drag_drop_handler()
                .additional_browser_args(&browser_args)
                // 让页面知道自己运行在客户端里
                .initialization_script(concat!(
                    "window.__NOEL_DESKTOP__ = { version: '",
                    env!("CARGO_PKG_VERSION"),
                    "' };"
                ))
                .on_navigation(move |url| {
                    if stays_inside(url) {
                        return true;
                    }
                    let _ = handle.opener().open_url(url.as_str(), None::<&str>);
                    false
                })
                .on_new_window(move |url, _features| {
                    if stays_inside(&url) {
                        return NewWindowResponse::Allow;
                    }
                    let _ = popup_handle.opener().open_url(url.as_str(), None::<&str>);
                    NewWindowResponse::Deny
                })
                .build()?;
            #[cfg(windows)]
            allow_camera(&window);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start Noël");
}
