mod db;
mod search;
mod watcher;
mod clipboard;

use tokio::{net::UnixListener, io::{AsyncReadExt, AsyncWriteExt}};
use std::sync::Arc;
use serde_json::Value;
use std::os::unix::fs::PermissionsExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let vault_dir = shellexpand::tilde("~/.qfinder").to_string();
    std::fs::create_dir_all(&vault_dir)?;
    
    let socket_path = format!("{}/socket.sock", vault_dir);
    let db_path = format!("{}/db.sqlite", vault_dir);
    let _ = std::fs::remove_file(&socket_path);

    let conn = db::initialize_db(std::path::Path::new(&db_path))?;
    search::register_fuzzy_match(&conn)?;
    let safe_conn = Arc::new(std::sync::Mutex::new(conn));

    let config_path = format!("{}/config.json", vault_dir);
    let config_val: Option<serde_json::Value> = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());

    let watch_dirs: Vec<String> = config_val.as_ref()
        .and_then(|v| v["watch_dirs"].as_array().cloned())
        .map(|arr| arr.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_else(|| vec![
            "~/Downloads".to_string(),
            "~/Desktop".to_string(),
            "~/Documents".to_string(),
            "~/Movies".to_string(),
            "~/Pictures".to_string(),
            "~/.qfinder/notes".to_string(),
        ]);

    let clipboard_enabled = config_val.as_ref()
        .and_then(|v| v["clipboard_enabled"].as_bool())
        .unwrap_or(true);

    watcher::start_fs_watcher(db_path.clone(), watch_dirs);
    
    if clipboard_enabled {
        clipboard::start_clipboard_monitor(db_path);
    }

    let listener = UnixListener::bind(&socket_path)?;
    std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))?;

    loop {
        if let Ok((mut stream, _)) = listener.accept().await {
            let conn_ref = safe_conn.clone();
            tokio::spawn(async move {
                let mut len_buf = [0u8; 4];
                loop {
                    if stream.read_exact(&mut len_buf).await.is_err() { break; }
                    let msg_len = u32::from_be_bytes(len_buf) as usize;
                    if msg_len == 0 || msg_len > 1_048_576 { break; }
                    let mut msg_buf = vec![0u8; msg_len];
                    if stream.read_exact(&mut msg_buf).await.is_err() { break; }

                    if let Ok(request) = serde_json::from_slice::<Value>(&msg_buf) {
                        let response = {
                            let locked_conn = conn_ref.lock().unwrap();
                            match request["command"].as_str() {
                                Some("search") => {
                                    let query = request["query"].as_str().unwrap_or("");
                                    let scope = request["scope"].as_str();
                                    serde_json::to_string(&search::execute_search(&locked_conn, query, scope).unwrap_or_default()).unwrap_or_default()
                                },
                                Some("clear_clipboard") => {
                                    let vault_dir = shellexpand::tilde("~/.qfinder").to_string();
                                    let clip_dir = format!("{}/clipboard", vault_dir);
                                    let _ = locked_conn.execute("DELETE FROM items WHERE type='clipboard'", []);
                                    if let Ok(entries) = std::fs::read_dir(clip_dir) {
                                        for entry in entries.flatten() {
                                            let _ = std::fs::remove_file(entry.path());
                                        }
                                    }
                                    "{\"status\":\"cleared\"}".to_string()
                                },
                                _ => "{\"error\":\"unknown_command\"}".to_string(),
                            }
                        };

                        let resp_bytes = response.into_bytes();
                        let resp_len = (resp_bytes.len() as u32).to_be_bytes();
                        if stream.write_all(&resp_len).await.is_err() { break; }
                        if stream.write_all(&resp_bytes).await.is_err() { break; }
                    }
                }
            });
        }
    }
}