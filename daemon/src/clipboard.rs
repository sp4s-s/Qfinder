use objc::{msg_send, sel, sel_impl, runtime::{Class, Object}};
use std::{thread, time::Duration, path::Path, fs};

#[link(name = "AppKit", kind = "framework")]
extern "C" {}


pub fn start_clipboard_monitor(conn_path: String) {
    thread::spawn(move || {
        let conn = rusqlite::Connection::open(&conn_path).unwrap();
        
        let vault_dir = shellexpand::tilde("~/.qfinder").to_string();
        let clip_dir = format!("{}/clipboard", vault_dir);
        fs::create_dir_all(&clip_dir).unwrap();

        let cls = Class::get("NSPasteboard").unwrap();
        let pasteboard: *mut Object = unsafe { msg_send![cls, generalPasteboard] };
        let mut last_change_count: i64 = unsafe { msg_send![pasteboard, changeCount] };
        
        let mut sleep_time = 250;
        let mut no_change_ticks = 0;

        loop {
            thread::sleep(Duration::from_millis(sleep_time));
            let current_change_count: i64 = unsafe { msg_send![pasteboard, changeCount] };

            if current_change_count != last_change_count {
                last_change_count = current_change_count;
                sleep_time = 250;
                no_change_ticks = 0;

                unsafe {
                    let nsstring_cls = Class::get("NSString").unwrap();
                    let now = chrono::Utc::now().timestamp();

                    let type_file: *mut Object = msg_send![nsstring_cls, stringWithUTF8String: b"public.file-url\0".as_ptr()];
                    let nsarray_cls = Class::get("NSArray").unwrap();
                    let file_array: *mut Object = msg_send![nsarray_cls, arrayWithObject: type_file];
                    let has_file: bool = msg_send![pasteboard, availableTypeFromArray: file_array];
                    
                    if has_file {
                        let ns_string: *mut Object = msg_send![pasteboard, stringForType: type_file];
                        if !ns_string.is_null() {
                            let utf8_str: *const libc::c_char = msg_send![ns_string, UTF8String];
                            if !utf8_str.is_null() {
                                let file_url_str = std::ffi::CStr::from_ptr(utf8_str).to_string_lossy().into_owned();
                                let clean_path = file_url_str.replace("file://localhost", "").replace("file://", "");
                                let decoded_path = urlencoding::decode(&clean_path).unwrap_or(std::borrow::Cow::Borrowed(&clean_path)).into_owned();
                                
                                let filename = Path::new(&decoded_path).file_name().unwrap_or_default().to_string_lossy().into_owned();
                                let fingerprint = crate::search::sort_chars(&filename);

                                let _ = conn.execute(
                                    "INSERT INTO items (type, title, path, content, fingerprint, created_at, last_accessed) 
                                     VALUES ('file', ?1, ?2, ?2, ?3, ?4, ?4) ON CONFLICT(path) DO UPDATE SET last_accessed=?4",
                                    rusqlite::params![format!("📄 File: {}", filename), decoded_path, fingerprint, now],
                                );
                                continue;
                            }
                        }
                    }

                    let type_png: *mut Object = msg_send![nsstring_cls, stringWithUTF8String: b"public.png\0".as_ptr()];
                    let png_array: *mut Object = msg_send![nsarray_cls, arrayWithObject: type_png];
                    let has_png: bool = msg_send![pasteboard, availableTypeFromArray: png_array];
                    
                    if has_png {
                        let ns_data: *mut Object = msg_send![pasteboard, dataForType: type_png];
                        if !ns_data.is_null() {
                            let data_bytes: *const u8 = msg_send![ns_data, bytes];
                            let data_len: usize = msg_send![ns_data, length];
                            
                            if data_bytes != std::ptr::null() && data_len > 0 {
                                let slice = std::slice::from_raw_parts(data_bytes, data_len);
                                let img_name = format!("clip_{}.png", now);
                                let img_path = format!("{}/{}", clip_dir, img_name);
                                
                                if fs::write(&img_path, slice).is_ok() {
                                    let _ = conn.execute(
                                        "INSERT INTO items (type, title, path, content, fingerprint, created_at, last_accessed) 
                                         VALUES ('clipboard', ?1, ?2, '🖼️ Image File Payload', '', ?3, ?3)",
                                        rusqlite::params![img_name, img_path, now],
                                    );
                                    continue;
                                }
                            }
                        }
                    }

                    let type_text: *mut Object = msg_send![nsstring_cls, stringWithUTF8String: b"public.utf8-plain-text\0".as_ptr()];
                    let text_array: *mut Object = msg_send![nsarray_cls, arrayWithObject: type_text];
                    let has_text: bool = msg_send![pasteboard, availableTypeFromArray: text_array];
                    
                    if has_text {
                        let ns_string: *mut Object = msg_send![pasteboard, stringForType: type_text];
                        if !ns_string.is_null() {
                            let utf8_str: *const libc::c_char = msg_send![ns_string, UTF8String];
                            if !utf8_str.is_null() {
                                let mut raw_text = std::ffi::CStr::from_ptr(utf8_str).to_string_lossy().into_owned();
                                
                                if raw_text.len() > 50 * 1024 * 1024 {
                                    raw_text.truncate(50 * 1024 * 1024);
                                }

                                let is_dup: bool = conn.query_row(
                                    "SELECT EXISTS(SELECT 1 FROM items WHERE type='clipboard' AND content = ?1 ORDER BY id DESC LIMIT 1)",
                                    rusqlite::params![raw_text], |row| row.get(0)
                                ).unwrap_or(false);

                                if !is_dup && !raw_text.trim().is_empty() {
                                    let preview = if raw_text.len() > 60 { format!("{}...", &raw_text[..60].replace("\n", " ")) } else { raw_text.clone() };
                                    let fingerprint = crate::search::sort_chars(&preview);
                                    
                                    let txt_name = format!("clip_{}.txt", now);
                                    let txt_path = format!("{}/{}", clip_dir, txt_name);
                                    let _ = fs::write(&txt_path, &raw_text);

                                    let _ = conn.execute(
                                        "INSERT INTO items (type, title, path, content, fingerprint, created_at, last_accessed) 
                                         VALUES ('clipboard', ?1, ?2, ?3, ?4, ?5, ?5)",
                                        rusqlite::params![preview, txt_path, raw_text, fingerprint, now],
                                    );
                                }
                            }
                        }
                    }
                }
            } else {
                no_change_ticks += 1;
                if no_change_ticks > 240 {
                    sleep_time = 2000;
                }
            }
        }
    });
}