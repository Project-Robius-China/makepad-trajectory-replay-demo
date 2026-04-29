# Makepad Trajectory Replay Demo

## 中文

### 这是什么

这是一个用 Makepad 2.0 构建的 Android 轨迹回放演示项目，用真实骑行 GPX 数据驱动一套 GPU-first 的移动端界面。它展示了 Makepad 在 Android 上处理自定义 shader、地图底图、轨迹动画、HUD 数据联动、播放控制和结束弹窗的能力。

项目当前默认演示 cycling profile。数据层按通用 trajectory replay engine 设计：启动时优先从 `Project-Robius-China/trajectory-replay-data` 拉取默认轨迹数据，网络不可用时回退到本仓库内置的 `resources/cycling-track.gpx`。为了公开演示时动画更明显，运行时会把长轨迹裁剪成一段运动变化更集中的演示窗口，但不会改动原始 GPX 文件。

### 主要能力

- Makepad 2.0 + Rust 实现的 Android 原生 demo。
- 真实 GPX 轨迹解析，包含速度、心率、踏频、海拔等字段。
- 深色地图底图与轨迹 shader 回放。
- 播放、暂停、重新回放和拖动进度联动。
- 速度、心率、海拔、踏频 HUD 实时刷新。
- 网络数据优先，本地资源兜底。
- Android 打包资源统一放在 `resources/` 下，包括 GPX、SVG、地图瓦片等。

### 环境要求

需要先准备：

- Rust toolchain
- `cargo makepad`
- Android SDK / NDK
- 一台 Android 真机或模拟器

如果本机还没有 Makepad Android toolchain，可以先在项目目录运行：

```bash
cargo install --force --git https://github.com/makepad/makepad.git --branch dev cargo-makepad

cargo makepad android install-toolchain --full-ndk

```
来安装cargo makepad工具 和安卓工具链 下载需要较长时间请耐心等待

确认设备连接：

```bash
cargo makepad android adb devices -l
```

### 编译并安装运行

连接 Android 设备后运行：

```bash
 cargo makepad android run -p mobile_example --release
```

### 本地开发验证

常用检查命令：

```bash
cargo check
cargo test
cargo clippy -- -D warnings
git diff --check
```

桌面端快速运行：

```bash
cargo run
```

指定演示阶段运行：

```bash
MOBILE_EXAMPLE_DEMO_STAGE=S0 cargo run
MOBILE_EXAMPLE_DEMO_STAGE=S2 cargo run
MOBILE_EXAMPLE_DEMO_STAGE=S3 cargo run
MOBILE_EXAMPLE_DEMO_STAGE=S4 cargo run
```

### 资源说明

- `resources/cycling-track.gpx`: 内置 fallback 轨迹数据。
- `resources/map_tiles/`: 打包进应用的地图瓦片资源。
- `assets/`: 设计或开发期素材，不作为 Android 运行时资源目录。
- `plans/`: UI 调整、演示策略和实现决策记录。
- `design/refs/`: 视觉参考图和阶段目标图。

### 常见问题

如果遇到缺少 icon 的 warning，可以继续使用 `--no-icon`。这是预期行为，不影响 APK 构建。

如果 release 构建遇到 LTO 与 Android 动态库冲突，请确认 `Cargo.toml` 中 `[profile.release]` 使用 `lto = false`。Makepad Android wrapper 会把 app 作为动态库构建，当前 Android 构建路径不能启用 release LTO。

---

## English

### What Is This

This is a Makepad 2.0 Android trajectory replay demo. It uses real cycling GPX data to drive a GPU-first mobile interface with a dark map, animated route playback, data HUD, playback controls, and a completion modal.

The current demo focuses on the cycling profile. The data layer is designed as a general trajectory replay engine: at startup it tries to load the default dataset from `Project-Robius-China/trajectory-replay-data`, then falls back to the bundled `resources/cycling-track.gpx` when the network is unavailable. For public demos, long tracks are trimmed at runtime to a motion-rich window so the replay feels more visible and focused. The source GPX file is not modified.

### Features

- Native Android demo built with Makepad 2.0 and Rust.
- Real GPX parsing with speed, heart rate, cadence, and elevation data.
- Dark map background with shader-based route playback.
- Play, pause, replay, and scrubber interaction.
- Live HUD updates for speed, heart rate, elevation, and cadence.
- Network-first data loading with local fallback.
- Runtime Android resources are stored under `resources/`, including GPX, SVG, and map tiles.

### Requirements

Install or prepare:

- Rust toolchain
- `cargo makepad`
- Android SDK / NDK
- An Android device or emulator

If the Makepad Android toolchain is not installed yet, run the following commands from the project directory:

```bash
cargo install --force --git https://github.com/makepad/makepad.git --branch dev cargo-makepad

cargo makepad android install-toolchain --full-ndk
```

This installs the `cargo makepad` tool and the Android toolchain. The download can take a while, so wait for it to finish.

Check connected devices:

```bash
cargo makepad android adb devices -l
```

### Build, Install, And Run

With an Android device connected:

```bash
cargo makepad android run -p mobile_example --release
```

### Local Development Checks

Useful verification commands:

```bash
cargo check
cargo test
cargo clippy -- -D warnings
git diff --check
```

Run on desktop:

```bash
cargo run
```

Run a specific demo stage:

```bash
MOBILE_EXAMPLE_DEMO_STAGE=S0 cargo run
MOBILE_EXAMPLE_DEMO_STAGE=S2 cargo run
MOBILE_EXAMPLE_DEMO_STAGE=S3 cargo run
MOBILE_EXAMPLE_DEMO_STAGE=S4 cargo run
```

### Resources

- `resources/cycling-track.gpx`: bundled fallback trajectory data.
- `resources/map_tiles/`: bundled map tile resources.
- `assets/`: design or development-time assets, not the Android runtime resource directory.
- `plans/`: UI tuning, demo strategy, and implementation decision notes.
- `design/refs/`: visual references and target screenshots.

### Troubleshooting

If you see a warning about missing custom app icons, keep using `--no-icon`. It is expected and does not block APK generation.

If release builds fail with an LTO/dynamic library conflict, make sure `[profile.release]` in `Cargo.toml` uses `lto = false`. The Makepad Android wrapper builds the app as a dynamic library, and that path currently cannot use release LTO.
