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
- `design/auto/` — 跑 demo 时 `makepad-screenshot` skill 自动落盘的截图, 不入 git

## 实现使用的 skill 清单

软链在 `.claude/skills/` 下 16 个 skill, 来自两个 upstream 仓库:

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

### 截图自动调试 (来自 ZhangHanDong/makepad-component 1 个)

| skill | 用途 | 必要性 |
|---|---|---|
| `makepad-screenshot` | 自动 `ps aux 找进程 → osascript 置顶 → screencapture -x → Read 工具读 PNG → 对照 visual.spec.md 22 条 BDD` 闭环 | ★★★ 必读 (一天工时内做 5-10 轮视觉迭代靠它) |

## skill 触发约定

- `/screenshot` 或中文 "截图" / "看看 UI" / "查看界面" → 调用 `makepad-screenshot`
- `/run-and-capture <package>` → 调用 `makepad-screenshot` 完整 build + run + capture 流程
- 遇到 shader 编写需求 → 优先读 `xor-shader-techniques` + `makepad-2.0-shaders`
- 遇到 widget 拼装需求 → 读 `makepad-2.0-widgets` + `makepad-2.0-layout`

## macOS 权限前置

`makepad-screenshot` skill 实际跑起来需要 3 个 macOS 系统权限:

| 权限 | 触发命令 | 在哪批 | 当前状态 |
|---|---|---|---|
| 屏幕录制 | `screencapture -x` | 系统设置 → 隐私与安全性 → 屏幕录制 | ✅ 已授权 (2026-04-27 验证通过, 实测 1.0M PNG 落盘) |
| 自动化 (Apple Events) | `osascript ... tell System Events` | 系统设置 → 隐私与安全性 → 自动化 | ✅ 已授权 (实测可读 process name) |
| 辅助功能 | 同上, 部分 osascript 需要 | 系统设置 → 隐私与安全性 → 辅助功能 | ✅ 默认通过 |

## 截图尺寸约束 (硬性, 防 context 爆炸)

cc 用 `Read` 工具读截图前**必须先用 `sips` 降采样**。Mac 默认 Retina 屏截图 3840×2160, 单张 ≈1MB, 直接 Read 会消耗 30-50k tokens, 几张就把 context 装满。

### 标准降采样命令

```bash
sips -Z 800 -s formatOptions 70 input.png --out output_800w.png
# -Z 800       最长边压到 800px (保持长宽比)
# formatOptions 70  PNG 优化质量
# 实测: 3840×2160 / 1.0MB → 800×450 / 123KB, 视觉信息无损
```

### 截图工作流模板 (cc 必须按此顺序)

```bash
SCRATCHPAD="/Users/zhaoyue/workspace/matrix/mobile_example/design/auto"
RAW="$SCRATCHPAD/raw_$(date +%H%M%S).png"
SMALL="${RAW%.png}_800w.png"

# 1. 把 makepad 窗口提到最前 (否则截到的是 IDE / 浏览器)
#    进程名按需替换: 实际跑 `cargo run -p mobile_example` 时进程名一般是
#    "mobile_example" 或 "makepad" 开头, 先用 ps aux 确认一次。
APP_NAME="mobile_example"   # ← 按当前 demo 二进制名改
osascript -e "tell application \"System Events\" to set frontmost of (first process whose name contains \"$APP_NAME\") to true"
sleep 0.3   # 给窗口提前一帧时间, 不然偶尔会截到过渡态

# 2. 截图原图 (-x 静默, 无快门音)
screencapture -x "$RAW"

# 3. 立即降采样
sips -Z 800 -s formatOptions 70 "$RAW" --out "$SMALL"

# 4. 删除原图 (省磁盘 + 防误 Read)
rm "$RAW"

# 5. Read 降采样版本
# Read $SMALL
```

**禁止** Read 任何未经 `sips -Z 800` 处理的原始截图。如果截图来自 Android `adb shell screencap` 同样要先降采样。

**强制** 截图前必须先 osascript 把目标窗口提到最前。看到 demo 跑起来不等于它在最前 — IDE / 浏览器 / Finder 任何一个挡住都会让截图作废, 自审会跑偏。如果 osascript 找不到进程 (报 -1719 invalid index), 说明 demo 根本没启动, 先回去查 `cargo run` 是不是崩了, 不要在没截到目标窗口的情况下声称 BDD 通过。

## skill 来源 worktree

- makepad-component 的 skill 通过 `git worktree` 拉到 `~/.claude/skills-source/makepad-component-zhanghandong` (pinned to `zhanghandong/main`)
- 更新方法: `git -C ~/.claude/skills-source/makepad-component-zhanghandong fetch zhanghandong main && git -C ~/.claude/skills-source/makepad-component-zhanghandong checkout zhanghandong/main`
- makepad-skills 的 14 个 skill 直接软链自 marketplace cache (`~/.claude/plugins/marketplaces/makepad-skills/skills/`)

## 工作流推荐

1. **理解阶段** — 读 prd.md + visual.spec.md + spec.spec.md, 确认契约
2. **实现阶段** — 按 skill 必要性 ★★★ 顺序读 SKILL.md, 然后写代码
3. **验收阶段** — 跑 `/run-and-capture` 截图, Read 截图, 对照 visual.spec.md 22 条 BDD 自审
4. **迭代阶段** — diff 出来后调 shader uniform 或 token, 重跑 → 重截 → 再对照
