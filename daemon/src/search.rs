use rusqlite::{functions::FunctionFlags, Connection, Result};
use serde::Serialize;
use strsim::damerau_levenshtein;

#[derive(Serialize)]
pub struct SearchResult {
    pub id: i64,
    pub r#type: String,
    pub title: String,
    pub path: Option<String>,
    pub score: f64,
    pub match_pct: u8,
}

pub fn register_fuzzy_match(conn: &Connection) -> Result<()> {
    conn.create_scalar_function(
        "fuzzy_score",
        2,
        FunctionFlags::SQLITE_DETERMINISTIC | FunctionFlags::SQLITE_UTF8,
        move |ctx| {
            let query: String = ctx.get(0)?;
            let target: String = ctx.get(1)?;
            if target.is_empty() { return Ok(0.0f64); }
            let distance = damerau_levenshtein(&query.to_lowercase(), &target.to_lowercase());
            let max_len = std::cmp::max(query.len(), target.len()) as f64;
            Ok(1.0 - (distance as f64 / max_len))
        },
    )?;

    conn.create_scalar_function(
        "token_score",
        2,
        FunctionFlags::SQLITE_DETERMINISTIC | FunctionFlags::SQLITE_UTF8,
        move |ctx| {
            let query: String = ctx.get(0)?;
            let target: String = ctx.get(1)?;
            if query.is_empty() || target.is_empty() { return Ok(0.0f64); }
            
            let query_norm = query.chars().map(|c| if c == '-' || c == '_' { ' ' } else { c }).collect::<String>().to_lowercase();
            let target_norm = target.chars().map(|c| if c == '-' || c == '_' { ' ' } else { c }).collect::<String>().to_lowercase();
            
            let q_tokens: Vec<&str> = query_norm.split_whitespace().collect();
            let t_tokens: Vec<&str> = target_norm.split_whitespace().collect();
            
            let mut matched = 0;
            for q in &q_tokens {
                if t_tokens.contains(q) {
                    matched += 3;
                } else if target_norm.contains(q) {
                    matched += 1;
                }
            }
            
            let all_matched = q_tokens.iter().all(|q| target_norm.contains(q));
            let bonus = if all_matched { 6.0 } else { 0.0 };
            
            let exact_bonus = if target_norm == query_norm {
                10.0
            } else if target_norm.starts_with(&query_norm) {
                5.0
            } else if target_norm.contains(&query_norm) {
                3.0
            } else {
                0.0
            };
            
            Ok((matched as f64) + bonus + exact_bonus)
        },
    )
}

pub fn sort_chars(input: &str) -> String {
    let mut chars: Vec<char> = input.to_lowercase().chars().collect();
    chars.sort_unstable();
    chars.into_iter().collect()
}

// normalize - and _ to space so spass-cv == spass_cv == spass cv
fn normalize(s: &str) -> String {
    s.chars().map(|c| if c == '-' || c == '_' { ' ' } else { c }).collect()
}

// SQL expression that normalizes a column the same way
fn sql_norm(col: &str) -> String {
    format!("replace(replace(lower({}), '-', ' '), '_', ' ')", col)
}

fn prepare_trigram_query(query: &str) -> String {
    let norm = normalize(query);
    let tokens: Vec<String> = norm
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() >= 3)
        .map(|s| format!("\"{}\"", s))
        .collect();
    tokens.join(" AND ")
}

fn score_to_pct(score: f64) -> u8 {
    if score >= 8.0 { 99 }
    else if score >= 4.0 { ((score / 8.0) * 100.0) as u8 }
    else if score >= 1.0 { ((score / 4.0) * 60.0) as u8 }
    else { ((score.max(0.0)) * 30.0) as u8 }
}

pub fn execute_search(conn: &Connection, query: &str, scope: Option<&str>) -> Result<Vec<SearchResult>> {
    let mut results: Vec<SearchResult> = Vec::new();
    let current_time = chrono::Utc::now().timestamp();
    let clean_query = query.trim();
    if clean_query.is_empty() { return Ok(results); }

    let type_filter = match scope {
        Some("note")      => "AND (items.type = 'note' OR items.type = 'pdf')",
        Some("file")      => "AND (items.type = 'file' OR items.type = 'image' OR items.type = 'video' OR items.type = 'pdf')",
        Some("clipboard") => "AND items.type = 'clipboard'",
        _ => "",
    };

    let norm_q = normalize(clean_query).to_lowercase();
    let title_norm = sql_norm("title");

    // title bonus: use our custom token scorer
    let title_bonus = "token_score(:nq, title)".to_string();

    let folder_boost = "CASE
        WHEN path LIKE '%/Downloads/%' THEN 1.5
        WHEN path LIKE '%/Desktop/%'   THEN 1.5
        WHEN path LIKE '%/Documents/%' THEN 0.8
        WHEN path LIKE '%/.qfinder/notes/%' THEN 0.8
        ELSE 0.0 END";

    let recency = "CASE
        WHEN (:now - last_accessed) < 3600   THEN 1.5
        WHEN (:now - last_accessed) < 86400  THEN 0.8
        WHEN (:now - last_accessed) < 604800 THEN 0.3
        ELSE 0.0 END";

    // --- short query: no FTS possible, title LIKE only ---
    if clean_query.len() < 3 {
        let sql = format!(
            "SELECT id, type, title, path, ({} + {} + {}) as score
             FROM items WHERE {title_norm} LIKE '%' || lower(:nq) || '%' {}
             ORDER BY score DESC, last_accessed DESC LIMIT 25",
            title_bonus, folder_boost, recency, type_filter,
            title_norm = title_norm
        );
        if let Ok(mut stmt) = conn.prepare(&sql) {
            if let Ok(iter) = stmt.query_map(
                rusqlite::named_params! { ":nq": norm_q, ":now": current_time },
                |row| { let s: f64 = row.get(4)?; Ok(mk(row, s)) },
            ) { for r in iter.flatten() { results.push(r); } }
        }
        return Ok(results);
    }

    // --- tier 1: FTS5 with normalized title re-scoring ---
    let fts_query = prepare_trigram_query(clean_query);
    if !fts_query.is_empty() {
        let sql = format!(
            "SELECT id, type, title, path,
             ((-bm25(search) * 0.8) + {tb} + {fb} + (0.2 * MIN(access_count,10)) + {rec}) as score
             FROM items JOIN search ON items.id = search.rowid
             WHERE search MATCH :fts {tf}
             ORDER BY score DESC LIMIT 50",
            tb = title_bonus, fb = folder_boost, rec = recency, tf = type_filter
        );
        if let Ok(mut stmt) = conn.prepare(&sql) {
            if let Ok(iter) = stmt.query_map(
                rusqlite::named_params! { ":fts": fts_query, ":nq": norm_q, ":now": current_time },
                |row| { let s: f64 = row.get(4)?; Ok(mk(row, s)) },
            ) { for r in iter.flatten() { results.push(r); } }
        }
    }

    // --- tier 2: normalized title LIKE (catches when FTS dropped short tokens like "cv") ---
    // run always, merge with dedup so we don't lose good title matches that FTS missed
    {
        // pick longest token as anchor for LIKE
        let best = normalize(clean_query).to_lowercase();
        let anchor = best.split_whitespace().max_by_key(|s| s.len()).unwrap_or(&best).to_string();
        if anchor.len() >= 2 {
            let sql = format!(
                "SELECT id, type, title, path, ({tb} + {fb} + {rec}) as score
                 FROM items WHERE {tn} LIKE '%' || :anchor || '%' {tf}
                 ORDER BY score DESC, last_accessed DESC LIMIT 50",
                tb = title_bonus, fb = folder_boost, rec = recency,
                tn = title_norm, tf = type_filter
            );
            if let Ok(mut stmt) = conn.prepare(&sql) {
                if let Ok(iter) = stmt.query_map(
                    rusqlite::named_params! { ":nq": norm_q, ":anchor": anchor, ":now": current_time },
                    |row| { let s: f64 = row.get(4)?; Ok(mk(row, s)) },
                ) {
                    let existing_ids: std::collections::HashSet<i64> = results.iter().map(|r| r.id).collect();
                    for r in iter.flatten() {
                        if !existing_ids.contains(&r.id) { results.push(r); }
                    }
                }
            }
        }
    }

    // sort combined results by score
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(25);

    // --- tier 3: fuzzy edit distance (only if still empty) ---
    if results.is_empty() {
        let sql = format!(
            "SELECT id, type, title, path, fuzzy_score(:q, title) AS similarity
             FROM items WHERE abs(length(title) - length(:q)) <= 5 AND similarity > 0.35 {tf}
             ORDER BY similarity DESC LIMIT 25",
            tf = type_filter
        );
        if let Ok(mut stmt) = conn.prepare(&sql) {
            if let Ok(iter) = stmt.query_map(&[(":q", &clean_query)], |row| {
                let s: f64 = row.get(4)?;
                Ok(SearchResult { id: row.get(0)?, r#type: row.get(1)?, title: row.get(2)?,
                    path: row.get(3)?, score: s, match_pct: (s * 100.0) as u8 })
            }) { for r in iter.flatten() { results.push(r); } }
        }
    }

    Ok(results)
}

fn mk(row: &rusqlite::Row, score: f64) -> SearchResult {
    SearchResult {
        id: row.get(0).unwrap(),
        r#type: row.get(1).unwrap(),
        title: row.get(2).unwrap(),
        path: row.get(3).unwrap(),
        score,
        match_pct: score_to_pct(score),
    }
}
