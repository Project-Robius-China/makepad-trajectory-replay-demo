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

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*

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
        particle_density: 0.5
        elevation_z: 0.
        guard_pulse_phase: 0.
        walked_segment_ratio: 0.
        scrubber_echo_phase: 0.
        overlay_dim: 0.
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
            let c1 = mix(self.speed_low, self.speed_mid, clamp(s * 2., 0., 1.))
            let c2 = mix(self.speed_mid, self.speed_high, clamp((s - 0.5) * 2., 0., 1.))
            return mix(c1, c2, step(0.5, s))
        }

        pixel: fn() {
            let p = self.pos * self.rect_size
            let line_w = 4.0
            let glow_w = 12.0
            let d = self.capsule_sdf(p, self.start_xy, self.end_xy, line_w)
            let alpha = clamp(0.5 - d, 0., 1.)
            let t_mid = (self.t_a + self.t_b) * 0.5
            let s_mid = (self.speed_a + self.speed_b) * 0.5
            let walked = step(t_mid, self.walked_segment_ratio)
            let walked_color = self.speed_color(s_mid)
            let unwalked_color = self.unwalked
            let base = mix(unwalked_color, walked_color, walked)
            let glow_d = self.capsule_sdf(p, self.start_xy, self.end_xy, glow_w)
            let glow = exp(-max(glow_d, 0.0) * 0.4) * 0.55 * walked
            let final_rgb = base + walked_color * glow
            let final_a = alpha + glow * 0.3 * (1. - alpha)
            let dim = 1. - self.overlay_dim * 0.7
            return Pal.premul(vec4(final_rgb * dim, final_a * dim))
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

    let TrackCanvasBase = #(TrackCanvas::register_widget(vm))
    let TrackCanvas = set_type_default() do TrackCanvasBase{
        width: Fill
        height: Fill
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
                        padding: Inset{ left: 16 right: 16 top: 14 bottom: 12 }
                        align: Align{ y: 0.5 }
                        show_bg: true
                        new_batch: true
                        draw_bg.color: #x0A0A0F

                        route_name_label := Label{
                            text: "—"
                            width: Fill
                            draw_text.color: #xF5F5FA
                            draw_text.text_style.font_size: 13
                        }

                        sync_badge := RoundedView{
                            width: Fit height: Fit
                            padding: Inset{ left: 10 right: 10 top: 5 bottom: 5 }
                            flow: Right
                            spacing: 5
                            align: Align{ y: 0.5 }
                            new_batch: true
                            draw_bg.color: #x14141C
                            draw_bg.radius: 12.
                            sync_badge_dot := RoundedView{
                                width: 6 height: 6
                                new_batch: true
                                draw_bg.color: #x4ADE80
                                draw_bg.radius: 3.
                            }
                            sync_badge_label := Label{
                                text: "同步中..."
                                draw_text.color: #xF5F5FA
                                draw_text.text_style.font_size: 11
                            }
                        }

                        profile_pill := RoundedView{
                            width: Fit height: Fit
                            padding: Inset{ left: 10 right: 10 top: 5 bottom: 5 }
                            new_batch: true
                            draw_bg.color: #x14141C
                            draw_bg.radius: 12.
                            profile_label := Label{
                                text: "骑行"
                                draw_text.color: #xF5F5FA
                                draw_text.text_style.font_size: 11
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

                        guard_edge := View{
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

                                compass_button := RoundedView{
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

                                lock_2d_button := RoundedView{
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

                        sync_overlay := View{
                            width: Fill height: Fill
                            flow: Down
                            align: Center
                            spacing: 12

                            View{ width: Fill height: 240 }

                            sync_overlay_label := Label{
                                text: "同步中..."
                                draw_text.color: #xF5F5FA
                                draw_text.text_style.font_size: 18
                            }
                            sync_overlay_subtext := Label{
                                text: "正在从 Project-Robius-China 拉取数据"
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

                        stats_overlay := View{
                            width: Fill height: Fill
                            flow: Down
                            align: Center
                            spacing: 14
                            padding: Inset{ left: 32 right: 32 top: 96 bottom: 32 }
                            visible: false

                            stats_title := Label{
                                text: "本次回放总览"
                                draw_text.color: #xF5F5FA
                                draw_text.text_style.font_size: 16
                            }

                            View{
                                width: Fill height: Fit
                                flow: Right
                                spacing: 16
                                stat_distance_cell := View{
                                    width: Fill height: Fit
                                    flow: Down
                                    spacing: 4
                                    align: Align{ x: 0.0 }
                                    Label{
                                        text: "总距离"
                                        draw_text.color: #x7A7B8C
                                        draw_text.text_style.font_size: 10
                                    }
                                    stat_distance_value := Label{
                                        text: "—"
                                        draw_text.color: #xF5F5FA
                                        draw_text.text_style.font_size: 24
                                    }
                                }
                                stat_duration_cell := View{
                                    width: Fill height: Fit
                                    flow: Down
                                    spacing: 4
                                    align: Align{ x: 0.0 }
                                    Label{
                                        text: "总时长"
                                        draw_text.color: #x7A7B8C
                                        draw_text.text_style.font_size: 10
                                    }
                                    stat_duration_value := Label{
                                        text: "—"
                                        draw_text.color: #xF5F5FA
                                        draw_text.text_style.font_size: 24
                                    }
                                }
                            }
                            View{
                                width: Fill height: Fit
                                flow: Right
                                spacing: 16
                                stat_climb_cell := View{
                                    width: Fill height: Fit
                                    flow: Down
                                    spacing: 4
                                    align: Align{ x: 0.0 }
                                    Label{
                                        text: "累计爬升"
                                        draw_text.color: #x7A7B8C
                                        draw_text.text_style.font_size: 10
                                    }
                                    stat_climb_value := Label{
                                        text: "—"
                                        draw_text.color: #xF5F5FA
                                        draw_text.text_style.font_size: 24
                                    }
                                }
                                stat_avg_hr_cell := View{
                                    width: Fill height: Fit
                                    flow: Down
                                    spacing: 4
                                    align: Align{ x: 0.0 }
                                    Label{
                                        text: "平均心率"
                                        draw_text.color: #x7A7B8C
                                        draw_text.text_style.font_size: 10
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

                    hud := View{
                        width: Fill height: Fit
                        flow: Right
                        padding: Inset{ left: 12 right: 12 top: 8 bottom: 8 }
                        spacing: 8
                        show_bg: true
                        new_batch: true
                        draw_bg.color: #x0A0A0F

                        hud_speed_cell := RoundedView{
                            width: Fill height: Fit
                            flow: Down
                            spacing: 2
                            padding: Inset{ left: 10 right: 10 top: 8 bottom: 8 }
                            new_batch: true
                            draw_bg.color: #x14141C
                            draw_bg.radius: 6.

                            Label{
                                text: "速度 km/h"
                                draw_text.color: #x7A7B8C
                                draw_text.text_style.font_size: 9
                            }
                            hud_speed_value := Label{
                                text: "—"
                                draw_text.color: #xF5F5FA
                                draw_text.text_style.font_size: 18
                            }
                            hud_speed_bar := View{
                                width: Fill height: 4 flow: Right
                                hud_speed_bar_fill := RoundedView{
                                    width: Fill height: 4
                                    new_batch: true
                                    draw_bg.color: #xFF8A3D
                                    draw_bg.radius: 2.
                                }
                                hud_speed_bar_rest := RoundedView{
                                    width: 0 height: 4
                                    new_batch: true
                                    draw_bg.color: #x3B3B46
                                    draw_bg.radius: 2.
                                }
                            }
                        }

                        hud_hr_cell := RoundedView{
                            width: Fill height: Fit
                            flow: Down
                            spacing: 2
                            padding: Inset{ left: 10 right: 10 top: 8 bottom: 8 }
                            new_batch: true
                            draw_bg.color: #x14141C
                            draw_bg.radius: 6.

                            Label{
                                text: "心率 bpm"
                                draw_text.color: #x7A7B8C
                                draw_text.text_style.font_size: 9
                            }
                            hud_hr_value := Label{
                                text: "—"
                                draw_text.color: #xF5F5FA
                                draw_text.text_style.font_size: 18
                            }
                            hud_hr_bar := View{
                                width: Fill height: 4 flow: Right
                                hud_hr_bar_fill := RoundedView{
                                    width: Fill height: 4
                                    new_batch: true
                                    draw_bg.color: #xFF3B6E
                                    draw_bg.radius: 2.
                                }
                                hud_hr_bar_rest := RoundedView{
                                    width: 0 height: 4
                                    new_batch: true
                                    draw_bg.color: #x3B3B46
                                    draw_bg.radius: 2.
                                }
                            }
                        }

                        hud_ele_cell := RoundedView{
                            width: Fill height: Fit
                            flow: Down
                            spacing: 2
                            padding: Inset{ left: 10 right: 10 top: 8 bottom: 8 }
                            new_batch: true
                            draw_bg.color: #x14141C
                            draw_bg.radius: 6.

                            Label{
                                text: "海拔 m"
                                draw_text.color: #x7A7B8C
                                draw_text.text_style.font_size: 9
                            }
                            hud_ele_value := Label{
                                text: "—"
                                draw_text.color: #xF5F5FA
                                draw_text.text_style.font_size: 18
                            }
                            hud_ele_bar := View{
                                width: Fill height: 4 flow: Right
                                hud_ele_bar_fill := RoundedView{
                                    width: Fill height: 4
                                    new_batch: true
                                    draw_bg.color: #x00E5FF
                                    draw_bg.radius: 2.
                                }
                                hud_ele_bar_rest := RoundedView{
                                    width: 0 height: 4
                                    new_batch: true
                                    draw_bg.color: #x3B3B46
                                    draw_bg.radius: 2.
                                }
                            }
                        }

                        hud_cad_cell := RoundedView{
                            width: Fill height: Fit
                            flow: Down
                            spacing: 2
                            padding: Inset{ left: 10 right: 10 top: 8 bottom: 8 }
                            new_batch: true
                            draw_bg.color: #x14141C
                            draw_bg.radius: 6.

                            Label{
                                text: "踏频 rpm"
                                draw_text.color: #x7A7B8C
                                draw_text.text_style.font_size: 9
                            }
                            hud_cad_value := Label{
                                text: "—"
                                draw_text.color: #xF5F5FA
                                draw_text.text_style.font_size: 18
                            }
                            hud_cad_bar := View{
                                width: Fill height: 4 flow: Right
                                hud_cad_bar_fill := RoundedView{
                                    width: Fill height: 4
                                    new_batch: true
                                    draw_bg.color: #xE8E8F0
                                    draw_bg.radius: 2.
                                }
                                hud_cad_bar_rest := RoundedView{
                                    width: 0 height: 4
                                    new_batch: true
                                    draw_bg.color: #x3B3B46
                                    draw_bg.radius: 2.
                                }
                            }
                        }
                    }

                    bottom_bar := View{
                        width: Fill height: Fit
                        flow: Right
                        padding: Inset{ left: 16 right: 16 top: 6 bottom: 14 }
                        spacing: 10
                        align: Align{ y: 0.5 }
                        show_bg: true
                        new_batch: true
                        draw_bg.color: #x0A0A0F

                        current_time_label := Label{
                            text: "00:00"
                            draw_text.color: #xF5F5FA
                            draw_text.text_style.font_size: 11
                        }

                        scrubber_track := View{
                            width: Fill height: 18
                            flow: Right
                            align: Align{ y: 0.5 }
                            scrubber_walked := RoundedView{
                                width: 0 height: 2
                                new_batch: true
                                draw_bg.color: #xF5F5FA
                                draw_bg.radius: 1.
                            }
                            scrubber_thumb := RoundedView{
                                width: 12 height: 12
                                new_batch: true
                                draw_bg.color: #xF5F5FA
                                draw_bg.radius: 6.
                            }
                            scrubber_unwalked := RoundedView{
                                width: Fill height: 2
                                new_batch: true
                                draw_bg.color: #x3B3B46
                                draw_bg.radius: 1.
                            }
                        }

                        total_time_label := Label{
                            text: "00:00"
                            draw_text.color: #x7A7B8C
                            draw_text.text_style.font_size: 11
                        }

                        speed_1x_button := RoundedView{
                            width: 36 height: 28
                            align: Center
                            new_batch: true
                            draw_bg.color: #x14141C
                            draw_bg.radius: 8.
                            speed_1x_label := Label{
                                text: "1x"
                                draw_text.color: #xCCCCDD
                                draw_text.text_style.font_size: 12
                            }
                        }
                        speed_4x_button := RoundedView{
                            width: 36 height: 28
                            align: Center
                            new_batch: true
                            draw_bg.color: #x4A60D9
                            draw_bg.radius: 8.
                            speed_4x_label := Label{
                                text: "4x"
                                draw_text.color: #xFFFFFF
                                draw_text.text_style.font_size: 12
                            }
                        }
                        speed_16x_button := RoundedView{
                            width: 36 height: 28
                            align: Center
                            new_batch: true
                            draw_bg.color: #x14141C
                            draw_bg.radius: 8.
                            speed_16x_label := Label{
                                text: "16x"
                                draw_text.color: #xCCCCDD
                                draw_text.text_style.font_size: 12
                            }
                        }
                        pause_button := RoundedView{
                            width: 40 height: 40
                            align: Center
                            flow: Right
                            spacing: 5
                            new_batch: true
                            draw_bg.color: #x1F1F2C
                            draw_bg.radius: 20.
                            pause_left_bar := RoundedView{
                                width: 4 height: 14
                                new_batch: true
                                draw_bg.color: #xF5F5FA
                                draw_bg.radius: 1.
                            }
                            pause_right_bar := RoundedView{
                                width: 4 height: 14
                                new_batch: true
                                draw_bg.color: #xF5F5FA
                                draw_bg.radius: 1.
                            }
                            pause_play_triangle := Label{
                                text: ""
                                draw_text.color: #xF5F5FA
                                draw_text.text_style.font_size: 14
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
    #[deref] draw_super: DrawQuad,
    #[live] track_progress: f32,
    #[live] polyline_color_mix: f32,
    #[live] particle_density: f32,
    #[live] elevation_z: f32,
    #[live] guard_pulse_phase: f32,
    #[live] walked_segment_ratio: f32,
    #[live] scrubber_echo_phase: f32,
    #[live] overlay_dim: f32,
    #[live] seg_count: f32,
    #[live] seg_idx: f32,
    #[live] start_xy: Vec2,
    #[live] end_xy: Vec2,
    #[live] t_a: f32,
    #[live] t_b: f32,
    #[live] speed_a: f32,
    #[live] speed_b: f32,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawMarker {
    #[deref] draw_super: DrawQuad,
    #[live] marker_color: Vec3,
    #[live] marker_radius: f32,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawHalo {
    #[deref] draw_super: DrawQuad,
    #[live] hr_phase: f32,
    #[live] guard_pulse_phase: f32,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawGuardEdge {
    #[deref] draw_super: DrawQuad,
    #[live] guard_pulse_phase: f32,
}

#[derive(Script, ScriptHook)]
#[repr(C)]
pub struct DrawWater {
    #[deref] draw_super: DrawQuad,
    #[live] water_color: Vec3,
    #[live] edge_softness: f32,
}

#[derive(Script, ScriptHook, Widget)]
pub struct TrackCanvas {
    #[uid] uid: WidgetUid,
    #[walk] walk: Walk,
    #[layout] layout: Layout,

    #[redraw] #[live] draw_track: DrawTrack,
    #[live] draw_start_marker: DrawMarker,
    #[live] draw_end_marker: DrawMarker,
    #[live] draw_halo: DrawHalo,
    #[live] draw_water: DrawWater,

    #[rust] track: Option<Arc<Track>>,
    #[rust] geom: Option<TrackGeom>,
    #[rust] last_rect_size: Vec2,
    #[rust] walked_ratio: f32,
    #[rust] track_progress: f32,
    #[rust] hr_phase_val: f32,
    #[rust] guard_pulse: f32,
    #[rust] overlay_dim_val: f32,

    #[rust] area: Area,
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

impl Widget for TrackCanvas {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        let rect = cx.turtle().rect();
        self.ensure_geom(rect);

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

                let marker_size: f64 = 14.0;
                let marker_rect = |p: Vec2| Rect {
                    pos: dvec2(p.x as f64 - marker_size * 0.5, p.y as f64 - marker_size * 0.5),
                    size: dvec2(marker_size, marker_size),
                };
                self.draw_start_marker.marker_color = vec3(0.063, 0.722, 0.506);
                self.draw_start_marker.marker_radius = 5.0;
                self.draw_start_marker.draw_abs(cx, marker_rect(geom.start_screen));
                self.draw_end_marker.marker_color = vec3(0.478, 0.482, 0.549);
                self.draw_end_marker.marker_radius = 5.0;
                self.draw_end_marker.draw_abs(cx, marker_rect(geom.end_screen));

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
            }
        }

        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
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
        let Some(track) = self.track.as_ref() else { return };
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
        let target_segs: usize = 180;
        let stride = (n / target_segs).max(1);
        let mut indices: Vec<usize> = (0..n).step_by(stride).collect();
        if *indices.last().unwrap() != n - 1 {
            indices.push(n - 1);
        }
        let speed_lo = track.stats.speed_min_mps;
        let speed_hi = track.stats.speed_max_mps.max(speed_lo + 1e-3);
        let normalize = |s: f32| ((s - speed_lo) / (speed_hi - speed_lo)).clamp(0.0, 1.0);

        let total_n = (n - 1).max(1) as f32;
        let mut segs: Vec<TrackSegment> = Vec::with_capacity(indices.len().saturating_sub(1));
        for i in 0..indices.len().saturating_sub(1) {
            let ia = indices[i];
            let ib = indices[i + 1];
            let pa = &track.points[ia];
            let pb = &track.points[ib];
            let start = project(pa.lat, pa.lon);
            let end = project(pb.lat, pb.lon);
            let speed_a = normalize(pa.speed_mps.unwrap_or(0.0));
            let speed_b = normalize(pb.speed_mps.unwrap_or(0.0));
            let t_a = ia as f32 / total_n;
            let t_b = ib as f32 / total_n;
            segs.push(TrackSegment {
                start,
                end,
                t_a,
                t_b,
                speed_a,
                speed_b,
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
}

#[derive(Default)]
struct GuardWindow {
    start_idx: usize,
    end_idx: usize,
    valid: bool,
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live] ui: WidgetRef,

    #[rust] track: Option<Arc<Track>>,
    #[rust] state: PlaybackState,
    #[rust] user: UserProfile,
    #[rust] network_rx: Option<Receiver<FetchResult>>,
    #[rust] worker_thread_id: Option<std::thread::ThreadId>,
    #[rust] pending_fetch: Option<FetchResult>,
    #[rust] fetching_started_at_secs: Option<f64>,

    #[rust] phase: i32,
    #[rust] now_secs: f64,
    #[rust] last_now_secs: f64,
    #[rust] phase_entered_secs: f64,
    #[rust] success_entered_secs: Option<f64>,

    #[rust] guard_window: GuardWindow,
    #[rust] guard_active_started_at_secs: f64,
    #[rust] guard_card_visible: bool,
    #[rust] last_scrubber_drag_secs: f64,

    #[live] draw_guard_edge: DrawGuardEdge,

    #[rust] next_frame: NextFrame,
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

    fn refresh_top_bar(&mut self, cx: &mut Cx) {
        let sync_text = self.state.sync_status_text();
        let route = self.route_name().to_string();
        let profile_label = self.state.profile.label_zh();
        self.ui
            .label(cx, ids!(route_name_label))
            .set_text(cx, &route);
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
            NetworkState::Idle | NetworkState::Fetching => {
                "正在从 Project-Robius-China 拉取数据"
            }
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
        self.ui
            .label(cx, ids!(legend_max_label))
            .set_text(cx, &txt);
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
        let Some(track) = self.track.as_ref().cloned() else { return };
        if std::env::var("MOBILE_EXAMPLE_DEMO_GUARD").is_ok() {
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
        let dt = (now - self.last_now_secs).max(0.0);
        self.last_now_secs = now;
        let mut track_progress_v: f32 = 0.0;
        let mut walked_ratio_v: f32 = 0.0;
        let mut overlay_dim_v: f32 = 0.0;
        match self.phase {
            PHASE_SYNCING => {
                if matches!(
                    self.state.network_state,
                    NetworkState::Success | NetworkState::Fallback
                ) {
                    if let Some(t) = self.success_entered_secs {
                        if now - t >= SUCCESS_LABEL_VISIBLE_SECS {
                            self.enter_phase(cx, PHASE_PATH_DRAW, now);
                        }
                    }
                }
            }
            PHASE_PATH_DRAW => {
                let elapsed = now - self.phase_entered_secs;
                let p = (elapsed / PATH_DRAW_DURATION_SECS).clamp(0.0, 1.0) as f32;
                track_progress_v = p;
                walked_ratio_v = 0.0;
                if p >= 1.0 {
                    self.enter_phase(cx, PHASE_PLAYBACK, now);
                }
            }
            PHASE_PLAYBACK => {
                if let Some(track) = self.track.as_ref().cloned() {
                    let total = (track.stats.duration_ms_total.max(1)) as f64;
                    if !self.state.is_paused {
                        let inc = (dt * self.state.playback_speed as f64) * 1000.0 / total;
                        let new_p = (self.state.playback_progress as f64 + inc).clamp(0.0, 1.0)
                            as f32;
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
                overlay_dim_v = ((now - self.phase_entered_secs) / 0.5).clamp(0.0, 1.0) as f32;
            }
            _ => {}
        }

        let echo_age = (now - self.last_scrubber_drag_secs).max(0.0);
        let scrubber_echo_v = if echo_age < SCRUBBER_ECHO_FADE_SECS {
            (1.0 - (echo_age / SCRUBBER_ECHO_FADE_SECS)).max(0.0) as f32
        } else {
            0.0
        };
        self.state.scrubber_echo_phase = scrubber_echo_v;

        let guard_age = (now - self.guard_active_started_at_secs).max(0.0);
        let guard_pulse_v = if self.state.contract_guard_active
            && guard_age < GUARD_PULSE_DURATION_SECS
        {
            (1.0 - (guard_age / GUARD_PULSE_DURATION_SECS)).max(0.0) as f32
        } else {
            0.0
        };

        let (hr_phase_now, _) = self.hr_phase(now);

        self.draw_guard_edge.guard_pulse_phase = guard_pulse_v;

        let track_arc = self.track.clone();
        let canvas_ref = self.ui.widget(cx, ids!(track_canvas));
        if let Some(mut canvas) = canvas_ref.borrow_mut::<TrackCanvas>() {
            canvas.set_track(cx, track_arc);
            canvas.set_progress(cx, walked_ratio_v, track_progress_v);
            canvas.set_hr_phase(cx, hr_phase_now);
            canvas.set_guard_pulse(cx, guard_pulse_v);
            canvas.set_overlay_dim(cx, overlay_dim_v);
            canvas.set_scrubber_echo(cx, scrubber_echo_v);
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

        set_bar_ratio(
            cx, &self.ui, ids!(hud_speed_bar), ids!(hud_speed_bar_fill), ids!(hud_speed_bar_rest), s_ratio,
        );
        set_bar_ratio(
            cx, &self.ui, ids!(hud_hr_bar), ids!(hud_hr_bar_fill), ids!(hud_hr_bar_rest), hr_ratio,
        );
        set_bar_ratio(
            cx, &self.ui, ids!(hud_ele_bar), ids!(hud_ele_bar_fill), ids!(hud_ele_bar_rest), ele_ratio,
        );
        set_bar_ratio(
            cx, &self.ui, ids!(hud_cad_bar), ids!(hud_cad_bar_fill), ids!(hud_cad_bar_rest), cad_ratio,
        );
    }

    fn refresh_scrubber_labels(&mut self, cx: &mut Cx, track: &Track) {
        let total_secs = (track.stats.duration_ms_total / 1000).max(0);
        let current_secs = (total_secs as f32 * self.state.playback_progress) as i64;
        let cur = format_mmss(current_secs);
        let tot = format_mmss(total_secs);
        self.ui
            .label(cx, ids!(current_time_label))
            .set_text(cx, &cur);
        self.ui
            .label(cx, ids!(total_time_label))
            .set_text(cx, &tot);

        let track_view = self.ui.view(cx, ids!(scrubber_track));
        let total = track_view.area().rect(cx).size.x as f32 - 12.0;
        if total > 1.0 {
            let walked_w = (total * self.state.playback_progress.clamp(0.0, 1.0)) as f64;
            let walked_ref = self.ui.view(cx, ids!(scrubber_walked));
            if let Some(mut v) = walked_ref.borrow_mut() {
                v.walk.width = Size::Fixed(walked_w);
            }
            track_view.redraw(cx);
        }
    }

    fn refresh_pause_glyph(&mut self, cx: &mut Cx) {
        let paused = self.state.is_paused;
        self.ui.view(cx, ids!(pause_left_bar)).set_visible(cx, !paused);
        self.ui.view(cx, ids!(pause_right_bar)).set_visible(cx, !paused);
        self.ui
            .label(cx, ids!(pause_play_triangle))
            .set_text(cx, if paused { "▶" } else { "" });
        self.ui.view(cx, ids!(pause_button)).redraw(cx);
    }

    fn refresh_speed_buttons(&mut self, cx: &mut Cx) {
        let active = self.state.playback_speed.round() as i32;
        for (path, val) in [
            (ids!(speed_1x_button) as &[LiveId], 1),
            (ids!(speed_4x_button) as &[LiveId], 4),
            (ids!(speed_16x_button) as &[LiveId], 16),
        ] {
            let view = self.ui.view(cx, path);
            let active_color = [0.290, 0.376, 0.851, 1.0];
            let inactive_color = [0.078, 0.078, 0.110, 1.0];
            let c = if val == active { active_color } else { inactive_color };
            view.set_uniform(cx, live_id!(color), &c);
            view.redraw(cx);
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

fn set_bar_ratio(
    cx: &mut Cx,
    ui: &WidgetRef,
    parent_id: &[LiveId],
    fill_id: &[LiveId],
    rest_id: &[LiveId],
    ratio: f32,
) {
    let parent = ui.view(cx, parent_id);
    let total = parent.area().rect(cx).size.x as f32;
    if total < 1.0 {
        return;
    }
    let fill_w = (total * ratio.clamp(0.0, 1.0)) as f64;
    let rest_w = (total as f64 - fill_w).max(0.0);
    let fill_ref = ui.view(cx, fill_id);
    if let Some(mut v) = fill_ref.borrow_mut() {
        v.walk.width = Size::Fixed(fill_w);
    }
    let rest_ref = ui.view(cx, rest_id);
    if let Some(mut v) = rest_ref.borrow_mut() {
        v.walk.width = Size::Fixed(rest_w);
    }
    parent.redraw(cx);
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

        if let Ok(seek) = std::env::var("MOBILE_EXAMPLE_DEMO_SEEK") {
            if let Ok(p) = seek.parse::<f32>() {
                self.state.playback_progress = p.clamp(0.0, 0.999);
            }
        }
        if let Some(t) = self.track.clone() {
            let p0 = self.state.playback_progress;
            self.state.apply_progress(&t, p0);
        }
        self.compute_guard_window();

        let (rx, tid) = spawn_fetch_worker();
        self.network_rx = Some(rx);
        self.worker_thread_id = Some(tid);
        self.fetching_started_at_secs = None;
        self.pending_fetch = None;
        self.guard_card_visible = false;
        self.last_scrubber_drag_secs = -10.0;

        self.refresh_top_bar(cx);
        self.refresh_sync_overlay(cx);
        self.refresh_legend_max(cx);
        self.next_frame = cx.new_next_frame();
    }

    fn handle_actions(&mut self, cx: &mut Cx, actions: &Actions) {
        if self.ui.button(cx, ids!(guard_dismiss_button)).clicked(actions) {
            self.guard_card_visible = false;
            self.ui.view(cx, ids!(guard_card)).set_visible(cx, false);
        }
    }
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
                self.poll_network(cx, now);
                self.maybe_advance_phase(cx, now);
                self.next_frame = cx.new_next_frame();
            }
        }

        let pause_area = self.ui.view(cx, ids!(pause_button)).area();
        if let Hit::FingerUp(fe) = event.hits(cx, pause_area) {
            if fe.is_over && fe.was_tap() {
                self.state.is_paused = !self.state.is_paused;
                self.refresh_pause_glyph(cx);
            }
        }

        for (path, speed) in [
            (ids!(speed_1x_button) as &[LiveId], 1.0_f32),
            (ids!(speed_4x_button) as &[LiveId], 4.0),
            (ids!(speed_16x_button) as &[LiveId], 16.0),
        ] {
            let area = self.ui.view(cx, path).area();
            if let Hit::FingerUp(fe) = event.hits(cx, area) {
                if fe.is_over && fe.was_tap() {
                    self.state.playback_speed = speed;
                    self.refresh_speed_buttons(cx);
                }
            }
        }

        let scrubber_area = self.ui.view(cx, ids!(scrubber_track)).area();
        match event.hits(cx, scrubber_area) {
            Hit::FingerDown(fe) => {
                let r = scrubber_area.rect(cx);
                if r.size.x > 1.0 {
                    let p = ((fe.abs.x - r.pos.x) / r.size.x).clamp(0.0, 1.0) as f32;
                    if let Some(t) = self.track.clone() {
                        self.state.apply_progress(&t, p);
                    }
                    self.last_scrubber_drag_secs = self.now_secs;
                }
            }
            Hit::FingerMove(fe) => {
                let r = scrubber_area.rect(cx);
                if r.size.x > 1.0 {
                    let p = ((fe.abs.x - r.pos.x) / r.size.x).clamp(0.0, 1.0) as f32;
                    if let Some(t) = self.track.clone() {
                        self.state.apply_progress(&t, p);
                    }
                    self.last_scrubber_drag_secs = self.now_secs;
                }
            }
            _ => {}
        }

        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
