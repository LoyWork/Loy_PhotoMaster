// Mirrors src/common/CommonTypes.ts.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;


// ----- Database types -----


pub type PhotoId = i64;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Photo {
    pub id: PhotoId,
    /// The directory of the original image. Example: '/User/me/Pictures'
    pub master_dir: String,
    /// The filename (without directory) of the original image. Example: 'IMG_9700.JPG'
    pub master_filename: String,
    /// The width of the original image - only with EXIF rotation applied (in px).
    pub master_width: u32,
    /// The height of the original image - only with EXIF rotation applied (in px).
    pub master_height: u32,
    // NOTE: the DB still has a `master_is_raw` column (kept with `DEFAULT '0'` so no migration is needed),
    // but it is no longer used: RAW is displayed on demand from its embedded JPEG preview
    // (see `raw_reader` / `WebGLCanvas`), so the master path is used for every format.
    /// The width of the original image - after EXIF rotation and all PhotoWork have been applied (in px).
    pub edited_width: Option<u32>,
    /// The height of the original image - after EXIF rotation and all PhotoWork have been applied (in px).
    pub edited_height: Option<u32>,
    /// Example: '2016-09-18'
    pub date_section: String,
    /// The timestamp when the photo was created
    pub created_at: i64,
    /// The timestamp when the photo was modified
    pub updated_at: i64,
    /// The timestamp when the photo was imported
    pub imported_at: i64,
    /// Whether the image is flagged (= marked as favorite).
    pub flag: bool,
    /// Whether the image is in the trash (Picturama trash - not the file system's trash).
    pub trashed: bool,
}


pub type TagId = i64;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: TagId,
    pub title: String,
    pub slug: String,
    pub created_at: i64,
    pub updated_at: Option<i64>,
}


// ----- Other types (not database) -----


#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcErrorInfo {
    pub message: String,
    pub error_code: Option<String>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub photo_dirs: Vec<String>,
    pub export_options: Option<PhotoExportOptions>,
    /// UI appearance. Optional so older settings files (and the legacy readers below) keep parsing.
    #[serde(default)]
    pub appearance: Option<AppearanceSettings>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppearanceSettings {
    /// Absolute path of the custom wallpaper image shown behind the UI (`None` = built-in aurora).
    pub wallpaper_path: Option<String>,
    /// Backdrop blur radius in px for the frosted panels (0–60, `None` = the built-in default).
    pub blur_strength: Option<f64>,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        AppearanceSettings { wallpaper_path: None, blur_strength: None }
    }
}


#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiConfig {
    pub version: String,
    pub platform: String,
    pub window_style: WindowStyle,
    pub has_native_menu: bool,
    pub raw_locale: String,
    pub thumbnail_path: String,
    /// The extensions the renderer has to treat specially, straight from `import_scanner`, so the
    /// web view doesn't keep a second copy that could drift apart from the scanner's.
    pub accepted_heic_extensions: Vec<String>,
    pub accepted_raw_extensions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// The style of the main window:
///   - 'nativeTrafficLight': Window uses native MacOS traffic light buttons (top left corner)
///   - 'windowsButtons': Window shows HTML buttons in Windows 10 look (top right corner)
pub enum WindowStyle {
    NativeTrafficLight,
    WindowsButtons,
}


#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportPhase {
    ScanDirs,
    Cleanup,
    ImportPhotos,
    Error,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportProgress {
    pub phase: ImportPhase,
    pub is_paused: bool,
    pub total: u32,
    pub processed: u32,
    pub added: u32,
    pub removed: u32,
    pub current_path: Option<String>,
}


#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoDetail {
    //pub versions: Vec<Version>,
    /// The tags attached to this photo. This may also contain new tags which don't exist in DB yet.
    pub tags: Vec<String>,
}

/// One colour grading wheel: where a hue and how much of it is blended into a tonal range.
/// Mirrors `ColorGradingWheel` in `src/common/CommonTypes.ts`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ColorGradingWheel {
    /// Hue in degrees 0..360 (0 = red)
    pub hue: Option<f64>,
    /// How much of the hue is blended in, 0..100
    pub sat: Option<f64>,
    /// Brightness shift of the tonal range, -100..100
    pub lum: Option<f64>,
}

/// 3-way colour grading (shadows / midtones / highlights wheels plus blending and balance).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ColorGrading {
    pub shadows: Option<ColorGradingWheel>,
    pub midtones: Option<ColorGradingWheel>,
    pub highlights: Option<ColorGradingWheel>,
    pub global: Option<ColorGradingWheel>,
    /// How far the tonal ranges reach into each other, 0..100 (default 50)
    pub blending: Option<f64>,
    /// Shifts the split between shadows and highlights, -100..100
    pub balance: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PhotoWork {
    /// 1 = 90°, 2 = 180°, 3 = 270°
    pub rotation_turns: Option<u8>,
    pub tilt: Option<f64>,
    pub crop_rect: Option<crate::types::geometry_types::Rect>,
    pub color_grading: Option<ColorGrading>,
    /// Preserve all develop controls across IPC and sidecar reloads.
    pub develop: Option<serde_json::Value>,
    pub flagged: Option<bool>,
    pub tags: Option<Vec<String>>,
}

pub type PhotoSectionId = String;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoSection {
    pub id: PhotoSectionId,
    pub title: String,
    pub count: u32,
    /// Only set for LoadedPhotoSection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub photo_ids: Option<Vec<PhotoId>>,
    /// Only set for LoadedPhotoSection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub photo_data: Option<HashMap<PhotoId, Photo>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoSet {
    pub photo_ids: Vec<PhotoId>,
    pub photo_data: HashMap<PhotoId, Photo>,
}

/// An image-bearing directory or ancestor, identified by its full path.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoFolder {
    pub name: String,
    pub path: String,
    pub parent_path: Option<String>,
    pub depth: usize,
    pub count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// The style of the main window:
///   - 'nativeTrafficLight': Window uses native MacOS traffic light buttons (top left corner)
///   - 'windowsButtons': Window shows HTML buttons in Windows 10 look (top right corner)
pub enum PhotoFilterType {
    All,
    Favorites,
    Trash,
    Tag,
    Folder,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoFilter {
    pub filter_type: PhotoFilterType,
    pub tag_id: Option<TagId>,
    /// The top-level folder name (first path segment inside a photo root directory). Only used by
    /// `PhotoFilterType::Folder`. Photos in nested directories count when this is their top-level folder.
    pub folder: Option<String>,
    /// The configured photo root directories. Not sent by the web view — the command layer fills this
    /// in from the settings before the filter reaches the store, so the folder filter can resolve the
    /// folder name against the roots. `serde(default)` keeps old callers (and tests) working.
    #[serde(default)]
    #[serde(skip_serializing)]
    pub photo_roots: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoRenderOptions {
    pub format: PhotoRenderFormat,
    /// Quality 0.0–1.0 (ignored for PNG)
    pub quality: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PhotoRenderFormat {
    Jpg,
    Webp,
    Png,
}

impl PhotoRenderFormat {
    /// File extension used for exported files (matches the TS `PhotoRenderFormat` string values).
    pub fn extension(self) -> &'static str {
        match self {
            PhotoRenderFormat::Jpg => "jpg",
            PhotoRenderFormat::Webp => "webp",
            PhotoRenderFormat::Png => "png",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoExportOptions {
    // Inherited from PhotoRenderOptions on the TS side (`extends`); needed here for the render call and
    // the output file extension.
    pub format: PhotoRenderFormat,
    /// Quality 0.0–1.0 (ignored for PNG)
    pub quality: f64,
    /// 'S' | 'M' | 'L' | 'original' | 'custom'
    pub size: String,
    /// 'width' | 'height' | 'size'
    pub custom_size_side: String,
    pub custom_size_pixels: u32,
    pub with_metadata: bool,
    /// 'like-original' | 'sequence'
    pub file_name_style: String,
    pub file_name_prefix: String,
    pub folder_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyTrashResult {
    pub photo_ids: Vec<PhotoId>,
    pub updated_tags: Option<Vec<Tag>>,
    pub failed: Vec<String>,
}

/// Files whose complete byte content is identical. Paths, rather than photo ids, are returned so the
/// duplicate report remains useful when the copies have unrelated names or live in different folders.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicatePhotoGroup {
    pub file_size: u64,
    pub paths: Vec<String>,
}


#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaData {
    pub img_width: Option<u32>,
    pub img_height: Option<u32>,
    /// The assumed image width (in px). This width is not sure and should only be used if there is no other way for determining it
    pub img_width_assumed: Option<u32>,
    /// The assumed image height (in px). This height is not sure and should only be used if there is no other way for determining it
    pub img_height_assumed: Option<u32>,
    /// Example: 'SONY DSC-N2'
    pub camera: Option<String>,
    /// Example: 0.0166
    pub exposure_time: Option<f64>,
    /// Example: 200
    pub iso: Option<u32>,
    /// Example: 5.6
    pub aperture: Option<f64>,
    /// Example: 5
    pub focal_length: Option<f64>,
    pub created_at: Option<i64>,
    /// EXIF orientation (1=Up, 3=Bottom, 6=Right, 8=Left)
    pub orientation: u32,
    pub tags: Vec<String>,
}


/// One EXIF segment: an insertion-ordered map of exifr-style tag names to their (mostly
/// human-readable) values. `IndexMap` preserves the natural EXIF field order, which keeps the info
/// panel's "first 10 keys" fallback sensible.
pub type ExifSegment = IndexMap<String, JsonValue>;

/// The full per-segment EXIF dump for the info panel (the `getExifData` command), mirroring the
/// frontend `ExifData` type. Segments filled by `exif_reader`/`xmp_reader`: `ifd0`, `ifd1`, `exif`,
/// `gps`, `interop`, `xmp`, plus the raw `makerNote`/`userComment` byte blobs.
/// `iptc`, `icc` and `jfif` are non-EXIF standards not produced yet; they are kept for type parity and the frontend
/// renders "Photo has no …" for a missing segment.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExifData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exif: Option<ExifSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ifd0: Option<ExifSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ifd1: Option<ExifSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gps: Option<ExifSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interop: Option<ExifSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jfif: Option<ExifSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iptc: Option<ExifSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xmp: Option<ExifSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icc: Option<ExifSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maker_note: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_comment: Option<Vec<u8>>,
}
