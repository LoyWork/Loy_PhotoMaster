//! Local, unedited near-copy detection. The UI only receives database-owned photos and previews.
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use image::{imageops::FilterType, DynamicImage, ImageReader};
use rusqlite::params;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use crate::app_config_builder::AppConfig;
use crate::store::{db::DbHandle, photo_store};
use crate::types::common_types::{Photo, PhotoId};
use crate::scene_encoder::{self, SceneEncoder};

const ALGORITHM_VERSION: u32 = 2;
const MAX_PIXELS: u64 = 80_000_000;

type ActiveScan = (String, Arc<AtomicBool>);
#[derive(Default)]
pub struct ScanState {
    active: Arc<Mutex<Option<ActiveScan>>>,
}
struct ScanLease(Arc<Mutex<Option<ActiveScan>>>);
impl Drop for ScanLease {
    fn drop(&mut self) {
        if let Ok(mut active) = self.0.lock() {
            *active = None;
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    scan_id: String,
    phase: &'static str,
    processed: usize,
    total: usize,
    skipped: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimilarPhoto {
    photo: Photo,
    file_size: u64,
    preview_path: String,
    fingerprint: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimilarGroup {
    photos: Vec<SimilarPhoto>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    groups: Vec<SimilarGroup>,
    total: usize,
    skipped: usize,
    cancelled: bool,
    mode: &'static str,
}

#[derive(Clone, Serialize, Deserialize)]
struct Feature {
    hash: u64,
    aspect: f64,
    colour: [f64; 3],
    low_detail: bool,
}
struct Candidate {
    item: SimilarPhoto,
    feature: Feature,
    embedding: Option<Vec<f32>>,
}

#[tauri::command]
pub async fn scan_similar_photos(
    app: AppHandle,
    db: State<'_, DbHandle>,
    config: State<'_, AppConfig>,
    state: State<'_, ScanState>,
    scan_id: String,
    sensitivity: String,
    mode: Option<String>,
) -> Result<ScanResult, String> {
    let threshold = match sensitivity.as_str() {
        "strict" => 6,
        "standard" => 10,
        "loose" => 14,
        _ => return Err("Invalid sensitivity".into()),
    };
    let mode = mode.as_deref().unwrap_or("quick");
    let deep = match mode { "quick" | "exact" => false, "deep" => true, _ => return Err("Invalid scan mode".into()) };
    let exact = mode == "exact";
    let scene_threshold = match sensitivity.as_str() { "strict" => 0.92, "standard" => 0.85, _ => 0.78 };
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut active = state.active.lock().map_err(|_| "Scan state unavailable")?;
        if active.is_some() { return Err("A similar-photo scan is already running".into()); }
        *active = Some((scan_id.clone(), cancelled.clone()));
    }
    let lease = ScanLease(state.active.clone());
    let db = db.inner().clone();
    let home = config.picturama_home_dir.clone();
    let assets = scene_encoder::assets_dir(&config);
    tokio::task::spawn_blocking(move || {
        let _lease = lease;
        let progress = |phase, processed, total, skipped| {
            let _ = app.emit("similar-scan-progress", Progress { scan_id: scan_id.clone(), phase, processed, total, skipped });
        };
        if exact {
            scan_exact(&db, &home, &cancelled, progress)
        } else if deep {
            progress("loadingModel", 0, 0, 0);
            let mut encoder = SceneEncoder::open(&assets)?;
            scan_with_encoder(&db, &home, threshold, &cancelled, Some(&mut encoder), scene_threshold, progress)
        } else {
            scan(&db, &home, threshold, &cancelled, progress)
        }
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn cancel_similar_scan(state: State<'_, ScanState>, scan_id: String) -> Result<(), String> {
    let active = state.active.lock().map_err(|_| "Scan state unavailable")?;
    if let Some((id, flag)) = active.as_ref() {
        if *id == scan_id { flag.store(true, Ordering::Relaxed); }
    }
    Ok(())
}

fn fingerprint(path: &Path) -> Result<(String, u64), String> {
    let metadata = path.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > 512 * 1024 * 1024 { return Err("Not a file or exceeds 512 MiB".into()); }
    let modified = metadata.modified().map_err(|e| e.to_string())?.duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?;
    Ok((format!("{}:{}:{}:{}", ALGORITHM_VERSION, path.to_string_lossy(), metadata.len(), modified.as_nanos()), metadata.len()))
}

fn scan_exact(db: &DbHandle, home: &Path, cancelled: &AtomicBool,
    mut progress: impl FnMut(&'static str, usize, usize, usize)) -> Result<ScanResult, String> {
    let photos = {
        let conn = db.lock().map_err(|_| "Database unavailable")?;
        let mut statement = conn.prepare("SELECT * FROM photos WHERE trashed = 0 ORDER BY id").map_err(|e| e.to_string())?;
        let photos = statement.query_map([], photo_store::photo_from_row).map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?;
        photos
    };
    let total = photos.len();
    let paths = photos.iter().map(|p| format!("{}/{}", p.master_dir, p.master_filename)).collect();
    let report = crate::commands::photos::find_duplicate_photos_with_progress(paths, cancelled, &mut progress);
    if report.cancelled { return Ok(ScanResult { groups: vec![], total, skipped: report.skipped, cancelled: true, mode: "exact" }); }
    let mut by_path: HashMap<String, Photo> = photos.into_iter().map(|p| (format!("{}/{}", p.master_dir, p.master_filename), p)).collect();
    let preview_dir = home.join("thumbnails/similar");
    std::fs::create_dir_all(&preview_dir).map_err(|e| e.to_string())?;
    let preview_total: usize = report.groups.iter().map(|g| g.paths.len()).sum();
    let mut processed = 0;
    let mut groups = Vec::new();
    progress("previews", 0, preview_total, report.skipped);
    for group in report.groups {
        let mut items = Vec::new();
        for path in group.paths {
            if cancelled.load(Ordering::Relaxed) { return Ok(ScanResult { groups: vec![], total, skipped: report.skipped, cancelled: true, mode: "exact" }); }
            if let Some(photo) = by_path.remove(&path) {
                let preview = preview_dir.join(format!("{}.jpg", photo.id));
                // Decode only matched files. A failed preview must not hide an exact byte match.
                // Byte detection also covers unsupported formats, uniform images, and large files.
                let preview_result = decode(Path::new(&path)).and_then(|image| {
                    let small = image.thumbnail(960, 720).to_rgb8();
                    let mut bytes = Vec::new();
                    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 88)
                        .encode_image(&DynamicImage::ImageRgb8(small)).map_err(|e| e.to_string())?;
                    std::fs::write(&preview, bytes).map_err(|e| e.to_string())
                });
                let modified = std::fs::metadata(&path).ok().and_then(|m| m.modified().ok())
                    .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map(|m| m.as_nanos()).unwrap_or(0);
                let preview_path = match preview_result {
                    Ok(()) => preview.to_string_lossy().into_owned(),
                    Err(error) => {
                        log::warn!("Duplicate preview unavailable for {}: {}", path, error);
                        // Use a distinct missing path instead of showing an old, stale preview.
                        preview_dir.join(format!("{}-unavailable.jpg", photo.id)).to_string_lossy().into_owned()
                    }
                };
                items.push(SimilarPhoto { photo, file_size: group.file_size, preview_path,
                    fingerprint: format!("exact:{}:{}", group.file_size, modified) });
            }
            processed += 1;
            progress("previews", processed, preview_total, report.skipped);
        }
        if items.len() > 1 { groups.push(SimilarGroup { photos: items }); }
    }
    let cancelled = cancelled.load(Ordering::Relaxed);
    if cancelled { groups.clear(); }
    Ok(ScanResult { groups, total, skipped: report.skipped, cancelled, mode: "exact" })
}

fn scan(db: &DbHandle, home: &Path, threshold: u32, cancelled: &AtomicBool,
    progress: impl FnMut(&'static str, usize, usize, usize)) -> Result<ScanResult, String> {
    scan_with_encoder(db, home, threshold, cancelled, None, 0.85, progress)
}

fn scan_with_encoder(db: &DbHandle, home: &Path, threshold: u32, cancelled: &AtomicBool,
    mut encoder: Option<&mut SceneEncoder>, scene_threshold: f32,
    mut progress: impl FnMut(&'static str, usize, usize, usize)) -> Result<ScanResult, String> {
    let deep = encoder.is_some();
    let mode = if deep { "deep" } else { "quick" };
    let (photos, cached, cached_scenes) = {
        let conn = db.lock().map_err(|_| "Database unavailable")?;
        conn.execute("DELETE FROM photo_similarity_features WHERE photo_id NOT IN (SELECT id FROM photos)", []).map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM photo_scene_features WHERE photo_id NOT IN (SELECT id FROM photos)", []).map_err(|e| e.to_string())?;
        let photos = conn.prepare("SELECT * FROM photos WHERE trashed = 0 ORDER BY id").map_err(|e| e.to_string())?
            .query_map([], photo_store::photo_from_row).map_err(|e| e.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>().map_err(|e| e.to_string())?;
        let cached = conn.prepare("SELECT photo_id, fingerprint, feature FROM photo_similarity_features").map_err(|e| e.to_string())?
            .query_map([], |r| Ok((r.get::<_, PhotoId>(0)?, (r.get::<_, String>(1)?, r.get::<_, String>(2)?))))
            .map_err(|e| e.to_string())?.collect::<rusqlite::Result<HashMap<_, _>>>().map_err(|e| e.to_string())?;
        let cached_scenes = if deep {
            conn.prepare("SELECT photo_id, fingerprint, embedding FROM photo_scene_features WHERE model_version = ?1").map_err(|e| e.to_string())?
                .query_map([scene_encoder::MODEL_VERSION], |r| Ok((r.get::<_, PhotoId>(0)?, (r.get::<_, String>(1)?, r.get::<_, Vec<u8>>(2)?))))
                .map_err(|e| e.to_string())?.collect::<rusqlite::Result<HashMap<_, _>>>().map_err(|e| e.to_string())?
        } else { HashMap::new() };
        (photos, cached, cached_scenes)
    };
    let total = photos.len();
    let preview_dir = home.join("thumbnails/similar");
    std::fs::create_dir_all(&preview_dir).map_err(|e| e.to_string())?;
    let mut candidates = Vec::new();
    let mut skipped = 0;
    progress("features", 0, total, skipped);
    // Bound full-size decodes (up to 384 MiB each) and queued previews.
    let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(1, 2);
    let pool = rayon::ThreadPoolBuilder::new().num_threads(workers).build().map_err(|e| e.to_string())?;
    let mut photos = photos.into_iter();
    let mut processed = 0;
    loop {
        if cancelled.load(Ordering::Relaxed) { return Ok(ScanResult { groups: vec![], total, skipped, cancelled: true, mode }); }
        let batch: Vec<_> = photos.by_ref().take(8).collect();
        if batch.is_empty() { break; }
        // Decode/hash/preview in parallel. Keep one model with bounded intra-op threads.
        let prepared: Vec<_> = pool.install(|| batch.into_par_iter().map(|photo| {
            if cancelled.load(Ordering::Relaxed) { return Ok(None); }
            prepare_photo(photo, &preview_dir, &cached, &cached_scenes, deep).map(Some)
        }).collect());
        let mut writes = Vec::new();
        for result in prepared {
            if cancelled.load(Ordering::Relaxed) { break; }
            match result {
                Ok(Some((mut candidate, image, need_feature, need_vector))) => {
                    if need_vector {
                        candidate.embedding = Some(encoder.as_deref_mut().unwrap()
                            .encode(image.as_ref().unwrap())?);
                    }
                    writes.push((candidate.item.photo.id, candidate.item.fingerprint.clone(),
                        need_feature.then(|| serde_json::to_string(&candidate.feature)).transpose().map_err(|e| e.to_string())?,
                        candidate.embedding.as_ref().filter(|_| need_vector).map(|v| scene_encoder::to_bytes(v))));
                    if candidate.feature.low_detail { skipped += 1; } else { candidates.push(candidate); }
                }
                Ok(None) => {},
                Err(error) => { skipped += 1; log::warn!("Similar scan skipped: {error}"); }
            }
            processed += 1;
            progress("features", processed, total, skipped);
        }
        // Commit once per small batch; never hold the DB lock during IO/inference.
        let mut conn = db.lock().map_err(|_| "Database unavailable")?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        for (id, key, feature, vector) in writes {
            if let Some(feature) = feature {
                tx.execute("INSERT OR REPLACE INTO photo_similarity_features(photo_id, fingerprint, feature) VALUES (?1, ?2, ?3)",
                    params![id, key, feature]).map_err(|e| e.to_string())?;
            }
            if let Some(vector) = vector {
                tx.execute("INSERT OR REPLACE INTO photo_scene_features(photo_id, fingerprint, model_version, embedding) VALUES (?1, ?2, ?3, ?4)",
                    params![id, key, scene_encoder::MODEL_VERSION, vector]).map_err(|e| e.to_string())?;
            }
        }
        tx.commit().map_err(|e| e.to_string())?;
    }
    let count = candidates.len();
    let groups = if deep {
        progress("indexing", 0, count, skipped);
        group_scene_candidates(candidates, threshold, scene_threshold, cancelled,
            |processed| progress("grouping", processed, count, skipped))?
    } else {
        group_candidates(candidates, threshold, cancelled, |processed| progress("grouping", processed, count, skipped))
    };
    Ok(ScanResult { groups, total, skipped, cancelled: cancelled.load(Ordering::Relaxed), mode })
}

type FeatureCache = HashMap<PhotoId, (String, String)>;
type SceneCache = HashMap<PhotoId, (String, Vec<u8>)>;
type PreparedPhoto = (Candidate, Option<DynamicImage>, bool, bool);

fn prepare_photo(photo: Photo, preview_dir: &Path, cached: &FeatureCache,
    cached_scenes: &SceneCache, deep: bool) -> Result<PreparedPhoto, String> {
    let path = PathBuf::from(&photo.master_dir).join(&photo.master_filename);
    let preview = preview_dir.join(format!("{}.jpg", photo.id));
    let (key, file_size) = fingerprint(&path)?;
    let cached_feature = cached.get(&photo.id).filter(|(old_key, _)| *old_key == key)
        .and_then(|(_, json)| serde_json::from_str::<Feature>(json).ok())
        .filter(|f| f.low_detail || preview.exists());
    let cached_vector = cached_scenes.get(&photo.id).filter(|(old_key, _)| *old_key == key)
        .and_then(|(_, bytes)| scene_encoder::from_bytes(bytes));
    let need_feature = cached_feature.is_none();
    let mut image = if need_feature {
        // Area-sample once; don't repeatedly convolve the multi-megapixel original.
        Some(decode(&path)?.thumbnail(1024, 1024))
    } else { None };
    let feature = cached_feature.unwrap_or_else(|| describe(image.as_ref().unwrap()));
    if need_feature && !feature.low_detail {
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 88)
            .encode_image(image.as_ref().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&preview, bytes).map_err(|e| e.to_string())?;
    }
    let need_vector = deep && cached_vector.is_none() && !feature.low_detail;
    if need_vector {
        // Dedicated unedited preview, never a user-edited library thumbnail.
        // Use identical JPEG pixels for cold scans and cached-preview scans.
        image = Some(decode(&preview)?);
    } else { image = None; }
    if fingerprint(&path)?.0 != key { return Err("File changed during scan; retry".into()); }
    Ok((Candidate { item: SimilarPhoto { photo, file_size,
        preview_path: preview.to_string_lossy().into_owned(), fingerprint: key },
        feature, embedding: cached_vector }, image, need_feature, need_vector))
}

fn decode(path: &Path) -> Result<DynamicImage, String> {
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
    if let Ok(size) = imagesize::size(path) {
        if size.width as u64 * size.height as u64 > MAX_PIXELS { return Err("Image exceeds 80 MP limit".into()); }
    }
    if picturama_raw::EXTENSIONS.contains(&ext.as_str()) {
        // Camera-developed preview is enough for recognition, avoiding full sensor processing.
        let decoded = picturama_raw::decode_file(&path.to_string_lossy(), true)?;
        return image::RgbImage::from_raw(decoded.width, decoded.height, decoded.data.clone())
            .map(DynamicImage::ImageRgb8).ok_or_else(|| "Invalid RAW pixels".into());
    }
    if ["heic", "heif", "hif"].contains(&ext.as_str()) {
        let decoded = crate::image::heif::decode_file(&path.to_string_lossy())?;
        return image::RgbImage::from_raw(decoded.width, decoded.height, decoded.data)
            .map(DynamicImage::ImageRgb8).ok_or_else(|| "Invalid HEIF pixels".into());
    }
    let mut reader = ImageReader::open(path).map_err(|e| e.to_string())?.with_guessed_format().map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(384 * 1024 * 1024);
    reader.limits(limits);
    let mut img = reader.decode().map_err(|e| e.to_string())?;
    if img.width() as u64 * img.height() as u64 > MAX_PIXELS { return Err("Image exceeds 80 MP limit".into()); }
    // Include EXIF mirror orientations, not just rotations. HEIF/RAW already orient their output.
    if let Ok(file) = std::fs::File::open(path) {
        if let Ok(exif) = exif::Reader::new().read_from_container(&mut std::io::BufReader::new(file)) {
            if let Some(value) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY).and_then(|f| f.value.get_uint(0)) {
                img.apply_orientation(image::metadata::Orientation::from_exif(value as u8).unwrap_or(image::metadata::Orientation::NoTransforms));
            }
        }
    }
    Ok(img)
}

fn describe(img: &DynamicImage) -> Feature {
    let gray = img.resize_exact(32, 32, FilterType::Triangle).to_luma8();
    let mean = gray.as_raw().iter().map(|v| *v as f64).sum::<f64>() / 1024.;
    let variance = gray.as_raw().iter().map(|v| (*v as f64 - mean).powi(2)).sum::<f64>() / 1024.;
    let mut basis = [[0.; 32]; 8];
    for (u, row) in basis.iter_mut().enumerate() {
        for (x, value) in row.iter_mut().enumerate() { *value = ((2 * x + 1) as f64 * u as f64 * std::f64::consts::PI / 64.).cos(); }
    }
    let mut coeffs = Vec::with_capacity(63);
    for u in 0..8 {
        for v in 0..8 {
            if u == 0 && v == 0 { continue; }
            let mut value = 0.;
            for y in 0..32 {
                for x in 0..32 {
                    value += (gray.get_pixel(x as u32, y as u32)[0] as f64 - mean)
                        * basis[u][x] * basis[v][y];
                }
            }
            coeffs.push(value);
        }
    }
    let mut ordered = coeffs.clone();
    ordered.sort_by(f64::total_cmp);
    let median = ordered[31];
    let hash = coeffs.iter().enumerate().fold(0u64, |hash, (i, c)| hash | (u64::from(*c > median) << i));
    let rgb = img.resize_exact(16, 16, FilterType::Triangle).to_rgb8();
    let mut colour = [0.; 3];
    for p in rgb.pixels() { for c in 0..3 { colour[c] += p[c] as f64; } }
    let sum = colour.iter().sum::<f64>().max(1.);
    for c in &mut colour { *c /= sum; }
    Feature { hash, aspect: img.width() as f64 / img.height().max(1) as f64, colour, low_detail: variance < 16. }
}

fn similar(a: &Feature, b: &Feature, threshold: u32) -> bool {
    (a.hash ^ b.hash).count_ones() <= threshold &&
        (a.aspect / b.aspect).ln().abs() <= 0.12 &&
        a.colour.iter().zip(b.colour).map(|(x,y)| (x-y).abs()).sum::<f64>() <= 0.28
}

// BK tree over group representatives. Duplicate hashes share a node instead of a deep zero chain.
#[derive(Default)]
struct HashIndex {
    nodes: Vec<HashNode>,
}
struct HashNode {
    hash: u64,
    groups: Vec<usize>,
    children: BTreeMap<u32, usize>,
}
impl HashIndex {
    fn insert(&mut self, hash: u64, group: usize) {
        let mut node = 0;
        if self.nodes.is_empty() { self.nodes.push(HashNode { hash, groups: vec![group], children: BTreeMap::new() }); return; }
        loop {
            let distance = (self.nodes[node].hash ^ hash).count_ones();
            if distance == 0 { self.nodes[node].groups.push(group); return; }
            if let Some(next) = self.nodes[node].children.get(&distance) { node = *next; }
            else {
                let next = self.nodes.len();
                self.nodes.push(HashNode { hash, groups: vec![group], children: BTreeMap::new() });
                self.nodes[node].children.insert(distance, next);
                return;
            }
        }
    }
    fn query(&self, hash: u64, radius: u32) -> Vec<usize> {
        let mut found = Vec::new();
        if self.nodes.is_empty() { return found; }
        let mut pending = vec![0];
        while let Some(i) = pending.pop() {
            let node = &self.nodes[i];
            let distance = (node.hash ^ hash).count_ones();
            if distance <= radius { found.extend(&node.groups); }
            pending.extend(node.children.range(distance.saturating_sub(radius)..=distance + radius).map(|(_, index)| *index));
        }
        found.sort_unstable();
        found
    }
}

fn group_candidates(candidates: Vec<Candidate>, threshold: u32, cancelled: &AtomicBool,
    mut progress: impl FnMut(usize)) -> Vec<SimilarGroup> {
    let mut groups: Vec<Vec<Candidate>> = Vec::new();
    let mut index = HashIndex::default();
    let total = candidates.len();
    progress(0);
    for (i, candidate) in candidates.into_iter().enumerate() {
        if cancelled.load(Ordering::Relaxed) { return vec![]; }
        let target = index.query(candidate.feature.hash, threshold).into_iter().find(|g|
            groups[*g].iter().all(|other| similar(&candidate.feature, &other.feature, threshold)));
        match target {
            Some(g) => groups[g].push(candidate),
            None => { index.insert(candidate.feature.hash, groups.len()); groups.push(vec![candidate]); }
        }
        if i % 128 == 0 || i + 1 == total { progress(i + 1); }
    }
    groups.into_iter().filter(|g| g.len() > 1).map(|group| SimilarGroup { photos: group.into_iter().map(|c| c.item).collect() }).collect()
}

#[derive(Clone)]
struct ScenePoint(Arc<[f32]>);
impl instant_distance::Point for ScenePoint {
    fn distance(&self, other: &Self) -> f32 { 1. - scene_encoder::cosine(&self.0, &other.0) }
}

fn scene_match(a: &Candidate, b: &Candidate, hash_threshold: u32, scene_threshold: f32) -> bool {
    similar(&a.feature, &b.feature, hash_threshold) ||
        a.embedding.as_ref().zip(b.embedding.as_ref())
            .map_or(false, |(a,b)| scene_encoder::cosine(a,b) >= scene_threshold)
}

fn group_scene_candidates(candidates: Vec<Candidate>, hash_threshold: u32, scene_threshold: f32,
    cancelled: &AtomicBool, mut progress: impl FnMut(usize)) -> Result<Vec<SimilarGroup>, String> {
    if candidates.len() < 2 || cancelled.load(Ordering::Relaxed) { return Ok(vec![]); }
    let points: Vec<ScenePoint> = candidates.iter().map(|c| c.embedding.as_ref()
        .map(|v| ScenePoint(Arc::from(v.clone())))
        .ok_or_else(|| "Missing scene vector".to_string())).collect::<Result<_,_>>()?;
    // Exact search for small libraries, HNSW retrieval for larger ones. Hash retrieval runs in
    // parallel as an independent candidate route; hash mismatches never filter out DINO results.
    let index = if points.len() > 256 {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(1,4);
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().map_err(|e| e.to_string())?;
        Some(pool.install(|| instant_distance::Builder::default().seed(42).ef_search(128).ef_construction(128)
            .build(points.clone(), (0..points.len()).collect::<Vec<_>>())))
    } else { None };
    let mut search = instant_distance::Search::default();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut assigned = vec![None; candidates.len()];
    let mut hashes = HashIndex::default();
    // Approximate search can omit an identical vector when many distances tie. Keep a
    // direct route for these copies, while retaining HNSW for other scene neighbors.
    let mut identical_scenes = HashMap::new();
    progress(0);
    for (i, candidate) in candidates.iter().enumerate() {
        if cancelled.load(Ordering::Relaxed) { return Ok(vec![]); }
        let mut possible = hashes.query(candidate.feature.hash, hash_threshold);
        let scene_key = scene_encoder::to_bytes(&points[i].0);
        if let Some(group) = identical_scenes.get(&scene_key) { possible.push(*group); }
        if let Some(index) = index.as_ref() {
            for item in index.search(&points[i], &mut search).take(128) {
                let neighbor = *item.value;
                if neighbor < i {
                    if let Some(group) = assigned[neighbor] { possible.push(group); }
                }
            }
        } else { possible.extend(0..groups.len()); }
        possible.sort_unstable();
        possible.dedup();
        // Prefer the closest representative, then check *every* group member to prevent chains.
        possible.sort_by(|a,b| {
            let a_score = scene_encoder::cosine(&points[i].0, &points[groups[*a][0]].0);
            let b_score = scene_encoder::cosine(&points[i].0, &points[groups[*b][0]].0);
            b_score.total_cmp(&a_score).then(a.cmp(b))
        });
        let target = possible.into_iter().find(|g| groups[*g].iter()
            .all(|other| scene_match(candidate, &candidates[*other], hash_threshold, scene_threshold)));
        let group = match target {
            Some(g) => { groups[g].push(i); g },
            None => {
                let g = groups.len();
                hashes.insert(candidate.feature.hash, g);
                groups.push(vec![i]);
                g
            }
        };
        assigned[i] = Some(group);
        identical_scenes.insert(scene_key, group);
        if i % 64 == 0 || i + 1 == candidates.len() { progress(i+1); }
    }
    let mut items: Vec<Option<SimilarPhoto>> = candidates.into_iter().map(|c| Some(c.item)).collect();
    Ok(groups.into_iter().filter(|g| g.len() > 1)
        .map(|g| SimilarGroup { photos: g.into_iter().map(|i| items[i].take().unwrap()).collect() }).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "manual disk/CPU benchmark; generates 24 twelve-megapixel JPEGs"]
    fn benchmark_cold_and_warm_library_scan() {
        let temp = std::env::temp_dir().join(format!("picturama-scan-bench-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let fixture = image::open(root.join("../submodules/test-data/photos/landscape.jpg")).unwrap()
            .resize_exact(4000, 3000, FilterType::Triangle).to_rgb8();
        DynamicImage::ImageRgb8(fixture).save(temp.join("source.jpg")).unwrap();
        let db = crate::store::db::open(&temp.join("test.db"), &root.join("../migrations")).unwrap();
        for id in 1..=24 {
            let filename = format!("{id}.jpg");
            std::fs::copy(temp.join("source.jpg"), temp.join(&filename)).unwrap();
            db.lock().unwrap().execute("INSERT INTO photos(id, master_dir, master_filename, master_width, master_height, edited_width, edited_height, date_section, created_at, updated_at, imported_at) VALUES (?1, ?2, ?3, 4000, 3000, 4000, 3000, '2026-01-01', 0, 0, 0)",
                params![id, temp.to_string_lossy(), filename]).unwrap();
        }
        let start = std::time::Instant::now();
        let cold = scan(&db, &temp, 10, &AtomicBool::new(false), |_,_,_,_|{}).unwrap();
        let cold_time = start.elapsed();
        let start = std::time::Instant::now();
        let warm = scan(&db, &temp, 10, &AtomicBool::new(false), |_,_,_,_|{}).unwrap();
        let warm_time = start.elapsed();
        println!("BENCH 24 x 4000x3000 JPEG: cold={:.3}s warm={:.3}s", cold_time.as_secs_f64(), warm_time.as_secs_f64());
        assert_eq!((cold.total, cold.skipped, cold.groups.len()), (24, 0, 1));
        assert_eq!(warm.groups[0].photos.len(), 24);
        if std::env::var_os("PICTURAMA_BENCH_DEEP").is_some() {
            let mut encoder = SceneEncoder::open(&root.join("models")).unwrap();
            let start = std::time::Instant::now();
            let deep = scan_with_encoder(&db, &temp, 10, &AtomicBool::new(false), Some(&mut encoder), 0.85, |_,_,_,_|{}).unwrap();
            println!("BENCH cold deep: {:.3}s", start.elapsed().as_secs_f64());
            assert_eq!(deep.groups[0].photos.len(), 24);
            let start = std::time::Instant::now();
            scan_with_encoder(&db, &temp, 10, &AtomicBool::new(false), Some(&mut encoder), 0.85, |_,_,_,_|{}).unwrap();
            println!("BENCH warm deep: {:.3}s", start.elapsed().as_secs_f64());
        }
        let preview = image::open(temp.join("thumbnails/similar/1.jpg")).unwrap();
        assert!(preview.width() <= 1024 && preview.height() <= 1024);
        drop(db);
        assert!(temp.starts_with(std::env::temp_dir()) && temp.file_name().unwrap().to_string_lossy().starts_with("picturama-scan-bench-"));
        std::fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn exact_scan_returns_photo_groups_and_keeps_matches_without_previews() {
        let temp = std::env::temp_dir().join(format!("picturama-exact-test-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let db = crate::store::db::open(&temp.join("test.db"), &root.join("../migrations")).unwrap();
        // Uniform images are intentionally excluded from perceptual scans, but must still be
        // found by exact matching. Different file bytes must remain separate even if pixels match.
        DynamicImage::new_rgb8(24,24).save(temp.join("first.png")).unwrap();
        let original = std::fs::read(temp.join("first.png")).unwrap();
        std::fs::write(temp.join("copy.png"), &original).unwrap();
        let mut modified = original.clone();
        modified.extend_from_slice(b"different trailing metadata");
        std::fs::write(temp.join("different.png"), modified).unwrap();
        std::fs::write(temp.join("trashed.png"), &original).unwrap();
        std::fs::write(temp.join("broken.jpg"), b"not an image").unwrap();
        std::fs::write(temp.join("broken-copy.jpg"), b"not an image").unwrap();
        for (i, name) in ["first.png","copy.png","different.png","trashed.png","broken.jpg","broken-copy.jpg","missing.jpg"].iter().enumerate() {
            db.lock().unwrap().execute("INSERT INTO photos(id,master_dir,master_filename,master_width,master_height,edited_width,edited_height,date_section,created_at,updated_at,imported_at,trashed) VALUES (?1,?2,?3,24,24,24,24,'2026-01-01',0,0,0,?4)",
                params![i as i64+1,temp.to_string_lossy(),name,i64::from(i==3)]).unwrap();
        }
        let report = scan_exact(&db,&temp,&AtomicBool::new(false),|_,_,_,_|{}).unwrap();
        assert_eq!((report.mode,report.total,report.skipped,report.groups.len()),("exact",6,1,2));
        let mut ids: Vec<_> = report.groups.iter().flat_map(|g|g.photos.iter().map(|p|p.photo.id)).collect();
        ids.sort_unstable();
        assert_eq!(ids,vec![1,2,5,6]);
        assert_eq!(report.groups.iter().flat_map(|g|&g.photos).filter(|p|p.preview_path.ends_with("-unavailable.jpg")).count(),2);
        assert!(temp.join("thumbnails/similar/1.jpg").exists());
        let cancelled = scan_exact(&db,&temp,&AtomicBool::new(true),|_,_,_,_|{}).unwrap();
        assert!(cancelled.cancelled && cancelled.groups.is_empty());
        photo_store::update_photos(&db,&[2,6],&serde_json::json!({"trashed":true})).unwrap();
        assert!(scan_exact(&db,&temp,&AtomicBool::new(false),|_,_,_,_|{}).unwrap().groups.is_empty());
        assert_eq!(std::fs::read(temp.join("copy.png")).unwrap(),original);
        assert!(temp.join("broken-copy.jpg").exists());
        drop(db);
        assert!(temp.starts_with(std::env::temp_dir()) && temp.file_name().unwrap().to_string_lossy().starts_with("picturama-exact-test-"));
        std::fs::remove_dir_all(temp).unwrap();
    }

    fn candidate(id: i64, hash: u64) -> Candidate {
        Candidate {
            item: SimilarPhoto { photo: Photo {
                id, master_dir: "test".into(), master_filename: format!("{id}.jpg"),
                master_width: 128, master_height: 96, edited_width: None, edited_height: None,
                date_section: "2026-01-01".into(), created_at: 0, updated_at: 0, imported_at: 0, flag: false, trashed: false,
            }, file_size: 10, preview_path: "test".into(), fingerprint: "test".into() },
            feature: Feature { hash, aspect: 1.5, colour: [0.3,0.4,0.3], low_detail: false },
            embedding: None,
        }
    }
    #[test]
    fn grouping_does_not_chain_together_dissimilar_endpoints() {
        // A~B and B~C, but A is not similar to C.
        let groups = group_candidates(vec![candidate(1,0), candidate(2,0b11), candidate(3,0b1111)], 2, &AtomicBool::new(false), |_| {});
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].photos.iter().map(|p| p.photo.id).collect::<Vec<_>>(), vec![1,2]);
        assert!(group_candidates(vec![candidate(1,0), candidate(2,0)], 2, &AtomicBool::new(true), |_| {}).is_empty());
    }

    #[test]
    fn scene_grouping_crosses_hash_and_aspect_boundaries_without_chaining() {
        let make = |id: i64, degrees: f32| {
            let mut item = candidate(id, (id as u64).wrapping_mul(0x9e3779b97f4a7c15));
            let mut vector = vec![0.; scene_encoder::DIMENSIONS];
            vector[0] = degrees.to_radians().cos();
            vector[1] = degrees.to_radians().sin();
            item.embedding = Some(vector);
            item.feature.aspect = id as f64;
            item
        };
        let groups = group_scene_candidates(vec![make(1,0.), make(2,15.), make(3,30.)], 0, 0.95,
            &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(groups.len(),1);
        assert_eq!(groups[0].photos.iter().map(|p|p.photo.id).collect::<Vec<_>>(),vec![1,2]);
    }

    #[test]
    fn indexed_scene_search_finds_a_match_outside_hash_candidates() {
        let items = (0..301).map(|i| {
            let mut item = candidate(i+1, (i as u64 + 1).wrapping_mul(0x9e3779b97f4a7c15));
            let mut vector = vec![0.; scene_encoder::DIMENSIONS];
            vector[if i == 300 { 0 } else { i as usize }] = 1.;
            item.embedding = Some(vector);
            item
        }).collect();
        let groups = group_scene_candidates(items,0,0.99,&AtomicBool::new(false),|_|{}).unwrap();
        assert_eq!(groups.len(),1);
        assert_eq!(groups[0].photos.iter().map(|p|p.photo.id).collect::<Vec<_>>(),vec![1,301]);
    }

    #[test]
    #[ignore = "requires prepared model assets; validates real inference plus migrated SQLite caches"]
    fn real_scene_scan_finds_hash_misses_and_reuses_vectors() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut encoder = SceneEncoder::open(&root.join("models")).unwrap();
        let original = image::open(root.join("../submodules/test-data/photos/landscape.jpg")).unwrap();
        let variant = original.crop_imm(original.width()/8,original.height()/12,original.width()*3/4,original.height()*5/6).brighten(18);
        let unrelated = image::open(root.join("../submodules/test-data/photos/portrait.jpg")).unwrap();
        assert!(!similar(&describe(&original),&describe(&variant),10), "fixture must exercise a hash miss");
        let temp = std::env::temp_dir().join(format!("picturama-scene-test-{}-{}",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp).unwrap();
        let db = crate::store::db::open(&temp.join("test.db"),&root.join("../migrations")).unwrap();
        for (i,img) in [original,variant,unrelated].iter().enumerate() {
            let name = format!("{}.png",i+1);
            img.save(temp.join(&name)).unwrap();
            db.lock().unwrap().execute("INSERT INTO photos(id,master_dir,master_filename,master_width,master_height,edited_width,edited_height,date_section,created_at,updated_at,imported_at) VALUES (?1,?2,?3,?4,?5,?4,?5,'2026-01-01',0,0,0)",
                params![i as i64+1,temp.to_string_lossy(),name,img.width(),img.height()]).unwrap();
        }
        let start = std::time::Instant::now();
        let result = scan_with_encoder(&db,&temp,10,&AtomicBool::new(false),Some(&mut encoder),0.85,|_,_,_,_|{}).unwrap();
        let cold = start.elapsed();
        assert_eq!(result.mode,"deep");
        assert_eq!(result.skipped,0);
        assert_eq!(result.groups.len(),1);
        assert_eq!(result.groups[0].photos.iter().map(|p|p.photo.id).collect::<Vec<_>>(),vec![1,2]);
        let cached: Vec<u8> = db.lock().unwrap().query_row("SELECT embedding FROM photo_scene_features WHERE photo_id=1",[],|r|r.get(0)).unwrap();
        let start = std::time::Instant::now();
        let warm = scan_with_encoder(&db,&temp,10,&AtomicBool::new(false),Some(&mut encoder),0.85,|_,_,_,_|{}).unwrap();
        println!("Deep scan, 3 photos: cold={cold:?}, warm={:?}",start.elapsed());
        assert_eq!(warm.groups.len(),1);
        assert_eq!(db.lock().unwrap().query_row("SELECT embedding FROM photo_scene_features WHERE photo_id=1",[],|r|r.get::<_,Vec<u8>>(0)).unwrap(),cached);
        // Cache corruption and model-version changes must trigger a real recomputation.
        db.lock().unwrap().execute("UPDATE photo_scene_features SET embedding=x'00', model_version='old' WHERE photo_id=1",[]).unwrap();
        let repaired = scan_with_encoder(&db,&temp,10,&AtomicBool::new(false),Some(&mut encoder),0.85,|_,_,_,_|{}).unwrap();
        assert_eq!(repaired.groups.len(),1);
        assert_eq!(db.lock().unwrap().query_row("SELECT model_version FROM photo_scene_features WHERE photo_id=1",[],|r|r.get::<_,String>(0)).unwrap(),scene_encoder::MODEL_VERSION);
        DynamicImage::new_rgb8(50,50).save(temp.join("2.png")).unwrap();
        let changed = scan_with_encoder(&db,&temp,10,&AtomicBool::new(false),Some(&mut encoder),0.85,|_,_,_,_|{}).unwrap();
        assert!(changed.groups.is_empty());
        assert_eq!(changed.skipped,1);
        let cancelled = scan_with_encoder(&db,&temp,10,&AtomicBool::new(true),Some(&mut encoder),0.85,|_,_,_,_|{}).unwrap();
        assert!(cancelled.cancelled && cancelled.groups.is_empty());
        drop(db);
        assert!(temp.starts_with(std::env::temp_dir()) && temp.file_name().unwrap().to_string_lossy().starts_with("picturama-scene-test-"));
        std::fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn scan_caches_features_excludes_trash_and_invalidates_changed_files() {
        let temp = std::env::temp_dir().join(format!("picturama-similar-test-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp).unwrap();
        let db = crate::store::db::open(&temp.join("test.db"), &Path::new(env!("CARGO_MANIFEST_DIR")).join("../migrations")).unwrap();
        let original = DynamicImage::ImageRgb8(image::RgbImage::from_fn(128, 96, |x,y|
            image::Rgb([((x*13 + y*7 + (x/15)*50) % 160 + 20) as u8; 3])));
        for id in 1..=3 {
            let filename = format!("{id}.png");
            original.save(temp.join(&filename)).unwrap();
            db.lock().unwrap().execute("INSERT INTO photos(id, master_dir, master_filename, master_width, master_height, edited_width, edited_height, date_section, created_at, updated_at, imported_at, trashed) VALUES (?1, ?2, ?3, 128, 96, 128, 96, '2026-01-01', 0, 0, 0, ?4)", params![id, temp.to_string_lossy(), filename, i64::from(id == 3)]).unwrap();
        }
        // One missing file is reported rather than aborting the scan.
        db.lock().unwrap().execute("INSERT INTO photos(master_dir, master_filename, master_width, master_height, edited_width, edited_height, date_section, created_at, updated_at, imported_at) VALUES (?1,'missing.jpg',128,96,128,96,'2026-01-01',0,0,0)", [temp.to_string_lossy()]).unwrap();
        let result = scan(&db, &temp, 10, &AtomicBool::new(false), |_,_,_,_| {}).unwrap();
        assert_eq!((result.total, result.skipped, result.groups.len()), (3,1,1));
        assert_eq!(result.groups[0].photos.len(), 2);
        let preview = temp.join("thumbnails/similar/1.jpg");
        let written = preview.metadata().unwrap().modified().unwrap();
        let again = scan(&db, &temp, 10, &AtomicBool::new(false), |_,_,_,_| {}).unwrap();
        assert_eq!(again.groups.len(), 1);
        assert_eq!(preview.metadata().unwrap().modified().unwrap(), written, "warm scan should reuse the preview");
        // Changing file size guarantees invalidation even on coarse timestamp filesystems.
        DynamicImage::new_rgb8(80,80).save(temp.join("2.png")).unwrap();
        let changed = scan(&db, &temp, 10, &AtomicBool::new(false), |_,_,_,_| {}).unwrap();
        assert!(changed.groups.is_empty());
        assert_eq!(changed.skipped, 2);
        let cancelled = scan(&db, &temp, 10, &AtomicBool::new(true), |_,_,_,_| {}).unwrap();
        assert!(cancelled.cancelled && cancelled.groups.is_empty());
        // Marking a photo as trashed leaves the source file and allows restoration.
        photo_store::update_photos(&db, &[1], &serde_json::json!({"trashed": true})).unwrap();
        assert!(temp.join("1.png").exists());
        let after_trash = scan(&db, &temp, 10, &AtomicBool::new(false), |_,_,_,_| {}).unwrap();
        assert_eq!(after_trash.total, 2);
        photo_store::update_photos(&db, &[1], &serde_json::json!({"trashed": false})).unwrap();
        drop(db);
        assert!(temp.starts_with(std::env::temp_dir()) && temp.file_name().unwrap().to_string_lossy().starts_with("picturama-similar-test-"));
        std::fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn hash_survives_resize_and_brightness_but_distinguishes_other_images() {
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_fn(128, 96, |x,y| {
            let n = ((x * 13 + y * 7 + (x / 15) * 50) % 160 + 20) as u8;
            image::Rgb([n, n.saturating_add(15), n.saturating_sub(10)])
        }));
        let a = describe(&img);
        let b = describe(&img.resize_exact(256, 192, FilterType::Triangle).brighten(15));
        assert!(!a.low_detail);
        assert!(similar(&a, &b, 10), "distance {}", (a.hash ^ b.hash).count_ones());
        let other = DynamicImage::ImageRgb8(image::RgbImage::from_fn(128, 96, |x,y| image::Rgb([((x * x + y * 47) % 256) as u8; 3])));
        assert!(!similar(&a, &describe(&other), 10));
        assert!(describe(&DynamicImage::new_rgb8(32,32)).low_detail);
    }
    #[test]
    fn index_matches_exhaustive_hamming_search() {
        let mut index = HashIndex::default();
        let hashes: Vec<u64> = (0..300u64).map(|i| i.wrapping_mul(0x9e3779b97f4a7c15)).collect();
        for (i, hash) in hashes.iter().enumerate() { index.insert(*hash, i); }
        index.insert(hashes[2], 300);
        for hash in &hashes {
            let mut expected: Vec<usize> = hashes.iter().enumerate().filter(|(_,h)| (**h ^ hash).count_ones() <= 14).map(|(i,_)| i).collect();
            if (hashes[2] ^ hash).count_ones() <= 14 { expected.push(300); }
            assert_eq!(index.query(*hash, 14), expected);
        }
    }
}
