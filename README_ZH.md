# Logo Studio Desktop

面向设计师和开发者的跨平台桌面端 Logo 设计工具，内置离线 AI 智能抠图，基于 Tauri 2.0 + Vue 3 + Rust 构建。

[English](./README.md)

---

## 项目简介

Logo Studio Desktop 采用本地优先、数据不出本机的设计理念，为 Logo 设计和多平台图标导出提供完整工作流。核心能力是 **SAM2 ONNX** 本地 CPU 推理抠图，无需联网、无需 Python 运行时，并在模型不可用时自动降级到多阶段经典算法。

![main](D:/Work/Code/Cross/logo-studio-desktop/logo-studio/docs/pic/main.png)设计结果输出（可带符合移动端审美的背景混色模板):

![sample1](D:/Work/Code/Cross/logo-studio-desktop/logo-studio/docs/pic/sample1.png)

![sample2](./docs/pic/sample2.png)

---

## 技术栈

| 层级 | 技术 |
|---|---|
| 桌面运行时 | Tauri 2.0 |
| 前端框架 | Vue 3 · TypeScript · Vite 6 |
| 状态管理 | Pinia 3 |
| 国际化 | vue-i18n 11（zh / en） |
| 后端 / 推理 | Rust stable |
| ONNX 推理 | `ort` 2.0-rc.12（ONNX Runtime） |
| 图像处理 | `image` 0.25（Rust） |
| 张量运算 | `ndarray` 0.17 |

---

## 功能特性

- **AI 智能抠图** — SAM2 ONNX 本地运行，无 API Key，无隐私风险
  - 点击模式：点击主体指定前景点 → 生成精准遮罩
  - 自动模式：基于中心启发式自动识别主体
  - 多阶段降级：Alpha 透明通道直通 → 纯色背景识别 + 颜色距离抠图 → 洪水填充 + Alpha 抠边
- **图像导入与编辑** — 支持 PNG / JPG / WebP，亮度、对比度、饱和度、旋转、缩放
- **文字与排版** — Logo 主标题 + Slogan 双图层，支持本地字体文件，独立样式控制
- **背景生成器** — 40+ 预设（iOS 风格、Material、Neon 等），纯色 / 线性 / 径向渐变，阴影、玻璃态效果
- **图标导出** — 单张 PNG（1024 / 512 / 256 px）或完整图标集：
  - Web（favicon 16 → 512 px）
  - iOS（16 尺寸）
  - Android（7 种 DPI 变体）
  - macOS（10 种 Retina 尺寸）
- **国际化** — 中文 / English，运行时切换

---

## 系统架构

```
┌──────────────────────────────────────────────────────────────┐
│  前端（Vue 3 + Pinia）                                        │
│                                                              │
│  CenterCanvas ── LeftPanel ── RightPanel                     │
│       │              │              │                        │
│  useImageEditor  useSegmentation  useBackground              │
│  useExport       useTypography                               │
│                                                              │
│  Pinia Store:  canvasStore · appStore                        │
│                backgroundStore · typographyStore             │
└───────────────────────┬──────────────────────────────────────┘
                        │  Tauri IPC（invoke）
┌───────────────────────▼──────────────────────────────────────┐
│  Rust 后端                                                    │
│                                                              │
│  commands/                                                   │
│  ├─ segment.rs   SAM2 主路 + 三级降级抠图流水线              │
│  ├─ image.rs     read_image / save_image                     │
│  └─ export.rs    export_icon_set（多尺寸缩放）                │
│                                                              │
│  sam2.rs         ONNX 推理引擎                               │
│  ├─ Encoder  [1,3,1024,1024] → embeddings + FPN features    │
│  ├─ Decoder  embeddings + prompts → pred_mask logits         │
│  └─ Cache    按图像哈希缓存 Encoder 输出                     │
│                                                              │
│  src-tauri/models/  （已加入 .gitignore，需手动下载）         │
│  ├─ image_encoder.onnx + .data  (~815 MB)                    │
│  └─ image_decoder.onnx + .data  (~18 MB)                     │
└──────────────────────────────────────────────────────────────┘
```

---

## SAM2 ONNX 推理详解

### 模型输入输出

```
Encoder（图像编码器）
  输入:   "input"              [1, 3, 1024, 1024]  float32  NCHW
  输出:   "image_embeddings"   [1, 256, 64, 64]
          "high_res_features1" [1, 32, 256, 256]
          "high_res_features2" [1, 64, 128, 128]

Decoder（掩码解码器）
  输入:   "image_embed"        [1, 256, 64, 64]
          "high_res_feats_0"   [1, 32, 256, 256]
          "high_res_feats_1"   [1, 64, 128, 128]
          "point_coords"       [1, N, 2]    float32  原图坐标空间
          "point_labels"       [1, N]       float32  1=前景, 0=背景
  输出:   "pred_mask"          [1, M, 256, 256]  logits
          "mask_for_mem"       [1, M, 1024, 1024] sigmoid
```

### 图像预处理

1. 最长边 padding / resize 到 1024
2. ImageNet 归一化：mean = [0.485, 0.456, 0.406]，std = [0.229, 0.224, 0.225]
3. 透明像素（α < 30）填充为 ImageNet 中性灰 ≈ (127, 116, 104)

### Encoder 缓存

会话以 `OnceLock<Mutex<Session>>` 持有（进程生命周期内复用）。Encoder 输出按图像哈希缓存，同一张图片的后续点击直接复用缓存，无需重新编码，响应近乎即时。

```rust
static ENCODER: OnceLock<Mutex<Session>> = OnceLock::new();
static DECODER: OnceLock<Mutex<Session>> = OnceLock::new();
static CACHE:   OnceLock<Mutex<Option<EncoderCache>>> = OnceLock::new();
```

### Decoder 稳定性处理

图优化级别显式设为 `GraphOptimizationLevel::Disable`，并关闭 `FusedGemmTransposeFusion` —— 该优化在点数量变化时会导致推理结果错误。

### 降级抠图流水线

```
SAM2（主路）
  ↓ 模型不可用或推理失败
Level 1 — Alpha 直通       图像本身有透明通道 → 直接用作遮罩
  ↓
Level 2 — 纯色背景检测     背景方差 < 28² → 颜色距离抠图 + Alpha 抠边
  ↓
Level 3 — 洪水填充 + 抠边  BFS 从用户点 / 质心出发 → Alpha Matting 精修边缘
```

**Alpha Matting 原理：** 腐蚀/膨胀粗糙遮罩 → 生成 trimap（确定前景 / 确定背景 / 未知区域）→ BFS 最近邻颜色传播 → 对每个未知像素用闭合形式 alpha 公式求解：`alpha = dot(C-B, F-B) / |F-B|²`。

---

## SAM2 ONNX 模型导出（`sam2-onnx-cpp`）

伴生目录 `../sam2-onnx-cpp` 包含独立的 Python 导出流水线和 C++ ONNX Runtime 封装，用于生成 Logo Studio 所需的模型文件。

### 导出流程

```bash
# 1. 创建 Python 虚拟环境
python -m venv sam2_env
source sam2_env/bin/activate          # macOS / Linux
# sam2_env\Scripts\Activate           # Windows

pip install -r requirements_mac.txt   # 或 requirements_win.txt

# 2. 稀疏克隆 SAM2 源码 + 下载检查点（无需完整仓库历史）
./fetch_sparse.sh    # macOS / Linux
fetch_sparse.bat     # Windows

# 3. 导出 ONNX  （可选：tiny | small | base_plus | large）
python export/onnx_export.py --model_size base_plus
```

输出文件位于 `checkpoints/base_plus/`：

| 文件 | 大小 | 用途 |
|---|---|---|
| `image_encoder.onnx` | 4.2 MB | Hiera ViT 骨干网络（权重外置） |
| `image_encoder.onnx.data` | ~815 MB | 编码器外置权重 |
| `image_decoder.onnx` | 882 KB | 掩码解码器 |
| `image_decoder.onnx.data` | ~17 MB | 解码器外置权重 |
| `memory_attention.onnx` + `.data` | ~24 MB | 时序注意力（视频模式） |
| `memory_encoder.onnx` + `.data` | ~6 MB | 时序记忆编码（视频模式） |

Logo Studio 仅使用 image encoder 和 image decoder。将这两对文件复制到 `logo-studio/src-tauri/models/` 即可。

### C++ 推理封装（独立测试）

`sam2-onnx-cpp/cpp/` 提供基于 C++20 的 ONNX Runtime 封装，可脱离 Python 独立测试推理：

```bash
cd sam2-onnx-cpp/cpp
cmake -S . -B build_release -DCMAKE_BUILD_TYPE=Release \
  -DOpenCV_DIR="/opt/homebrew/opt/opencv" \
  -DONNXRUNTIME_DIR="/opt/onnxruntime-osx-arm64-1.23.2"
cmake --build build_release
cmake --install build_release --prefix ./package

# 图像分割测试（种子点 / 边界框 Prompt）
./package/Segment.app/Contents/MacOS/Segment --onnx_test_image --prompt seed_points
```

**构建依赖：** CMake ≥ 3.14，C++20 编译器，OpenCV，ONNX Runtime ≥ 1.22。Windows 支持 CUDA 12.5 + cuDNN GPU 加速。

---

## 项目结构

```
logo-studio/
├── src/                         # Vue 3 前端
│   ├── components/
│   │   ├── layout/
│   │   │   ├── CenterCanvas.vue   # 主画布（800×800）
│   │   │   ├── LeftPanel.vue      # 左侧面板（图像 / 抠图 / 编辑）
│   │   │   └── RightPanel.vue     # 右侧面板（文字 / 背景 / 导出）
│   │   └── ui/
│   │       ├── GlassCard.vue
│   │       ├── SliderControl.vue
│   │       └── ThemeToggle.vue
│   ├── modules/
│   │   ├── segmentation/useSegmentation.ts  # 调用 segment_image 命令
│   │   ├── image-editor/useImageEditor.ts   # Canvas 2D 渲染 + 变换
│   │   ├── background/useBackground.ts      # 背景渐变生成
│   │   ├── typography/useTypography.ts      # 文字图层管理
│   │   └── export/useExport.ts             # 图标集导出
│   ├── store/
│   │   ├── useAppStore.ts          # 全局：主题、语言、Toast
│   │   ├── useCanvasStore.ts       # 画布：图层、抠图状态
│   │   ├── useBackgroundStore.ts   # 背景：40+ 渐变预设
│   │   └── useTypographyStore.ts   # 文字：图层列表、字体
│   ├── i18n/index.ts
│   └── styles/
│       ├── variables.css   # 100+ CSS 自定义属性
│       ├── theme.css       # 深色 / 浅色主题
│       ├── glass.css       # 玻璃态样式
│       └── global.css      # 全局重置
│
├── src-tauri/                   # Rust 后端
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── sam2.rs              # ONNX 推理引擎
│   │   └── commands/
│   │       ├── mod.rs
│   │       ├── segment.rs       # 抠图 + 降级流水线（559 行）
│   │       ├── image.rs         # 文件读写命令
│   │       └── export.rs        # 图标集导出命令
│   ├── models/                  # SAM2 ONNX 文件（已 gitignore）
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── .gitignore
├── package.json
├── vite.config.ts
└── tsconfig.json
```

---

## Tauri IPC 命令

所有命令均为 `async`，返回 `Result<T, String>`。

### `segment_image`

```typescript
invoke('segment_image', {
  imageSrc:       string,            // data:image/png;base64,...
  points:         { x, y, label }[], // 画布坐标空间 [0, 1024]
  mode:           'auto' | 'point',
  tolerance?:     number,            // 0–200 → 内部映射 0–150
  sam2Threshold?: number,            // 0.05–0.95，默认 0.50
  matteRadius?:   number,            // 1–30 px，默认 8
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
}) → number  // 写入文件数
```

### `check_sam2`（调试）

```typescript
invoke('check_sam2') → string  // 'exe=/path | sam2_available=true'
```

---

## 开发环境搭建

### 前置依赖

- Node.js ≥ 18
- Rust stable（通过 rustup 安装）
- Tauri v2 前置依赖：<https://v2.tauri.app/start/prerequisites/>
  - Windows 需要 Microsoft C++ Build Tools 和 WebView2

### 安装与启动

```bash
cd logo-studio
npm install
npm run tauri dev
```

### SAM2 模型文件

模型文件**未包含在仓库中**（合计约 836 MB），需手动下载并放置到 `src-tauri/models/`：

```
src-tauri/models/
├── image_encoder.onnx
├── image_encoder.onnx.data   (~815 MB)
├── image_decoder.onnx
└── image_decoder.onnx.data   (~17 MB)
```

通过环境变量覆盖模型目录：

```bash
SAM2_MODELS_DIR=/your/path npm run tauri dev
```

支持的文件名别名（按顺序匹配第一个）：

| 角色 | 可接受的文件名 |
|---|---|
| Encoder | `encoder.onnx`、`sam2_encoder.onnx`、`image_encoder.onnx` |
| Decoder | `decoder.onnx`、`sam2_decoder.onnx`、`image_decoder.onnx` |

模型缺失时应用正常启动，自动降级到经典抠图算法。

### 生产构建

```bash
npm run tauri build
```

产物位于 `src-tauri/target/release/bundle/`。

---

## 开发说明

- 画布固定 800×800 px，缩放/平移由 `useCanvasStore` 管理
- 背景预设定义在 `useBackgroundStore`（约 40 条渐变配置）
- `useExport` 在导出前先在离屏 Canvas 以目标尺寸渲染，再调用 `export_icon_set`
- ONNX Runtime 在编译时由 `ort` crate 的 `download-binaries` feature 自动下载
- 推荐 VS Code 插件：`Vue.volar`、`tauri-apps.tauri-vscode`、`rust-lang.rust-analyzer`

---

## 相关项目

- [`../sam2-onnx-cpp`](../sam2-onnx-cpp) — SAM2 ONNX 导出流水线与 C++ 推理封装

---

## 开源协议

MIT
