#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;
use tauri::{Manager, Emitter, RunEvent};

struct PendingFile(Mutex<Option<String>>);

fn url_to_path(url: &tauri::Url) -> String {
    if url.scheme() == "file" {
        url.to_file_path()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| url.to_string())
    } else {
        url.to_string()
    }
}

fn is_md_path(path: &str) -> bool {
    path.ends_with(".md") || path.ends_with(".markdown") || path.ends_with(".txt")
}

#[tauri::command]
fn get_pending_file(state: tauri::State<PendingFile>) -> Option<String> {
    state.0.lock().unwrap().take()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(PendingFile(Mutex::new(None)))
        .invoke_handler(tauri::generate_handler![get_pending_file])
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // App 已运行时通过命令行参数打开
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
            for arg in &args {
                if is_md_path(arg) {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.eval(&format!(
                            "if(window.__openFileFromArg)window.__openFileFromArg('{}')",
                            arg.replace('\\', "\\\\").replace('\'', "\\'")
                        ));
                    }
                    break;
                }
            }
        }))
        .setup(|app| {
            // 命令行启动时检查参数中的文件路径
            let args = app.env().args_os;
            for arg in args {
                let path = arg.to_string_lossy().to_string();
                if is_md_path(&path) {
                    let state = app.state::<PendingFile>();
                    *state.0.lock().unwrap() = Some(path);
                    break;
                }
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            // macOS 文件关联打开：通过 RunEvent::Opened 接收 file:// URL
            #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
            if let RunEvent::Opened { urls } = event {
                for url in &urls {
                    let path = url_to_path(url);
                    if is_md_path(&path) {
                        // 存入 state，供 JS 初始化后 invoke 获取
                        {
                            let state = app.state::<PendingFile>();
                            *state.0.lock().unwrap() = Some(path.clone());
                        }
                        // 尝试 eval（webview 已加载时直接打开）
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.eval(&format!(
                                "if(window.__openFileFromArg)window.__openFileFromArg('{}')",
                                path.replace('\\', "\\\\").replace('\'', "\\'")
                            ));
                        }
                        // 发射事件作为备选
                        let _ = app.emit("file-opened", &path);
                        break;
                    }
                }
            }
        });
}
