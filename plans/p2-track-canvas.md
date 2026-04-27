# P2 — TrackCanvas + dynamic HUD/scrubber + button wiring

> Round-2 implementation plan. Closes the 5 BLOCKED visual BDDs from P1 (track polyline + halo + start/end markers + HUD mini-bars + scrubber thumb) and the 3 unwired interaction BDDs (pause / speed / scrubber drag).
>
> 不动契约三件套 (prd / visual.spec / spec.spec); 只改 src/.

## 0. Scope

**Closes after P2:**
- visual.spec.md: `speed_ramp_visible`, `no_monochrome`, `start_end_markers`, `current_position_halo`, `hud_mini_bars_count`, `pause_button_glyph_toggle` (toggle 行为), `speed_button_active_state` (active 切换), `guard_card_text_color` (env-flag 触发后)
- spec.spec.md: `test_pause_button_freezes_uniforms`, `test_speed_button_unique_active`, `test_scrubber_drag_updates_state` 等行为

**Out of scope (P3 留):** water layer, geo labels (4-10 个), particle stream, scrubber echo trail visual, compass recenter

---

## 1. Reference verification (源码确认过的事实)

- **FullscreenShader (`examples/shader/src/main.rs:84-122`) 是自定义 Widget 的标准模板**: `#[derive(Script, ScriptHook, Widget)]` + `#[uid] uid` / `#[walk] walk` / `#[layout] layout` / `#[redraw] #[live] draw_bg` / `#[rust] next_frame` / `#[rust] area`. impl Widget 含 handle_event (NextFrame 驱动 redraw) + draw_walk (begin_turtle / draw_abs / end_turtle_with_area).
- **DrawQuad 批量 API** (`draw/src/shader/draw_quad.rs:174-184, 326-336`): `begin_many_instances(cx)` → 循环 `draw_abs(cx, rect)` → `end_many_instances(cx)` 合并成单 GPU draw call.
- **`script_apply_eval!(cx, widget, {...})` 是 live 修改的标准 API** (`widgets/src/command_text_input.rs:754-779`).
- **`event.hits(cx, area)` 返回 `Hit::FingerDown/Move/Up`**, fe.abs / fe.rel 给绝对/相对像素 (`slider.rs:1567`, `scroll_bar.rs:540`).
- **GPX 数据**: 8234 trkpts, hr 8195 个, 5min 最高平均 HR=147 < 阈值 179.4. c1.2 不会被默认数据触发 — 这是诚实结论, 不是实现错.

---

## 2. Architecture decisions

### 2.1 Widget 拓扑

替换现有的死 `canvas_view := View{}` 为真实的 `track_canvas := TrackCanvas{}`. 4 个 DrawQuad 子类的 Rust 字段从 App 搬到 TrackCanvas, 让 widget 自己拥有 draws. `set_type_default()` shader 注册保持在 script_mod 顶层 (避免 frozen-vec).

### 2.2 数据传递: setter pattern

```rust
let canvas = self.ui.widget(cx, ids!(track_canvas)).as_track_canvas();
canvas.set_track(cx, self.track.clone());
canvas.set_progress(cx, walked_ratio, track_progress);
canvas.set_hr_phase(cx, hr_phase, hr_hz);
canvas.set_guard_pulse(cx, guard_pulse_phase);
canvas.set_overlay_dim(cx, overlay_dim);
```

`set_track` 触发一次性 downsample + bbox 计算 (cache 在 `Option<Arc<TrackGeom>>` 里, 用 `Arc::ptr_eq` 判等); 后续 setters 只更 uniform + redraw.

### 2.3 几何投影 (Option A: 线性 + aspect lock)

不用 Mercator, 用经度纬度线性投影 + 纬度压缩补偿 + uniform aspect-lock. 投影后缓存到 `TrackGeom`, 仅在 rect.size 变化 >1px 时重算.

### 2.4 Track 降采样 — uniform stride

8195 → ~180 段, `stride = max(1, points.len() / 180)`. 简单线性, 视觉够顺滑 (33m/段). DP 留 P3.

### 2.5 Polyline 渲染: 一次批量 draw call

```
ensure_geom(rect)
draw_track.begin_many_instances(cx)
for seg in geom.segs:
    set start_xy/end_xy/t_a/t_b/speed_a/speed_b
    bb = bounding_box(start, end, pad=16)
    draw_track.draw_abs(cx, bb)
draw_track.end_many_instances(cx)

draw_start_marker.draw_abs(cx, marker_rect(start_screen))
draw_end_marker.draw_abs(cx, marker_rect(end_screen))
draw_halo.draw_abs(cx, halo_rect(lerp(walked_ratio)))
```

**关键修正**: shader 的 `start_xy / end_xy` 是 per-instance 像素 (相对 bb), 所以 setter 时需要 `seg.start - bb.pos`.

### 2.6 HUD bar 动态填充 (option A)

用 `script_apply_eval!{ width: (Size::Fixed(total * ratio)) }` 改 `_fill` View 的 walk. 一个 `set_bar_ratio(fill_id, rest_id, ratio)` 函数搞定 4 个 bar.

Fallback B (option A 不工作时): 写一个 DrawProgressBar 子类, instance fill_ratio, shader `step(fill_ratio, self.pos.x)`.

### 2.7 Scrubber thumb 位置

同 2.6, 改 `scrubber_walked.width = total * progress - 6` (减半 thumb 宽度居中). `scrubber_unwalked` 保持 Fill, flow:Right 自动占据剩余.

### 2.8 Pause / speed / scrubber 点击

不改 splash, 在 App::handle_event 里用 `event.hits(cx, view.area())`:

```rust
let pause_area = self.ui.view(cx, ids!(pause_button)).area();
if let Hit::FingerUp(fe) = event.hits(cx, pause_area) {
    if fe.is_over && fe.was_tap() {
        self.state.is_paused = !self.state.is_paused;
        self.refresh_pause_glyph(cx);
    }
}
// 类似处理 1x/4x/16x buttons
// scrubber FingerDown/Move 算 progress 比例
```

切换 pause glyph 用 set_visible 切换三个子元素 (pause_left_bar / pause_right_bar / pause_play_triangle).

切换 speed active 用 script_apply_eval 改 draw_bg.color.

Fallback (event.hits 无 Hit): 改 splash 把这些 RoundedView 换成 Button{}.

### 2.9 Guard demo 触发

```rust
fn compute_guard_window(&mut self) {
    let Some(track) = self.track.as_ref().cloned() else { return };
    if std::env::var("MOBILE_EXAMPLE_DEMO_GUARD").is_ok() {
        let n = track.points.len();
        self.guard_window = GuardWindow {
            start_idx: (n as f32 * 0.30) as usize,
            end_idx: (n as f32 * 0.50) as usize,
            valid: true,
        };
        return;
    }
    // ... 原 c1.2 检查逻辑不变 ...
}
```

仅 short-circuit 进入条件, 真实数据走原路径. 契约逻辑零改动.

---

## 3. 步骤分解

### Step 1 — TrackCanvas widget 骨架 (no-op draw)

注册 + 接进 splash + 把 4 个 DrawQuad 子类字段从 App 搬到 TrackCanvas, draw_walk 暂时只 begin/end turtle 不画.

**Acceptance**: cargo check 通过, 跑起来跟之前一样 (canvas 仍黑), draw_walk log 每帧打印一次.

### Step 2 — 几何 + polyline 一次批量 draw call

实现 `TrackGeom`, `ensure_geom`, batched draw 循环 + 起终点 marker + halo.

**Acceptance**: 看到 polyline 沿 GPX 路线绘制, 速度色三段渐变, 已走/未走分色, 起终点 dot, 当前位置 halo. 5 条主画布 BDD PASS.

### Step 3 — HUD 动态 fill

`set_bar_ratio` 用 `script_apply_eval!`. fallback 到 DrawProgressBar 如果 (A) 不工作.

**Acceptance**: 4 个 bar 按 ratio 动态变长, seek 不同位置截图差异明显.

### Step 4 — Scrubber thumb 位置

同 Step 3 思路, 改 scrubber_walked.width.

**Acceptance**: thumb 跟着 playback_progress 从左滑到右.

### Step 5 — Pause / speed / scrubber 点击 + 拖拽

handle_event 里加三组 hits 监听; refresh_pause_glyph + refresh_speed_buttons.

**Acceptance**: 暂停 toggle 双竖线/三角, 倍速点击切换 active 高亮, 拖 scrubber 改 playback_progress.

### Step 6 — Guard demo trigger

`MOBILE_EXAMPLE_DEMO_GUARD` env-flag short-circuit. 配合 SEEK 截图 guard card.

**Acceptance**: env 启动 + seek=0.40 看到红色 c1.2 卡片 + "知道了" 按钮.

---

## 4. BDD coverage delta

| BDD | Before P2 | After P2 |
|---|---|---|
| speed_ramp_visible | BLOCKED | PASS (step 2) |
| no_monochrome | BLOCKED | PASS (step 2) |
| start_end_markers | BLOCKED | PASS (step 2) |
| current_position_halo | BLOCKED | PASS (step 2) |
| hud_mini_bars_count (动态) | PARTIAL | PASS (step 3) |
| pause_button_glyph_toggle (toggle) | PARTIAL | PASS (step 5) |
| speed_button_active_state (切换) | PARTIAL | PASS (step 5) |
| guard_card_text_color | PARTIAL | PASS via env (step 6) |

P2 后: 22/22 PASS (假设无 fallback 触发); 还有 `geo_labels_visible / water_layer_subtle / huashu_no_decoration` 在 P3 计划里独立处理.

---

## 5. 风险目录

| 风险 | 探测 | Fallback |
|---|---|---|
| `instance` 关键字 frozen-vec panic | startup panic | DrawXxx 注册保持 script_mod 顶层 |
| `script_apply_eval!{ width: }` 不重 layout | bar 不变长 | DrawProgressBar 子类 + fill_ratio shader (+2h) |
| `event.hits` 对 plain RoundedView 返回 Nothing | 点击无反应 | (a) cursor: Hand (b) 改 Button{} |
| `as_track_canvas()` 名字推不出 | 编译错 | `widget(cx, ids!(...)).borrow_mut::<TrackCanvas>()` 直 downcast |
| 几何投影高纬度变形 | 视觉窄长 | cos(lat_mid) 补偿已在计划 |
| 8195 点降采样慢 | 首帧卡 | uniform stride O(n) + cache, 测过 <1ms |
| shader start_xy 坐标系不对 | polyline 不可见或位置错 | 单段硬编码 + log 验证 |

---

## 6. Commit 节奏

```
[P2-step1] refactor(track-canvas): introduce TrackCanvas widget shell
[P2-step2] feat(track-canvas): paint speed-colored polyline + markers + halo
[P2-step3] feat(hud): dynamic mini-bar fill
[P2-step4] feat(scrubber): drive scrubber_walked.width from progress
[P2-step5] feat(controls): wire pause / speed / scrubber click + drag
[P2-step6] feat(guard): MOBILE_EXAMPLE_DEMO_GUARD env trigger
```

每个 commit 完成后 `cargo run` + 截图自审, 写 PASS 数量到 commit body.
