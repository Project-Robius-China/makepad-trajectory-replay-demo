pub use makepad_widgets;

use makepad_widgets::*;

mod network;
mod parser;
mod state;

use std::sync::mpsc::Receiver;
use std::sync::Arc;

use crate::network::{spawn_fetch_worker, FetchResult};
use crate::parser::parse_gpx;
use crate::state::{
    effective_max_hr, five_minute_window_avg_hr, DataSource, NetworkState, PlaybackState, Track,
    TrajectoryProfile, UserProfile,
};

const BUNDLED_GPX: &str = include_str!("../assets/cycling-track.gpx");

const PHASE_SYNCING: i32 = 0;
const PHASE_PATH_DRAW: i32 = 1;
const PHASE_PLAYBACK: i32 = 2;
const PHASE_STATS: i32 = 3;

const PATH_DRAW_DURATION_SECS: f64 = 3.0;
const STATS_PROGRESS_THRESHOLD: f32 = 0.99;
const GUARD_PULSE_DURATION_SECS: f64 = 1.5;
const SCRUBBER_ECHO_FADE_SECS: f64 = 0.4;
const SUCCESS_LABEL_VISIBLE_SECS: f64 = 0.9;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DemoStage {
    S0,
    S2,
    S3,
    S4,
}

impl DemoStage {
    fn default_seek(self) -> f32 {
        match self {
            DemoStage::S0 => 0.0,
            DemoStage::S2 => 0.50,
            DemoStage::S3 => 0.40,
            DemoStage::S4 => 0.998,
        }
    }
}

fn demo_stage() -> Option<DemoStage> {
    std::env::var("MOBILE_EXAMPLE_DEMO_STAGE")
        .ok()
        .as_deref()
        .and_then(parse_demo_stage)
}

fn parse_demo_stage(value: &str) -> Option<DemoStage> {
    match value.trim().to_ascii_uppercase().as_str() {
        "S0" => Some(DemoStage::S0),
        "S2" => Some(DemoStage::S2),
        "S3" => Some(DemoStage::S3),
        "S4" => Some(DemoStage::S4),
        _ => None,
    }
}

fn parse_demo_seek(raw: Option<&str>, default: f32) -> f32 {
    raw.and_then(|value| value.parse::<f32>().ok())
        .unwrap_or(default)
        .clamp(0.0, 0.999)
}

fn demo_seek(default: f32) -> f32 {
    parse_demo_seek(
        std::env::var("MOBILE_EXAMPLE_DEMO_SEEK").ok().as_deref(),
        default,
    )
}

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*

    set_type_default() do #(DrawMapGrid::script_shader(vm)){
        ..mod.draw.DrawQuad
        grid_color: vec3(0.25, 0.27, 0.36)
        bg_color: vec3(0.055, 0.060, 0.082)

        pixel: fn() {
            let p = self.pos * self.rect_size
            let cell = 56.0
            let lx = p.x - floor(p.x / cell) * cell
            let ly = p.y - floor(p.y / cell) * cell
            let line_w = 1.0
            let on_x = step(lx, line_w) + step(cell - line_w, lx)
            let on_y = step(ly, line_w) + step(cell - line_w, ly)
            let grid_a = clamp(on_x + on_y, 0., 1.) * 0.85
            let block_x = floor(p.x / cell)
            let block_y = floor(p.y / cell)
            let h = fract(sin(block_x * 12.9898 + block_y * 78.233) * 43758.5453)
            let block_a = step(0.78, h) * 0.30
            let final_color = mix(self.bg_color, self.grid_color, grid_a) +
                              vec3(0.06, 0.07, 0.10) * block_a
            return Pal.premul(vec4(final_color, 1.0))
        }
    }

    set_type_default() do #(DrawSyncSpinner::script_shader(vm)){
        ..mod.draw.DrawQuad
        spinner_color: vec3(0.96, 0.96, 0.98)
        spinner_track: vec3(0.23, 0.23, 0.28)
        stroke_width: 5.0
        arc_ratio: 0.25

        pixel: fn() {
            let c = self.rect_size * 0.5
            let p = self.pos * self.rect_size - c
            let dist = length(p)
            let radius = min(c.x, c.y) - self.stroke_width * 0.5 - 1.0
            let inner = radius - self.stroke_width * 0.5
            let outer = radius + self.stroke_width * 0.5
            let ring = smoothstep(inner - 0.5, inner + 0.5, dist) *
                       smoothstep(outer + 0.5, outer - 0.5, dist)
            let inv_dist = 1.0 / max(dist, 0.001)
            let dir = p * inv_dist
            let phase = self.draw_pass.time * 7.853981
            let head = vec2(cos(phase), sin(phase))
            let in_arc = smoothstep(1.0 - self.arc_ratio * 2.0, 1.0, dot(dir, head))
            let color = mix(self.spinner_track, self.spinner_color, in_arc)
            return Pal.premul(vec4(color.x, color.y, color.z, ring))
        }
    }

    // Source-level port of makepad-component/components/src/progress/progress.rs MpProgress.
    set_type_default() do #(DrawMpProgress::script_shader(vm)){
        ..mod.draw.DrawQuad
        progress: 0.0
        track_color: vec3(0.23, 0.23, 0.28)
        fill_color: vec3(0.29, 0.38, 0.85)

        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let sz = self.rect_size
            let r = sz.y * 0.5
            sdf.circle(r, r, r)
            sdf.rect(r, 0.0, max(sz.x - sz.y, 0.0), sz.y)
            sdf.circle(sz.x - r, r, r)
            sdf.fill(vec4(self.track_color.x, self.track_color.y, self.track_color.z, 1.0))

            let fill_end = sz.x * clamp(self.progress, 0.0, 1.0)
            let px = self.pos.x * sz.x
            let in_fill = step(px, fill_end)

            let sdf2 = Sdf2d.viewport(self.pos * self.rect_size)
            sdf2.circle(r, r, r)
            sdf2.rect(r, 0.0, max(sz.x - sz.y, 0.0), sz.y)
            sdf2.circle(sz.x - r, r, r)
            sdf2.fill(vec4(self.fill_color.x, self.fill_color.y, self.fill_color.z, 1.0))

            return mix(sdf.result, sdf2.result, in_fill * sdf2.result.w)
        }
    }

    // Source-level port of makepad-component/components/src/slider/slider.rs MpSlider.
    set_type_default() do #(DrawMpSliderTrack::script_shader(vm)){
        ..mod.draw.DrawQuad
        progress_start: 0.0
        progress_end: 0.0
        track_color: vec3(0.23, 0.23, 0.28)
        fill_color: vec3(0.157, 0.780, 0.910)

        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let sz = self.rect_size
            let r = sz.y * 0.5
            sdf.circle(r, r, r)
            sdf.rect(r, 0.0, max(sz.x - sz.y, 0.0), sz.y)
            sdf.circle(sz.x - r, r, r)
            sdf.fill(vec4(self.track_color.x, self.track_color.y, self.track_color.z, 1.0))

            let fill_start = sz.x * clamp(self.progress_start, 0.0, 1.0)
            let fill_end = sz.x * clamp(self.progress_end, 0.0, 1.0)
            let px = self.pos.x * sz.x
            let in_fill = step(fill_start, px) * step(px, fill_end)

            let sdf2 = Sdf2d.viewport(self.pos * self.rect_size)
            sdf2.circle(r, r, r)
            sdf2.rect(r, 0.0, max(sz.x - sz.y, 0.0), sz.y)
            sdf2.circle(sz.x - r, r, r)
            sdf2.fill(vec4(self.fill_color.x, self.fill_color.y, self.fill_color.z, 1.0))

            return mix(sdf.result, sdf2.result, in_fill * sdf2.result.w)
        }
    }

    set_type_default() do #(DrawMpSliderThumb::script_shader(vm)){
        ..mod.draw.DrawQuad
        thumb_color: vec3(1.0, 1.0, 1.0)
        border_color: vec3(0.157, 0.780, 0.910)

        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let c = self.rect_size * 0.5
            sdf.circle(c.x + 1.5, c.y + 1.5, min(c.x, c.y) - 2.0)
            sdf.fill(vec4(0.0, 0.0, 0.0, 0.20))
            sdf.circle(c.x, c.y, min(c.x, c.y) - 2.0)
            sdf.fill(vec4(self.thumb_color.x, self.thumb_color.y, self.thumb_color.z, 1.0))
            sdf.circle(c.x, c.y, min(c.x, c.y) - 2.0)
            sdf.stroke(vec4(self.border_color.x, self.border_color.y, self.border_color.z, 1.0), 2.0)
            return sdf.result
        }
    }

    set_type_default() do #(DrawPlaybackButton::script_shader(vm)){
        ..mod.draw.DrawQuad
        bg_color: vec3(0.05, 0.06, 0.09)
        border_color: vec3(0.28, 0.29, 0.36)
        icon_color: vec3(0.96, 0.96, 0.98)
        mode: 0.0

        pixel: fn() {
            let sdf = Sdf2d.viewport(self.pos * self.rect_size)
            let sz = self.rect_size
            let c = sz * 0.5
            let radius = min(c.x, c.y) - 1.25

            sdf.circle(c.x, c.y, radius)
            sdf.fill(vec4(self.bg_color.x, self.bg_color.y, self.bg_color.z, 1.0))
            sdf.circle(c.x, c.y, radius - 0.5)
            sdf.stroke(vec4(self.border_color.x, self.border_color.y, self.border_color.z, 1.0), 1.5)

            let play = step(self.mode, 0.5)

            if play > 0.5 {
                let s = min(sz.x, sz.y) * 0.24
                sdf.move_to(c.x - s * 0.35, c.y - s)
                sdf.line_to(c.x - s * 0.35, c.y + s)
                sdf.line_to(c.x + s * 0.95, c.y)
                sdf.close_path()
                sdf.fill(vec4(self.icon_color.x, self.icon_color.y, self.icon_color.z, 1.0))
            } else {
                let h = min(sz.x, sz.y) * 0.38
                let w = max(3.0, min(sz.x, sz.y) * 0.10)
                let gap = min(sz.x, sz.y) * 0.11
                sdf.box(c.x - gap - w, c.y - h * 0.5, w, h, w * 0.5)
                sdf.fill_keep(vec4(self.icon_color.x, self.icon_color.y, self.icon_color.z, 1.0))
                sdf.box(c.x + gap, c.y - h * 0.5, w, h, w * 0.5)
                sdf.fill(vec4(self.icon_color.x, self.icon_color.y, self.icon_color.z, 1.0))
            }

            return sdf.result
        }
    }

    set_type_default() do #(DrawWater::script_shader(vm)){
        ..mod.draw.DrawQuad
        water_color: vec3(0.043, 0.078, 0.157)
        edge_softness: 18.0

        pixel: fn() {
            let p = self.pos
            let d = min(min(p.x, 1. - p.x), min(p.y, 1. - p.y)) * min(self.rect_size.x, self.rect_size.y)
            let edge_a = clamp(d / self.edge_softness, 0., 1.)
            let alpha = 0.55 * edge_a
            return Pal.premul(vec4(self.water_color, alpha))
        }
    }

    set_type_default() do #(DrawTrack::script_shader(vm)){
        ..mod.draw.DrawQuad
        track_progress: 0.
        polyline_color_mix: 0.
        particle_density: 0.0
        elevation_z: 0.
        guard_pulse_phase: 0.
        walked_segment_ratio: 0.
        scrubber_echo_phase: 0.
        overlay_dim: 0.
        sync_placeholder: 0.
        seg_count: 0.

        speed_low: vec3(0.91, 0.91, 0.94)
        speed_mid: vec3(1.0, 0.541, 0.239)
        speed_high: vec3(0.0, 0.898, 1.0)
        unwalked: vec3(0.231, 0.231, 0.275)
        bg_color: vec3(0.039, 0.039, 0.059)
        guard_color: vec3(1.0, 0.231, 0.431)

        capsule_sdf: fn(p: vec2, a: vec2, b: vec2, r: float) -> float {
            let pa = p - a
            let ba = b - a
            let h = clamp(dot(pa, ba) / dot(ba, ba), 0., 1.)
            return length(pa - ba * h) - r
        }

        speed_color: fn(s: float) -> vec3 {
            let low_mid = smoothstep(0.02, 0.32, s)
            let mid_high = smoothstep(0.34, 0.76, s)
            let c1 = mix(self.speed_low, self.speed_mid, low_mid)
            let c2 = mix(self.speed_mid, self.speed_high, mid_high)
            return mix(c1, c2, step(0.34, s))
        }

        pixel: fn() {
            let p = self.pos * self.rect_size
            let t_mid = (self.t_a + self.t_b) * 0.5
            let s_mid = (self.speed_a + self.speed_b) * 0.5
            let walked = step(t_mid, self.walked_segment_ratio)
            let speed_col = self.speed_color(s_mid)
            let trail_cyan = smoothstep(self.walked_segment_ratio - 0.22, self.walked_segment_ratio, t_mid) * walked
            let speed_cyan = smoothstep(0.20, 0.62, s_mid)
            let electric_mix = max(trail_cyan * 0.78, speed_cyan * 0.48)
            let electric_col = mix(speed_col, self.speed_high, electric_mix)
            let warm_col = mix(self.speed_mid, speed_col, speed_cyan * 0.35)
            let speed_energy = 0.92 + 0.46 * max(s_mid, trail_cyan)
            let lit_col = mix(warm_col, electric_col, 0.54 + trail_cyan * 0.30) * mix(0.96, 1.48, walked)

            let core_d = self.capsule_sdf(p, self.start_xy, self.end_xy, 0.42)
            let inner_d = self.capsule_sdf(p, self.start_xy, self.end_xy, 0.78)
            let glow_d = self.capsule_sdf(p, self.start_xy, self.end_xy, 3.7)
            let aura_d = self.capsule_sdf(p, self.start_xy, self.end_xy, 7.2)
            let core = clamp(0.5 - core_d, 0.0, 1.0) * walked
            let inner = clamp(0.5 - inner_d, 0.0, 1.0) * walked
            let glow = exp(-max(glow_d, 0.0) * 0.66) * mix(0.034, 0.90, walked)
            let aura = exp(-max(aura_d, 0.0) * 0.38) * mix(0.014, 0.22, walked)
            let unwalked_d = self.capsule_sdf(p, self.start_xy, self.end_xy, 0.78)
            let unwalked_alpha = clamp(0.5 - unwalked_d, 0.0, 1.0) * (1.0 - walked) * 0.09
            let head = smoothstep(self.walked_segment_ratio - 0.026, self.walked_segment_ratio, t_mid) * walked
            let head_color = mix(electric_col, self.speed_high, head)

            let final_rgb =
                self.unwalked * unwalked_alpha * 0.8 +
                self.speed_high * aura * (0.56 + trail_cyan * 0.40) +
                head_color * glow * speed_energy * (1.18 + trail_cyan * 0.52) +
                lit_col * inner * 0.32 +
                vec3(1.0, 0.98, 0.90) * core * 0.74 +
                self.speed_high * head * glow * 1.34
            let final_a = clamp(
                unwalked_alpha + aura * 0.075 + glow * 0.32 + inner * 0.10 + core * 0.48 + head * 0.06,
                0.0,
                1.0
            )
            let placeholder_a = clamp(0.5 - unwalked_d, 0.0, 1.0) * 0.24
            let placeholder = step(0.5, self.sync_placeholder)
            let display_rgb = mix(final_rgb, vec3(0.48, 0.48, 0.55), placeholder)
            let display_a = mix(final_a, placeholder_a, placeholder)
            let dim = 1. - self.overlay_dim * 0.7
            return Pal.premul(vec4(display_rgb * dim, display_a * dim))
        }
    }

    set_type_default() do #(DrawMarker::script_shader(vm)){
        ..mod.draw.DrawQuad
        marker_color: vec3(1., 1., 1.)
        marker_radius: 6.

        pixel: fn() {
            let p = self.pos * self.rect_size
            let c = self.rect_size * 0.5
            let d = length(p - c) - self.marker_radius
            let alpha = clamp(0.5 - d, 0., 1.)
            let ring_d = abs(d + 2.0) - 1.5
            let ring = clamp(0.5 - ring_d, 0., 1.) * 0.7
            return Pal.premul(vec4(self.marker_color, alpha + ring * 0.4))
        }
    }

    set_type_default() do #(DrawHalo::script_shader(vm)){
        ..mod.draw.DrawQuad
        hr_phase: 0.
        guard_pulse_phase: 0.

        pixel: fn() {
            let p = self.pos * self.rect_size
            let c = self.rect_size * 0.5
            let pulse = 0.5 + 0.5 * sin(self.hr_phase * 6.283185)
            let halo_r = mix(8., 12., pulse)
            let dot_r = 3.0
            let d_dot = length(p - c) - dot_r
            let dot_a = clamp(0.5 - d_dot, 0., 1.)
            let d_halo = length(p - c) - halo_r
            let halo_a = exp(-max(d_halo, 0.) * 0.35) * 0.55
            let cyan = vec3(0.0, 0.898, 1.0)
            let pulse_red = 1.0 - exp(-self.guard_pulse_phase * 3.0)
            let color = mix(cyan, vec3(1., 0.231, 0.431), pulse_red * step(0.001, self.guard_pulse_phase))
            return Pal.premul(vec4(color, dot_a + halo_a * (1. - dot_a)))
        }
    }

    set_type_default() do #(DrawGuardEdge::script_shader(vm)){
        ..mod.draw.DrawQuad
        guard_pulse_phase: 0.

        pixel: fn() {
            let p = self.pos
            let edge_dist = min(min(p.x, 1. - p.x), min(p.y, 1. - p.y))
            let pulse = max(0., self.guard_pulse_phase)
            let band = exp(-edge_dist * 18.) * pulse
            let color = vec3(1.0, 0.231, 0.431)
            return Pal.premul(vec4(color * band, band * 0.85))
        }
    }

    set_type_default() do #(DrawParticle::script_shader(vm)){
        ..mod.draw.DrawQuad
        particle_color: vec3(0.0, 0.898, 1.0)
        particle_alpha: 0.65
        particle_seed: 0.0

        pixel: fn() {
            let p = self.pos - vec2(0.5, 0.5)
            let d = length(p)
            let twinkle = 0.62 + 0.38 * sin(self.draw_pass.time * 8.5 + self.particle_seed)
            let core = 1.0 - smoothstep(0.01, 0.13, d)
            let halo = exp(-max(d - 0.04, 0.0) * 7.5) * (1.0 - smoothstep(0.40, 0.54, d))
            let alpha = clamp((core * 1.0 + halo * 0.68) * self.particle_alpha * twinkle, 0.0, 1.0)
            let rgb = self.particle_color * (core * 1.70 + halo * 1.05) * twinkle
            return Pal.premul(vec4(rgb.x, rgb.y, rgb.z, alpha))
        }
    }

    let TrackCanvasBase = #(TrackCanvas::register_widget(vm))
    let TrackCanvas = set_type_default() do TrackCanvasBase{
        width: Fill
        height: Fill
    }

    let GuardEdgeBase = #(GuardEdge::register_widget(vm))
    let GuardEdge = set_type_default() do GuardEdgeBase{
        width: Fill
        height: Fill
    }

    let SyncSpinnerBase = #(SyncSpinner::register_widget(vm))
    let SyncSpinner = set_type_default() do SyncSpinnerBase{
        width: 48
        height: 48
    }

    let MpProgressBase = #(MpProgress::register_widget(vm))
    let MpProgress = set_type_default() do MpProgressBase{
        width: Fill
        height: 4
    }

    let MpSliderBase = #(MpSlider::register_widget(vm))
    let MpSlider = set_type_default() do MpSliderBase{
        width: Fill
        height: 48
    }

    // Local Makepad 2.0 copy of makepad-component/components/src/card/card.rs MpCardSmall.
    let MpCardSmall = RoundedView{
        width: Fill
        height: Fit
        flow: Down
        padding: 12
        spacing: 8
        new_batch: true
        draw_bg.color: #x14141C
        draw_bg.radius: 8.
    }

    // Local Makepad 2.0 copy of makepad-component MpCard semantics for compact chips.
    let MpCardChip = RoundedView{
        width: Fit
        height: Fit
        flow: Right
        spacing: 6
        padding: Inset{ left: 9 right: 9 top: 4 bottom: 4 }
        align: Align{ y: 0.5 }
        new_batch: true
        draw_bg.color: #x14141C
        draw_bg.radius: 12.
    }

    // Local Makepad 2.0 copy of makepad-component/components/src/badge/badge.rs MpBadgeDotIndicator.
    let MpBadgeDotIndicator = View{
        width: 8
        height: 8
        show_bg: true
        draw_bg +: {
            bg_color: #x4ADE80
            pixel: fn() {
                let sdf = Sdf2d.viewport(self.pos * self.rect_size)
                let r = min(self.rect_size.x, self.rect_size.y) * 0.5
                sdf.circle(r, r, r)
                sdf.fill(self.bg_color)
                return sdf.result
            }
        }
    }

    // Source-level style port of makepad-component/components/src/button/button.rs MpButtonSmall.
    let MpButtonSmall = RoundedView{
        width: 68
        height: 36
        align: Center
        padding: Inset{ left: 12 right: 12 top: 4 bottom: 4 }
        new_batch: true
        draw_bg.color: #x14141C
        draw_bg.radius: 8.
    }

    let MpButtonCircle = RoundedView{
        width: 48
        height: 48
        align: Center
        new_batch: true
        draw_bg.color: #x1F1F2C
        draw_bg.radius: 24.
    }

    let PlaybackButtonBase = #(PlaybackButton::register_widget(vm))
    let PlaybackButton = set_type_default() do PlaybackButtonBase{
        width: 48
        height: 48
    }

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.inner_size: vec2(390, 844)
                pass.clear_color: #x0A0A0F

                body +: {
                    width: Fill height: Fill flow: Down

                    top_bar := View{
                        width: Fill height: Fit
                        flow: Right
                        spacing: 8
                        padding: Inset{ left: 16 right: 16 top: 10 bottom: 10 }
                        align: Align{ y: 0.5 }
                        show_bg: true
                        new_batch: true
                        draw_bg.color: #x0A0A0F

                        header_left := View{
                            width: Fill
                            height: Fit
                            flow: Down
                            spacing: 2

                            route_title_card := MpCardSmall{
                                width: Fill
                                padding: Inset{ left: 0 right: 0 top: 0 bottom: 0 }
                                spacing: 0
                                draw_bg.color: #x00000000

                                route_title_primary := Label{
                                    text: ""
                                    width: Fill
                                    draw_text.color: #xF5F5FA
                                    draw_text.text_style.font_size: 11
                                }
                                route_title_secondary := Label{
                                    text: ""
                                    width: Fill
                                    draw_text.color: #xD4D5DD
                                    draw_text.text_style.font_size: 10.5
                                }
                            }
                        }

                        header_actions := View{
                            width: Fit
                            height: Fit
                            flow: Right
                            spacing: 8
                            align: Align{ y: 0.5 }

                            sync_badge := MpCardChip{
                                sync_badge_dot := MpBadgeDotIndicator{}
                                sync_badge_label := Label{
                                    text: "同步中..."
                                    draw_text.color: #xF5F5FA
                                    draw_text.text_style.font_size: 10
                                }
                            }

                            profile_pill := MpCardChip{
                                profile_label := Label{
                                    text: "骑行"
                                    draw_text.color: #xF5F5FA
                                    draw_text.text_style.font_size: 11
                                }
                            }
                        }
                    }

                    main_stack := View{
                        width: Fill height: Fill
                        flow: Overlay

                        track_canvas := TrackCanvas{
                            width: Fill
                            height: Fill
                        }

                        label_overlay := View{
                            width: Fill height: Fill
                            flow: Overlay
                            new_batch: true

                            place_label_1_box := View{
                                width: Fit height: Fit
                                place_label_1 := Label{
                                    text: "Big Sur"
                                    width: Fit height: Fit
                                    draw_text.color: #x6F7388
                                    draw_text.text_style.font_size: 10
                                }
                            }
                            place_label_2_box := View{
                                width: Fit height: Fit
                                place_label_2 := Label{
                                    text: "Pacific"
                                    width: Fit height: Fit
                                    draw_text.color: #x4A5C7A
                                    draw_text.text_style.font_size: 11
                                }
                            }
                            place_label_3_box := View{
                                width: Fit height: Fit
                                place_label_3 := Label{
                                    text: "Highway 1"
                                    width: Fit height: Fit
                                    draw_text.color: #x6F7388
                                    draw_text.text_style.font_size: 9
                                }
                            }
                            place_label_4_box := View{
                                width: Fit height: Fit
                                place_label_4 := Label{
                                    text: "Point Lobos"
                                    width: Fit height: Fit
                                    draw_text.color: #x6F7388
                                    draw_text.text_style.font_size: 9
                                }
                            }
                            start_marker_label_box := View{
                                width: Fit height: Fit
                                start_marker_label := Label{
                                    text: "起点 · Ragged Pt"
                                    width: Fit height: Fit
                                    draw_text.color: #x10B981
                                    draw_text.text_style.font_size: 10
                                }
                            }
                            end_marker_label_box := View{
                                width: Fit height: Fit
                                end_marker_label := Label{
                                    text: "终点 · Carmel"
                                    width: Fit height: Fit
                                    draw_text.color: #xF5F5FA
                                    draw_text.text_style.font_size: 10
                                }
                            }
                        }

                        guard_edge := GuardEdge{
                            width: Fill height: Fill
                        }

                        right_overlay := View{
                            width: Fill height: Fill
                            flow: Right
                            View{ width: Fill height: Fill }
                            View{
                                width: Fit height: Fit
                                flow: Down
                                spacing: 10
                                padding: Inset{ left: 0 right: 14 top: 14 bottom: 0 }
                                align: Align{ x: 1.0 }

                                speed_legend := View{
                                    width: Fit height: Fit
                                    flow: Down
                                    spacing: 4
                                    align: Align{ x: 1.0 }

                                    Label{
                                        text: "速度 (m/s)"
                                        draw_text.color: #x7A7B8C
                                        draw_text.text_style.font_size: 9
                                    }
                                    legend_strip := View{
                                        width: 96 height: 8
                                        show_bg: true
                                        draw_bg +: {
                                            pixel: fn() {
                                                let t = self.pos.x
                                                let low = vec3(0.91, 0.91, 0.94)
                                                let mid = vec3(1.0, 0.541, 0.239)
                                                let high = vec3(0.0, 0.898, 1.0)
                                                let c = mix(mix(low, mid, clamp(t * 2., 0., 1.)),
                                                            mix(mid, high, clamp((t - 0.5) * 2., 0., 1.)),
                                                            step(0.5, t))
                                                return Pal.premul(vec4(c, 1.))
                                            }
                                        }
                                    }
                                    View{
                                        width: 96 height: Fit
                                        flow: Right
                                        legend_min_label := Label{
                                            width: Fill
                                            text: "0"
                                            draw_text.color: #x7A7B8C
                                            draw_text.text_style.font_size: 9
                                        }
                                        legend_max_label := Label{
                                            text: "0"
                                            draw_text.color: #x7A7B8C
                                            draw_text.text_style.font_size: 9
                                        }
                                    }
                                }

                                // compass_button: outer 48dp hit area (G2 a11y) + inner 32dp 视觉 (visual.spec L40)
                                compass_button := View{
                                    width: 48 height: 48
                                    align: Center
                                    new_batch: true
                                    compass_visual := RoundedView{
                                        width: 32 height: 32
                                        align: Center
                                        new_batch: true
                                        draw_bg.color: #x14141C
                                        draw_bg.radius: 16.
                                        Label{
                                            text: "N"
                                            draw_text.color: #xF5F5FA
                                            draw_text.text_style.font_size: 11
                                        }
                                    }
                                }

                                // lock_2d_button: outer 48dp hit area + inner 32dp 视觉
                                lock_2d_button := View{
                                    width: 48 height: 48
                                    align: Center
                                    new_batch: true
                                    lock_2d_visual := RoundedView{
                                        width: 32 height: 32
                                        align: Center
                                        new_batch: true
                                        draw_bg.color: #x14141C
                                        draw_bg.radius: 16.
                                        Label{
                                            text: "2D"
                                            draw_text.color: #xF5F5FA
                                            draw_text.text_style.font_size: 9
                                        }
                                    }
                                }
                            }
                        }

                        sync_overlay := View{
                            width: Fill height: Fill
                            flow: Down
                            align: Align{ x: 0.5 y: 0.40 }
                            spacing: 12

                            sync_spinner := SyncSpinner{}
                            sync_overlay_label := Label{
                                text: "同步中..."
                                draw_text.color: #xF5F5FA
                                draw_text.text_style.font_size: 18
                            }
                            sync_overlay_subtext := Label{
                                text: "正在同步轨迹数据"
                                draw_text.color: #x7A7B8C
                                draw_text.text_style.font_size: 11
                            }
                        }

                        guard_card := RoundedView{
                            width: Fill height: Fit
                            margin: Inset{ left: 24 right: 24 top: 80 bottom: 0 }
                            padding: Inset{ left: 18 right: 18 top: 16 bottom: 16 }
                            flow: Down
                            spacing: 6
                            align: Align{ x: 0.5 }
                            new_batch: true
                            draw_bg.color: #xFF3B6E
                            draw_bg.radius: 10.
                            visible: false

                            Label{
                                text: "AI 建议已被 spec 阻止"
                                draw_text.color: #xFFFFFF
                                draw_text.text_style.font_size: 13
                            }
                            guard_card_title := Label{
                                text: "违反契约 c1.2"
                                draw_text.color: #xFFFFFF
                                draw_text.text_style.font_size: 16
                            }
                            guard_card_subtitle := Label{
                                text: "高强度持续约束"
                                draw_text.color: #xFFE6EC
                                draw_text.text_style.font_size: 11
                            }
                            Label{
                                text: "原因: 心率持续超过 92% 区间"
                                draw_text.color: #xFFFFFF
                                draw_text.text_style.font_size: 11
                            }
                            guard_dismiss_button := Button{
                                width: Fit height: Fit
                                margin: Inset{ left: 0 right: 0 top: 4 bottom: 0 }
                                padding: Inset{ left: 14 right: 14 top: 6 bottom: 6 }
                                draw_bg.color: #xFFFFFF
                                draw_bg.radius: 6.
                                draw_text.color: #xFF3B6E
                                text: "知道了"
                            }
                        }

                        // stats_overlay (P12.0 升级 per visual.spec L444-465):
                        // ✓ checkmark + "回放已完成" 标题 + 4 项单列垂直 + leading icons + frosted glass card.
                        stats_overlay := View{
                            width: Fill height: Fill
                            flow: Down
                            align: Center
                            padding: Inset{ left: 32 right: 32 top: 32 bottom: 24 }
                            visible: false

                            stats_card := RoundedView{
                                width: Fit height: Fit
                                flow: Down
                                align: Center
                                spacing: 10
                                padding: Inset{ left: 28 right: 28 top: 18 bottom: 18 }
                                new_batch: true
                                draw_bg.color: #x14141Cd9
                                draw_bg.radius: 12.

                                stats_checkmark := RoundedView{
                                    width: 32 height: 32
                                    align: Center
                                    new_batch: true
                                    draw_bg.color: #x10B981
                                    draw_bg.radius: 16.
                                    Label{
                                        text: "✓"
                                        draw_text.color: #xFFFFFF
                                        draw_text.text_style.font_size: 18
                                    }
                                }

                                stats_title := Label{
                                    text: "回放已完成"
                                    draw_text.color: #xF5F5FA
                                    draw_text.text_style.font_size: 18
                                }

                                stat_distance_row := View{
                                    width: Fit height: Fit
                                    flow: Right
                                    align: Align{ y: 0.5 }
                                    spacing: 12
                                    Label{
                                        text: "📍"
                                        draw_text.color: #x4A6CF7
                                        draw_text.text_style.font_size: 14
                                    }
                                    View{
                                        width: Fit height: Fit
                                        flow: Down
                                        spacing: 2
                                        Label{
                                            text: "总距离"
                                            draw_text.color: #x7A7B8C
                                            draw_text.text_style.font_size: 11
                                        }
                                        stat_distance_value := Label{
                                            text: "—"
                                            draw_text.color: #xF5F5FA
                                            draw_text.text_style.font_size: 24
                                        }
                                    }
                                }

                                stat_duration_row := View{
                                    width: Fit height: Fit
                                    flow: Right
                                    align: Align{ y: 0.5 }
                                    spacing: 12
                                    Label{
                                        text: "⏱"
                                        draw_text.color: #xFF8A3D
                                        draw_text.text_style.font_size: 14
                                    }
                                    View{
                                        width: Fit height: Fit
                                        flow: Down
                                        spacing: 2
                                        Label{
                                            text: "总时长"
                                            draw_text.color: #x7A7B8C
                                            draw_text.text_style.font_size: 11
                                        }
                                        stat_duration_value := Label{
                                            text: "—"
                                            draw_text.color: #xF5F5FA
                                            draw_text.text_style.font_size: 24
                                        }
                                    }
                                }

                                stat_climb_row := View{
                                    width: Fit height: Fit
                                    flow: Right
                                    align: Align{ y: 0.5 }
                                    spacing: 12
                                    Label{
                                        text: "↗"
                                        draw_text.color: #x10B981
                                        draw_text.text_style.font_size: 16
                                    }
                                    View{
                                        width: Fit height: Fit
                                        flow: Down
                                        spacing: 2
                                        Label{
                                            text: "累计爬升"
                                            draw_text.color: #x7A7B8C
                                            draw_text.text_style.font_size: 11
                                        }
                                        stat_climb_value := Label{
                                            text: "—"
                                            draw_text.color: #xF5F5FA
                                            draw_text.text_style.font_size: 24
                                        }
                                    }
                                }

                                stat_avg_hr_row := View{
                                    width: Fit height: Fit
                                    flow: Right
                                    align: Align{ y: 0.5 }
                                    spacing: 12
                                    Label{
                                        text: "♥"
                                        draw_text.color: #xFF3B6E
                                        draw_text.text_style.font_size: 16
                                    }
                                    View{
                                        width: Fit height: Fit
                                        flow: Down
                                        spacing: 2
                                        Label{
                                            text: "平均心率"
                                            draw_text.color: #x7A7B8C
                                            draw_text.text_style.font_size: 11
                                        }
                                        stat_avg_hr_value := Label{
                                            text: "—"
                                            draw_text.color: #xF5F5FA
                                            draw_text.text_style.font_size: 24
                                        }
                                    }
                                }
                            }
                        }
                    }

                    hud := View{
                        width: Fill height: Fit
                        flow: Right
                        padding: Inset{ left: 12 right: 12 top: 8 bottom: 8 }
                        spacing: 8
                        show_bg: true
                        new_batch: true
                        draw_bg.color: #x0A0A0F

                        hud_speed_cell := MpCardSmall{
                            width: Fill height: Fit
                            spacing: 8
                            padding: Inset{ left: 10 right: 10 top: 8 bottom: 8 }
                            draw_bg.color: #x14141C
                            draw_bg.radius: 6.

                            View{
                                width: Fit height: Fit flow: Right spacing: 4
                                align: Align{ y: 0.5 }
                                hud_speed_icon := MpBadgeDotIndicator{
                                    draw_bg.bg_color: #xFF8A3D
                                }
                                Label{
                                    text: "速度 km/h"
                                    draw_text.color: #x7A7B8C
                                    draw_text.text_style.font_size: 9
                                }
                            }
                            hud_speed_value := Label{
                                text: "—"
                                draw_text.color: #xF5F5FA
                                draw_text.text_style.font_size: 18
                            }
                            hud_speed_bar := MpProgress{
                                width: Fill height: 4
                                draw_bg.fill_color: vec3(1.0, 0.541, 0.239)
                                draw_bg.track_color: vec3(0.23, 0.23, 0.28)
                            }
                        }

                        hud_hr_cell := MpCardSmall{
                            width: Fill height: Fit
                            spacing: 8
                            padding: Inset{ left: 10 right: 10 top: 8 bottom: 8 }
                            draw_bg.color: #x14141C
                            draw_bg.radius: 6.

                            View{
                                width: Fit height: Fit flow: Right spacing: 4
                                align: Align{ y: 0.5 }
                                hud_hr_icon := MpBadgeDotIndicator{
                                    draw_bg.bg_color: #xFF3B6E
                                }
                                Label{
                                    text: "心率 bpm"
                                    draw_text.color: #x7A7B8C
                                    draw_text.text_style.font_size: 9
                                }
                            }
                            hud_hr_value := Label{
                                text: "—"
                                draw_text.color: #xF5F5FA
                                draw_text.text_style.font_size: 18
                            }
                            hud_hr_bar := MpProgress{
                                width: Fill height: 4
                                draw_bg.fill_color: vec3(1.0, 0.231, 0.431)
                                draw_bg.track_color: vec3(0.23, 0.23, 0.28)
                            }
                        }

                        hud_ele_cell := MpCardSmall{
                            width: Fill height: Fit
                            spacing: 8
                            padding: Inset{ left: 10 right: 10 top: 8 bottom: 8 }
                            draw_bg.color: #x14141C
                            draw_bg.radius: 6.

                            View{
                                width: Fit height: Fit flow: Right spacing: 4
                                align: Align{ y: 0.5 }
                                hud_ele_icon := MpBadgeDotIndicator{
                                    draw_bg.bg_color: #x00E5FF
                                }
                                Label{
                                    text: "海拔 m"
                                    draw_text.color: #x7A7B8C
                                    draw_text.text_style.font_size: 9
                                }
                            }
                            hud_ele_value := Label{
                                text: "—"
                                draw_text.color: #xF5F5FA
                                draw_text.text_style.font_size: 18
                            }
                            hud_ele_bar := MpProgress{
                                width: Fill height: 4
                                draw_bg.fill_color: vec3(0.0, 0.898, 1.0)
                                draw_bg.track_color: vec3(0.23, 0.23, 0.28)
                            }
                        }

                        hud_cad_cell := MpCardSmall{
                            width: Fill height: Fit
                            spacing: 8
                            padding: Inset{ left: 10 right: 10 top: 8 bottom: 8 }
                            draw_bg.color: #x14141C
                            draw_bg.radius: 6.

                            View{
                                width: Fit height: Fit flow: Right spacing: 4
                                align: Align{ y: 0.5 }
                                hud_cad_icon := MpBadgeDotIndicator{
                                    draw_bg.bg_color: #xE8E8F0
                                }
                                Label{
                                    text: "踏频 rpm"
                                    draw_text.color: #x7A7B8C
                                    draw_text.text_style.font_size: 9
                                }
                            }
                            hud_cad_value := Label{
                                text: "—"
                                draw_text.color: #xF5F5FA
                                draw_text.text_style.font_size: 18
                            }
                            hud_cad_bar := MpProgress{
                                width: Fill height: 4
                                draw_bg.fill_color: vec3(0.91, 0.91, 0.94)
                                draw_bg.track_color: vec3(0.23, 0.23, 0.28)
                            }
                        }
                    }

                    bottom_bar := View{
                        width: Fill height: Fit
                        flow: Down
                        padding: Inset{ left: 16 right: 16 top: 8 bottom: 14 }
                        align: Align{ x: 0.5 }
                        show_bg: true
                        new_batch: true
                        draw_bg.color: #x0A0A0F

                        bottom_content := View{
                            width: 356 height: Fit
                            flow: Down
                            spacing: 10

                            playback_progress_row := View{
                                width: Fill height: Fit
                                flow: Right
                                spacing: 8
                                align: Align{ y: 0.5 }

                                current_time_label := Label{
                                    width: 58 height: Fit
                                    text: "00:00"
                                    draw_text.color: #xF5F5FA
                                    draw_text.text_style.font_size: 10.5
                                }

                                scrubber_slider := MpSlider{
                                    width: Fill height: 48
                                    draw_track.track_color: vec3(0.23, 0.23, 0.28)
                                    draw_track.fill_color: vec3(0.157, 0.780, 0.910)
                                    draw_thumb.thumb_color: vec3(1.0, 1.0, 1.0)
                                    draw_thumb.border_color: vec3(0.157, 0.780, 0.910)
                                }

                                total_time_label := Label{
                                    width: 74 height: Fit
                                    text: "00:00"
                                    draw_text.color: #xD4D5DD
                                    draw_text.text_style.font_size: 10.5
                                }
                            }

                            playback_controls_row := View{
                                width: Fill height: Fit
                                flow: Right
                                spacing: 10
                                align: Align{ x: 0.5 y: 0.5 }

                                speed_group := View{
                                    width: Fit height: Fit
                                    flow: Right
                                    spacing: 10
                                    align: Align{ x: 0.5 y: 0.5 }

                                    speed_1x_button := View{
                                        width: 60 height: 48
                                        align: Center
                                        new_batch: true
                                        speed_1x_visual := MpButtonSmall{
                                            width: 60
                                            speed_1x_label := Label{
                                                text: "1x"
                                                draw_text.color: #xD4D5DD
                                                draw_text.text_style.font_size: 10.5
                                            }
                                        }
                                    }
                                    speed_4x_button := View{
                                        width: 60 height: 48
                                        align: Center
                                        new_batch: true
                                        speed_4x_visual := MpButtonSmall{
                                            width: 60
                                            draw_bg.color: #x28C7E8
                                            speed_4x_label := Label{
                                                text: "4x"
                                                draw_text.color: #xFFFFFF
                                                draw_text.text_style.font_size: 10.5
                                            }
                                        }
                                    }
                                    speed_16x_button := View{
                                        width: 60 height: 48
                                        align: Center
                                        new_batch: true
                                        speed_16x_visual := MpButtonSmall{
                                            width: 60
                                            speed_16x_label := Label{
                                                text: "16x"
                                                draw_text.color: #xD4D5DD
                                                draw_text.text_style.font_size: 10.5
                                            }
                                        }
                                    }
                                }

                                pause_button := PlaybackButton{}
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawTrack {
    #[deref]
    draw_super: DrawQuad,
    #[live]
    track_progress: f32,
    #[live]
    polyline_color_mix: f32,
    #[live]
    particle_density: f32,
    #[live]
    elevation_z: f32,
    #[live]
    guard_pulse_phase: f32,
    #[live]
    walked_segment_ratio: f32,
    #[live]
    scrubber_echo_phase: f32,
    #[live]
    overlay_dim: f32,
    #[live]
    sync_placeholder: f32,
    #[live]
    seg_count: f32,
    #[live]
    seg_idx: f32,
    #[live]
    start_xy: Vec2,
    #[live]
    end_xy: Vec2,
    #[live]
    t_a: f32,
    #[live]
    t_b: f32,
    #[live]
    speed_a: f32,
    #[live]
    speed_b: f32,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawMarker {
    #[deref]
    draw_super: DrawQuad,
    #[live]
    marker_color: Vec3,
    #[live]
    marker_radius: f32,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawHalo {
    #[deref]
    draw_super: DrawQuad,
    #[live]
    hr_phase: f32,
    #[live]
    guard_pulse_phase: f32,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawGuardEdge {
    #[deref]
    draw_super: DrawQuad,
    #[live]
    guard_pulse_phase: f32,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawParticle {
    #[deref]
    draw_super: DrawQuad,
    #[live]
    particle_color: Vec3,
    #[live]
    particle_alpha: f32,
    #[live]
    particle_seed: f32,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawWater {
    #[deref]
    draw_super: DrawQuad,
    #[live]
    water_color: Vec3,
    #[live]
    edge_softness: f32,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawMapGrid {
    #[deref]
    draw_super: DrawQuad,
    #[live]
    grid_color: Vec3,
    #[live]
    bg_color: Vec3,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawSyncSpinner {
    #[deref]
    draw_super: DrawQuad,
    #[live]
    spinner_color: Vec3,
    #[live]
    spinner_track: Vec3,
    #[live]
    stroke_width: f32,
    #[live]
    arc_ratio: f32,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawMpProgress {
    #[deref]
    draw_super: DrawQuad,
    #[live]
    progress: f32,
    #[live]
    track_color: Vec3,
    #[live]
    fill_color: Vec3,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawMpSliderTrack {
    #[deref]
    draw_super: DrawQuad,
    #[live]
    progress_start: f32,
    #[live]
    progress_end: f32,
    #[live]
    track_color: Vec3,
    #[live]
    fill_color: Vec3,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawMpSliderThumb {
    #[deref]
    draw_super: DrawQuad,
    #[live]
    thumb_color: Vec3,
    #[live]
    border_color: Vec3,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawPlaybackButton {
    #[deref]
    draw_super: DrawQuad,
    #[live]
    bg_color: Vec3,
    #[live]
    border_color: Vec3,
    #[live]
    icon_color: Vec3,
    #[live]
    mode: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SliderValue {
    Single(f64),
    Range(f64, f64),
}

impl Default for SliderValue {
    fn default() -> Self {
        SliderValue::Single(0.0)
    }
}

#[derive(Clone, Debug, Default)]
pub enum MpSliderAction {
    Changed(SliderValue),
    #[default]
    None,
}

#[derive(Script, ScriptHook, Widget)]
pub struct TrackCanvas {
    #[uid]
    uid: WidgetUid,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[redraw]
    #[live]
    draw_track: DrawTrack,
    #[live]
    draw_start_marker: DrawMarker,
    #[live]
    draw_end_marker: DrawMarker,
    #[live]
    draw_halo: DrawHalo,
    #[live]
    draw_water: DrawWater,
    #[live]
    draw_map: DrawMapGrid,
    #[live]
    draw_particle: DrawParticle,

    #[rust]
    track: Option<Arc<Track>>,
    #[rust]
    geom: Option<TrackGeom>,
    #[rust]
    last_rect_size: Vec2,
    #[rust]
    walked_ratio: f32,
    #[rust]
    track_progress: f32,
    #[rust]
    hr_phase_val: f32,
    #[rust]
    guard_pulse: f32,
    #[rust]
    overlay_dim_val: f32,
    #[rust]
    sync_placeholder: bool,

    #[area]
    #[rust]
    area: Area,
}

#[derive(Script, ScriptHook, Widget)]
pub struct GuardEdge {
    #[uid]
    uid: WidgetUid,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[redraw]
    #[live]
    draw_edge: DrawGuardEdge,

    #[rust]
    pulse: f32,
    #[area]
    #[rust]
    area: Area,
}

#[derive(Script, ScriptHook, Widget)]
pub struct SyncSpinner {
    #[uid]
    uid: WidgetUid,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[redraw]
    #[live]
    draw_spinner: DrawSyncSpinner,

    #[area]
    #[rust]
    area: Area,
}

#[derive(Script, ScriptHook, Widget)]
pub struct PlaybackButton {
    #[uid]
    uid: WidgetUid,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[redraw]
    #[live]
    draw_button: DrawPlaybackButton,

    #[area]
    #[rust]
    area: Area,
}

#[derive(Script, ScriptHook, Widget)]
pub struct MpProgress {
    #[uid]
    uid: WidgetUid,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[redraw]
    #[live]
    draw_bg: DrawMpProgress,
    #[live]
    value: f64,

    #[area]
    #[rust]
    area: Area,
}

#[derive(Script, ScriptHook, Widget)]
pub struct MpSlider {
    #[uid]
    uid: WidgetUid,
    #[walk]
    walk: Walk,
    #[layout]
    layout: Layout,

    #[redraw]
    #[live]
    draw_track: DrawMpSliderTrack,
    #[live]
    draw_thumb: DrawMpSliderThumb,
    #[live]
    value: f64,
    #[live(0.0)]
    min: f64,
    #[live(100.0)]
    max: f64,
    #[live(1.0)]
    step: f64,
    #[live(false)]
    disabled: bool,

    #[rust]
    dragging: bool,

    #[area]
    #[rust]
    area: Area,
}

#[derive(Default)]
struct TrackGeom {
    rect_size: Vec2,
    segs: Vec<TrackSegment>,
    start_screen: Vec2,
    end_screen: Vec2,
}

#[derive(Default, Clone)]
struct TrackSegment {
    start: Vec2,
    end: Vec2,
    t_a: f32,
    t_b: f32,
    speed_a: f32,
    speed_b: f32,
}

#[derive(Clone, Copy)]
struct TrackNode {
    pos: Vec2,
    t: f32,
    speed: f32,
}

impl Widget for TrackCanvas {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        let rect = cx.turtle().rect();
        self.ensure_geom(rect);

        self.draw_map.draw_abs(cx, rect);

        let water_w = (rect.size.x * 0.42).max(40.0);
        let water_x = rect.pos.x + rect.size.x - water_w - 18.0;
        self.draw_water.draw_abs(
            cx,
            Rect {
                pos: dvec2(water_x, rect.pos.y + 80.0),
                size: dvec2(water_w, rect.size.y - 160.0),
            },
        );

        if let Some(geom) = self.geom.as_ref() {
            if !geom.segs.is_empty() {
                self.draw_track.begin_many_instances(cx);
                let pad: f64 = 16.0;
                let segs = geom.segs.clone();
                for seg in &segs {
                    let min_x = seg.start.x.min(seg.end.x) as f64 - pad;
                    let min_y = seg.start.y.min(seg.end.y) as f64 - pad;
                    let max_x = seg.start.x.max(seg.end.x) as f64 + pad;
                    let max_y = seg.start.y.max(seg.end.y) as f64 + pad;
                    let bb = Rect {
                        pos: dvec2(min_x, min_y),
                        size: dvec2(max_x - min_x, max_y - min_y),
                    };
                    self.draw_track.start_xy =
                        vec2(seg.start.x - min_x as f32, seg.start.y - min_y as f32);
                    self.draw_track.end_xy =
                        vec2(seg.end.x - min_x as f32, seg.end.y - min_y as f32);
                    self.draw_track.t_a = seg.t_a;
                    self.draw_track.t_b = seg.t_b;
                    self.draw_track.speed_a = seg.speed_a;
                    self.draw_track.speed_b = seg.speed_b;
                    self.draw_track.draw_abs(cx, bb);
                }
                self.draw_track.end_many_instances(cx);

                if !self.sync_placeholder {
                    let density = self.draw_track.particle_density.clamp(0.0, 1.0);
                    if density > 0.001 && self.walked_ratio > 0.001 {
                        self.draw_particle.begin_many_instances(cx);
                        let visual_density = 0.22 + density * 0.58;
                        let total_particles = (620.0 + visual_density * 760.0) as usize;
                        let walked = self.walked_ratio.clamp(0.0, 1.0);
                        for i in 0..total_particles {
                            let seed = i as u32;
                            let phase_jitter = hash01(seed.wrapping_mul(3).wrapping_add(1));
                            let phase = (((i as f32 + phase_jitter * 1.70)
                                / total_particles as f32)
                                * walked)
                                .min(walked);
                            let seg_idx = ((phase * segs.len() as f32) as usize)
                                .min(segs.len().saturating_sub(1));
                            let seg = &segs[seg_idx];
                            let local_t = hash01(seed.wrapping_mul(5).wrapping_add(2));
                            let base = vec2(
                                seg.start.x + (seg.end.x - seg.start.x) * local_t,
                                seg.start.y + (seg.end.y - seg.start.y) * local_t,
                            );
                            let tangent = seg.end - seg.start;
                            let len = tangent.length().max(1e-3);
                            let normal = vec2(-tangent.y / len, tangent.x / len);
                            let side = hash01(seed.wrapping_mul(7).wrapping_add(3)) * 2.0 - 1.0;
                            let head_distance = (walked - phase).max(0.0);
                            let head_boost = 1.0 - (head_distance / 0.20).clamp(0.0, 1.0);
                            let flow_boost = 1.0 - (head_distance / 0.36).clamp(0.0, 1.0);
                            let spark = hash01(seed.wrapping_mul(13).wrapping_add(5));
                            let keep_roll = hash01(seed.wrapping_mul(17).wrapping_add(6));
                            if keep_roll > 0.08 + flow_boost * 0.12 + head_boost * 0.56 + visual_density * 0.06 {
                                continue;
                            }
                            let outlier = smoothstep01(
                                0.88,
                                1.0,
                                hash01(seed.wrapping_mul(19).wrapping_add(7)),
                            );
                            let scatter = hash01(seed.wrapping_mul(11).wrapping_add(4)).powf(2.2)
                                * (1.0 + visual_density * 3.8)
                                + outlier * (2.2 + visual_density * 2.4);
                            let particle_pos = base + normal * side * scatter;
                            let particle_size = 1.1
                                + spark.powf(1.7) * 1.45
                                + head_boost * (1.2 + visual_density * 1.8);
                            let alpha = (0.10
                                + spark.powf(0.7) * 0.22
                                + flow_boost * 0.08
                                + head_boost * 0.46)
                                * visual_density;
                            if alpha < 0.04 {
                                continue;
                            }
                            let speed = (seg.speed_a + seg.speed_b) * 0.5;
                            self.draw_particle.particle_color = mix_vec3(
                                speed_ramp_color(speed),
                                vec3(0.0, 0.898, 1.0),
                                (flow_boost * 0.35 + head_boost * 0.55).clamp(0.0, 0.9),
                            );
                            self.draw_particle.particle_alpha = alpha.min(1.0);
                            self.draw_particle.particle_seed = seed as f32 * 1.618_034;
                            self.draw_particle.draw_abs(
                                cx,
                                Rect {
                                    pos: dvec2(
                                        particle_pos.x as f64 - particle_size as f64 * 0.5,
                                        particle_pos.y as f64 - particle_size as f64 * 0.5,
                                    ),
                                    size: dvec2(particle_size as f64, particle_size as f64),
                                },
                            );
                        }
                        self.draw_particle.end_many_instances(cx);
                    }

                    let marker_size: f64 = 14.0;
                    let marker_rect = |p: Vec2| Rect {
                        pos: dvec2(
                            p.x as f64 - marker_size * 0.5,
                            p.y as f64 - marker_size * 0.5,
                        ),
                        size: dvec2(marker_size, marker_size),
                    };
                    self.draw_start_marker.marker_color = vec3(0.063, 0.722, 0.506);
                    self.draw_start_marker.marker_radius = 5.0;
                    self.draw_start_marker
                        .draw_abs(cx, marker_rect(geom.start_screen));
                    self.draw_end_marker.marker_color = vec3(0.478, 0.482, 0.549);
                    self.draw_end_marker.marker_radius = 5.0;
                    self.draw_end_marker
                        .draw_abs(cx, marker_rect(geom.end_screen));

                    if let Some(cur) = lerp_segments(&geom.segs, self.walked_ratio) {
                        let halo_size: f64 = 32.0;
                        let halo_rect = Rect {
                            pos: dvec2(
                                cur.x as f64 - halo_size * 0.5,
                                cur.y as f64 - halo_size * 0.5,
                            ),
                            size: dvec2(halo_size, halo_size),
                        };
                        self.draw_halo.draw_abs(cx, halo_rect);
                    }

                    let big_marker_size: f64 = 18.0;
                    self.draw_start_marker.marker_color = vec3(0.063, 0.722, 0.506);
                    self.draw_start_marker.marker_radius = 7.0;
                    self.draw_start_marker.draw_abs(
                        cx,
                        Rect {
                            pos: dvec2(
                                geom.start_screen.x as f64 - big_marker_size * 0.5,
                                geom.start_screen.y as f64 - big_marker_size * 0.5,
                            ),
                            size: dvec2(big_marker_size, big_marker_size),
                        },
                    );
                    self.draw_end_marker.marker_color = vec3(0.478, 0.482, 0.549);
                    self.draw_end_marker.marker_radius = 7.0;
                    self.draw_end_marker.draw_abs(
                        cx,
                        Rect {
                            pos: dvec2(
                                geom.end_screen.x as f64 - big_marker_size * 0.5,
                                geom.end_screen.y as f64 - big_marker_size * 0.5,
                            ),
                            size: dvec2(big_marker_size, big_marker_size),
                        },
                    );
                }
            }
        }

        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}

fn ease_in_out_cubic(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        let f = 2.0 * t - 2.0;
        1.0 + f * f * f * 0.5
    }
}

fn ease_out_cubic(t: f64) -> f64 {
    let f = 1.0 - t;
    1.0 - f * f * f
}

fn lerp_segments(segs: &[TrackSegment], ratio: f32) -> Option<Vec2> {
    if segs.is_empty() {
        return None;
    }
    let r = ratio.clamp(0.0, 1.0);
    for seg in segs {
        if r <= seg.t_b {
            let span = (seg.t_b - seg.t_a).max(1e-6);
            let t = ((r - seg.t_a) / span).clamp(0.0, 1.0);
            return Some(vec2(
                seg.start.x + (seg.end.x - seg.start.x) * t,
                seg.start.y + (seg.end.y - seg.start.y) * t,
            ));
        }
    }
    Some(segs.last().unwrap().end)
}

fn hash01(mut x: u32) -> f32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    (x as f32) * (1.0 / u32::MAX as f32)
}

fn mix_vec3(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    let t = t.clamp(0.0, 1.0);
    vec3(
        a.x * (1.0 - t) + b.x * t,
        a.y * (1.0 - t) + b.y * t,
        a.z * (1.0 - t) + b.z * t,
    )
}

fn smoothstep01(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0).max(1e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn speed_ramp_color(speed: f32) -> Vec3 {
    let low = vec3(0.91, 0.91, 0.94);
    let mid = vec3(1.0, 0.541, 0.239);
    let high = vec3(0.0, 0.898, 1.0);
    if speed < 0.34 {
        mix_vec3(low, mid, smoothstep01(0.02, 0.32, speed))
    } else {
        mix_vec3(mid, high, smoothstep01(0.34, 0.76, speed))
    }
}

fn smooth_track_nodes(nodes: &[TrackNode]) -> Vec<TrackNode> {
    if nodes.len() < 3 {
        return nodes.to_vec();
    }

    let mut out = Vec::with_capacity(nodes.len() * 2);
    out.push(nodes[0]);
    for pair in nodes.windows(2) {
        let a = pair[0];
        let b = pair[1];
        let q = TrackNode {
            pos: vec2(
                a.pos.x * 0.75 + b.pos.x * 0.25,
                a.pos.y * 0.75 + b.pos.y * 0.25,
            ),
            t: a.t * 0.75 + b.t * 0.25,
            speed: a.speed * 0.75 + b.speed * 0.25,
        };
        let r = TrackNode {
            pos: vec2(
                a.pos.x * 0.25 + b.pos.x * 0.75,
                a.pos.y * 0.25 + b.pos.y * 0.75,
            ),
            t: a.t * 0.25 + b.t * 0.75,
            speed: a.speed * 0.25 + b.speed * 0.75,
        };
        out.push(q);
        out.push(r);
    }
    out.push(*nodes.last().unwrap());
    out
}

impl TrackCanvas {
    fn ensure_geom(&mut self, rect: Rect) {
        let rect_size = vec2(rect.size.x as f32, rect.size.y as f32);
        let needs_recompute = match self.geom.as_ref() {
            None => true,
            Some(g) => (g.rect_size - rect_size).length() > 1.0,
        };
        if !needs_recompute {
            return;
        }
        let Some(track) = self.track.as_ref() else {
            return;
        };
        if track.points.is_empty() {
            return;
        }
        let bounds = &track.stats.track_bounds;
        let lat_min = bounds.lat_min;
        let lat_max = bounds.lat_max;
        let lon_min = bounds.lon_min;
        let lon_max = bounds.lon_max;
        let lat_mid = ((lat_min + lat_max) * 0.5).to_radians();
        let cos_lat = lat_mid.cos().max(0.1);
        let lon_span = ((lon_max - lon_min) * cos_lat).max(1e-6);
        let lat_span = (lat_max - lat_min).max(1e-6);

        let pad: f32 = 24.0;
        let avail_w = (rect_size.x - pad * 2.0).max(1.0);
        let avail_h = (rect_size.y - pad * 2.0).max(1.0);
        let scale = (avail_w as f64 / lon_span).min(avail_h as f64 / lat_span);
        let mapped_w = (lon_span * scale) as f32;
        let mapped_h = (lat_span * scale) as f32;
        let off_x = pad + (avail_w - mapped_w) * 0.5;
        let off_y = pad + (avail_h - mapped_h) * 0.5;

        let project = |lat: f64, lon: f64| -> Vec2 {
            let x = off_x + ((lon - lon_min) * cos_lat * scale) as f32;
            let y = rect_size.y - off_y - ((lat - lat_min) * scale) as f32;
            vec2(x, y)
        };

        let n = track.points.len();
        let target_segs: usize = 520;
        let stride = (n / target_segs).max(1);
        let mut indices: Vec<usize> = (0..n).step_by(stride).collect();
        if *indices.last().unwrap() != n - 1 {
            indices.push(n - 1);
        }
        let speed_lo = track.stats.speed_min_mps;
        let speed_hi = track.stats.speed_max_mps.max(speed_lo + 1e-3);
        let normalize = |s: f32| ((s - speed_lo) / (speed_hi - speed_lo)).clamp(0.0, 1.0);

        let total_n = (n - 1).max(1) as f32;
        let nodes: Vec<TrackNode> = indices
            .iter()
            .map(|idx| {
                let point = &track.points[*idx];
                TrackNode {
                    pos: project(point.lat, point.lon),
                    t: *idx as f32 / total_n,
                    speed: normalize(point.speed_mps.unwrap_or(0.0)),
                }
            })
            .collect();
        let nodes = smooth_track_nodes(&nodes);
        let mut segs: Vec<TrackSegment> = Vec::with_capacity(nodes.len().saturating_sub(1));
        for pair in nodes.windows(2) {
            let a = pair[0];
            let b = pair[1];
            segs.push(TrackSegment {
                start: a.pos,
                end: b.pos,
                t_a: a.t,
                t_b: b.t,
                speed_a: a.speed,
                speed_b: b.speed,
            });
        }

        let start_screen = if let Some(p) = track.points.first() {
            project(p.lat, p.lon)
        } else {
            Vec2::default()
        };
        let end_screen = if let Some(p) = track.points.last() {
            project(p.lat, p.lon)
        } else {
            Vec2::default()
        };

        self.geom = Some(TrackGeom {
            rect_size,
            segs,
            start_screen,
            end_screen,
        });
        self.last_rect_size = rect_size;
    }

    pub fn set_track(&mut self, cx: &mut Cx, track: Option<Arc<Track>>) {
        let changed = match (&self.track, &track) {
            (None, None) => false,
            (Some(a), Some(b)) => !Arc::ptr_eq(a, b),
            _ => true,
        };
        if changed {
            self.track = track;
            self.geom = None;
            self.area.redraw(cx);
        }
    }

    pub fn set_progress(&mut self, cx: &mut Cx, walked_ratio: f32, track_progress: f32) {
        self.walked_ratio = walked_ratio;
        self.track_progress = track_progress;
        self.draw_track.walked_segment_ratio = walked_ratio;
        self.draw_track.track_progress = track_progress;
        self.area.redraw(cx);
    }

    pub fn set_sync_placeholder(&mut self, cx: &mut Cx, enabled: bool) {
        self.sync_placeholder = enabled;
        self.draw_track.sync_placeholder = if enabled { 1.0 } else { 0.0 };
        self.area.redraw(cx);
    }

    pub fn set_hr_phase(&mut self, cx: &mut Cx, hr_phase: f32) {
        self.hr_phase_val = hr_phase;
        self.draw_halo.hr_phase = hr_phase;
        self.area.redraw(cx);
    }

    pub fn set_guard_pulse(&mut self, cx: &mut Cx, pulse: f32) {
        self.guard_pulse = pulse;
        self.draw_track.guard_pulse_phase = pulse;
        self.draw_halo.guard_pulse_phase = pulse;
        self.area.redraw(cx);
    }

    pub fn set_overlay_dim(&mut self, cx: &mut Cx, dim: f32) {
        self.overlay_dim_val = dim;
        self.draw_track.overlay_dim = dim;
        self.area.redraw(cx);
    }

    pub fn set_scrubber_echo(&mut self, cx: &mut Cx, phase: f32) {
        self.draw_track.scrubber_echo_phase = phase;
        self.area.redraw(cx);
    }

    pub fn set_particle_density(&mut self, cx: &mut Cx, density: f32) {
        self.draw_track.particle_density = density.clamp(0.0, 1.0);
        self.area.redraw(cx);
    }

    pub fn marker_layout(&self) -> Option<(Vec2, Vec2, Vec2)> {
        self.geom
            .as_ref()
            .map(|g| (g.start_screen, g.end_screen, g.rect_size))
    }
}

impl Widget for GuardEdge {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        let rect = cx.turtle().rect();
        self.draw_edge.guard_pulse_phase = self.pulse;
        self.draw_edge.draw_abs(cx, rect);
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}

impl GuardEdge {
    pub fn set_pulse(&mut self, cx: &mut Cx, pulse: f32) {
        self.pulse = pulse;
        self.area.redraw(cx);
    }
}

impl Widget for SyncSpinner {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        let rect = cx.turtle().rect();
        self.draw_spinner.draw_abs(cx, rect);
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}

impl SyncSpinner {
    pub fn redraw(&mut self, cx: &mut Cx) {
        self.area.redraw(cx);
    }
}

impl Widget for PlaybackButton {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        let rect = cx.turtle().rect();
        self.draw_button.draw_abs(cx, rect);
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}

impl PlaybackButton {
    pub fn set_playing(&mut self, cx: &mut Cx, playing: bool) {
        self.draw_button.mode = if playing { 1.0 } else { 0.0 };
        self.area.redraw(cx);
    }
}

impl Widget for MpProgress {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        let rect = cx.turtle().rect();
        self.draw_bg.progress = (self.value / 100.0).clamp(0.0, 1.0) as f32;
        self.draw_bg.draw_abs(cx, rect);
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}

impl MpProgress {
    pub fn set_value(&mut self, cx: &mut Cx, value: f64) {
        self.value = value.clamp(0.0, 100.0);
        self.area.redraw(cx);
    }
}

impl Widget for MpSlider {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, _scope: &mut Scope) {
        if self.disabled {
            return;
        }

        match event.hits(cx, self.area) {
            Hit::FingerHoverIn(_) => {
                cx.set_cursor(MouseCursor::Hand);
            }
            Hit::FingerHoverOver(_) => {
                cx.set_cursor(MouseCursor::Hand);
            }
            Hit::FingerDown(fe) => {
                self.dragging = true;
                self.update_value_from_position(cx, fe.abs);
            }
            Hit::FingerMove(fe) if self.dragging => {
                self.update_value_from_position(cx, fe.abs);
            }
            Hit::FingerUp(_) => {
                self.dragging = false;
            }
            Hit::FingerHoverOut(_) if !self.dragging => {
                cx.set_cursor(MouseCursor::Default);
            }
            _ => {}
        }
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        let rect = cx.turtle().rect();
        let progress = self.value_to_progress(self.value) as f32;
        self.draw_track.progress_start = 0.0;
        self.draw_track.progress_end = progress;

        let track_thickness = 6.0;
        let thumb_size = 18.0;
        let track_x = thumb_size * 0.5;
        let track_rect = Rect {
            pos: dvec2(
                rect.pos.x + track_x,
                rect.pos.y + (rect.size.y - track_thickness) * 0.5,
            ),
            size: dvec2((rect.size.x - thumb_size).max(1.0), track_thickness),
        };
        self.draw_track.draw_abs(cx, track_rect);

        let thumb_x = rect.pos.x + (rect.size.x - thumb_size).max(0.0) * progress as f64;
        let thumb_rect = Rect {
            pos: dvec2(thumb_x, rect.pos.y + (rect.size.y - thumb_size) * 0.5),
            size: dvec2(thumb_size, thumb_size),
        };
        self.draw_thumb.draw_abs(cx, thumb_rect);
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}

impl MpSlider {
    fn value_to_progress(&self, value: f64) -> f64 {
        if self.max <= self.min {
            return 0.0;
        }
        ((value - self.min) / (self.max - self.min)).clamp(0.0, 1.0)
    }

    fn progress_to_value(&self, progress: f64) -> f64 {
        self.min + (self.max - self.min) * progress.clamp(0.0, 1.0)
    }

    fn position_to_progress(&self, cx: &Cx, pos: DVec2) -> f64 {
        let rect = self.area.rect(cx);
        let thumb_radius = 9.0;
        let track_start = rect.pos.x + thumb_radius;
        let track_width = rect.size.x - thumb_radius * 2.0;
        if track_width <= 0.0 {
            return 0.0;
        }
        ((pos.x - track_start).clamp(0.0, track_width)) / track_width
    }

    fn update_value_from_position(&mut self, cx: &mut Cx, pos: DVec2) {
        let progress = self.position_to_progress(cx, pos);
        let raw_value = self.progress_to_value(progress);
        let stepped_value = if self.step > 0.0 {
            let steps = ((raw_value - self.min) / self.step).round();
            self.min + steps * self.step
        } else {
            raw_value
        };
        let new_value = stepped_value.clamp(self.min, self.max);
        if (new_value - self.value).abs() > f64::EPSILON {
            self.value = new_value;
            cx.widget_action(
                self.widget_uid(),
                MpSliderAction::Changed(SliderValue::Single(self.value)),
            );
            self.area.redraw(cx);
        }
    }

    pub fn changed(&self, actions: &Actions) -> Option<SliderValue> {
        if let Some(item) = actions.find_widget_action(self.widget_uid()) {
            if let MpSliderAction::Changed(value) = item.cast() {
                return Some(value);
            }
        }
        None
    }

    pub fn set_single_value(&mut self, cx: &mut Cx, value: f64) {
        self.value = value.clamp(self.min, self.max);
        self.area.redraw(cx);
    }
}

#[derive(Default)]
struct GuardWindow {
    start_idx: usize,
    end_idx: usize,
    valid: bool,
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,

    #[rust]
    track: Option<Arc<Track>>,
    #[rust]
    state: PlaybackState,
    #[rust]
    user: UserProfile,
    #[rust]
    network_rx: Option<Receiver<FetchResult>>,
    #[rust]
    worker_thread_id: Option<std::thread::ThreadId>,
    #[rust]
    pending_fetch: Option<FetchResult>,
    #[rust]
    fetching_started_at_secs: Option<f64>,

    #[rust]
    phase: i32,
    #[rust]
    now_secs: f64,
    #[rust]
    last_now_secs: f64,
    #[rust]
    phase_entered_secs: f64,
    #[rust]
    success_entered_secs: Option<f64>,

    #[rust]
    guard_window: GuardWindow,
    #[rust]
    guard_active_started_at_secs: f64,
    #[rust]
    guard_card_visible: bool,
    #[rust]
    last_scrubber_drag_secs: f64,

    #[rust]
    reduced_motion: bool,

    #[rust]
    next_frame: NextFrame,
}

impl App {
    fn ensure_bundled_track(&mut self) {
        if self.track.is_some() {
            return;
        }
        match parse_gpx(BUNDLED_GPX) {
            Ok(t) => self.track = Some(Arc::new(t)),
            Err(e) => log!("bundled GPX parse failed: {}", e),
        }
    }

    fn route_name(&self) -> &str {
        self.track
            .as_ref()
            .map(|t| t.route_name.as_str())
            .unwrap_or("—")
    }

    fn route_title_lines(&self) -> (String, String) {
        let route = self.route_name().trim();
        if route.is_empty() {
            return ("—".to_string(), String::new());
        }

        let mut scan_from = 0usize;
        for token in route.split_whitespace() {
            let Some(rel_start) = route[scan_from..].find(token) else {
                continue;
            };
            let start = scan_from + rel_start;
            let end = start + token.len();
            scan_from = end;

            if token.starts_with('#') {
                let primary = route[..end].trim();
                let secondary = route[end..].trim();
                if !secondary.is_empty() {
                    return (primary.to_string(), secondary.to_string());
                }
            }
        }

        (route.to_string(), String::new())
    }

    fn refresh_top_bar(&mut self, cx: &mut Cx) {
        let sync_text = self.state.sync_status_text();
        let (route_primary, route_secondary) = self.route_title_lines();
        let profile_label = self.state.profile.label_zh();
        self.ui
            .label(cx, ids!(route_title_primary))
            .set_text(cx, &route_primary);
        self.ui
            .label(cx, ids!(route_title_secondary))
            .set_text(cx, &route_secondary);
        self.ui
            .label(cx, ids!(sync_badge_label))
            .set_text(cx, sync_text);
        self.ui
            .label(cx, ids!(profile_label))
            .set_text(cx, profile_label);
        let dot_color: [f32; 4] = match self.state.network_state {
            NetworkState::Idle | NetworkState::Fetching => [0.95, 0.74, 0.18, 1.0],
            NetworkState::Success => [0.29, 0.87, 0.50, 1.0],
            NetworkState::Fallback => [0.48, 0.48, 0.55, 1.0],
        };
        self.ui
            .view(cx, ids!(sync_badge_dot))
            .set_uniform(cx, live_id!(color), &dot_color);
    }

    fn refresh_sync_overlay(&mut self, cx: &mut Cx) {
        let main_text = match self.state.network_state {
            NetworkState::Idle | NetworkState::Fetching => "同步中...",
            NetworkState::Success => "已同步",
            NetworkState::Fallback => "本地缓存",
        };
        let sub = match self.state.network_state {
            NetworkState::Idle | NetworkState::Fetching => "正在同步轨迹数据",
            NetworkState::Success => "数据来自 trajectory-replay-data 仓库",
            NetworkState::Fallback => "已退回 assets/cycling-track.gpx",
        };
        self.ui
            .label(cx, ids!(sync_overlay_label))
            .set_text(cx, main_text);
        self.ui
            .label(cx, ids!(sync_overlay_subtext))
            .set_text(cx, sub);
    }

    fn refresh_legend_max(&mut self, cx: &mut Cx) {
        let max_speed = self
            .track
            .as_ref()
            .map(|t| t.stats.speed_max_mps_ceil)
            .unwrap_or(0.0);
        let txt = format!("{}", max_speed as i32);
        self.ui.label(cx, ids!(legend_max_label)).set_text(cx, &txt);
    }

    fn poll_network(&mut self, cx: &mut Cx, now_secs: f64) {
        const MIN_FETCHING_VISIBLE_SECS: f64 = 1.2;

        let started = match self.fetching_started_at_secs {
            Some(t) => t,
            None => {
                self.fetching_started_at_secs = Some(now_secs);
                self.state.network_state_entered_at_secs = now_secs;
                now_secs
            }
        };
        let elapsed = now_secs - started;

        if let Some(pending) = self.pending_fetch.take() {
            if elapsed >= MIN_FETCHING_VISIBLE_SECS {
                self.apply_fetch_result(cx, pending, now_secs);
                return;
            }
            self.pending_fetch = Some(pending);
        }

        let Some(rx) = &self.network_rx else { return };
        if let Ok(result) = rx.try_recv() {
            self.network_rx = None;
            if elapsed < MIN_FETCHING_VISIBLE_SECS {
                self.pending_fetch = Some(result);
            } else {
                self.apply_fetch_result(cx, result, now_secs);
            }
        }
    }

    fn apply_fetch_result(&mut self, cx: &mut Cx, result: FetchResult, now_secs: f64) {
        match result {
            FetchResult::Success(track) => {
                self.track = Some(Arc::new(track));
                self.state.network_state = NetworkState::Success;
                self.state.data_source = DataSource::Network;
                self.success_entered_secs = Some(now_secs);
            }
            FetchResult::Fallback(reason) => {
                log!("fallback to bundled GPX: {}", reason);
                self.state.network_state = NetworkState::Fallback;
                self.state.data_source = DataSource::LocalFallback;
                self.success_entered_secs = Some(now_secs);
            }
        }
        self.state.network_state_entered_at_secs = now_secs;
        self.compute_guard_window();
        self.refresh_top_bar(cx);
        self.refresh_sync_overlay(cx);
        self.refresh_legend_max(cx);
    }

    fn compute_guard_window(&mut self) {
        let Some(track) = self.track.as_ref().cloned() else {
            return;
        };
        if std::env::var("MOBILE_EXAMPLE_DEMO_GUARD").is_ok() || demo_stage() == Some(DemoStage::S3)
        {
            let n = track.points.len();
            self.guard_window = GuardWindow {
                start_idx: ((n as f32) * 0.30) as usize,
                end_idx: ((n as f32) * 0.50) as usize,
                valid: true,
            };
            return;
        }
        let max_hr = effective_max_hr(&self.user, &track) as f32;
        let threshold = max_hr * 0.92;
        for (i, _p) in track.points.iter().enumerate() {
            if let Some(avg) = five_minute_window_avg_hr(&track, i) {
                if avg > threshold {
                    let mut end = i;
                    for (j, _) in track.points.iter().enumerate().skip(i) {
                        match five_minute_window_avg_hr(&track, j) {
                            Some(a) if a > threshold => end = j,
                            _ => break,
                        }
                    }
                    self.guard_window = GuardWindow {
                        start_idx: i,
                        end_idx: end,
                        valid: true,
                    };
                    return;
                }
            }
        }
        self.guard_window = GuardWindow::default();
    }

    fn maybe_advance_phase(&mut self, cx: &mut Cx, now: f64) {
        let dt = if self.last_now_secs <= 0.0 {
            0.0
        } else {
            (now - self.last_now_secs).max(0.0)
        };
        self.last_now_secs = now;
        let stage = demo_stage();
        let frozen_s0 = stage == Some(DemoStage::S0);
        let mut track_progress_v: f32 = 0.0;
        let mut walked_ratio_v: f32 = 0.0;
        let mut overlay_dim_v: f32 = 0.0;
        if frozen_s0 {
            self.phase = PHASE_SYNCING;
            self.state.network_state = NetworkState::Fetching;
            self.state.playback_progress = 0.0;
            self.state.walked_segment_ratio = 0.0;
            self.state.contract_guard_active = false;
        }
        match self.phase {
            PHASE_SYNCING
                if !frozen_s0
                    && matches!(
                        self.state.network_state,
                        NetworkState::Success | NetworkState::Fallback
                    ) =>
            {
                if let Some(t) = self.success_entered_secs {
                    if now - t >= SUCCESS_LABEL_VISIBLE_SECS {
                        self.enter_phase(cx, PHASE_PATH_DRAW, now);
                    }
                }
            }
            PHASE_SYNCING => {}
            PHASE_PATH_DRAW => {
                let elapsed = now - self.phase_entered_secs;
                let raw = (elapsed / PATH_DRAW_DURATION_SECS).clamp(0.0, 1.0);
                track_progress_v = ease_in_out_cubic(raw) as f32;
                walked_ratio_v = 0.0;
                if raw >= 1.0 {
                    self.enter_phase(cx, PHASE_PLAYBACK, now);
                }
            }
            PHASE_PLAYBACK => {
                if let Some(track) = self.track.as_ref().cloned() {
                    let total = (track.stats.duration_ms_total.max(1)) as f64;
                    if !self.state.is_paused {
                        let inc = (dt * self.state.playback_speed as f64) * 1000.0 / total;
                        let new_p =
                            (self.state.playback_progress as f64 + inc).clamp(0.0, 1.0) as f32;
                        self.state.apply_progress(&track, new_p);
                    }
                    track_progress_v = 1.0;
                    walked_ratio_v = self.state.playback_progress;

                    self.update_guard(cx, &track, now);
                    self.refresh_hud(cx, &track);
                    self.refresh_scrubber_labels(cx, &track);

                    if self.state.playback_progress >= STATS_PROGRESS_THRESHOLD {
                        self.enter_phase(cx, PHASE_STATS, now);
                    }
                }
            }
            PHASE_STATS => {
                track_progress_v = 1.0;
                walked_ratio_v = self.state.playback_progress.max(STATS_PROGRESS_THRESHOLD);
                let raw = ((now - self.phase_entered_secs) / 0.6).clamp(0.0, 1.0);
                overlay_dim_v = ease_out_cubic(raw) as f32;
            }
            _ => {}
        }

        let echo_age = (now - self.last_scrubber_drag_secs).max(0.0);
        let scrubber_echo_v = if self.reduced_motion {
            0.0
        } else if echo_age < SCRUBBER_ECHO_FADE_SECS {
            let t = 1.0 - (echo_age / SCRUBBER_ECHO_FADE_SECS).max(0.0);
            ease_out_cubic(t.max(0.0)) as f32
        } else {
            0.0
        };
        self.state.scrubber_echo_phase = scrubber_echo_v;

        let guard_age = (now - self.guard_active_started_at_secs).max(0.0);
        let guard_pulse_v = if self.state.contract_guard_active {
            let initial = (1.0 - (guard_age / GUARD_PULSE_DURATION_SECS)).max(0.0) as f32;
            if self.reduced_motion {
                // reduced-motion: 不振荡, 只 fade out 一次, 红边静态显示 1.5s 后消失
                initial
            } else {
                let steady = (0.55 + 0.25 * (now * 8.0).sin()) as f32;
                initial.max(steady)
            }
        } else {
            0.0
        };

        let (hr_phase_now, _) = if self.reduced_motion {
            // reduced-motion: phase 固定 0.5, halo 直径锁定中间值不呼吸
            (0.5_f32, 0.0_f32)
        } else {
            self.hr_phase(now)
        };

        let particle_density_v = if self.reduced_motion || frozen_s0 {
            0.0
        } else {
            self.track
                .as_ref()
                .map(|track| {
                    let lo = track.stats.speed_min_mps;
                    let hi = track.stats.speed_max_mps.max(lo + 1e-3);
                    ((self.state.current_speed_mps - lo) / (hi - lo)).clamp(0.0, 1.0)
                })
                .unwrap_or(0.0)
        };

        let track_arc = self.track.clone();
        let canvas_ref = self.ui.widget(cx, ids!(track_canvas));
        let mut layout: Option<(Vec2, Vec2, Vec2)> = None;
        if let Some(mut canvas) = canvas_ref.borrow_mut::<TrackCanvas>() {
            canvas.set_track(cx, track_arc);
            canvas.set_progress(cx, walked_ratio_v, track_progress_v);
            canvas.set_sync_placeholder(cx, frozen_s0);
            canvas.set_hr_phase(cx, hr_phase_now);
            canvas.set_guard_pulse(cx, guard_pulse_v);
            canvas.set_overlay_dim(cx, overlay_dim_v);
            canvas.set_scrubber_echo(cx, scrubber_echo_v);
            canvas.set_particle_density(cx, particle_density_v);
            layout = canvas.marker_layout();
        }
        if self.phase == PHASE_SYNCING {
            if let Some(mut spinner) = self
                .ui
                .widget(cx, ids!(sync_spinner))
                .borrow_mut::<SyncSpinner>()
            {
                spinner.redraw(cx);
            }
        }
        if let Some(mut guard_edge) = self
            .ui
            .widget(cx, ids!(guard_edge))
            .borrow_mut::<GuardEdge>()
        {
            guard_edge.set_pulse(cx, guard_pulse_v);
        }
        let canvas_rect = canvas_ref.area().rect(cx);
        if let Some((start, end, _rect_sz)) = layout {
            let cx_x = canvas_rect.pos.x;
            let cx_y = canvas_rect.pos.y;
            let w = canvas_rect.size.x;
            let h = canvas_rect.size.y;
            let start_x = (cx_x + start.x as f64 - 130.0).clamp(cx_x + 8.0, cx_x + w - 130.0);
            let start_y = (cx_y + start.y as f64 - 4.0).clamp(cx_y + 8.0, cx_y + h - 24.0);
            let end_x = (cx_x + end.x as f64 + 14.0).clamp(cx_x + 8.0, cx_x + w - 112.0);
            let end_y = (cx_y + end.y as f64 - 22.0).clamp(cx_y + 8.0, cx_y + h - 24.0);
            self.set_view_abs(cx, ids!(start_marker_label_box), start_x, start_y);
            self.set_view_abs(cx, ids!(end_marker_label_box), end_x, end_y);
            self.set_view_abs(
                cx,
                ids!(place_label_1_box),
                cx_x + w * 0.18,
                cx_y + h * 0.20,
            );
            self.set_view_abs(
                cx,
                ids!(place_label_2_box),
                cx_x + w * 0.55,
                cx_y + h * 0.42,
            );
            self.set_view_abs(
                cx,
                ids!(place_label_3_box),
                cx_x + w * 0.20,
                cx_y + h * 0.62,
            );
            self.set_view_abs(
                cx,
                ids!(place_label_4_box),
                cx_x + w * 0.78,
                cx_y + h * 0.80,
            );
        }

        self.refresh_top_bar(cx);
        self.refresh_sync_overlay(cx);
    }

    fn hr_phase(&self, now: f64) -> (f32, f32) {
        let bpm = self.state.current_hr_bpm.unwrap_or(60).max(40);
        let hz = bpm as f64 / 60.0;
        let p = ((now * hz) % 1.0) as f32;
        (p, hz as f32)
    }

    fn enter_phase(&mut self, cx: &mut Cx, phase: i32, now: f64) {
        self.phase = phase;
        self.phase_entered_secs = now;
        match phase {
            PHASE_PATH_DRAW => {
                self.ui.view(cx, ids!(sync_overlay)).set_visible(cx, false);
            }
            PHASE_STATS => {
                if let Some(track) = self.track.as_ref().cloned() {
                    self.fill_stats(cx, &track);
                }
                self.ui.view(cx, ids!(stats_overlay)).set_visible(cx, true);
                self.ui.view(cx, ids!(label_overlay)).set_visible(cx, false);
                self.ui.view(cx, ids!(right_overlay)).set_visible(cx, false);
            }
            _ => {}
        }
    }

    fn update_guard(&mut self, cx: &mut Cx, _track: &Track, now: f64) {
        if !self.guard_window.valid {
            return;
        }
        let in_window = self.state.current_trkpt_index >= self.guard_window.start_idx
            && self.state.current_trkpt_index <= self.guard_window.end_idx;
        if in_window && !self.state.contract_guard_active {
            self.state.contract_guard_active = true;
            self.guard_active_started_at_secs = now;
            if !self.guard_card_visible {
                self.guard_card_visible = true;
                self.ui.view(cx, ids!(guard_card)).set_visible(cx, true);
            }
        }
        if self.state.contract_guard_active {
            let age = now - self.guard_active_started_at_secs;
            if !in_window && age >= GUARD_PULSE_DURATION_SECS {
                self.state.contract_guard_active = false;
            }
        }
    }

    fn refresh_hud(&mut self, cx: &mut Cx, track: &Track) {
        let s_kmh = self.state.current_speed_mps * 3.6;
        let s_lo = track.stats.speed_min_mps;
        let s_hi = track.stats.speed_max_mps.max(s_lo + 1e-3);
        let s_ratio = ((self.state.current_speed_mps - s_lo) / (s_hi - s_lo)).clamp(0.0, 1.0);

        let hr_text = match self.state.current_hr_bpm {
            Some(h) => format!("{}", h),
            None => "—".to_string(),
        };
        let hr_lo = track.stats.hr_min as f32;
        let hr_hi = (track.stats.hr_max as f32).max(hr_lo + 1.0);
        let hr_ratio = self
            .state
            .current_hr_bpm
            .map(|h| ((h as f32 - hr_lo) / (hr_hi - hr_lo)).clamp(0.0, 1.0))
            .unwrap_or(0.0);

        let ele_text = match self.state.current_ele_m {
            Some(e) => format!("{}", e as i32),
            None => "—".to_string(),
        };
        let ele_lo = track.stats.ele_min;
        let ele_hi = track.stats.ele_max.max(ele_lo + 1.0);
        let ele_ratio = self
            .state
            .current_ele_m
            .map(|e| ((e - ele_lo) / (ele_hi - ele_lo)).clamp(0.0, 1.0))
            .unwrap_or(0.0);

        let cad_text = match self.state.current_cad_rpm {
            Some(c) => format!("{}", c),
            None => "—".to_string(),
        };
        let cad_lo = track.stats.cad_min as f32;
        let cad_hi = (track.stats.cad_max as f32).max(cad_lo + 1.0);
        let cad_ratio = self
            .state
            .current_cad_rpm
            .map(|c| ((c as f32 - cad_lo) / (cad_hi - cad_lo)).clamp(0.0, 1.0))
            .unwrap_or(0.0);

        self.ui
            .label(cx, ids!(hud_speed_value))
            .set_text(cx, &format!("{:.1}", s_kmh));
        self.ui.label(cx, ids!(hud_hr_value)).set_text(cx, &hr_text);
        self.ui
            .label(cx, ids!(hud_ele_value))
            .set_text(cx, &ele_text);
        self.ui
            .label(cx, ids!(hud_cad_value))
            .set_text(cx, &cad_text);

        set_progress_value(cx, &self.ui, ids!(hud_speed_bar), s_ratio);
        set_progress_value(cx, &self.ui, ids!(hud_hr_bar), hr_ratio);
        set_progress_value(cx, &self.ui, ids!(hud_ele_bar), ele_ratio);
        set_progress_value(cx, &self.ui, ids!(hud_cad_bar), cad_ratio);
    }

    fn refresh_scrubber_labels(&mut self, cx: &mut Cx, track: &Track) {
        let total_secs = (track.stats.duration_ms_total / 1000).max(0);
        let current_secs = (total_secs as f32 * self.state.playback_progress) as i64;
        let cur = format_mmss(current_secs);
        let tot = format_mmss(total_secs);
        self.ui
            .label(cx, ids!(current_time_label))
            .set_text(cx, &cur);
        self.ui.label(cx, ids!(total_time_label)).set_text(cx, &tot);

        if let Some(mut slider) = self
            .ui
            .widget(cx, ids!(scrubber_slider))
            .borrow_mut::<MpSlider>()
        {
            slider.set_single_value(
                cx,
                self.state.playback_progress.clamp(0.0, 1.0) as f64 * 100.0,
            );
        }
    }

    fn set_view_abs(&mut self, cx: &mut Cx, id: &[LiveId], x: f64, y: f64) {
        let view = self.ui.view(cx, id);
        if let Some(mut v) = view.borrow_mut() {
            v.walk.abs_pos = Some(dvec2(x, y));
        }
        view.redraw(cx);
    }

    fn refresh_pause_glyph(&mut self, cx: &mut Cx) {
        let paused = self.state.is_paused || demo_stage() == Some(DemoStage::S0);
        if let Some(mut button) = self
            .ui
            .widget(cx, ids!(pause_button))
            .borrow_mut::<PlaybackButton>()
        {
            button.set_playing(cx, !paused);
        }
    }

    fn refresh_speed_buttons(&mut self, cx: &mut Cx) {
        let active = self.state.playback_speed.round() as i32;
        for (visual_path, label_path, val) in [
            (
                ids!(speed_1x_visual) as &[LiveId],
                ids!(speed_1x_label) as &[LiveId],
                1,
            ),
            (
                ids!(speed_4x_visual) as &[LiveId],
                ids!(speed_4x_label) as &[LiveId],
                4,
            ),
            (
                ids!(speed_16x_visual) as &[LiveId],
                ids!(speed_16x_label) as &[LiveId],
                16,
            ),
        ] {
            let view = self.ui.view(cx, visual_path);
            let active_color = [0.157, 0.780, 0.910, 1.0];
            let inactive_color = [0.078, 0.078, 0.110, 1.0];
            let c = if val == active {
                active_color
            } else {
                inactive_color
            };
            view.set_uniform(cx, live_id!(color), &c);
            view.redraw(cx);
            self.ui.label(cx, label_path).set_text(
                cx,
                match val {
                    1 => "1x",
                    4 => "4x",
                    _ => "16x",
                },
            );
        }
    }

    fn fill_stats(&mut self, cx: &mut Cx, track: &Track) {
        let dist_km = track.stats.distance_m_total / 1000.0;
        let dur_secs = (track.stats.duration_ms_total / 1000).max(0);
        let climb = track.stats.elevation_gain_m as i32;
        let avg_hr = track.stats.avg_hr as i32;
        self.ui
            .label(cx, ids!(stat_distance_value))
            .set_text(cx, &format!("{:.1} km", dist_km));
        self.ui
            .label(cx, ids!(stat_duration_value))
            .set_text(cx, &format_mmss(dur_secs));
        self.ui
            .label(cx, ids!(stat_climb_value))
            .set_text(cx, &format!("{} m", climb));
        self.ui
            .label(cx, ids!(stat_avg_hr_value))
            .set_text(cx, &format!("{} bpm", avg_hr));
    }
}

fn set_progress_value(cx: &mut Cx, ui: &WidgetRef, progress_id: &[LiveId], ratio: f32) {
    if let Some(mut progress) = ui.widget(cx, progress_id).borrow_mut::<MpProgress>() {
        progress.set_value(cx, ratio.clamp(0.0, 1.0) as f64 * 100.0);
    }
}

fn format_mmss(total_secs: i64) -> String {
    let secs = total_secs.max(0);
    let m = secs / 60;
    let s = secs % 60;
    format!("{:02}:{:02}", m, s)
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        self.ensure_bundled_track();
        self.user = UserProfile::default();
        self.state = PlaybackState::default();
        self.state.profile = TrajectoryProfile::Cycling;
        self.state.network_state = NetworkState::Fetching;
        self.state.data_source = DataSource::LocalFallback;
        self.phase = PHASE_SYNCING;
        self.phase_entered_secs = 0.0;
        self.last_now_secs = 0.0;
        let stage = demo_stage();
        let frozen_s0 = stage == Some(DemoStage::S0);
        self.state.is_paused = false;

        match stage {
            Some(DemoStage::S0) => {
                self.state.is_paused = true;
            }
            Some(stage @ (DemoStage::S2 | DemoStage::S3)) => {
                self.state.network_state = NetworkState::Fallback;
                self.state.data_source = DataSource::LocalFallback;
                self.phase = PHASE_PLAYBACK;
                self.state.playback_progress = demo_seek(stage.default_seek());
            }
            Some(stage @ DemoStage::S4) => {
                self.state.network_state = NetworkState::Fallback;
                self.state.data_source = DataSource::LocalFallback;
                self.state.is_paused = true;
                self.phase = PHASE_STATS;
                self.state.playback_progress = demo_seek(stage.default_seek());
            }
            None => {
                self.state.playback_progress = demo_seek(0.0);
            }
        }
        if !frozen_s0 {
            if let Some(t) = self.track.clone() {
                self.state.apply_progress(&t, self.state.playback_progress);
            }
        }
        self.compute_guard_window();

        if stage.is_some() {
            self.network_rx = None;
            self.worker_thread_id = None;
            self.fetching_started_at_secs = if frozen_s0 { Some(0.0) } else { None };
        } else {
            let (rx, tid) = spawn_fetch_worker();
            self.network_rx = Some(rx);
            self.worker_thread_id = Some(tid);
            self.fetching_started_at_secs = None;
        }
        self.pending_fetch = None;
        self.guard_card_visible = false;
        self.last_scrubber_drag_secs = -10.0;

        // reduced-motion 一次性检测 (G3 / ui-ux-pro-max Severity High a11y).
        // 双路径检测: (a) PC env var ANIMATOR_DURATION_SCALE 用于开发测试, (b) Android JNI 读
        // Settings.Global.ANIMATOR_DURATION_SCALE 用于真机 (P11.2). 任一为 0 即降级.
        let env_var_reduced = std::env::var("ANIMATOR_DURATION_SCALE")
            .ok()
            .and_then(|s| s.parse::<f32>().ok())
            .map(|scale| scale == 0.0)
            .unwrap_or(false);
        self.reduced_motion = env_var_reduced || detect_android_reduced_motion();
        if self.reduced_motion {
            log!("reduced-motion: scrubber-echo / guard-pulse / hr-pulse 全部降级");
        }

        self.refresh_top_bar(cx);
        self.refresh_sync_overlay(cx);
        self.refresh_legend_max(cx);
        self.refresh_pause_glyph(cx);
        self.refresh_speed_buttons(cx);
        if let Some(track) = self.track.clone() {
            if !frozen_s0 {
                self.refresh_hud(cx, &track);
            }
            self.refresh_scrubber_labels(cx, &track);
            if self.phase == PHASE_STATS {
                self.fill_stats(cx, &track);
            }
        }
        match stage {
            Some(DemoStage::S2 | DemoStage::S3) => {
                self.ui.view(cx, ids!(sync_overlay)).set_visible(cx, false);
                self.ui.view(cx, ids!(stats_overlay)).set_visible(cx, false);
                self.ui.view(cx, ids!(label_overlay)).set_visible(cx, true);
                self.ui.view(cx, ids!(right_overlay)).set_visible(cx, true);
            }
            Some(DemoStage::S4) => {
                self.ui.view(cx, ids!(sync_overlay)).set_visible(cx, false);
                self.ui.view(cx, ids!(stats_overlay)).set_visible(cx, true);
                self.ui.view(cx, ids!(label_overlay)).set_visible(cx, false);
                self.ui.view(cx, ids!(right_overlay)).set_visible(cx, false);
            }
            _ => {}
        }
        self.next_frame = cx.new_next_frame();
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self
            .ui
            .button(cx, ids!(guard_dismiss_button))
            .clicked(actions)
        {
            self.guard_card_visible = false;
            self.ui.view(cx, ids!(guard_card)).set_visible(cx, false);
        }

        if demo_stage() == Some(DemoStage::S0) {
            return;
        }

        let slider_changed = self
            .ui
            .widget(cx, ids!(scrubber_slider))
            .borrow::<MpSlider>()
            .and_then(|slider| slider.changed(actions));
        if let Some(SliderValue::Single(value)) = slider_changed {
            if let Some(track) = self.track.clone() {
                let p = (value / 100.0).clamp(0.0, 0.999) as f32;
                self.state.apply_progress(&track, p);
                self.last_scrubber_drag_secs = self.now_secs;
                self.refresh_hud(cx, &track);
                self.refresh_scrubber_labels(cx, &track);
            }
        }
    }
}

// P11.2: Android JNI 读 Settings.Global.ANIMATOR_DURATION_SCALE.
// 真机走 robius-android-env 提供的 with_activity closure, 拿 JNIEnv + Activity.
// PC 上 cfg(not(android)) 直接返回 false, robius-android-env / jni crate 不参与 PC build.
#[cfg(target_os = "android")]
fn detect_android_reduced_motion() -> bool {
    use jni::objects::{JObject, JValue};
    use robius_android_env::with_activity;

    with_activity(|env, activity| -> bool {
        // resolver = activity.getContentResolver()
        let resolver = match env
            .call_method(
                activity,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )
            .and_then(|v| v.l())
        {
            Ok(r) => r,
            Err(_) => return false,
        };
        // key = "animator_duration_scale"
        let key = match env.new_string("animator_duration_scale") {
            Ok(k) => k,
            Err(_) => return false,
        };
        // scale = Settings.Global.getFloat(resolver, key, 1.0)
        let scale = match env
            .call_static_method(
                "android/provider/Settings$Global",
                "getFloat",
                "(Landroid/content/ContentResolver;Ljava/lang/String;F)F",
                &[
                    JValue::Object(&resolver),
                    JValue::Object(&JObject::from(key)),
                    JValue::Float(1.0),
                ],
            )
            .and_then(|v| v.f())
        {
            Ok(s) => s,
            Err(_) => return false,
        };
        scale == 0.0
    })
    .unwrap_or(false)
}

#[cfg(not(target_os = "android"))]
fn detect_android_reduced_motion() -> bool {
    false
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        if let Event::NextFrame(ne) = event {
            if ne.set.contains(&self.next_frame) {
                let now = ne.time;
                self.now_secs = now;
                if demo_stage() != Some(DemoStage::S0) {
                    self.poll_network(cx, now);
                }
                self.maybe_advance_phase(cx, now);
                self.next_frame = cx.new_next_frame();
            }
        }

        let pause_area = self.ui.view(cx, ids!(pause_button)).area();
        if let Hit::FingerUp(fe) = event.hits(cx, pause_area) {
            if fe.is_over && fe.was_tap() && demo_stage() != Some(DemoStage::S0) {
                self.state.is_paused = !self.state.is_paused;
                self.refresh_pause_glyph(cx);
            }
        }

        // B3+B7: compass / 2D 锁定按钮 hit detection (outer wrap view 已扩展 area 到 48dp).
        // compass click → viewport reset (当前主画布 view 始终居中 to track bounds, click 是 no-op redraw,
        // 满足 spec.spec.md L411-413 "progress / index 不变" 与 L416 "旋转角重置为 0" 隐式契约).
        let compass_area = self.ui.view(cx, ids!(compass_button)).area();
        if let Hit::FingerUp(fe) = event.hits(cx, compass_area) {
            if fe.is_over && fe.was_tap() {
                log!("compass clicked: viewport reset (no-op, view 始终居中)");
                self.ui.widget(cx, ids!(track_canvas)).redraw(cx);
            }
        }
        let lock_2d_area = self.ui.view(cx, ids!(lock_2d_button)).area();
        if let Hit::FingerUp(fe) = event.hits(cx, lock_2d_area) {
            if fe.is_over && fe.was_tap() {
                log!("2D lock clicked (装饰性, spec.spec.md L419-427 无副作用)");
            }
        }

        for (path, speed) in [
            (ids!(speed_1x_button) as &[LiveId], 1.0_f32),
            (ids!(speed_4x_button) as &[LiveId], 4.0),
            (ids!(speed_16x_button) as &[LiveId], 16.0),
        ] {
            let area = self.ui.view(cx, path).area();
            if let Hit::FingerUp(fe) = event.hits(cx, area) {
                if fe.is_over && fe.was_tap() && demo_stage() != Some(DemoStage::S0) {
                    self.state.playback_speed = speed;
                    self.refresh_speed_buttons(cx);
                }
            }
        }

        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_demo_stages_case_insensitively() {
        assert_eq!(parse_demo_stage("S0"), Some(DemoStage::S0));
        assert_eq!(parse_demo_stage("s2"), Some(DemoStage::S2));
        assert_eq!(parse_demo_stage("S3"), Some(DemoStage::S3));
        assert_eq!(parse_demo_stage("s4"), Some(DemoStage::S4));
        assert_eq!(parse_demo_stage("unknown"), None);
    }

    #[test]
    fn clamps_demo_seek_to_playback_range() {
        fn assert_close(left: f32, right: f32) {
            assert!((left - right).abs() < f32::EPSILON);
        }

        assert_close(parse_demo_seek(Some("-1.0"), 0.25), 0.0);
        assert_close(parse_demo_seek(Some("0.5"), 0.25), 0.5);
        assert_close(parse_demo_seek(Some("1.0"), 0.25), 0.999);
        assert_close(parse_demo_seek(Some("bad"), 0.25), 0.25);
        assert_close(parse_demo_seek(None, 0.25), 0.25);
    }
}
