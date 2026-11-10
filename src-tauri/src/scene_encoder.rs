//! DINOv2 ViT-S/14 feature extraction using the packaged, local CPU ONNX Runtime.
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use image::{imageops::FilterType, DynamicImage};
use ort::{session::{builder::GraphOptimizationLevel, Session}, value::Tensor};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::State;

use crate::app_config_builder::AppConfig;

pub const DIMENSIONS: usize = 384;
// Include preprocessing and pooling in the cache version, not just the model filename.
pub const MODEL_VERSION: &str = "dinov2-small-f22797ea-rgb-bicubic256-crop224-cls-l2-v1";
const MODEL_HASH: &str = "f22797eabf810a75e41de68d378541ebea372122b25c4ce3ef25ff618250c20a";
const RUNTIME_HASH: &str = "579b636403983254346a5c1d80bd28f1519cd1e284cd204f8d4ff41f8d711559";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    ready: bool,
    supported: bool,
}

pub fn assets_dir(config: &AppConfig) -> PathBuf {
    if cfg!(debug_assertions) { config.app_dir.join("src-tauri/models") }
    else { config.app_dir.join("models") }
}

#[tauri::command]
pub fn fetch_similar_model_status(config: State<'_, AppConfig>) -> ModelStatus {
    let dir = assets_dir(&config);
    let supported = cfg!(all(target_os = "windows", target_arch = "x86_64"));
    ModelStatus {
        ready: supported && dir.join("dinov2-small.onnx").is_file() && dir.join("onnxruntime.dll").is_file(),
        supported,
    }
}

pub struct SceneEncoder {
    session: Session,
}

impl SceneEncoder {
    pub fn open(dir: &Path) -> Result<Self, String> {
        if !cfg!(all(target_os = "windows", target_arch = "x86_64")) {
            return Err("Bundled deep scanning currently supports Windows x64".into());
        }
        verify_file(&dir.join("dinov2-small.onnx"), MODEL_HASH)?;
        verify_file(&dir.join("onnxruntime.dll"), RUNTIME_HASH)?;
        // ort's dynamic-loader panics if the DLL or its OS dependencies cannot be loaded.
        // Keep that failure inside this worker and present a recoverable scan error.
        std::panic::catch_unwind(|| -> Result<Self, String> {
            ort::init_from(dir.join("onnxruntime.dll").to_string_lossy())
                .with_name("picturama-scene-matching").commit().map_err(|e| e.to_string())?;
            let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(1, 4);
            let session = Session::builder().map_err(|e| e.to_string())?
                .with_intra_threads(threads).map_err(|e| e.to_string())?
                .with_inter_threads(1).map_err(|e| e.to_string())?
                .with_optimization_level(GraphOptimizationLevel::Level3).map_err(|e| e.to_string())?
                .commit_from_file(dir.join("dinov2-small.onnx")).map_err(|e| e.to_string())?;
            if session.inputs.len() != 1 || session.inputs[0].name != "pixel_values" {
                return Err("Unexpected DINOv2 input schema".into());
            }
            Ok(Self { session })
        }).map_err(|_| "Unable to load the deep scanning runtime. Repair the app and ensure the Microsoft Visual C++ x64 runtime is installed.".to_string())?
    }

    pub fn encode(&mut self, image: &DynamicImage) -> Result<Vec<f32>, String> {
        let pixels = preprocess(image)?;
        let input = Tensor::from_array(([1usize, 3, 224, 224], pixels)).map_err(|e| e.to_string())?;
        let outputs = self.session.run(ort::inputs!["pixel_values" => input]).map_err(|e| e.to_string())?;
        // Use the normalized CLS token, never an average over all output tokens.
        if let Some(output) = outputs.get("pooler_output") {
            let (shape, data) = output.try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
            if shape.as_ref() != [1, DIMENSIONS as i64] { return Err(format!("Unexpected DINOv2 pooled shape: {shape:?}")); }
            normalize(data.to_vec())
        } else {
            let output = outputs.get("last_hidden_state").ok_or("DINOv2 last_hidden_state missing")?;
            let (shape, data) = output.try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
            if shape.len() != 3 || shape[0] != 1 || shape[2] != DIMENSIONS as i64 || data.len() < DIMENSIONS {
                return Err(format!("Unexpected DINOv2 token shape: {shape:?}"));
            }
            // This export exposes the full token sequence. Token zero is the CLS representation.
            normalize(data[..DIMENSIONS].to_vec())
        }
    }
}

fn verify_file(path: &Path, expected: &str) -> Result<(), String> {
    let mut reader = BufReader::new(File::open(path).map_err(|e| format!("Deep scanning component missing: {}: {e}", path.display()))?);
    let mut digest = Sha256::new();
    let mut bytes = [0u8; 128 * 1024];
    loop {
        let count = reader.read(&mut bytes).map_err(|e| e.to_string())?;
        if count == 0 { break; }
        digest.update(&bytes[..count]);
    }
    if format!("{:x}", digest.finalize()) != expected { return Err(format!("Deep scanning component checksum mismatch: {}", path.display())); }
    Ok(())
}

fn preprocess(image: &DynamicImage) -> Result<Vec<f32>, String> {
    let (width, height) = (image.width(), image.height());
    if width == 0 || height == 0 { return Err("Empty image".into()); }
    // Pinned model's preprocessor_config: shortest edge 256, bicubic, centered 224 crop,
    // RGB in [0,1], ImageNet means/std, CHW. Guard extreme panoramas before allocation.
    let shortest = width.min(height) as u64;
    let resized_width = (width as u64 * 256 / shortest).max(256);
    let resized_height = (height as u64 * 256 / shortest).max(256);
    if resized_width * resized_height > 16_000_000 { return Err("Aspect ratio too extreme for scene matching".into()); }
    let resized = image.resize_exact(resized_width as u32, resized_height as u32, FilterType::CatmullRom);
    let rgb = resized.crop_imm((resized_width as u32 - 224) / 2, (resized_height as u32 - 224) / 2, 224, 224).to_rgb8();
    let mean = [0.485f32, 0.456, 0.406];
    let std = [0.229f32, 0.224, 0.225];
    let mut pixels = vec![0.; 3 * 224 * 224];
    for (i, pixel) in rgb.pixels().enumerate() {
        for channel in 0..3 { pixels[channel * 224 * 224 + i] = (pixel[channel] as f32 / 255. - mean[channel]) / std[channel]; }
    }
    Ok(pixels)
}

fn normalize(mut values: Vec<f32>) -> Result<Vec<f32>, String> {
    if values.len() != DIMENSIONS || values.iter().any(|v| !v.is_finite()) { return Err("Invalid scene vector".into()); }
    let norm = values.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm <= 1e-8 || !norm.is_finite() { return Err("Empty scene vector".into()); }
    values.iter_mut().for_each(|v| *v /= norm);
    Ok(values)
}

pub fn to_bytes(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

pub fn from_bytes(bytes: &[u8]) -> Option<Vec<f32>> {
    if bytes.len() != DIMENSIONS * 4 { return None; }
    let values: Vec<f32> = bytes.chunks_exact(4).map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap())).collect();
    let norm_squared = values.iter().map(|v| v*v).sum::<f32>();
    if !norm_squared.is_finite() || (norm_squared - 1.).abs() > 1e-3 { return None; }
    Some(values)
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(a,b)| a*b).sum::<f32>().clamp(-1.,1.)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preprocessing_and_vector_cache_reject_invalid_data() {
        let image = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(400,300,image::Rgb([255,0,0])));
        let data = preprocess(&image).unwrap();
        assert_eq!(data.len(), 3*224*224);
        assert!((data[0] - (1. - 0.485)/0.229).abs() < 1e-5);
        assert!((data[224*224] + 0.456/0.224).abs() < 1e-5);
        let unit = normalize(vec![1.;DIMENSIONS]).unwrap();
        assert!(cosine(&unit, &from_bytes(&to_bytes(&unit)).unwrap()) > 0.9999);
        assert!(from_bytes(&[]).is_none());
        assert!(from_bytes(&to_bytes(&vec![f32::NAN;DIMENSIONS])).is_none());
        assert!(from_bytes(&to_bytes(&vec![0.;DIMENSIONS])).is_none());
    }

    #[test]
    #[ignore = "requires npm run models:prepare; explicitly run to validate real local inference"]
    fn real_model_recognizes_changed_crop() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut encoder = SceneEncoder::open(&root.join("models")).unwrap();
        let image = image::open(root.join("../submodules/test-data/photos/landscape.jpg")).unwrap();
        let original = encoder.encode(&image).unwrap();
        let changed = image.crop_imm(image.width()/8, image.height()/12, image.width()*3/4, image.height()*5/6).brighten(18);
        let variation = encoder.encode(&changed).unwrap();
        let portrait = image::open(root.join("../submodules/test-data/photos/portrait.jpg")).unwrap();
        let unrelated = encoder.encode(&portrait).unwrap();
        let matching = cosine(&original,&variation);
        let different = cosine(&original,&unrelated);
        println!("DINOv2 similarity: changed crop={matching:.4}, unrelated={different:.4}");
        assert!(matching > 0.8 && matching > different + 0.15);
    }
}
