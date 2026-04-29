# Demo Data Trim Plan

## Goal

Make the replay animation more obvious during live demos without changing the source GPX file or the remote data contract.

## Decision

- Keep `resources/cycling-track.gpx` intact as the canonical bundled dataset.
- After parsing a track, trim the runtime track to a short demo window before rendering.
- Apply the same trim to both network-loaded tracks and local fallback tracks.
- Target a roughly 6 minute track window so the default `4x` playback advances visibly while still leaving enough time for manual inspection.
- Select the window by a motion score that favors visible distance, bounding-box spread, and heading changes. This avoids picking a visually flat segment.
- Recompute derived stats after trimming so HUD ranges, duration labels, guard windows, and playback progress stay consistent with the rendered segment.

## Verification

- Unit tests cover long-track trimming, motion-rich window selection, and short-track no-op behavior.
- Full verification should include `cargo test`, `cargo clippy -- -D warnings`, and Android release build before committing.
