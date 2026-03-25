# Logo Studio Desktop

A cross-platform desktop logo design tool with offline AI-powered background removal, built on Tauri 2.0 + Vue 3 + Rust.

[中文文档](./README_ZH.md)

---

## Overview

Logo Studio Desktop provides a local-first, privacy-preserving workflow for designing logos and generating multi-platform icon sets. The core feature is **SAM2 ONNX** inference running entirely on-device (CPU), with a multi-stage classical fallback for environments where the model is unavailable.

---

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop runtime | Tauri 2.0 |
| Frontend | Vue 3 · TypeScript · Vite 6 |
| State management | Pinia 3 |
| i18n | vue-i18n 11 (zh / en) |
| Backend / inference | Rust (stable) |
| ONNX inference | `ort` 2.0-rc.12 (ONNX Runtime) |
| Image processing | `image` 0.25 (Rust) |
| Tensor ops | `ndarray` 0.17 |

---

## Features

- **AI Background Removal** — SAM2 ONNX runs locally; no cloud, no API key
  - Click mode: provide foreground points → mask
  - Auto mode: center-based heuristic → mask
  - Multi-stage fallback: alpha passthrough → solid-color removal → flood-fill + alpha matting
- **Image Editing** — Import PNG / JPG / WebP, adjust brightness, contrast, saturation, rotate, scale
- **Typography** — Logo title + slogan layers, local font files, per-layer styling
- **Background Generator** — 40+ presets (iOS-style, Material, Neon…), solid / linear / radial gradient, shadow, glassmorphism
- **Icon Export** — Single PNG (1024 / 512 / 256 px) or full icon sets:
  - Web (favicon 16 → 512 px)
  - iOS (16 sizes)
  - Android (7 DPI variants)
  - macOS (10 Retina sizes)
- **Internationalization** — Chinese (zh) and English (en) with runtime switching

---

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│  Frontend (Vue 3 + Pinia)                                    │
│                                                              │
│  CenterCanvas ── LeftPanel ── RightPanel                     │
│       │              │              │                        │
│  useImageEditor  useSegmentation  useBackground              │
│  useExport       useTypography                               │
│                                                              │
│  Pinia Stores:  canvasStore · appStore                       │
│                 backgroundStore · typographyStore            │
└───────────────────────┬──────────────────────────────────────┘
                        │  Tauri IPC (invoke)
┌───────────────────────▼──────────────────────────────────────┐
│  Rust Backend                                                │
│                                                              │
│  commands/                                                   │
│  ├─ segment.rs   SAM2 primary + 3-level fallback pipeline    │
│  ├─ image.rs     read_image / save_image                     │
│  └─ export.rs    export_icon_set (multi-size resize)         │
│                                                              │
│  sam2.rs         ONNX inference engine                       │
│  ├─ Encoder  [1,3,1024,1024] → embeddings + FPN features    │
│  ├─ Decoder  embeddings + prompts → pred_mask logits         │
│  └─ Cache    per-image hash, reuse encoder output            │
│                                                              │
│  src-tauri/models/  (git-ignored — download separately)      │
│  ├─ image_encoder.onnx + .data  (~815 MB)                    │
│  └─ image_decoder.onnx + .data  (~18 MB)                     │
└──────────────────────────────────────────────────────────────┘
```

---

## SAM2 ONNX Integration

### Model IO

```
Encoder
  input:   "input"              [1, 3, 1024, 1024]  float32  NCHW
  outputs: "image_embeddings"   [1, 256, 64, 64]
           "high_res_features1" [1, 32, 256, 256]
           "high_res_features2" [1, 64, 128, 128]

Decoder
  inputs:  "image_embed"        [1, 256, 64, 64]
           "high_res_feats_0"   [1, 32, 256, 256]
           "high_res_feats_1"   [1, 64, 128, 128]
           "point_coords"       [1, N, 2]    float32  original image space
           "point_labels"       [1, N]       float32  1=fg, 0=bg
  outputs: "pred_mask"          [1, M, 256, 256]  logits
           "mask_for_mem"       [1, M, 1024, 1024] sigmoid
```

### Preprocessing

1. Pad / resize longest dimension to 1024
2. ImageNet normalization: mean = [0.485, 0.456, 0.406], std = [0.229, 0.224, 0.225]
3. Transparent pixels (α < 30) replaced with neutral gray ≈ (127, 116, 104)

### Encoder Caching

Sessions are held in `OnceLock<Mutex<Session>>` (process-lifetime). Encoder output is cached keyed on an image hash; subsequent point clicks reuse the cached embeddings, giving instant re-segmentation on the same image.

```rust
static ENCODER: OnceLock<Mutex<Session>> = OnceLock::new();
static DECODER: OnceLock<Mutex<Session>> = OnceLock::new();
static CACHE:   OnceLock<Mutex<Option<EncoderCache>>> = OnceLock::new();
```

### Decoder Stability

Graph optimization is explicitly disabled (`GraphOptimizationLevel::Disable`) and `FusedGemmTransposeFusion` is suppressed — this optimization breaks inference when the point count varies between calls.

### Fallback Pipeline

```
SAM2 (primary)
  ↓ unavailable or error
Level 1 — Alpha passthrough    image already has transparency → use as mask
  ↓
Level 2 — Solid bg detection   uniform background (variance < 28²) + color-distance matte
  ↓
Level 3 — Flood fill + matte   BFS from user points / centroid + alpha-matting refinement
```

Alpha matting: erode/dilate mask → trimap (fg / bg / unknown) → BFS nearest-neighbor color propagation → closed-form alpha per unknown pixel.

---

## SAM2 ONNX Model Export (`sam2-onnx-cpp`)

The companion directory `../sam2-onnx-cpp` contains a standalone Python export pipeline and C++ ONNX Runtime wrapper used to produce the model files consumed by Logo Studio.

### Export Pipeline

```bash
# 1. Create Python environment
python -m venv sam2_env
source sam2_env/bin/activate          # macOS / Linux
# sam2_env\Scripts\Activate           # Windows

pip install -r requirements_mac.txt   # or requirements_win.txt

# 2. Fetch SAM2 source + checkpoints (sparse clone, no full history)
./fetch_sparse.sh    # macOS / Linux
fetch_sparse.bat     # Windows

# 3. Export ONNX  (tiny | small | base_plus | large)
python export/onnx_export.py --model_size base_plus
```

Output files land in `checkpoints/base_plus/`:

| File | Size | Purpose |
|---|---|---|
| `image_encoder.onnx` | 4.2 MB | Hiera ViT backbone (weights external) |
| `image_encoder.onnx.data` | ~815 MB | External encoder weights |
| `image_decoder.onnx` | 882 KB | Mask decoder |
| `image_decoder.onnx.data` | ~17 MB | External decoder weights |
| `memory_attention.onnx` + `.data` | ~24 MB | Temporal attention (video mode) |
| `memory_encoder.onnx` + `.data` | ~6 MB | Temporal memory encoder (video mode) |

Logo Studio uses only the image encoder and decoder pairs. Copy them to `logo-studio/src-tauri/models/`.

### C++ Wrapper (standalone testing)

`sam2-onnx-cpp/cpp/` provides a native C++20 wrapper around ONNX Runtime for testing inference without Python:

```bash
cd sam2-onnx-cpp/cpp
cmake -S . -B build_release -DCMAKE_BUILD_TYPE=Release \
  -DOpenCV_DIR="/opt/homebrew/opt/opencv" \
  -DONNXRUNTIME_DIR="/opt/onnxruntime-osx-arm64-1.23.2"
cmake --build build_release
cmake --install build_release --prefix ./package

# Image segmentation test (seed points or bounding box prompt)
./package/Segment.app/Contents/MacOS/Segment --onnx_test_image --prompt seed_points
```

**Build dependencies:** CMake ≥ 3.14, C++20 compiler, OpenCV, ONNX Runtime ≥ 1.22. GPU support available on Windows via CUDA 12.5 + cuDNN.

---

## Project Structure

```
logo-studio/
├── src/                         # Vue 3 frontend
│   ├── components/
│   │   ├── layout/
│   │   │   ├── CenterCanvas.vue
│   │   │   ├── LeftPanel.vue
│   │   │   └── RightPanel.vue
│   │   └── ui/
│   │       ├── GlassCard.vue
│   │       ├── SliderControl.vue
│   │       └── ThemeToggle.vue
│   ├── modules/
│   │   ├── segmentation/useSegmentation.ts
│   │   ├── image-editor/useImageEditor.ts
│   │   ├── background/useBackground.ts
│   │   ├── typography/useTypography.ts
│   │   └── export/useExport.ts
│   ├── store/
│   │   ├── useAppStore.ts
│   │   ├── useCanvasStore.ts
│   │   ├── useBackgroundStore.ts
│   │   └── useTypographyStore.ts
│   ├── i18n/index.ts
│   └── styles/
│       ├── variables.css
│       ├── theme.css
│       ├── glass.css
│       └── global.css
│
├── src-tauri/                   # Rust backend
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── sam2.rs              # ONNX inference engine
│   │   └── commands/
│   │       ├── mod.rs
│   │       ├── segment.rs       # Segmentation + fallback pipeline
│   │       ├── image.rs         # File I/O commands
│   │       └── export.rs        # Icon set export
│   ├── models/                  # SAM2 ONNX files (git-ignored)
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── .gitignore
├── package.json
├── vite.config.ts
└── tsconfig.json
```

---

## Tauri IPC Commands

All commands are `async` and return `Result<T, String>`.

### `segment_image`

```typescript
invoke('segment_image', {
  imageSrc:       string,            // data:image/png;base64,...
  points:         { x, y, label }[], // canvas space [0, 1024]
  mode:           'auto' | 'point',
  tolerance?:     number,            // 0–200 → mapped to 0–150 internal
  sam2Threshold?: number,            // 0.05–0.95, default 0.50
  matteRadius?:   number,            // 1–30 px, default 8
}) → SegmentResult { success, mask: string, error?, method }
// method: 'sam2' | 'alpha' | 'color+matte' | 'flood_fill+matte' | 'error'
```

### `read_image`

```typescript
invoke('read_image', { path: string })
→ { width, height, format, data: string }  // data = base64
```

### `save_image`

```typescript
invoke('save_image', { dataUrl: string, path: string })
```

### `export_icon_set`

```typescript
invoke('export_icon_set', {
  dataUrl:   string,
  outputDir: string,
  entries:   { size: number, relpath: string }[],
}) → number  // files written
```

### `check_sam2` (debug)

```typescript
invoke('check_sam2') → string  // 'exe=/path | sam2_available=true'
```

---

## Setup

### Prerequisites

- Node.js ≥ 18
- Rust stable (via rustup)
- Tauri v2 prerequisites: <https://v2.tauri.app/start/prerequisites/>

### Install & Run

```bash
cd logo-studio
npm install
npm run tauri dev
```

### SAM2 Model Files

Model files are **not included in the repository** (total ~836 MB). Place them in `src-tauri/models/` before running:

```
src-tauri/models/
├── image_encoder.onnx
├── image_encoder.onnx.data   (~815 MB)
├── image_decoder.onnx
└── image_decoder.onnx.data   (~17 MB)
```

Override the model directory via environment variable:

```bash
SAM2_MODELS_DIR=/your/path npm run tauri dev
```

Accepted filenames (first match wins):

| Role | Accepted names |
|---|---|
| Encoder | `encoder.onnx`, `sam2_encoder.onnx`, `image_encoder.onnx` |
| Decoder | `decoder.onnx`, `sam2_decoder.onnx`, `image_decoder.onnx` |

If models are absent the app starts normally and falls back to classical segmentation.

### Build for Production

```bash
npm run tauri build
```

Bundles are emitted to `src-tauri/target/release/bundle/`.

---

## Development Notes

- Canvas is 800×800 px; zoom/pan handled in `useCanvasStore`
- Background presets live in `useBackgroundStore` (~40 gradient definitions)
- `useExport` renders a temporary off-screen canvas at target size before invoking `export_icon_set`
- ONNX Runtime is downloaded automatically at build time via the `ort` crate feature `download-binaries`
- Recommended VS Code extensions: `Vue.volar`, `tauri-apps.tauri-vscode`, `rust-lang.rust-analyzer`

---

## Related

- [`../sam2-onnx-cpp`](../sam2-onnx-cpp) — SAM2 ONNX export pipeline and C++ inference wrapper

---

## License

MIT
