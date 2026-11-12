use rusqlite::types::Value;

use crate::types::common_types::*;
use crate::store::db::DbHandle;
use crate::store::tag_store;


pub fn fetch_total_photo_count(db: &DbHandle) -> Result<u32, String> {
    let conn = db.lock().unwrap();
    conn.query_row("SELECT count(*) FROM photos", [], |row| row.get(0))
        .map_err(|e| e.to_string())
}

/// Existing library records keep the preview geometry their saved edits were made against.
pub fn raw_uses_legacy_preview(db: &DbHandle, path: &str) -> Result<bool, String> {
    let normalized = path.replace('\\', "/");
    let conn = db.lock().map_err(|_| "Photo database is unavailable")?;
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM photos WHERE master_dir || '/' || master_filename = ?1 AND raw_sensor_render = 0)",
        [&normalized], |row| row.get(0),
    ).map_err(|e| e.to_string())
}

/// Every non-trashed master file currently in the library. Duplicate detection deliberately works
/// from these database-owned paths instead of accepting arbitrary filesystem paths from the web view.
pub fn fetch_active_photo_paths(db: &DbHandle) -> Result<Vec<String>, String> {
    let conn = db.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT master_dir, master_filename FROM photos WHERE trashed = 0")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let dir: String = row.get(0)?;
            let filename: String = row.get(1)?;
            Ok(format!("{}/{}", dir, filename))
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Folders (all image-bearing directories and their ancestors inside the photo roots)
// ---------------------------------------------------------------------------

/// Imported, non-trashed photos are the source of valid image directories. Include every ancestor
/// needed to reach them, but never enumerate empty/unsupported-only directories from disk.
pub fn fetch_photo_folders(db: &DbHandle, photo_roots: &[String]) -> Result<Vec<PhotoFolder>, String> {
    let conn = db.lock().unwrap();
    let mut stmt = conn.prepare("SELECT master_dir, count(*) FROM photos WHERE trashed = 0 GROUP BY master_dir")
        .map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?;
    let mut folders = std::collections::BTreeMap::<String, PhotoFolder>::new();
    for (dir, count) in rows {
        let dir = normalise_path(&dir);
        let mut visited = std::collections::HashSet::new();
        for root in photo_roots {
            let root = normalise_path(root);
            if !path_is_within(&dir, &root) { continue; }
            let mut path = root.clone();
            let mut parent_path = None;
            let mut depth = 0;
            let rest = dir[root.len()..].trim_start_matches('/');
            for segment in std::iter::once("").chain(rest.split('/').filter(|s| !s.is_empty())) {
                if !segment.is_empty() {
                    parent_path = Some(path.clone());
                    path = format!("{}/{}", path, segment);
                    depth += 1;
                }
                let key = path.to_lowercase();
                if !visited.insert(key.clone()) { continue; }
                let folder = folders.entry(key).or_insert_with(|| PhotoFolder {
                    name: path.rsplit('/').next().unwrap_or(&path).to_string(),
                    path: path.clone(), parent_path: parent_path.clone(), depth, count: 0,
                });
                folder.count += count;
            }
        }
    }
    // Resolve nesting again after aggregation, including overlapping configured roots.
    for key in folders.keys().cloned().collect::<Vec<_>>() {
        let parent = folders[&key].path.rsplit_once('/').map(|(path, _)| path.to_lowercase());
        let parent_info = parent.and_then(|parent| folders.get(&parent).map(|f| (f.path.clone(), f.depth + 1)));
        let folder = folders.get_mut(&key).unwrap();
        folder.parent_path = parent_info.as_ref().map(|(path, _)| path.clone());
        folder.depth = parent_info.map_or(0, |(_, depth)| depth);
    }
    Ok(folders.into_values().collect())
}

fn path_is_within(path: &str, root: &str) -> bool {
    path.eq_ignore_ascii_case(root) ||
        (path.get(..root.len()).map_or(false, |prefix| prefix.eq_ignore_ascii_case(root)) &&
         path.as_bytes().get(root.len()) == Some(&b'/'))
}

/// The name of the top-level folder `master_dir` lies in (the first path segment after the photo root
/// containing it), or `None` when `master_dir` is a photo root itself. The photo root is found by a
/// case-insensitive longest-prefix match on path components — Windows paths may differ in case and in
/// separator (`\` vs `/`) between settings and import, and a root must not match a sibling that merely
/// shares a prefix (`…/photos2` is not inside `…/photos`).
fn top_level_folder(master_dir: &str, photo_roots: &[String]) -> Option<String> {
    let dir = normalise_path(master_dir);
    let mut best_root: Option<String> = None;
    for root in photo_roots {
        let root = normalise_path(root);
        if dir.len() > root.len()
            && dir.as_bytes()[root.len()] == b'/'
            && dir[..root.len()].eq_ignore_ascii_case(&root)
            && best_root.as_ref().map_or(true, |best| root.len() > best.len())
        {
            best_root = Some(root);
        }
    }
    let root = best_root?;
    let rest = &dir[root.len() + 1..];
    let segment_end = rest.find('/').unwrap_or(rest.len());
    if segment_end == 0 {
        return None;
    }
    Some(rest[..segment_end].to_string())
}

/// Replaces `\` with `/` and drops trailing separators, so paths from settings (Windows backslash
/// form) and from the import scanner (forward-slash joins) compare equal.
fn normalise_path(path: &str) -> String {
    let trimmed = path.trim_end_matches(['/', '\\']);
    if trimmed.contains('\\') {
        trimmed.replace('\\', "/")
    } else {
        trimmed.to_string()
    }
}

/// The `root/folder` prefixes to match a folder filter against: one per photo root (same longest-prefix
/// rule as `top_level_folder`). A prefix under a root that has no such folder simply matches nothing,
/// so no filesystem check is needed here. Prefixes are normalised to forward slashes, and
/// `filter_where` normalises the `master_dir` column the same way — rows hold whatever separator form
/// the photo root had in the settings when it was scanned (`D:\dir/sub` on Windows), so the comparison
/// must be separator-agnostic on the row side.
fn folder_prefixes(folder: &str, photo_roots: &[String]) -> Vec<String> {
    let folder = normalise_path(folder);
    if folder.starts_with('/') || folder.as_bytes().get(1) == Some(&b':') {
        return if photo_roots.iter().any(|root| path_is_within(&folder, &normalise_path(root))) {
            vec![folder]
        } else { vec![] };
    }
    photo_roots
        .iter()
        .map(|root| format!("{}/{}", normalise_path(root), folder))
        .collect()
}

pub fn fetch_sections(
    db: &DbHandle,
    filter: &PhotoFilter,
    section_ids_to_keep_loaded: Option<&[PhotoSectionId]>,
) -> Result<Vec<PhotoSection>, String> {
    let conn = db.lock().unwrap();
    let (where_sql, where_params) = filter_where(filter);
    let sql = format!(
        "SELECT date_section AS id, date_section AS title, count(*) AS count \
         FROM photos WHERE {where_sql} \
         GROUP BY date_section ORDER BY date_section DESC"
    );

    let mut sections: Vec<PhotoSection> = {
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt.query_map(rusqlite::params_from_iter(where_params.iter()), |row| {
            Ok(PhotoSection {
                id:         row.get("id")?,
                title:      row.get("title")?,
                count:      row.get("count")?,
                photo_ids:  None,
                photo_data: None,
            })
        })
        .map_err(|e| e.to_string())?;
        let collected: Vec<_> = rows.filter_map(|r| r.ok()).collect();
        drop(stmt);
        collected
    };
    drop(conn);

    // Eagerly load photos for sections that should stay loaded
    if let Some(ids) = section_ids_to_keep_loaded {
        if !ids.is_empty() {
            let photo_sets = fetch_section_photos(db, ids, filter)?;
            let mut set_by_id: std::collections::HashMap<&str, PhotoSet> = ids
                .iter()
                .map(|id| id.as_str())
                .zip(photo_sets.into_iter())
                .collect();

            for section in &mut sections {
                if let Some(set) = set_by_id.remove(section.id.as_str()) {
                    section.photo_ids  = Some(set.photo_ids);
                    section.photo_data = Some(set.photo_data);
                }
            }
        }
    }

    Ok(sections)
}

pub fn fetch_section_photos(
    db: &DbHandle,
    section_ids: &[PhotoSectionId],
    filter: &PhotoFilter,
) -> Result<Vec<PhotoSet>, String> {
    if section_ids.is_empty() {
        return Ok(vec![]);
    }

    let conn = db.lock().unwrap();
    let (where_sql, mut where_params) = filter_where(filter);

    let placeholders: Vec<&str> = section_ids.iter().map(|_| "?").collect();
    let in_clause = placeholders.join(", ");

    // Section IDs go before filter params in the param list
    let mut params: Vec<Value> = section_ids
        .iter()
        .map(|id| Value::Text(id.clone()))
        .collect();
    params.append(&mut where_params);

    let sql = format!(
        "SELECT * FROM photos \
         WHERE date_section IN ({in_clause}) AND {where_sql} \
         ORDER BY created_at ASC"
    );

    // Build per-section containers in the same order as section_ids
    let mut result: Vec<PhotoSet> = section_ids
        .iter()
        .map(|_| PhotoSet {
            photo_ids:  vec![],
            photo_data: std::collections::HashMap::new(),
        })
        .collect();
    let section_index: std::collections::HashMap<&str, usize> = section_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let photos: Vec<Photo> = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), photo_from_row)
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    for photo in photos {
        if let Some(&idx) = section_index.get(photo.date_section.as_str()) {
            let id = photo.id;
            result[idx].photo_ids.push(id);
            // TypeScript uses string-keyed objects: { "42": Photo, ... }
            result[idx].photo_data.insert(id, photo);
        }
    }

    Ok(result)
}

pub fn fetch_photo_detail(db: &DbHandle, photo_id: PhotoId) -> Result<PhotoDetail, String> {
    let conn = db.lock().unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT title FROM tags \
             WHERE id IN (SELECT tag_id FROM photos_tags WHERE photo_id = ?) \
             ORDER BY slug",
        )
        .map_err(|e| e.to_string())?;
    let tags: Vec<String> = stmt
        .query_map(rusqlite::params![photo_id], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(PhotoDetail { tags })
}

pub fn update_photos(
    db: &DbHandle,
    photo_ids: &[PhotoId],
    update: &serde_json::Value,
) -> Result<(), String> {
    let obj = update.as_object().ok_or("update must be a JSON object")?;
    if obj.is_empty() || photo_ids.is_empty() {
        return Ok(());
    }

    // Whitelist of columns that the frontend is allowed to update
    let allowed = &[
        "flag", "trashed", "edited_width", "edited_height",
        "master_width", "master_height",
    ];

    let filtered: Vec<(&String, &serde_json::Value)> = obj
        .iter()
        .filter(|(k, _)| allowed.contains(&k.as_str()))
        .collect();

    if filtered.is_empty() {
        return Ok(());
    }

    let set_parts: Vec<String> = filtered
        .iter()
        .enumerate()
        .map(|(i, (k, _))| format!("{} = ?{}", k, i + 1))
        .collect();

    let values: Vec<Value> = filtered
        .iter()
        .map(|(_, v)| json_to_sql_value(v))
        .collect();

    let ids_csv = photo_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let sql = format!(
        "UPDATE photos SET {} WHERE id IN ({})",
        set_parts.join(", "),
        ids_csv
    );

    let conn = db.lock().unwrap();
    conn.execute(&sql, rusqlite::params_from_iter(values.iter()))
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// emptyTrash helpers
// ---------------------------------------------------------------------------

pub struct TrashedPhoto {
    pub id:              PhotoId,
    pub master_dir:      String,
    pub master_filename: String,
}

pub fn fetch_trashed_photos(db: &DbHandle) -> Result<Vec<TrashedPhoto>, String> {
    let conn = db.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT id, master_dir, master_filename FROM photos WHERE trashed = 1")
        .map_err(|e| e.to_string())?;
    let result = stmt.query_map([], |row| {
        Ok(TrashedPhoto {
            id:              row.get("id")?,
            master_dir:      row.get("master_dir")?,
            master_filename: row.get("master_filename")?,
        })
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();
    Ok(result)
}

/// Fetches the master paths of the given ids. Ids that are not in the library are silently skipped —
/// the caller works with what exists. Used by the disk-delete command to resolve paths from the DB
/// itself, so the web view never names files.
pub fn fetch_photos_by_ids(db: &DbHandle, photo_ids: &[PhotoId]) -> Result<Vec<TrashedPhoto>, String> {
    if photo_ids.is_empty() {
        return Ok(vec![]);
    }
    let ids_csv = photo_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let conn = db.lock().unwrap();
    let mut stmt = conn
        .prepare(&format!(
            "SELECT id, master_dir, master_filename FROM photos WHERE id IN ({})",
            ids_csv
        ))
        .map_err(|e| e.to_string())?;
    let result = stmt
        .query_map([], |row| {
            Ok(TrashedPhoto {
                id:              row.get("id")?,
                master_dir:      row.get("master_dir")?,
                master_filename: row.get("master_filename")?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(result)
}

pub fn delete_photos(db: &DbHandle, photo_ids: &[PhotoId]) -> Result<(), String> {
    if photo_ids.is_empty() {
        return Ok(());
    }
    let ids_csv = photo_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let mut conn = db.lock().unwrap();
    let transaction = conn.transaction().map_err(|e| e.to_string())?;
    transaction.execute_batch(&format!(
        "DELETE FROM photos_tags WHERE photo_id IN ({ids_csv});
         DELETE FROM versions     WHERE photo_id IN ({ids_csv});
         DELETE FROM photos       WHERE id       IN ({ids_csv});"
    ))
    .map_err(|e| e.to_string())?;
    transaction.commit().map_err(|e| e.to_string())?;

    // The deleted photos may have had tags
    crate::store::tag_store::mark_tags_deleted();

    Ok(())
}

// ---------------------------------------------------------------------------
// Import helpers
// ---------------------------------------------------------------------------

/// A photo row that is about to be inserted. `id` is assigned by the DB, and `trashed` is always 0
/// for freshly imported photos, so neither is part of this struct.
pub struct NewPhoto {
    pub master_dir: String,
    pub master_filename: String,
    pub master_width: u32,
    pub master_height: u32,
    pub edited_width: u32,
    pub edited_height: u32,
    pub date_section: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub imported_at: i64,
    pub flag: bool,
}

/// Inserts a batch of photos (and their tags) in a single transaction. This is the durable-write side
/// of the importer: probing/decoding already happened, so the write lock is held only briefly.
/// Returns `(inserted_count, tags_changed)` where `tags_changed` is true if any new tag was created.
pub fn insert_photos_batch(db: &DbHandle, items: &[(NewPhoto, Vec<String>)]) -> Result<(u32, bool), String> {
    if items.is_empty() {
        return Ok((0, false));
    }

    let conn = db.lock().unwrap();
    conn.execute_batch("BEGIN").map_err(|e| e.to_string())?;

    let result = (|| -> Result<(u32, bool), String> {
        let mut added = 0u32;
        let mut tags_changed = false;
        for (photo, tags) in items {
            // New imports use the sensor decoder. Migration 005 keeps existing RAW photos on their
            // embedded previews so saved pixel crop coordinates and thumbnails stay compatible.
            conn.execute(
                "INSERT INTO photos (\
                   master_dir, master_filename, master_width, master_height, \
                   edited_width, edited_height, date_section, created_at, updated_at, imported_at, flag, trashed, raw_sensor_render\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, 1)",
                rusqlite::params![
                    photo.master_dir,
                    photo.master_filename,
                    photo.master_width,
                    photo.master_height,
                    photo.edited_width,
                    photo.edited_height,
                    photo.date_section,
                    photo.created_at,
                    photo.updated_at,
                    photo.imported_at,
                    photo.flag as i64,
                ],
            )
            .map_err(|e| e.to_string())?;

            let photo_id = conn.last_insert_rowid();
            if !tags.is_empty() && tag_store::apply_photo_tags(&conn, photo_id, tags)? {
                tags_changed = true;
            }
            added += 1;
        }
        Ok((added, tags_changed))
    })();

    match result {
        Ok(value) => {
            conn.execute_batch("COMMIT").map_err(|e| e.to_string())?;
            Ok(value)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

/// Returns the `(id, master_filename)` of every photo already stored for the given directory.
/// Used to diff the filesystem against the DB (insert new files, delete vanished ones).
pub fn fetch_photos_of_directory(db: &DbHandle, dir: &str) -> Result<Vec<(PhotoId, String)>, String> {
    let conn = db.lock().unwrap();
    let mut stmt = conn
        .prepare("SELECT id, master_filename FROM photos WHERE master_dir = ?")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(rusqlite::params![dir], |row| {
            Ok((row.get::<_, PhotoId>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// Deletes all photos whose `master_dir` is not in `existing_dirs` (directories that no longer exist
/// or no longer contain photos). Returns the number of deleted photos.
pub fn delete_photos_of_removed_dirs(db: &DbHandle, existing_dirs: &[String]) -> Result<u32, String> {
    let ids: Vec<PhotoId> = {
        let conn = db.lock().unwrap();
        let (sql, params): (String, Vec<Value>) = if existing_dirs.is_empty() {
            ("SELECT id FROM photos".to_string(), vec![])
        } else {
            let placeholders = existing_dirs.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
            (
                format!("SELECT id FROM photos WHERE master_dir NOT IN ({})", placeholders),
                existing_dirs.iter().map(|d| Value::Text(d.clone())).collect(),
            )
        };
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params.iter()), |row| row.get::<_, PhotoId>(0))
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    if ids.is_empty() {
        return Ok(0);
    }
    delete_photos(db, &ids)?;
    Ok(ids.len() as u32)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build the WHERE clause for a PhotoFilter.
/// Returns (sql_fragment, bound_params).
fn filter_where(filter: &PhotoFilter) -> (String, Vec<Value>) {
    let trashed: i64 = if matches!(&filter.filter_type, PhotoFilterType::Trash) { 1 } else { 0 };
    let mut sql = "trashed = ?".to_string();
    let mut params: Vec<Value> = vec![Value::Integer(trashed)];

    if matches!(&filter.filter_type, PhotoFilterType::Favorites) {
        sql += " AND flag = 1";
    }

    if matches!(&filter.filter_type, PhotoFilterType::Tag) {
        if let Some(tag_id) = filter.tag_id {
            sql += " AND id IN (SELECT photo_id FROM photos_tags WHERE tag_id = ?)";
            params.push(Value::Integer(tag_id));
        }
    }

    if matches!(&filter.filter_type, PhotoFilterType::Folder) {
        if let Some(folder) = &filter.folder {
            if let Some(roots) = filter.photo_roots.as_deref() {
                // One condition per photo root, OR-combined: a photo matches when it lies in the
                // folder under *any* of the configured roots. AND-combining them (as a naïve
                // condition-per-root loop would) can never match — a photo lives under one root.
                let prefixes = folder_prefixes(folder, roots);
                if prefixes.is_empty() { return (format!("{} AND 0", sql), params); }
                let folder_conditions: Vec<String> = prefixes.iter().map(|_| {
                    "(REPLACE(master_dir, '\\', '/') = ? OR REPLACE(master_dir, '\\', '/') LIKE ? ESCAPE '\\')".to_string()
                }).collect();
                sql += &format!(" AND ({})", folder_conditions.join(" OR "));
                for prefix in prefixes {
                    // The folder itself or anything inside it (subdirectories included). LIKE's
                    // wildcards in the folder name are escaped; the root needs no escaping: it was
                    // matched component-wise against the real directory, so it may contain wildcards.
                    // The column is normalised to forward slashes so rows written with a backslash
                    // photo root (`D:\dir/sub`) match the normalised prefix (`D:/dir/folder`).
                    params.push(Value::Text(prefix.clone()));
                    params.push(Value::Text(format!("{}{}", like_escape(&prefix), "/%")));
                }
            }
        }
    }

    (sql, params)
}

/// Escapes LIKE wildcards (`%`, `_`) and the escape character (`\`) so a literal match can be made
/// with `LIKE ? ESCAPE '\'`.
fn like_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_")
}

/// Map a rusqlite Row to a Photo.
/// SQLite stores booleans as 0/1 integers; we convert them here.
pub(crate) fn photo_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Photo> {
    Ok(Photo {
        id:              row.get("id")?,
        master_dir:      row.get("master_dir")?,
        master_filename: row.get("master_filename")?,
        master_width:    row.get("master_width")?,
        master_height:   row.get("master_height")?,
        edited_width:    row.get("edited_width")?,
        edited_height:   row.get("edited_height")?,
        date_section:    row.get("date_section")?,
        created_at:      row.get("created_at")?,
        updated_at:      row.get("updated_at")?,
        imported_at:     row.get("imported_at")?,
        flag:            row.get::<_, i64>("flag")? != 0,
        trashed:         row.get::<_, i64>("trashed")? != 0,
    })
}

fn json_to_sql_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null      => Value::Null,
        serde_json::Value::Bool(b)   => Value::Integer(if *b { 1 } else { 0 }),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() { Value::Integer(i) }
            else if let Some(f) = n.as_f64() { Value::Real(f) }
            else { Value::Null }
        }
        serde_json::Value::String(s) => Value::Text(s.clone()),
        _ => Value::Null,
    }
}


#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn failed_photo_deletion_rolls_back_and_allows_retry() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE photos (id INTEGER PRIMARY KEY);
            CREATE TABLE photos_tags (photo_id INTEGER);
            CREATE TABLE versions (photo_id INTEGER);
            INSERT INTO photos VALUES (1);
            INSERT INTO photos_tags VALUES (1);
            CREATE TRIGGER fail_delete BEFORE DELETE ON photos BEGIN SELECT RAISE(ABORT, 'test failure'); END;").unwrap();
        let db: DbHandle = std::sync::Arc::new(std::sync::Mutex::new(conn));
        assert!(delete_photos(&db, &[1]).is_err());
        {
            let conn = db.lock().unwrap();
            assert!(conn.is_autocommit());
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM photos_tags", [], |row| row.get::<_, i64>(0)).unwrap(), 1);
            conn.execute_batch("DROP TRIGGER fail_delete").unwrap();
        }
        delete_photos(&db, &[1]).unwrap();
        assert_eq!(db.lock().unwrap().query_row("SELECT COUNT(*) FROM photos", [], |row| row.get::<_, i64>(0)).unwrap(), 0);
    }

    fn filter(filter_type: PhotoFilterType, folder: Option<&str>, photo_roots: &[&str]) -> PhotoFilter {
        PhotoFilter {
            filter_type,
            tag_id: None,
            folder: folder.map(String::from),
            photo_roots: Some(photo_roots.iter().map(|r| r.to_string()).collect()),
        }
    }

    #[test]
    fn top_level_folder_finds_first_segment_after_root() {
        let roots = vec!["C:/photos".to_string()];
        assert_eq!(top_level_folder("C:/photos/2025 Beijing", &roots).as_deref(), Some("2025 Beijing"));
        assert_eq!(top_level_folder("C:/photos/2025 Beijing/day1", &roots).as_deref(), Some("2025 Beijing"));
        assert_eq!(top_level_folder("C:/photos", &roots), None);
        assert_eq!(top_level_folder("C:/somewhere-else/2025", &roots), None);
        // A sibling sharing a prefix must not count as the root
        assert_eq!(top_level_folder("C:/photos2/2025", &roots), None);
    }

    #[test]
    fn top_level_folder_prefers_longest_root() {
        let roots = vec!["C:/photos".to_string(), "C:/photos/imported".to_string()];
        assert_eq!(top_level_folder("C:/photos/imported/2025", &roots).as_deref(), Some("2025"));
        assert_eq!(top_level_folder("C:/photos/2026", &roots).as_deref(), Some("2026"));
    }

    #[test]
    fn top_level_folder_matches_case_insensitively() {
        let roots = vec!["C:/Photos".to_string()];
        assert_eq!(top_level_folder("c:/photos/2025 Beijing", &roots).as_deref(), Some("2025 Beijing"));
    }

    #[test]
    fn top_level_folder_tolerates_trailing_slash_in_root() {
        let roots = vec!["C:/photos/".to_string()];
        assert_eq!(top_level_folder("C:/photos/2025", &roots).as_deref(), Some("2025"));
    }

    #[test]
    fn fetch_photo_folders_aggregates_and_sorts() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE photos (id integer PRIMARY KEY, master_dir text, trashed boolean DEFAULT 0);
             INSERT INTO photos (master_dir) VALUES
               ('C:/photos/2026 Chengdu'), ('C:/photos/2026 Chengdu'), ('C:/photos/2026 Chengdu/day1'),
               ('C:/photos/2025 Beijing'), ('C:/photos'), ('C:/photos2/2025'),
               ('C:/photos/Trash can');",
        )
        .unwrap();
        conn.execute("UPDATE photos SET trashed = 1 WHERE master_dir = 'C:/photos/Trash can'", [])
            .unwrap();

        let db: DbHandle = std::sync::Arc::new(std::sync::Mutex::new(conn));
        let folders = fetch_photo_folders(&db, &["C:/photos".to_string()]).unwrap();

        let names: Vec<&str> = folders.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["photos", "2025 Beijing", "2026 Chengdu", "day1"]);
        assert_eq!(folders[0].count, 5);
        assert_eq!(folders[1].count, 1);
        assert_eq!(folders[2].count, 3);
        assert_eq!(folders[3].count, 1);
        assert_eq!(folders[3].parent_path.as_deref(), Some("C:/photos/2026 Chengdu"));
        assert_eq!(folders[3].depth, 2);
    }

    #[test]
    fn folders_keep_full_paths_and_exclude_empty_or_trash_only_branches() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE photos (master_dir text, trashed boolean DEFAULT 0);
            INSERT INTO photos (master_dir) VALUES
            ('C:/photos/same/deep/leaf'), ('D:/archive/same/deep/leaf'),
            ('C:/photos2/outside');
            INSERT INTO photos (master_dir, trashed) VALUES ('C:/photos/trash-only', 1);") .unwrap();
        let db: DbHandle = std::sync::Arc::new(std::sync::Mutex::new(conn));
        let roots = vec!["C:/photos".into(), "D:/archive".into(), "C:/photos/same".into()];
        let folders = fetch_photo_folders(&db, &roots).unwrap();
        assert_eq!(folders.len(), 8);
        assert!(folders.iter().all(|f| f.count == 1)); // overlapping roots never double-count
        assert!(folders.iter().any(|f| f.path == "C:/photos/same/deep/leaf" && f.depth == 3));
        assert!(folders.iter().any(|f| f.path == "D:/archive/same/deep/leaf"));
        assert!(!folders.iter().any(|f| f.path.contains("trash-only") || f.path.contains("outside")));
        let selected = filter(PhotoFilterType::Folder, Some("C:/photos/same/deep/leaf"), &["C:/photos", "D:/archive"]);
        let (sql, params) = filter_where(&selected);
        let matched: u32 = db.lock().unwrap().query_row(&format!("SELECT count(*) FROM photos WHERE {}", sql),
            rusqlite::params_from_iter(params.iter()), |row| row.get(0)).unwrap();
        assert_eq!(matched, 1);
        let outside = filter(PhotoFilterType::Folder, Some("C:/photos2/outside"), &["C:/photos"]);
        assert!(filter_where(&outside).0.ends_with("AND 0"));
    }

    #[test]
    fn filter_where_folder_matches_folder_and_subfolders() {
        let filter = filter(PhotoFilterType::Folder, Some("2025 Beijing"), &["C:/photos"]);
        let (sql, params) = filter_where(&filter);
        assert!(sql.contains(r"REPLACE(master_dir, '\', '/') = ?"), "sql: {}", sql);
        assert!(sql.contains(r"LIKE ? ESCAPE '\'"), "sql: {}", sql);
        assert_eq!(params[1], Value::Text("C:/photos/2025 Beijing".to_string()));
        assert_eq!(params[2], Value::Text("C:/photos/2025 Beijing/%".to_string()));
    }

    /// A photo root stored in backslash form (typical Windows settings.json) must still match rows
    /// that keep that backslash root in `master_dir`: both sides are normalised to forward slashes.
    #[test]
    fn filter_where_folder_normalises_backslash_root() {
        let filter = filter(PhotoFilterType::Folder, Some("2025 Beijing"), &["C:\\photos"]);
        let (sql, params) = filter_where(&filter);
        assert!(sql.contains(r"REPLACE(master_dir, '\', '/')"), "sql: {}", sql);
        assert_eq!(params[1], Value::Text("C:/photos/2025 Beijing".to_string()));
        assert_eq!(params[2], Value::Text("C:/photos/2025 Beijing/%".to_string()));

        // Prove the SQL matches real rows: a row stored with the backslash root...
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE photos (id integer PRIMARY KEY, master_dir text, trashed boolean DEFAULT 0);
             INSERT INTO photos (master_dir) VALUES ('C:\\photos/2025 Beijing/day1/x.png');",
        )
        .unwrap();
        let matched: i64 = conn
            .query_row(&format!("SELECT count(*) FROM photos WHERE {}", sql), rusqlite::params_from_iter(params.iter()), |r| r.get(0))
            .unwrap();
        assert_eq!(matched, 1);
    }

    #[test]
    fn filter_where_folder_escapes_like_wildcards() {
        let filter = filter(PhotoFilterType::Folder, Some("100%_good"), &["C:/photos"]);
        let (_, params) = filter_where(&filter);
        assert_eq!(params[2], Value::Text(r"C:/photos/100\%\_good/%".to_string()));
    }

    #[test]
    fn filter_where_folder_without_matching_root_matches_nothing_extra() {
        // Two roots: the folder conditions must be OR-combined (a photo lives under one root, so
        // AND-combining them can never match — the bug that broke the sidebar with multiple roots).
        let filter = filter(PhotoFilterType::Folder, Some("2025"), &["C:/photos", "D:/archive"]);
        let (sql, params) = filter_where(&filter);
        assert_eq!(params.len(), 5);
        assert_eq!(params[1], Value::Text("C:/photos/2025".to_string()));
        assert_eq!(params[3], Value::Text("D:/archive/2025".to_string()));
        assert_eq!(sql.matches(r"REPLACE(master_dir, '\', '/') = ?").count(), 2);

        // Prove the OR semantics against real SQLite: a row under the second root matches even
        // though the first root's condition doesn't.
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE photos (id integer PRIMARY KEY, master_dir text, trashed boolean DEFAULT 0, flag boolean DEFAULT 0);
             INSERT INTO photos (master_dir) VALUES ('D:/archive/2025/day1/x.png');",
        )
        .unwrap();
        let matched: i64 = conn
            .query_row(&format!("SELECT count(*) FROM photos WHERE {}", sql),
                rusqlite::params_from_iter(params.iter()), |r| r.get(0))
            .unwrap();
        assert_eq!(matched, 1, "second root's rows must match (OR, not AND)");
    }

    /// End-to-end: photos in nested directories (like the scanner would import them), then folder
    /// listing and folder-filtered section queries against the real migration schema.
    #[test]
    fn folder_filter_end_to_end() {
        // --- Fixtures: a photo root with two folders, one nested, plus a root-level photo ---
        let root = TempDir::new("folder-root");
        let beijing = root.path.join("2025 Beijing");
        let day1 = beijing.join("day1");
        let chengdu = root.path.join("2026 Chengdu");
        std::fs::create_dir_all(&day1).unwrap();
        std::fs::create_dir_all(&chengdu).unwrap();
        std::fs::create_dir_all(beijing.join("empty/child")).unwrap();
        std::fs::write(chengdu.join("notes.txt"), "not an image").unwrap();
        write_png(&beijing, "b1.png", 4, 6);
        write_png(&day1, "d1.png", 8, 3);
        write_png(&chengdu, "c1.png", 9, 9);
        write_png(&root.path, "root.png", 2, 2);

        let home = TempDir::new("folder-home");
        let db = crate::store::db::open(&home.path.join("db.sqlite3"), &migrations_dir()).unwrap();

        // Import like the scanner does: every photo of every directory, in one batch per directory.
        // The scanner always joins directories with '/', so `master_dir` uses forward slashes even
        // on Windows - mirror that here, or the folder-prefix match below would find nothing.
        let mut batch: Vec<(NewPhoto, Vec<String>)> = Vec::new();
        for dir in [root.str(), path_str(&beijing), path_str(&day1), path_str(&chengdu)] {
            let dir = dir.replace('\\', "/");
            for entry in std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()) {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.ends_with(".png") {
                    continue;
                }
                batch.push((NewPhoto {
                    master_dir: dir.clone(),
                    master_filename: name,
                    master_width: 10,
                    master_height: 10,
                    edited_width: 10,
                    edited_height: 10,
                    date_section: "2025-01-01".to_string(),
                    created_at: 0,
                    updated_at: 0,
                    imported_at: 0,
                    flag: false,
                }, vec![]));
            }
        }
        let (added, _) = insert_photos_batch(&db, &batch).unwrap();
        assert_eq!(added, 4);

        let roots = vec![root.str()];

        // --- Folder listing: includes root photos and every image-bearing level ---
        let folders = fetch_photo_folders(&db, &roots).unwrap();
        let names: Vec<&str> = folders.iter().skip(1).map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["2025 Beijing", "day1", "2026 Chengdu"]);
        assert_eq!(folders[0].count, 4);
        assert_eq!(folders[1].count, 2); // b1.png + d1.png (nested)
        assert_eq!(folders[2].count, 1);
        assert_eq!(folders[3].count, 1);
        let nested_filter = make_filter(PhotoFilterType::Folder, Some(&folders[2].path), &roots);
        let nested_sections = fetch_sections(&db, &nested_filter, None).unwrap();
        assert_eq!(nested_sections[0].count, 1);

        // --- Folder-filtered sections: "2025 Beijing" yields both its photos ---
        let filter = make_filter(PhotoFilterType::Folder, Some("2025 Beijing"), &roots);
        let sections = fetch_sections(&db, &filter, None).unwrap();
        assert_eq!(sections.len(), 1);
        let photo_sets = fetch_section_photos(&db, &[sections[0].id.clone()], &filter).unwrap();
        let total: usize = photo_sets.iter().map(|s| s.photo_ids.len()).sum();
        assert_eq!(total, 2);

        // --- "2026 Chengdu" yields only its own photo (photos were all imported with the same date) ---
        let filter = make_filter(PhotoFilterType::Folder, Some("2026 Chengdu"), &roots);
        let photo_sets = fetch_section_photos(&db, &["2025-01-01".to_string()], &filter).unwrap();
        assert_eq!(photo_sets[0].photo_ids.len(), 1);

        // --- A folder name that exists under no root matches nothing ---
        let filter = make_filter(PhotoFilterType::Folder, Some("No Such Folder"), &roots);
        let sections = fetch_sections(&db, &filter, None).unwrap();
        assert!(sections.is_empty());

        // --- Folder filter stacks with the favorites filter ---
        let mut folder_filter = make_filter(PhotoFilterType::Folder, Some("2026 Chengdu"), &roots);
        folder_filter.filter_type = PhotoFilterType::Favorites;
        let sections = fetch_sections(&db, &folder_filter, None).unwrap();
        assert!(sections.is_empty()); // nothing is flagged
    }

    struct TempDir {
        path: std::path::PathBuf,
    }

    impl TempDir {
        fn new(tag: &str) -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!("picturama-photos-{}-{}-{}", tag, nanos, n));
            std::fs::create_dir_all(&path).unwrap();
            TempDir { path }
        }

        fn str(&self) -> String {
            self.path.to_string_lossy().to_string()
        }
    }

    fn path_str(path: &std::path::Path) -> String {
        path.to_string_lossy().to_string()
    }

    /// A minimal PNG (valid signature + IHDR; enough for the DB row, no pixel decoding involved here).
    fn write_png(dir: &std::path::Path, filename: &str, w: u32, h: u32) {
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        bytes.extend_from_slice(&[0, 0, 0, 0x0D]);
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&w.to_be_bytes());
        bytes.extend_from_slice(&h.to_be_bytes());
        bytes.extend_from_slice(&[8, 2, 0, 0, 0]);
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        std::fs::write(dir.join(filename), &bytes).unwrap();
    }

    fn make_filter(filter_type: PhotoFilterType, folder: Option<&str>, photo_roots: &[String]) -> PhotoFilter {
        PhotoFilter {
            filter_type,
            tag_id: None,
            folder: folder.map(String::from),
            photo_roots: Some(photo_roots.to_vec()),
        }
    }

    fn migrations_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("migrations")
    }
}
