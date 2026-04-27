# 实现笔记 / Implementation Notes

> 自主 cc session 完工记录, 给下一个接手者 (人或 agent) 看. 不是产品文档, 是
> 决策档案 — 记录"为什么这样做"和"哪些坑踩过".

---

## 一句话状态

P0 (kickoff.md 同步屏 + S2 主回放骨架) + P1 (S3 Guard + S4 stats + 数据驱动) +
P2 (TrackCanvas widget 真正绘制 polyline + markers + halo + HUD 动态 fill +
scrubber thumb + click handlers + guard env trigger) 全部完成.
P3 (向参考图视觉趋近) 完成 5/6.

22 视觉 BDD: 17 PASS / 5 BLOCKED (主画布相关 4 + Guard 1)
→ 22 视觉 BDD: 22/22 PASS (经过 P2/P3) [需复审]

唯一未做: 起终点 / 地名 *文字* labels — 因 DrawText 字体加载链未跑通.

---

## 关键决策档案

### 1. Splash 关键字 `instance` 在此 makepad 分支不可用

**事实**: kevinaboos/makepad@5e6d7b3 的 `script_mod!{}` 块中, 无论是 `+:` 扩展还
是 `set_type_default() do #(Foo::script_shader(vm))` 注册块, 写 `instance xx: 0.`
都会运行时 panic "cannot push to frozen vec".

**为什么**: instance vec (per-vertex GPU 属性布局) 在 Rust derive 时就被冻结. Splash
时已不可扩展.

**解决**: per-instance 数据全部用 `#[derive(Script, ScriptHook)]` `#[repr(C)]`
+ `#[live]` Rust 字段表达. 这些字段自动出现在 GPU vertex layout 里. Splash 端只
做 uniform 初始化 (无 instance 关键字).

### 2. `fwidth` shader 内建在此分支不存在

**事实**: 任何 pixel: fn() 里调 `fwidth(d)` 都报 "variable fwidth not found in
scope".

**解决**: SDF 用像素空间坐标 (`p = self.pos * self.rect_size`), 1px AA 用
`alpha = clamp(0.5 - d, 0., 1.)` 代替 `clamp(0.5 - d/fwidth(d), 0., 1.)`. 牺牲
极端缩放下的精度, 但单屏 demo 视觉无差别.

### 3. 字体不渲染 ⏸ U+23F8

**事实**: 默认字体不包含 U+23F8 (双竖线), Label 显示空块.

**解决**: 用 2 个 RoundedView 矩形 (4×14, radius 1) 拼出 "双竖线". 字体无关.
配套切换: paused 状态隐藏两矩形, 显示 "▶" Label (基本 ASCII 字形可渲染).

### 4. 自定义 Widget 模板 = FullscreenShader pattern

**位置**: `~/.cargo/git/checkouts/makepad-69d78fae78fc8901/5e6d7b3/examples/shader/src/main.rs:84-122`

**关键字段** (TrackCanvas 直接照搬):
```rust
#[derive(Script, ScriptHook, Widget)]
pub struct TrackCanvas {
    #[uid] uid: WidgetUid,
    #[walk] walk: Walk,
    #[layout] layout: Layout,
    #[redraw] #[live] draw_track: DrawTrack,  // ← 必须有 #[redraw] 标记主 draw
    #[live] ... 其他 draw 子类,
    #[rust] ... 状态字段,
    #[rust] area: Area,
}
```

**Splash 注册**:
```
let TrackCanvasBase = #(TrackCanvas::register_widget(vm))
let TrackCanvas = set_type_default() do TrackCanvasBase{
    width: Fill height: Fill
}
```

**App→Widget 数据传递**: `WidgetRef::borrow_mut::<T>()` 拿 `Option<RefMut<T>>`,
直接调 setter 方法.

### 5. 动态宽度 = `View::walk.width` 直改

**做法**: `ViewRef::borrow_mut()` (注意非泛型) → `view.walk.width = Size::Fixed(N)`
→ 下一帧 layout 自动 pick up.

**1px 保护**: 首帧 `area().rect()` 尺寸是 (0,0), 必须 `if total > 1.0 { return; }`
跳过, 等下一 NextFrame 触发后再算.

**应用**: 4 个 HUD bars + scrubber_walked 全部走这条路, set_bar_ratio + set 在
scrubber 部分.

### 6. 点击监听 = `event.hits(cx, view.area())`

**做法**: 不改 Splash widget tree, 直接在 App::handle_event 里:
```rust
let area = self.ui.view(cx, ids!(pause_button)).area();
if let Hit::FingerUp(fe) = event.hits(cx, area) {
    if fe.is_over && fe.was_tap() { ... }
}
```

**为什么不用 Button{}**: pause_button / speed_*_button 是 RoundedView (要做特殊
glyph + 自定义 active 高亮), 转 Button 会丢掉这些自由度. event.hits 在 RoundedView
上工作良好.

**Scrubber 拖拽**: 同样路径, 但用 `Hit::FingerDown` + `Hit::FingerMove` 持续算
`fe.abs.x - r.pos.x` 比例.

### 7. 修改 draw_bg.color 在运行时 = `set_uniform(live_id!(color), &[r,g,b,a])`

**坑**: `view.borrow_mut::<View>()` 得到的 View 没有直接的 `draw_bg.color` 字段
(`DrawQuad` 没 `color`, color 是 splash 注册的 uniform).

**做法**:
```rust
view.set_uniform(cx, live_id!(color), &[0.29, 0.38, 0.85, 1.0]);
```

**应用**: speed_*_button active 切换 + sync_badge_dot 状态颜色切换.

### 8. Guard 演示触发 = env-flag short-circuit

**做法**: `MOBILE_EXAMPLE_DEMO_GUARD=1` 启动时, `compute_guard_window` 顶层 short-
circuit 注入 30%-50% 伪窗口. 真实 c1.2 阈值检查路径完全不变.

**为什么**: 默认 GPX 最高 5min 平均 HR 147 bpm, 阈值 195*0.92=179.4, 不会触发.
但视觉契约要 review 卡片样式, 必须有手段触发.

**契约安全**: env-flag 仅前置短路, 真实数据路径无任何修改. 没有任何 spec.spec.md
场景测试这条路径. 未来可加 `assert!(env_var_unset())` 在 release build 里.

### 9. 演示快进 = env-flag SEEK

**做法**: `MOBILE_EXAMPLE_DEMO_SEEK=0.50` 启动时, `state.playback_progress` 跳到
指定值. 不影响普通跑.

**为什么**: 默认 651:08 总时长在 4x 速度下要跑 41 分钟才能验收 S4 stats overlay.
SEEK=0.99 让 stats 在几秒内进入.

---

## 已知 BLOCKED / 未做

### B1. 起终点 + 地名 *文字* labels (BLOCKED)

**问题**: TrackCanvas 加 `#[live] draw_label: DrawText` + Splash type_default 设
`color` + `text_style.font_size` 后, `draw_label.draw_abs(cx, pos, "起点")` 静默
不渲染.

**根因猜测**: DrawText 需要完整的 font_family 继承链 (mod.theme.font_family 等),
单设 color/font_size 不够. Label widget 工作是因为它从 `mod.widgets.Label` 继承
全套 text_style.

**Workaround 已用**: 起终点改用 7px 大 marker (绿 / 白), 视觉上能区分.

**未来修复路线** (按优先级):
1. 把 labels 提到 Splash `main_stack` overlay 层, 用 `Label{ margin: Inset{...} }`,
   每帧 Rust 计算 margin 推过去 (`label.borrow_mut().walk.margin = Margin::...`).
   优点: 直接复用 Label 完整字体链.
2. 写 DrawTextWithFont 子类, splash 注册时显式继承 `..mod.theme.font_family`.
3. 嵌入 DrawSvg + SVG 字体. 最重最干净.

### B2. HUD cells icons (SKIPPED)

参考图每个 HUD cell 有 icon (速度计 / 心 / 山 / 齿轮). 当前只有文字 ("速度 km/h"
等). spec 不要求 icons, 仅参考图加分项.

**未来路线**: 用 4 个 SVG 资源 + DrawSvg 子类. 或简化版: Unicode 几何字符
("▲", "♥") 但风险同 B1 (默认字体不全).

### B3. 真实地图背景 (SKIPPED)

参考图有地图网格 / 道路 / 建筑. 当前只有 DrawWater (右半模糊水域矩形). 实现需要:
Mapbox style JSON parse + tile renderer 或简化 SVG path 集合. 远超 demo scope.

### B4. Path-draw 动画的视觉效果

逻辑上 PATH_DRAW phase 在 3 秒内 track_progress 0→1, shader 里有
`step(t_mid, walked_segment_ratio)` 切换. 但实际转换感不强 — 因为整个 polyline
本就一直绘制, 只是颜色从 unwalked gray 切到 speed_color.

**未来增强**: 在 pixel: fn() 里加 `if t_b > self.track_progress { discard }`,
让 polyline 真的"由起点画到终点", 而不是颜色刷新.

---

## 视觉对照状态 (vs 用户提供的参考图)

| 元素 | 参考图 | 当前实现 | 差距 |
|---|---|---|---|
| 主画布 polyline | ✅ 蓝橙渐变 + 当前位置 halo | ✅ 已实现 | OK |
| 速度色 3 段 | ✅ 三色 ramp (低白/中橙/高青) | ✅ 已实现 | OK |
| 起终点 marker | ✅ 绿圆 (起) + 黑白棋盘旗 (终) + 文字 | ⚠️ 大圆 dot, 无文字 | B1 |
| 地名 labels | ✅ 钱塘江/钱塘新区/之江路... | ❌ 无 | B1 |
| 水域层 | ✅ 暗蓝河流形状 | ⚠️ 暗蓝矩形 (右半) | 形状简化 |
| 顶栏 | ✅ ←箭头+路名 + 单独行 pill | ⚠️ 路名+pills 同行 (按 spec 强制) | 契约要求 |
| profile pill | ✅ 🚴 Cycling | ⚠️ 骑行 (按 spec 中文化要求) | 契约要求 |
| sync 状态 pill | ✅ ✓ 已同步 (绿) | ✅ ● 状态 + 文字 | OK |
| HUD 4 cells | ✅ icon + 数字 + 单位 | ⚠️ 文字标签 + 数字 | B2 |
| 速度图例 | ✅ 0/5/10+ 渐变 | ✅ 0/N 渐变 | OK |
| compass + 2D | ✅ 圆形按钮 | ✅ 圆角矩形 | 形状简化 |
| 暂停按钮 | ✅ 圆形 ⏸ | ✅ 圆形双竖线 / ▶ | OK |
| 倍速按钮 | ✅ 1x/4x/16x, 4x 鲜蓝 | ✅ 已实现, 4x #x4A60D9 | OK |
| Scrubber | ✅ 时间 + 蓝白 progress + thumb | ✅ 全实现 | OK |

---

## 测试 / 验收

```bash
cd /Users/zhaoyue/workspace/matrix/mobile_example

# 普通跑
cargo run

# 跳到特定 progress 截图
MOBILE_EXAMPLE_DEMO_SEEK=0.50 cargo run

# 触发 contract guard 卡片
MOBILE_EXAMPLE_DEMO_GUARD=1 MOBILE_EXAMPLE_DEMO_SEEK=0.40 cargo run

# 跳到 stats overlay
MOBILE_EXAMPLE_DEMO_SEEK=0.998 cargo run
```

**截图工作流**: 见 CLAUDE.md "截图工作流模板" — osascript 提前 + take_screenshot.py
+ sips -Z 800. design/auto/ 不入 git.

---

## Commit 历史 (本轮)

```
135f05d feat(track-canvas): bigger 7px markers; remove non-rendering DrawText path [P3-C BLOCKED]
11dee65 feat(visual): water layer + active button bright + pill badges [P3-B+D+F partial]
c4ba409 feat(guard): MOBILE_EXAMPLE_DEMO_GUARD env trigger [P2-step6]
72f2602 feat(controls): pause + speed + scrubber click/drag wiring [P2-step5]
4bdba23 feat(hud+scrubber): dynamic bar fills + scrubber thumb position [P2-step3+4]
a7dc43e feat(track-canvas): paint speed-colored polyline + markers + halo [P2-step2]
3b8fd2e refactor(track-canvas): introduce TrackCanvas widget shell [P2-step1]
0c905e0 docs(plan): P2 implementation roadmap
76d91d8 feat(S3+S4): Guard 翻可见 + 收尾 stats 4 cells (BDD 17/22 PASS, 5 BLOCKED)
d4fecbe feat(S0+S2): 同步屏 + 主回放骨架, 视觉 BDD 13/22 通过
```

每个 commit 单独可 revert. 视觉证据在 design/auto/ (不入 git).
