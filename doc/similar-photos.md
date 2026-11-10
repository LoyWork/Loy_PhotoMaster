# Similar photo cleanup

Surface: inspection and selection in the existing desktop library. The job is to compare near copies and choose which to keep.

Hierarchy: scan controls → actual grouped photos → metadata and preview → selected-photo trash action. All photos start kept. Real imported library records only.

Navigation: Trash → Similar photo cleanup → Duplicate photo cleanup. Exact duplicate scanning has moved out of Settings into its own main-window page. Both cleanup pages share previews, pagination, keep/delete selection, confirmation, and app-trash restoration. The duplicate page hides scene-matching and sensitivity controls and does not require model assets.

Each cleanup page keeps an independent in-memory session in Redux: scan results, scan options, delete selections, pagination, expanded groups, and scroll position survive sidebar navigation. Starting a new scan replaces that page's session results; closing/reloading the application clears the sessions. Leaving during an unfinished scan still cancels it. Photo trash/deletion actions reconcile both cached results, remove singleton groups and stale selections, and clear a group's deletion choices if its last retained photo was removed elsewhere. Newly imported or restored photos require another scan to join the groups.

Exact matching retains the original size-prefilter plus streaming SHA-256 algorithm. It resolves library-owned paths back to photo IDs, decodes previews only for matched files, and keeps exact groups even when their previews cannot be decoded. Cancellation is checked between file reads. Files changed during hashing are skipped, and existing trash records are excluded.

Wide: `[existing sidebar | scan controls / group heading / photo photo photo / selection footer]`.
Narrow: `[scan controls] / [group heading] / [photo] / [photo] / [selection footer]`; the existing sidebar can be collapsed.

System: existing neutral photo surface, teal focus, muted metadata, red plus explicit text for deletion; existing Blueprint controls and Font Awesome icons. Spacing 8/16/24 px. No new decorative effects.

Signature: each group is a contact sheet with an explicit keep/delete state beneath each image. Photos retain their full aspect ratio.

Risks: long paths must truncate without widening the page; the selection footer must not obscure the last group. Verify narrow and wide screens, empty/error/scanning states, keyboard selection, confirmation, and partial failure.

Detection has two routes. Quick scanning uses 64-bit DCT perceptual hashes with aspect and coarse colour checks. Deep scanning (the default) extracts normalized 384-dimensional DINOv2 ViT-S/14 CLS vectors using the packaged ONNX CPU runtime. It independently retrieves scene neighbors and hash neighbors, then checks every member before adding a photo to a group; hash mismatches, different aspect ratios, folders, and dates do not exclude scene matches.

Deep matching uses cosine thresholds 0.92 / 0.85 / 0.78 for strict / standard / broad. These are initial application thresholds, not calibrated similarity probabilities. Small libraries use exact search; larger ones use HNSW with 128 retrieved neighbors, so retrieval is approximate and can miss matches. Grouping prevents transitive chains. Other visually similar scenes can still be included; every photo starts kept and requires manual selection for deletion.

Features and unedited previews are cached by source path, size, modification timestamp, and algorithm version. DINO vectors have a separate SQLite table with a model/preprocessing/pooling version and validated normalized float32 blobs. Changed models, source files, corrupt vectors, and missing previews trigger rebuilding. Scans exclude the app trash. Photos with too little visual detail or decode failures are counted as skipped; model/runtime/database failures stop the scan instead of masquerading as zero matches.

The pinned ONNX export comes from `onnx-community/dinov2-small` revision `8b1f705a3a7f6f062f6bdd21986c1583d3ef105d`, exported from `facebook/dinov2-small`. `models:prepare` verifies its SHA-256 and the official Microsoft ONNX Runtime 1.22.0 archive/DLL. It bundles about 101 MB of assets and notices, with no installed-app downloads or uploads. Local CPU deep inference currently supports Windows x64; quick scanning remains available elsewhere. See `src-tauri/models/README.md` and `DEVELOP.md` for setup and the Visual C++ runtime prerequisite.

Deletion: reuse the existing `trashed` flag, update the Redux photo data, and refresh sections/folders. Original files stay on disk until the user empties the app trash. No automatic deletion or preselected deletion.

## Verification

- Rust suite: 80 tests passed including the opt-in real-model tests. Coverage includes actual migrated SQLite scans, warm-cache reuse, source-change invalidation, missing files, cancellation, trash exclusion, prevention of transitive similarity chains, independent scene retrieval across hash/aspect boundaries, and HNSW retrieval of an otherwise missed pair. Exact matching additionally covers uniform images, unreadable previews, changed files, cancellation, library IDs, and recoverable trash selection without modifying source files.
- Jest: 40 tests passed, including last-photo protection, independent cleanup sessions, shared trash/deletion reconciliation, and removal of stale selections without selecting an entire group.
- Frontend production build and native debug build passed. Full TypeScript diagnostics compared against HEAD: 43 before and 43 after; no new diagnostics.
- Browser verification used the real App, sidebar, and cleanup components with isolated IPC fixtures and repository sample photos. Inspected 390×844 (sidebar collapsed), 768×1024, 1440×900, 1920×1080, and 2560×1080. Checked navigation, scan progress/cancellation, empty/error/retry, preview switching, keep-only selection, confirmation, successful cleanup, failed cleanup preserving selection, and pagination retaining selection.
- Wide-screen inspection found unused space with fixed photo columns; auto-fit columns now use the available group width. Pagination inspection found unstable scrolling with speculative group heights; removed content-visibility because the page is already bounded to twelve groups.
- DINOv2 CPU inference found a crop/brightness variant missed by pHash: cosine 0.9388 versus -0.0015 for an unrelated photo. The latest optimized dev test processed three photos in about 442 ms cold and 0.44 ms warm (excluding model load); this tiny fixture is not a large-library benchmark or validation of different human poses.
- Deep UI checks cover the default mode, independent quick/deep calls, actual-result mode labels, component readiness/missing state, and recovery to quick scanning. Both modes share preview, pagination, selection, confirmation, and the existing trash workflow.
- Exact UI checks verify sidebar order below Similar photo cleanup, removal from Settings, grouped main-window results, absence of model/sensitivity controls, keep-only selection, last-photo protection, confirmation, successful app-trash updates, and switching back to the similar-photo page.
- Session UI checks verify returning from the normal library and switching between cleanup pages without rescanning, independent pending deletion choices, page 2 restoration, expanded groups, exact scroll restoration, and deletion on one page removing that photo from the other cached result. Cached model readiness avoids a transient status message shifting the restored scroll position.
- Remaining acceptance: native desktop rendering against a real library, actual same-scene/different-pose threshold calibration, clean-machine installer testing, and large-library timing/memory. The in-page preview uses a cached image up to 960×720, not a full-resolution sharpness comparison. GPU acceleration is not enabled.
