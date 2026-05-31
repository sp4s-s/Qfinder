use rusqlite::{Connection, Result};
use std::path::Path;

pub fn initialize_db(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA cache_size = -8000;
         PRAGMA temp_store = MEMORY;
         PRAGMA mmap_size = 0;
         PRAGMA wal_autocheckpoint = 1000;",
    )?;

    let version: i32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    if version < 2 {
        conn.execute_batch(
            "DROP TABLE IF EXISTS items;
             DROP TABLE IF EXISTS search;
             
             CREATE TABLE items(
                 id INTEGER PRIMARY KEY,
                 type TEXT NOT NULL,
                 title TEXT NOT NULL,
                 path TEXT UNIQUE,
                 content TEXT,
                 fingerprint TEXT,
                 created_at INTEGER,
                 access_count INTEGER DEFAULT 0,
                 last_accessed INTEGER
             );
             
             CREATE INDEX idx_items_fingerprint ON items(fingerprint);
             CREATE INDEX idx_items_type ON items(type);
             CREATE INDEX idx_items_last_accessed ON items(last_accessed);

             CREATE VIRTUAL TABLE search USING fts5(
                 title,
                 content,
                 content='items',
                 content_rowid='id',
                 tokenize='trigram'
             );

             CREATE TRIGGER items_ai AFTER INSERT ON items BEGIN
                 INSERT INTO search(rowid, title, content) VALUES (new.id, new.title, new.content);
             END;

             CREATE TRIGGER items_ad AFTER DELETE ON items BEGIN
                 INSERT INTO search(search, rowid, title, content) VALUES('delete', old.id, old.title, old.content);
             END;

             CREATE TRIGGER items_au AFTER UPDATE ON items BEGIN
                 INSERT INTO search(search, rowid, title, content) VALUES('delete', old.id, old.title, old.content);
                 INSERT INTO search(rowid, title, content) VALUES (new.id, new.title, new.content);
             END;
             
             PRAGMA user_version = 2;",
        )?;
    }

    let _ = conn.execute("INSERT INTO search(search) VALUES('rebuild');", []);

    Ok(conn)
}
