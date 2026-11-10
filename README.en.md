**English** | [简体中文](README.md)

# Picturama (Loy fork)

A local photo organizer with date and folder browsing, photo adjustments, and grouped cleanup of similar and identical photos.

> This project is a fork of [picturama/picturama](https://github.com/picturama/picturama), originally created by Til Schneider and Loïc Nogues. This repository extends the original and maintains its own releases.

## Download

- [Latest release](https://github.com/LoyWork/Loy_Picturama/releases/latest): Windows x64 installers are currently provided.
- [v2.1.1 Windows installer](https://github.com/LoyWork/Loy_Picturama/releases/download/v2.1.1/Picturama_2.1.1_x64-setup.exe) · [Release notes (Chinese)](doc/releases/v2.1.1.md)

The Windows installer bundles DINOv2 and the CPU inference runtime. Installed scans do not download models or upload photos. Deep scanning requires the Microsoft Visual C++ x64 runtime on Windows.

## Screenshots

### Organize and browse your photo library

Browse a date-grouped thumbnail grid and use the sidebar to access folders, tags, favorites, and trash.

![Photo library with date-grouped thumbnails, folder navigation, favorites and trash](doc/screenshots/photo-library.png)

### View and adjust your photos

Adjust exposure, contrast, highlights, shadows, white balance, vibrance, and saturation with a live RGB histogram. Color Mixer, Color Grading, and Effects controls are also available.

![Photo detail view with RGB histogram and light and color adjustment controls](doc/screenshots/photo-adjustments.png)

### Clean up similar and duplicate photos

Both cleanup tools show grouped results in the main window. Preview photos, mark individual photos for deletion, or choose to keep only one photo. Every photo starts kept, and each group must retain at least one photo.

| Scan | Useful for | Method |
| --- | --- | --- |
| Similar photos: quick | Resized, recompressed, or brightness-adjusted copies | Perceptual hashing with aspect and color checks |
| Similar photos: deep | The same scene with changes in pose or composition | DINOv2 visual features plus near-copy retrieval |
| Duplicate photos | Identical file contents under different names or paths | File-size prefilter plus SHA-256 |

Deep scanning is the default, with strict, standard, and broad matching ranges. Visually similar unrelated scenes can appear in a group: review every photo before deleting. Deep inference currently runs on CPU and supports Windows x64.

1. Open Similar photo cleanup or Duplicate photo cleanup directly below it in the sidebar.
2. Scan, compare the groups in the main window, and choose which photos to keep or delete.
3. Confirm moving selected photos to the app trash. They can be restored until the trash is emptied, which deletes the original files.

#### Caching and scan speed

| Situation | Behavior |
| --- | --- |
| Navigate away and return | Each cleanup page independently retains results, deletion choices, scan options, page, expanded groups, and scroll position without rescanning |
| Rescan similar photos | Check for file changes, reuse unchanged perceptual hashes, DINOv2 features, and previews, then rebuild matching groups |
| New or changed photos | Recompute missing or invalid features during scanning; new or restored photos need a new scan to join groups |
| Restart the application | Page results are cleared; persisted local features and previews remain reusable |
| Rescan identical duplicates | Read same-size candidate files and compute SHA-256 again; SHA-256 caching is not implemented yet |

The first deep scan is usually slower. Later scans reuse feature extraction for unchanged photos, but still check files, load the model, rebuild the search index, and regroup results. Trash and deletion actions reconcile both cleanup pages' cached results.

## Other features

- Local file management without uploading photos.
- Non-destructive adjustments stored separately from the original photos.
- JPG, PNG, WebP, HEIC / HEIF, and TIF (macOS only).
- Camera RAW through the bundled LightCraft engine: NEF/NRW, CR2/CR3, ARW, RAF, DNG, RW2, PEF, ORF, SRW, and more; some variants use embedded previews. See [RAW coverage and limitations](doc/raw-support.md).
- Zoomable detail view, EXIF information, tags, favorites, rotation, and app-trash restoration.
- Unified sidebar for all photos, favorites, trash, cleanup tools, tags, and folders.
- Photo context menu: copy to clipboard, remove from the library, delete from disk (move to the system trash), and open the containing folder.

## Development and documentation

Built with Tauri 2, Rust, React, and Redux. See [DEVELOP.md](DEVELOP.md) for build prerequisites, commands, and dependencies.

- [Similar-photo implementation and verification](doc/similar-photos.md)
- [Local model assets](src-tauri/models/README.md)
- [RAW coverage and limitations](doc/raw-support.md)
- [Release history](https://github.com/LoyWork/Loy_Picturama/releases)
