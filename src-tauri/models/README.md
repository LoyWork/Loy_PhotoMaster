# Local scene matching assets

Run `npm run models:prepare` from the project root. Windows x64 builds bundle DINOv2 ViT-S/14 and the CPU ONNX Runtime; photos and inference remain local. These generated binaries are deliberately not committed.

Model: `onnx-community/dinov2-small`, revision `8b1f705a3a7f6f062f6bdd21986c1583d3ef105d`, float32 ONNX export of `facebook/dinov2-small` (Apache-2.0). Model SHA-256: `f22797eabf810a75e41de68d378541ebea372122b25c4ce3ef25ff618250c20a`.

Runtime: official Microsoft ONNX Runtime 1.22.0 Windows x64 CPU release (MIT). Download, archive checksum, individual runtime checksum, and third-party notices are handled by the preparation script. No downloads occur inside the installed application.
