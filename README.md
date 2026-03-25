# Logo Studio Desktop

> 面向设计师和开发者的桌面端 Logo 设计工具 —— 本地运行，无需联网，内置 AI 智能抠图。

**技术栈：** Tauri 2.0 · Vue 3 · TypeScript · Rust · SAM2 ONNX

---

## 功能特性

### 智能抠图（AI Segmentation）
- 基于 **SAM2 ONNX** 模型，纯本地 CPU 推理，无需 Python 环境
- 点击模式：点击主体一键生成精准遮罩
- 自动模式：自动识别图像主体
- Encoder 推理结果按图像缓存，多次点击无需重复编码，响应极快

### 图像导入与编辑
- 支持 PNG / JPG / SVG / WebP 拖拽导入
- 裁剪、缩放、旋转、亮度、对比度、曝光、饱和度调整

### 文字与排版
- Logo 主文字（标题）+ Slogan（副标题）
- 本地字体管理，分类：无衬线 / 科技 / 商业展示
- 推荐使用 [Google Fonts](https://fonts.google.com/) / [FontShare](https://www.fontshare.com/) 商用免费字体

### 背景与图标生成
- 纯色 / 渐变背景（iOS 风、Material、Neon 等预设）
- 圆角、阴影、内发光、亚克力（Glassmorphism）效果

### 导出
- PNG（多尺寸：1024 / 512 / 256）
- ICO（Windows 图标）
- SVG（计划中）

### 国际化
- 中文 / English（`vue-i18n`，JSON 语言包）

---

## 技术架构

```
Frontend (Vue 3 + Vite)
    ↕ Tauri IPC Commands
Backend (Rust)
    ├── SAM2 ONNX 推理 (ort 2.0)
    ├── 图像处理 (image crate)
    └── 文件导出
```

| 层级 | 技术 |
|------|------|
| 桌面框架 | Tauri 2.0 |
| 前端框架 | Vue 3 + TypeScript |
| 构建工具 | Vite 6 |
| 状态管理 | Pinia |
| 工具库 | VueUse |
| 国际化 | vue-i18n 11 |
| AI 推理 | ort 2.0 (ONNX Runtime) + ndarray |
| 图像处理 | image 0.25 (Rust) |

---

## 项目结构

```
logo-studio/
├── src/                        # 前端源码
│   ├── components/
│   │   ├── layout/             # 整体布局（三栏）
│   │   └── ui/                 # 通用 UI 组件
│   ├── modules/
│   │   ├── segmentation/       # AI 抠图模块
│   │   ├── image-editor/       # 图像编辑
│   │   ├── background/         # 背景生成
│   │   ├── typography/         # 字体与文字
│   │   └── export/             # 导出系统
│   ├── store/                  # Pinia 状态
│   ├── i18n/                   # 语言包（zh / en）
│   └── styles/                 # 全局样式
│
├── src-tauri/
│   ├── src/
│   │   ├── sam2.rs             # SAM2 ONNX 推理引擎
│   │   ├── commands/
│   │   │   ├── segment.rs      # 抠图 Tauri Command
│   │   │   ├── image.rs        # 图像处理 Command
│   │   │   └── export.rs       # 导出 Command
│   │   └── lib.rs / main.rs
│   ├── models/                 # SAM2 ONNX 模型文件（不入 git）
│   │   ├── image_encoder.onnx
│   │   ├── image_encoder.onnx.data
│   │   ├── image_decoder.onnx
│   │   └── image_decoder.onnx.data
│   └── tauri.conf.json
│
├── public/
└── package.json
```

---

## 开发环境搭建

### 前置依赖

- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://rustup.rs/) (stable)
- [Tauri 前置依赖](https://tauri.app/start/prerequisites/)（Windows 需要 Microsoft C++ Build Tools / WebView2）

### 安装与启动

```bash
# 安装前端依赖
npm install

# 启动开发模式（热更新）
npm run tauri dev

# 构建生产包
npm run tauri build
```

### SAM2 模型文件

模型文件体积较大（单文件 > 100 MB），不包含在仓库中，需手动下载后放置到 `src-tauri/models/` 目录：

```
src-tauri/models/
├── image_encoder.onnx
├── image_encoder.onnx.data
├── image_decoder.onnx
└── image_decoder.onnx.data
```

> 支持的文件名别名：`encoder.onnx` / `sam2_encoder.onnx` / `image_encoder.onnx`（decoder 同理），程序会自动查找。
>
> 也可通过环境变量 `SAM2_MODELS_DIR` 指定模型目录。

---

## SAM2 推理说明

推理引擎位于 `src-tauri/src/sam2.rs`，采用 Encoder-Decoder 两阶段结构：

| 阶段 | 模型 | 输入 | 输出 |
|------|------|------|------|
| Encoder | `image_encoder.onnx` | `[1, 3, 1024, 1024]` | embeddings + high_res_features |
| Decoder | `image_decoder.onnx` | embeddings + point prompts | mask logits `[1, M, 256, 256]` |

- 图像预处理：最长边 resize 到 1024，ImageNet 均值/方差归一化，透明像素填充为 ImageNet 中性灰
- Encoder 输出按图像哈希缓存，同一张图多次点击无需重新编码
- Decoder 禁用图优化（`ORT_DISABLE_ALL`），防止 FusedGemmTransposeFusion 因点数变化导致推理错误

---

## 推荐开发工具

- [VS Code](https://code.visualstudio.com/)
  - [Vue - Official](https://marketplace.visualstudio.com/items?itemName=Vue.volar)
  - [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
  - [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

---

## 版本规划

| 版本 | 目标 |
|------|------|
| v0.1 | 基础 UI · 图片导入 · SAM2 抠图 |
| v0.2 | 字体 + Slogan · 背景生成 |
| v0.3 | 导出系统 · 图像编辑 |
| v1.0 | AI 生成接入 · 用户系统 |

---

## License

MIT
