# S0 Freeze And Polish Implementation Plan

> **For Claude/Codex:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task.

**Goal:** Add a stable S0 demo mode so the app can be frozen on the sync screen for pixel-level visual adjustment and screenshot review.

**Architecture:** Keep the one-screen-at-a-time workflow. S0 freeze is an app-level demo mode, not a widget-local hack: `App` owns phase/network/progress, and widgets remain projections of that state. Use the existing `MOBILE_EXAMPLE_DEMO_*` pattern, then polish S0 visuals against `visual.spec.md` and `design/refs/v3/s0-sync-target-v2.png`.

**Tech Stack:** Rust, Makepad 2.0 `script_mod!`, `MatchEvent`, custom `TrackCanvas`, env-gated demo controls, `cargo run` screenshot workflow.

---

## 0. Current Inputs

**Spec truth:**
- `visual.spec.md:410-425` defines S0: Fetching state, faint dashed bundled fallback route, 48dp spinner, text `正在同步轨迹数据`, HUD dashes, play button `▶`, no path draw.
- 2026-04-28 design review overrides the dashed route treatment for now: the S0 placeholder should read as a smooth low-alpha route, not a dense sequence of source sample dots.
- `prd.md:166-172` defines S0/S1 timing, but for pixel work S0 must be manually frozen.

**Reference images:**
- `design/refs/v3/s0-sync-target.png` — first generated target, too bright and too generic.
- `design/refs/v3/s0-sync-target-v2.png` — preferred S0 target: closer to Makepad S2 style, 4x active, vertical Big Sur route.
- `design/refs/v3/s0-user-review-2026-04-28.png` — user's Desktop screenshot for current S0 review notes.
- `design/refs/v2/s2-main-replay.png` — existing app-rendered style anchor for map grid, labels, route geometry, HUD density.
- User review screenshot `Image #1` (2026-04-28): current S0 frozen render. Keep as review reference even if the chat attachment has no filesystem path in this session.

**User design feedback from Image #1:**
- Put `2019 BP USA #60 / Ragged Point - Carmel` into a card-like title container instead of free text.
- Title card text must stay dynamic: the card structure belongs in DSL, but `route_title_primary` and `route_title_secondary` are updated from the current GPX route name in Rust.
- S0 route placeholder is too point-like; it reads as many original sampled dots. It should be a smoother route line, not a bead chain.
- Prefer copying component patterns from `/Users/zhaoyue/workspace/matrix/Component/makepad-component`.
- Card style should follow `components/src/card/card.rs` patterns: `RoundedView`, `height: Fit`, `flow: Down`, padding, spacing, subdued card bg.
- HUD mini progress should later copy `components/src/progress/progress.rs` shader pattern.
- Playback scrubber should later copy `components/src/slider/slider.rs` structure so slider position and time are naturally linked.
- Avoid adding `makepad-components` as a direct dependency for now because it depends on a different Makepad branch; copy proven patterns locally unless/until Makepad versions are unified.

**2026-04-28 top/header refinement:**
- The top title is an example in review screenshots; production text must be the dynamic replay name from the current trajectory metadata.
- The header should read like the example: route/replay title as the primary item, profile chip below/near it, sync badge on the top right, and low-contrast sync subtext below.
- All card/chip surfaces in S0 should be copied from makepad-component card semantics, especially `MpCard`/`MpCardSmall`: `RoundedView`, `width: Fill/Fit`, `height: Fit`, `flow: Down`, `padding`, `spacing`, `new_batch`, subdued card color, small radius.
- The S0 spinner must be copied from makepad-component spinner semantics: 48dp `MpSpinnerXl`, ring track + active arc, `stroke_width: 5.0`, `arc_ratio: 0.25`. Current Makepad shader compatibility may require replacing `atan/mod` with an equivalent dot-product/time based arc, but the component size, color roles, stroke, and arc behavior should match.
- The sync spinner must be visually centered or slightly above center inside the map stage. The previous fixed 240px spacer pushed the spinner too low on the desktop screenshot; replace it with an overlay layout that positions the spinner group around 38-45% of the map height.

**2026-04-28 header alignment update:**
- Use the latest review screenshot as the S0 header target: two-line dynamic replay title on the left, sync badge and profile chip on the right, all vertically centered on the same row.
- Make title text smaller and denser: primary route line around 12px, secondary line around 12px with slightly muted color.
- Do not render phone chrome in Makepad: no status bar, time, battery, signal, or Android frame. The screenshot's phone chrome is only environmental context.
- Remove the extra sync subtext from the header. The sync explanation remains in the center spinner group.

**Component-source rule:**
- All visible S0 UI components must come from `/Users/zhaoyue/workspace/matrix/Component/makepad-component`.
- Current app cannot safely add `makepad-components` as a direct path dependency because that workspace pins `Project-Robius-China/makepad` while this app uses `kevinaboos/makepad:cargo_makepad_ndk_fix`; mixing them creates incompatible Makepad types.
- Until the Makepad branches are unified, "use makepad-component" means source-level porting: copy the component's structure, names, sizing, shader semantics, and state model into this app with only Makepad-version compatibility edits.
- Do not invent new card, badge, spinner, progress, slider, or button styling directly in the app when a matching makepad-component source exists.

**2026-04-28 HUD / bottom controls update:**
- The four HUD metric cells must be `MpCardSmall` source-level ports from `components/src/card/card.rs`, not bare `RoundedView`.
- The small colored meter inside each HUD card must use an `MpProgress` source-level port from `components/src/progress/progress.rs`: capsule track, fill by `progress`, `height: 4`, muted track color, metric-specific fill color.
- The bottom playback scrubber must use an `MpSlider` source-level port from `components/src/slider/slider.rs`: 48dp hit area, 4-6px capsule track, circular thumb, left time in `#xF5F5FA`, right duration in `#xD4D5DD`, font around 12px.
- Bottom layout target: first row is `00:00 [slider] total`, second row centers `1x / 4x / 16x` and play button. Speed buttons should use `MpButtonSmall` source-level semantics from `components/src/button/button.rs`: compact rounded rectangle, centered label, active `4x` cyan, inactive transparent/dark with border.
- The play/pause circle must use makepad-component button semantics where possible: 48dp hit area, circular visual, centered icon/glyph, no ad-hoc phone chrome.
- Runtime shader compatibility note: current `kevinaboos/makepad:cargo_makepad_ndk_fix` generates invalid Metal for `sdf.fill(vec3)` / `sdf.stroke(vec3)` because the generated `float3` overload accesses `.a`. Source-level ports from makepad-component must pass explicit `vec4(r, g, b, 1.0)` to SDF fill/stroke calls when the color is stored as `Vec3`.

**2026-04-28 bottom/title follow-up:**
- Top replay title remains dynamic, but the visual scale must be closer to the `骑行` chip: primary line around 11px, secondary line around 10.5px, tight line spacing.
- Do not change the four HUD metric cards in this pass; speed, heart rate, elevation, and cadence cards were accepted by the user.
- Bottom playback content should be centered and not fill the full desktop window width. Use a fixed mobile-width content rail around 340px inside the bottom bar.
- First bottom row: `00:00` left, `MpSlider` source-level port in the middle, total duration right. S0 should still show the real bundled GPX duration once the local track is loaded.
- Second bottom row: symmetric left spacer and right circular play button so the `1x / 4x / 16x` group stays centered. Speed buttons remain `MpButtonSmall` source-level semantics.
- In S0 frozen mode the circular button must show a centered play triangle, not pause bars. Use separate overlay groups for pause bars and play glyph so hidden pause bars do not affect play glyph layout.
- Keep S0 interactions visually inert: pause/speed/scrubber clicks should not mutate the frozen screenshot state.

**2026-04-28 bottom visual-test fix:**
- Observed issue: bottom time labels were `font_size: 12` with fixed widths 43/51, causing `00:00` and `651:08` to wrap onto two lines.
- Adjust bottom labels and speed button labels to `font_size: 10.5`, and widen time label columns so `00:00` / `651:08` remain single-line.
- Do not use font glyphs for the play icon. The current font rendered `▶` incorrectly. Use `xor-shader-techniques` style cheap SDF geometry: one filled triangle for play and two rounded bars for pause.

**2026-04-28 data-linkage unlock:**
- User asked to stop inspecting the play button and open the replay data linkage before moving to the next visual stage.
- Implement demo stages `S2`, `S3`, and `S4` as app-level states, not widget-local hacks.
- `S2` starts directly in `PHASE_PLAYBACK`, hides `sync_overlay`, uses bundled track data, applies `MOBILE_EXAMPLE_DEMO_SEEK` or default `0.50`, and lets playback advance so HUD, route progress, slider, and time labels share one `PlaybackState`.
- `S3` starts like S2 but forces the contract guard window without needing `MOBILE_EXAMPLE_DEMO_GUARD=1`; default seek is `0.40`.
- `S4` starts directly in `PHASE_STATS`, applies default seek `0.998`, fills stats from the same track, hides route labels/right controls, and keeps canvas progress complete under the stats overlay.
- Scrubber drag outside S0 must immediately call `PlaybackState::apply_progress`, then refresh HUD and scrubber labels; the next frame paints route progress from the same state.
- S0 remains frozen and inert: no network worker, no phase advance, no HUD data application.

**2026-04-28 S2 pixel-polish start:**
- Next screen to tune is `S2`, not `S3`/`S4`, because S2 defines the reusable baseline for route canvas, HUD density, top bar, bottom controls, and live data linkage.
- S2 reference anchor: `design/refs/v2/s2-main-replay.png`, plus current user screenshots. Use it for density and hierarchy, but keep later user decisions that made the bottom scrubber a two-row control rail.
- Do not change the accepted HUD metric cards in this pass: speed, heart rate, elevation, cadence card content and general proportions are locked unless a new screenshot review calls out a specific defect.
- Do not continue play-button icon debugging in this pass. The button can remain as the current shader widget while S2 data/layout is validated.
- All visible card/control surfaces in S2 must still be source-level ports from `/Users/zhaoyue/workspace/matrix/Component/makepad-component`: `MpCardSmall`, `MpCardChip`, `MpProgress`, `MpSlider`, `MpButtonSmall`.
- S2 operator command:

```bash
MOBILE_EXAMPLE_DEMO_STAGE=S2 MOBILE_EXAMPLE_DEMO_SEEK=0.50 cargo run
```

- First S2 adjustment batch:
  1. Ensure first playback frame does not jump because of a large initial `NextFrame` delta.
  2. Keep S2/S3/S4 off the network worker and immediately apply bundled track data.
  3. Keep the scrubber/time/HUD/route linked to the same progress value after drag or autoplay.
  4. Keep time labels single-line and bottom content centered within the mobile rail.
- S2 scrubber visual batch: align the local `MpSlider` port closer to makepad-component semantics with cyan primary fill, white thumb, cyan thumb border, and a light shadow. This is a component-source adjustment, not an ad-hoc control.
- 2026-04-28 S2 route shader review:
  - Current S2 route has two unwanted dot sources: procedural dot noise in `DrawTrack.pixel` and CPU-side instanced `DrawParticle` scatter in `TrackCanvas::draw_walk`.
  - User clarified Image #2 is the target: a dark-map running/replay path with a thin continuous orange-to-cyan luminous line, broad but soft glow, and no visible point array.
  - Replace the particle/dot look with a continuous Xor-inspired SDF glow shader: multi-width capsule bands, cheap exponential glow, warm orange to cyan progress gradient, cyan replay head, and no discrete particles.
  - Make the route smoother by increasing the GPX-to-screen segment budget and applying one Chaikin smoothing pass after projection. Do not fake smoothness with dots.
  - Preserve S0 placeholder behavior as a faint smooth route.
- 2026-04-28 S2 particle correction:
  - User asked to add the particle effect back, using `$xor-shader-techniques` and `$makepad-2.0-shaders`, to reach the stronger target visual.
  - Keep `DrawTrack` as the continuous route/glow layer. Add `DrawParticle` back as a separate single instanced draw layer, not per-particle widgets.
  - Particle placement uses Xor-style cheap deterministic chaos: integer hash + golden-ratio seed/twinkle, sparse scatter around the route normal, brighter and larger near the current replay head.
  - `particle_density` remains a shader uniform for the spec contract and is driven by current speed normalization. Reduced-motion sets density to `0.0`.
  - Avoid the previous uniform bead-chain look: particles should read as energetic sparks/flow around the route, not source sample dots.
- 2026-04-28 S2 second screenshot tuning:
  - Current render is structurally close but too brown/dim; cyan exists mostly at the current halo instead of reading as the main electric path.
  - Bias the speed ramp toward `speed_high` earlier while preserving low/mid/high token visibility, then boost route glow brightness without widening the line into a fog band.
  - Increase particle visibility using a visual density floor while keeping the `particle_density` uniform speed-driven and reduced-motion-gated.
  - Narrow particle scatter so sparks sit on/near the route instead of becoming a broad cloud.
- 2026-04-28 S2 third screenshot tuning:
  - Current route still reads as a thick brown ribbon; particles are visible but the route base dominates.
  - Shrink the core/ribbon/glow radii, lower brown ribbon alpha, and bias the emissive layer toward `speed_high` so the path reads as electric cyan/orange line instead of muddy trail.
  - Keep particles unchanged for this pass because the issue is the base route layer overpowering them.
- 2026-04-28 S2 fourth screenshot tuning:
  - Latest screenshot still needs more separation from the target: reduce the broad warm/brown body further and make the route read as a thin white-cyan electrical core with a short cyan tail near the replay head.
  - Replace the wide `ribbon` contribution with a narrower `inner` line, lower unwalked alpha, and let orange stay as a warm inner accent instead of the dominant band.
  - Rework particles into a randomly thinned spark layer: smaller average dots, tighter normal scatter, occasional outliers, and stronger brightness near the replay head. This follows `$xor-shader-techniques` cheap deterministic chaos without returning to uniform bead-chain dots.
- 2026-04-28 S2 fifth screenshot tuning:
  - New screenshot shows the path has improved color but still reads as a bright bead chain; the large cyan head bloom also pulls too much attention.
  - Make the continuous route layer carry the visual: reduce core/inner/glow/aura radii and alpha, shorten the cyan tail window, and lower head-specific glow.
  - Move particles from "whole walked path decoration" to "head-local energy": lower total particle count, aggressively thin particles away from the replay head, reduce average particle size/alpha, and keep only sparse outlier sparks along the route.
- 2026-04-28 S2 sixth screenshot tuning:
  - User feedback: current shape is good; route light effect can be brighter.
  - Keep radii and particle distribution unchanged so the path does not widen or return to a bead-chain look.
  - Increase only emissive intensity/alpha: brighter cyan aura, stronger SDF glow, clearer white-cyan core, and slightly stronger replay-head highlight.
- 2026-04-28 S2 seventh screenshot tuning:
  - Latest screenshot confirms brightness is closer, but capsule overlap still creates slight knotting along the route and the replay-head area is near the upper limit.
  - Keep geometry, radii, smoothing, and particle distribution unchanged.
  - Shift brightness from core/inner line to softer glow/aura: increase diffuse cyan emission while lowering core/inner/head alpha so joints look smoother and less segmented.
- 2026-04-28 bottom controls alignment fix:
  - Screenshot showed the visible bottom controls shifted right because the row centered the speed buttons with a hidden 56px left spacer while also drawing the 48px pause button on the right.
  - Remove the spacer and make `speed_group` `width: Fit`; center the combined `speed_group + pause_button` cluster with `playback_controls_row.align.x = 0.5`.
- 2026-04-28 S4 completion modal plan:
  - Target is the user-provided compact completion dialog: small centered modal, dark translucent card, cyan success check, four dense summary rows, and a cancel button.
  - Source-level port makepad-component `components/src/modal/modal.rs`: keep `MpModalContainer`, `MpModalBackdrop`, `MpModalSmall`, `MpModalBody`, and `MpModalFooter` semantics locally because direct dependency still risks Makepad branch mismatch.
  - Replace the oversized current S4 stats card: width around 282px, title around 15.5px, row labels around 10.5px, values around 13px, row height around 34px. Avoid large 24px statistic typography.
  - Avoid emoji icons that render inconsistently in the current font; use small monochrome glyphs (`•`, `○`, `↗`, `♥`) with metric colors.
  - Add `取消` as a compact `MpButtonSmall`-style footer button. Tap hides `stats_overlay`; it should not mutate playback progress or stats data.
- 2026-04-28 S4 modal re-open behavior:
  - After `取消`, the modal may be hidden while the app remains in `PHASE_STATS`.
  - If the user scrubs below `STATS_PROGRESS_THRESHOLD`, switch back to `PHASE_PLAYBACK`, restore route labels/right controls, and keep the modal hidden. This re-arms completion.
  - If progress later reaches the threshold again, enter `PHASE_STATS` normally so the completion modal appears again.

**Existing demo env flags:**
- `MOBILE_EXAMPLE_DEMO_SEEK=0.50 cargo run` seeds replay progress for S2/S3/S4 data-linked stages.
- `MOBILE_EXAMPLE_DEMO_GUARD=1 MOBILE_EXAMPLE_DEMO_SEEK=0.40 cargo run` helps trigger S3.
- `MOBILE_EXAMPLE_DEMO_STAGE=S0` freezes the sync state for pixel review.

---

## 1. Desired Operator Workflow

Use this workflow for every screen:

```bash
# S0 frozen sync target
MOBILE_EXAMPLE_DEMO_STAGE=S0 cargo run

# S2 later
MOBILE_EXAMPLE_DEMO_STAGE=S2 MOBILE_EXAMPLE_DEMO_SEEK=0.50 cargo run

# S3 later
MOBILE_EXAMPLE_DEMO_STAGE=S3 MOBILE_EXAMPLE_DEMO_GUARD=1 MOBILE_EXAMPLE_DEMO_SEEK=0.40 cargo run

# S4 later
MOBILE_EXAMPLE_DEMO_STAGE=S4 MOBILE_EXAMPLE_DEMO_SEEK=0.998 cargo run
```

S0 is the frozen sync screen. S2/S3/S4 are now wired as data-linked operator stages so visual testing can move screen by screen without waiting on network timing.

---

## 2. S0 Freeze Behavior

When `MOBILE_EXAMPLE_DEMO_STAGE=S0`:

- `phase = PHASE_SYNCING`.
- `state.network_state = NetworkState::Fetching`.
- `state.data_source = DataSource::LocalFallback`.
- `state.playback_progress = 0.0`.
- Do not spawn the network fetch worker.
- Do not call `poll_network`.
- Do not advance to `PHASE_PATH_DRAW`.
- Do not apply GPX point values into HUD.
- Keep bundled GPX loaded only as geometry input for the faint placeholder route.
- Keep sync overlay visible.
- Keep `track_progress = 0.0`, `walked_segment_ratio = 0.0`, `guard_pulse_phase = 0.0`, `scrubber_echo_phase = 0.0`.
- Ignore pause/speed/scrubber mutations or leave them visually inert during S0; S0 is a screenshot state, not an interaction state.

Why: freezing S0 via network throttling is not deterministic. A dedicated demo stage gives stable screenshot evidence.

---

## 3. Task 1 — Add Demo Stage Helper

**Files:**
- Modify: `src/main.rs`

**Step 1: Add helper near constants or `impl App` helpers**

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
enum DemoStage {
    S0,
}

fn demo_stage() -> Option<DemoStage> {
    match std::env::var("MOBILE_EXAMPLE_DEMO_STAGE").ok().as_deref() {
        Some("S0") | Some("s0") => Some(DemoStage::S0),
        _ => None,
    }
}
```

**Step 2: Verify compile**

Run:

```bash
cargo check
```

Expected: compile succeeds. The existing Makepad dependency duplicate `bitflags` Cargo metadata warning may still print.

---

## 4. Task 2 — Freeze Startup In S0

**Files:**
- Modify: `src/main.rs`

**Step 1: Gate startup behavior**

In `handle_startup`, after default state setup and before `MOBILE_EXAMPLE_DEMO_SEEK`, compute:

```rust
let frozen_s0 = demo_stage() == Some(DemoStage::S0);
```

Then:

- If `frozen_s0`, do not read `MOBILE_EXAMPLE_DEMO_SEEK`.
- If `frozen_s0`, do not call `self.state.apply_progress(&t, p0)`.
- If `frozen_s0`, do not spawn `spawn_fetch_worker()`.
- Set:

```rust
self.network_rx = None;
self.worker_thread_id = None;
self.fetching_started_at_secs = Some(0.0);
self.pending_fetch = None;
```

Otherwise preserve current behavior exactly.

**Step 2: Verify S0 HUD remains placeholder**

Run:

```bash
MOBILE_EXAMPLE_DEMO_STAGE=S0 cargo run
```

Expected visual: HUD values remain `—`, not first GPX point values.

---

## 5. Task 3 — Stop Frame Advancement In S0

**Files:**
- Modify: `src/main.rs`

**Step 1: Gate network polling**

In `AppMain::handle_event`, inside `Event::NextFrame`:

```rust
if demo_stage() != Some(DemoStage::S0) {
    self.poll_network(cx, now);
}
self.maybe_advance_phase(cx, now);
```

**Step 2: Gate phase advance**

At the top of `maybe_advance_phase` after `last_now_secs` update, add:

```rust
if demo_stage() == Some(DemoStage::S0) {
    self.phase = PHASE_SYNCING;
    self.state.network_state = NetworkState::Fetching;
    self.state.playback_progress = 0.0;
    self.state.walked_segment_ratio = 0.0;
    // continue to push stable S0 uniforms to TrackCanvas below, but never enter S1.
}
```

Then make the `match self.phase` branch for `PHASE_SYNCING` avoid `enter_phase` when S0 is frozen.

**Step 3: Verify no automatic transition**

Run:

```bash
MOBILE_EXAMPLE_DEMO_STAGE=S0 cargo run
```

Expected after 10 seconds: still S0; sync badge still `同步中...`; sync overlay still visible; no colored route.

---

## 6. Task 4 — S0 Visual Placeholder Pass

**Files:**
- Modify: `src/main.rs`

**Target image:**
- `design/refs/v3/s0-sync-target-v2.png`

**Visual adjustments:**
- Change `sync_overlay_label` text from `同步中...` to spinner-equivalent visual if an actual spinner widget exists; otherwise keep text as interim and add TODO.
- Change `sync_overlay_subtext` for Fetching from `正在从 Project-Robius-China 拉取数据` to exact `正在同步轨迹数据`.
- Make S0 placeholder route faint. If current `TrackCanvas` draws normal unwalked solid route at `track_progress = 0.0`, add an S0-specific draw mode later; do not fake it by changing global S2 colors.
- Keep map grid and low-opacity labels consistent with S2.
- Ensure `4x` is active by default even in S0.

**Step 1: Text correction**

Update `refresh_sync_overlay` Fetching subtext:

```rust
NetworkState::Idle | NetworkState::Fetching => "正在同步轨迹数据",
```

**Step 2: Screenshot check**

Run:

```bash
MOBILE_EXAMPLE_DEMO_STAGE=S0 cargo run
```

Expected: text exactly matches spec and v2 target.

---

## 7. Task 5 — Verification

Run:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
MOBILE_EXAMPLE_DEMO_STAGE=S0 cargo run
```

Expected:

- `cargo fmt --check`: existing repo-wide format may fail due historical `src/main.rs` rustfmt drift. If so, record it; do not mass-format unrelated code unless explicitly approved.
- `cargo clippy -- -D warnings`: exits 0, aside from Cargo duplicate dependency metadata warning.
- `cargo test`: parser tests pass.
- S0 remains frozen and screenshot-ready.

---

## 8. Commit Strategy

Commit S0 in small steps:

```bash
git add plans/s0-freeze-and-polish.md
git commit -m "plan: freeze and polish S0 screen"

git add src/main.rs
git commit -m "feat(S0): add frozen sync demo stage"

git add src/main.rs design/refs/v3/s0-sync-target*.png
git commit -m "style(S0): align sync screen target"
```

Do not commit `design/auto/` screenshots unless the user explicitly asks.

---

## 9. Next Screen Handoff

After S0 is screenshot-stable, repeat the same pattern for S1 or S4:

1. Generate or select one target image.
2. Save target under `design/refs/v3/`.
3. Create a `plans/sN-*.md` plan.
4. Add or reuse `MOBILE_EXAMPLE_DEMO_STAGE=SN`.
5. Implement only that screen.
6. Verify with screenshot and clippy/tests.
