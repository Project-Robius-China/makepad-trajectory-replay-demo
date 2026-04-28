# Agent 工作上下文

> 本文件是给 cc / Codex / Cursor 等 coding agent 看的项目工作指南, **不是** prd / spec / 视觉契约。

## 真值三件套 (优先级从高到低)

1. `visual.spec.md` — 视觉设计 BDD 契约 (22 场景, Quality 100%)
2. `spec.spec.md` — 功能行为 BDD 契约 (34 场景, Quality 100%)
3. `prd.md` — 产品需求文档 (含 §6 视觉锚定, §6.4 真值优先级条款)

实现轮中, 任何二义性以 `visual.spec.md` > `prd.md` §6 > `design/refs/*.png` 顺序裁决。

## 视觉参考底板

- `design/refs/storyboard-1.png` — S0 同步 / S1 path-draw / S3 Guard / S4 stats 4 屏拼图
- `design/refs/storyboard-2.png` — S2 主回放 / S3 Guard / S4 stats 3 屏拼图
- `design/auto/` — 跑 demo 时 `screenshot` skill 工作流落盘的截图 (raw + 默认 1200w 降采样版本), 不入 git

## 实现使用的 skill 清单

软链在 `.Codex/skills/` 下 16 个 skill, 来自两个 upstream 仓库:

### Makepad 框架核心 (来自 ZhangHanDong/makepad-skills 14 个)

| skill | 用途 | 必要性 |
|---|---|---|
| `makepad-2.0-design-judgment` | Elm 架构 + GPU 渲染基础观念 | ★★★ 必读 |
| `makepad-2.0-app-structure` | App 初始化 / 热重载 / Cargo 配置 | ★★★ 必读 |
| `makepad-2.0-dsl` | live DSL 语法与绑定 | ★★★ 必读 |
| `makepad-2.0-shaders` | shader 系统与 GPU 渲染 | ★★★ 必读 (本 demo 重 shader) |
| `makepad-2.0-widgets` | View / Button / Label / PortalList / Dock | ★★ 拼装 HUD / 顶栏 |
| `makepad-2.0-layout` | Flow / Fill / Fit / Inset / spacing | ★★ HUD / 底栏 |
| `makepad-2.0-events` | 事件与 hit detection | ★★ scrubber / 暂停按钮 |
| `makepad-2.0-animation` | Animator / states / easing | ★★ guard 红边 pulse / path-draw |
| `makepad-2.0-vector` | SVG path / 渐变 / 矢量 | ★★ 速度图例 / 起终点 marker |
| `makepad-2.0-theme` | 主题系统 (本 demo 锁深色) | ★ |
| `makepad-2.0-performance` | 优化技巧 | ★ 50fps 验收 |
| `makepad-2.0-troubleshooting` | 调试与常见错误 | ★ |
| `makepad-2.0-splash` | splash 脚本 + 热重载 | ★ |
| `makepad-2.0-migration` | 1.x → 2.x 迁移 | (不需要) |

### Shader 高级技法 (来自 ZhangHanDong/makepad-component 1 个)

| skill | 用途 | 必要性 |
|---|---|---|
| `xor-shader-techniques` | 7 类 shader 技法: Turbulence / Efficient Chaos / Dot Noise / Fractal Texturing / fwidth Outlines / Volumetric Raymarching / Analytic Anti-Aliasing | ★★★ 必读 (轨迹 SDF / glow / bloom / 抗锯齿全部命中) |

### 截图调试 (通用 OS 截图 skill)

| skill | 用途 | 必要性 |
|---|---|---|
| `screenshot` | 跨平台桌面截图 helper (Python/PowerShell), 支持 `--app` / `--window-id` / `--region` / `--mode temp`, 输出原始 PNG | ★★★ 必读 (一天工时内做 5-10 轮视觉迭代靠它) |

> ⚠️ 注意: `screenshot` skill **本身不做置顶 + 降采样**, 这两步必须由 cc 在调用 helper 前后手动完成 (见下方"截图尺寸约束"和"截图工作流模板")。否则要么截到 IDE / Finder, 要么 Read 原图爆 context。

## skill 触发约定

- 中文 "截图" / "看看 UI" / "查看界面" / `/screenshot` → 调用 `screenshot` skill 的 `take_screenshot.py`, 套用本文档下方 "截图工作流模板" (osascript 置顶 → 截图 → sips 降采样 → Read)
- 遇到 shader 编写需求 → 优先读 `xor-shader-techniques` + `makepad-2.0-shaders`
- 遇到 widget 拼装需求 → 读 `makepad-2.0-widgets` + `makepad-2.0-layout`

## macOS 权限前置

`screenshot` skill (以及手动 `osascript` 置顶) 跑起来需要 3 个 macOS 系统权限:

| 权限 | 触发命令 | 在哪批 | 当前状态 |
|---|---|---|---|
| 屏幕录制 | `screencapture -x` / `take_screenshot.py` | 系统设置 → 隐私与安全性 → 屏幕录制 | ✅ 已授权 (2026-04-27 验证通过, 实测 1.0M PNG 落盘) |
| 自动化 (Apple Events) | `osascript ... tell System Events` | 系统设置 → 隐私与安全性 → 自动化 | ✅ 已授权 (实测可读 process name) |
| 辅助功能 | 同上, 部分 osascript 需要 | 系统设置 → 隐私与安全性 → 辅助功能 | ✅ 默认通过 |

> 一次性预检 (新 skill 自带): `bash .Codex/skills/screenshot/scripts/ensure_macos_permissions.sh` — 一次性请求屏幕录制权限。

## 截图尺寸约束 (硬性, 防 context 爆炸)

cc 用 `Read` 工具读截图前**必须先用 `sips` 降采样**。默认可用 1200w; 原始 Retina 全屏图仍会大量消耗 context, 所以禁止直接 Read 原图。

### 标准降采样命令

```bash
sips -Z 1200 -s formatOptions 75 input.png --out output_1200w.png
# -Z 1200      默认最长边压到 1200px (保持长宽比), 适合单张 UI 自审
# -Z 1600      细节检查可临时放大到 1600px, 但一次只 Read 1 张
# -Z 800       多图对比或连续迭代时使用, 防止 context 膨胀
# formatOptions 75  PNG 优化质量
```

### 截图工作流模板 (cc 必须按此顺序)

新 `screenshot` skill **不做**置顶和降采样, 所以这两步要在调用 helper 前后手动加。

```bash
SCRATCHPAD="/Users/zhaoyue/workspace/matrix/mobile_example/design/auto"
RAW="$SCRATCHPAD/raw_$(date +%H%M%S).png"
SMALL="${RAW%.png}_1200w.png"

# 1. 把 makepad 窗口提到最前 (否则截到的是 IDE / 浏览器)
#    进程名按需替换: 实际跑 `cargo run -p mobile_example` 时进程名一般是
#    "mobile_example" 或 "makepad" 开头, 先用 ps aux 确认一次。
APP_NAME="mobile_example"   # ← 按当前 demo 二进制名改
osascript -e "tell application \"System Events\" to set frontmost of (first process whose name contains \"$APP_NAME\") to true"
sleep 0.3   # 给窗口提前一帧时间, 不然偶尔会截到过渡态

# 2. 截图 — 默认走原生 screencapture (-x 静默, 无快门音, 不依赖 swift toolchain)
screencapture -x "$RAW"

# 可选增强 (要求 macOS Swift toolchain 能跑通, 否则跳过):
#   python3 .Codex/skills/screenshot/scripts/take_screenshot.py --app "$APP_NAME" --path "$RAW"
# 优势: 直接抓单窗口 (不用截全屏后裁剪), 多显示器分文件落盘, 错误信息更友好。
# 如果 helper 报 swiftc / "Swift toolchain mismatch" 错误, 不要花轮次诊断, 直接用上面的 screencapture -x。
# 一次性修法 (不阻塞当前任务): sudo rm -rf /Library/Developer/CommandLineTools && sudo xcode-select --install
#
# 多窗口陷阱: --app 命中 N 个窗口时, helper 会输出 N 个 "${RAW%.png}-w<id>.png" 文件,
# 而 $RAW 本身**不存在**, 后面 sips "$RAW" 会报"找不到文件"。两种处置:
#   (a) 锁主窗口 — 先 list-windows 选面积最大的那条 ID, 再用 --window-id 单窗截:
#       WIN_ID=$(python3 .Codex/skills/screenshot/scripts/take_screenshot.py \
#                 --list-windows --app "$APP_NAME" 2>&1 \
#                 | awk '{ split($4, a, "[x+-]"); print a[1]*a[2], $1 }' \
#                 | sort -rn | head -1 | awk '{print $2}')
#       [ -z "$WIN_ID" ] && { echo "❌ ABORT: $APP_NAME 窗口不存在, 别盲截"; exit 1; }
#       python3 .Codex/skills/screenshot/scripts/take_screenshot.py --window-id "$WIN_ID" --path "$RAW"
#   (b) 全抓后挑 — 截多张, 用 ls -S 取面积/字节最大的:
#       python3 .Codex/skills/screenshot/scripts/take_screenshot.py --app "$APP_NAME" --path "$RAW"
#       RAW=$(ls -S "${RAW%.png}"-w*.png 2>/dev/null | head -1)
#       [ -z "$RAW" ] && { echo "❌ ABORT: 没有窗口被捕获"; exit 1; }
# 推荐用 (a), 因为它在 demo 没起来时**直接报错**, 不会偷偷退化截桌面。

# 3. 立即降采样 — 强制! 默认 1200w; 多图用 800w, 细节单图可临时 1600w
sips -Z 1200 -s formatOptions 75 "$RAW" --out "$SMALL"

# 4. 删除原图 (省磁盘 + 防误 Read)
rm "$RAW"

# 5. Read 降采样版本
# Read $SMALL
```

**禁止** Read 任何未经 `sips` 降采样处理的原始截图。默认使用 `sips -Z 1200 -s formatOptions 75`; 多图对比使用 `-Z 800`; 细节单图检查允许临时使用 `-Z 1600`。如果截图来自 Android `adb shell screencap` 同样要先降采样。

**强制** 截图前必须先 osascript 把目标窗口提到最前。看到 demo 跑起来不等于它在最前 — IDE / 浏览器 / Finder 任何一个挡住都会让截图作废, 自审会跑偏。如果 osascript 找不到进程 (报 -1719 invalid index), 说明 demo 根本没启动, 先回去查 `cargo run` 是不是崩了, 不要在没截到目标窗口的情况下声称 BDD 通过。

## skill 来源 worktree

- `xor-shader-techniques` 通过 `git worktree` 拉到 `~/.Codex/skills-source/makepad-component-zhanghandong` (pinned to `zhanghandong/main`)
- 更新方法: `git -C ~/.Codex/skills-source/makepad-component-zhanghandong fetch zhanghandong main && git -C ~/.Codex/skills-source/makepad-component-zhanghandong checkout zhanghandong/main`
- makepad-skills 的 14 个 skill 直接软链自 marketplace cache (`~/.Codex/plugins/marketplaces/makepad-skills/skills/`)
- `screenshot` skill 是项目本地真目录 (`.Codex/skills/screenshot/`), helper 脚本软链到 `~/.codex/vendor_imports/skills/skills/.curated/screenshot/scripts/` (codex 共享 skill, 跨平台桌面截图)

## 工作流推荐

1. **理解阶段** — 读 prd.md + visual.spec.md + spec.spec.md, 确认契约
2. **实现阶段** — 按 skill 必要性 ★★★ 顺序读 SKILL.md, 然后写代码
3. **验收阶段** — 按 "截图工作流模板" 跑一遍 (osascript 置顶 → `take_screenshot.py` → sips 降采样 → Read), 对照 visual.spec.md 22 条 BDD 自审
4. **迭代阶段** — diff 出来后调 shader uniform 或 token, 重跑 → 重截 → 再对照
