// P13.0 spike: GeoMapView widget fork from makepad-map-widget repo.
// Phase 1: 仅 stub, disk_cache + tiles 无 DSL 直接 fork; map_view.rs 是 2.x stub Widget.
// Phase 2 (P13.1): paste makepad-map src/map_view.rs 933 行并逐批 fix 1.x→2.x DSL.

pub mod disk_cache;
pub mod tiles;
pub mod map_view;

pub use map_view::GeoMapView;
