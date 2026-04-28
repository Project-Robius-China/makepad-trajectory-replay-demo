// P13.0 spike: GeoMapView 2.x stub (mirror TrackCanvas pattern with #[redraw] DrawQuad).
// 占位 widget, 仅画 draw_bg DrawQuad 占位区域. P13.1 paste 真渲染 + tile fetch.

use makepad_widgets::*;

#[derive(Script, ScriptHook, Widget)]
pub struct GeoMapView {
    #[uid] uid: WidgetUid,
    #[redraw] #[live] draw_bg: DrawQuad,
    #[walk] walk: Walk,
    #[layout] layout: Layout,

    #[live] center_lng: f64,
    #[live] center_lat: f64,
    #[live] zoom: f64,

    #[area] #[rust] area: Area,
}

impl Widget for GeoMapView {
    fn handle_event(&mut self, _cx: &mut Cx, _event: &Event, _scope: &mut Scope) {}

    fn draw_walk(&mut self, cx: &mut Cx2d, _scope: &mut Scope, walk: Walk) -> DrawStep {
        cx.begin_turtle(walk, self.layout);
        let rect = cx.turtle().rect();
        self.draw_bg.draw_abs(cx, rect);
        cx.end_turtle_with_area(&mut self.area);
        DrawStep::done()
    }
}

impl GeoMapView {
    pub fn set_center(&mut self, _cx: &mut Cx, lng: f64, lat: f64) {
        self.center_lng = lng;
        self.center_lat = lat;
    }

    pub fn set_zoom(&mut self, _cx: &mut Cx, zoom: f64) {
        self.zoom = zoom;
    }
}
