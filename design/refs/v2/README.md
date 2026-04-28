# design/refs/v2 — P11.3 demo ground truth

本目录是 **P11 阶段后 demo 实际渲染的视觉真值**, 用于替代 `design/refs/storyboard-{1,2}.png` 旧 mockup 的"残留 4 处"问题(顶栏返回箭头 / profile "Cycling" 英文 / 主回放仅暖橙单色 / 主画布右上 "3D" 按钮)。

## 文件清单

| 文件 | 阶段 | 抓取方式 |
|---|---|---|
| `s2-main-replay.png` | S2 主回放 | `cargo run` + sleep 7s (S0 fallback 1.2s + S1 path-draw 3s + 进入 S2) |
| `s4-stats.png` | S4 stats 收尾 | `MOBILE_EXAMPLE_DEMO_SEEK=0.99 cargo run` + sleep 10s |

均为 **1200w 单屏 PNG**, 通过 `take_screenshot.py --app mobile_example` 抓取 + `sips -Z 1200` 降采样。

## 旧 mockup 残留 4 处对照

| 残留 | 旧 storyboard | 新 ground truth (s2-main-replay.png) |
|---|---|---|
| 1. 顶栏返回箭头 | sb1+sb2 都有 ← 箭头 | ✅ 顶栏无返回箭头, 仅路线名+徽章+profile |
| 2. profile 英文 "Cycling" | sb1+sb2 显示 "Cycling" | ✅ 显示中文 "骑行" |
| 3. 主回放仅暖橙单色 | mockup walked 段多为暖橙 | ✅ shader 实施三段色 (#E8E8F0/#FF8A3D/#00E5FF), 实际比例依 playback 进度 |
| 4. 主画布右上 "3D" | sb2 显示 "3D" 按钮 | ✅ 显示 "2D" 按钮 |

## 已知 P12 待续(基于 ground truth 与 visual.spec 对照)

- **S4 stats 屏 (s4-stats.png)** 与 visual.spec L444-465 BDD 有 gap:
  - 当前: "本次回放总览" 标题, 4 项无图标无 frosted glass
  - spec: "回放已完成" + ✓ 32dp checkmark + leading icons (📍⏱↗♥) + 半透明 bg_secondary @70% alpha 卡片
  - **P12 任务**: 在 main.rs stats_overlay widget 实施 spec L444-465 细节

## 限制

- cc 无 image montage 工具 (imagemagick / PIL 缺), **4-grid 拼图重画归设计师 follow-up**
- v2/ 是单屏 ground truth, 不替代 storyboard-{1,2}.png 的 4-grid/3-grid 拼图叙事
- S0 同步屏 / S1 path-draw / S3 contract guard 触发屏未单独 capture (S0 短/S1 快/S3 需双击触发)
