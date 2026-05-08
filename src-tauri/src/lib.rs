mod process_scan;

use process_scan::{scan_processes_impl, terminate_process_group_impl, ScanResult};

#[tauri::command]
fn scan_processes() -> Result<ScanResult, String> {
    scan_processes_impl()
}

#[tauri::command]
fn terminate_process_group(pgid: i32, force: bool) -> Result<(), String> {
    terminate_process_group_impl(pgid, force)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            scan_processes,
            terminate_process_group
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

