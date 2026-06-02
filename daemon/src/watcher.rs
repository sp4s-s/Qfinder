use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;
use std::path::Path;
use std::fs;
use std::sync::mpsc;
use std::time::Duration;

fn get_smart_file_type(path: &Path) -> &'static str {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "heic" | "webp" => "image",
        "mp4" | "mov" | "mkv" | "avi" => "video",
        "pdf" => "pdf",
        "md" | "txt" | "rtf" => "note",
        _ => "file",
    }
}

fn initial_crawl(conn: &rusqlite::Connection, dir_path: &Path) {
    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();
            
            if path_str.contains(".git/") || path_str.contains("node_modules/") || path_str.contains(".DS_Store") {
                continue;
            }

            if path.is_dir() {
                initial_crawl(conn, &path);
            } else {
                let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                let fingerprint = crate::search::sort_chars(&filename);
                let now = chrono::Utc::now().timestamp();
                
                let item_type = get_smart_file_type(&path);

                let _ = conn.execute(
                    "INSERT INTO items (type, title, path, content, fingerprint, created_at, last_accessed, access_count) 
                     VALUES (?1, ?2, ?3, '', ?4, ?5, ?5, 1) 
                     ON CONFLICT(path) DO NOTHING",
                    rusqlite::params![item_type, filename, path_str, fingerprint, now],
                );
            }
        }
    }
}

pub fn start_fs_watcher(conn_path: String, watch_dirs: Vec<String>) {
    let conn_path_clone = conn_path.clone();
    let watch_dirs_clone = watch_dirs.clone();
    
    std::thread::spawn(move || {
        if let Ok(conn_scan) = rusqlite::Connection::open(&conn_path_clone) {
            for dir in &watch_dirs_clone {
                let p = shellexpand::tilde(dir).to_string();
                let path_ref = Path::new(&p);
                if path_ref.is_dir() {
                    initial_crawl(&conn_scan, path_ref);
                }
            }
        }

        let (tx, rx) = mpsc::channel();
        let mut debouncer = new_debouncer(Duration::from_millis(500), tx).unwrap();
        let watcher = debouncer.watcher();

        for dir in watch_dirs {
            let p = shellexpand::tilde(&dir).to_string();
            let _ = watcher.watch(Path::new(&p), RecursiveMode::Recursive);
        }

        let mut conn = rusqlite::Connection::open(&conn_path).unwrap();

        loop {
            if let Ok(Ok(events)) = rx.recv() {
                let tx = conn.transaction().unwrap();
                for event in events {
                    let path_str = event.path.to_string_lossy().to_string();
                    if path_str.contains(".git/") || path_str.contains("node_modules/") || path_str.contains(".DS_Store") {
                        continue;
                    }
                    
                    let filename = event.path.file_name().unwrap_or_default().to_string_lossy().to_string();
                    let fingerprint = crate::search::sort_chars(&filename);
                    let now = chrono::Utc::now().timestamp();
                    
                    let mut content = String::new();
                    if path_str.ends_with(".md") {
                        content = std::fs::read_to_string(&event.path).unwrap_or_default();
                    }

                    let item_type = get_smart_file_type(&event.path);

                    let _ = tx.execute(
                        "INSERT INTO items (type, title, path, content, fingerprint, created_at, last_accessed, access_count) 
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 1) 
                         ON CONFLICT(path) DO UPDATE SET title=?2, content=?4, fingerprint=?5, last_accessed=?6, access_count=access_count+1",
                        rusqlite::params![item_type, filename, path_str, content, fingerprint, now],
                    );
                }
                let _ = tx.commit();
            }
        }
    });
}
