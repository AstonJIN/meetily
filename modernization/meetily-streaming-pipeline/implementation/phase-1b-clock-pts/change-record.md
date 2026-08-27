# Change Record: phase-1b-clock-pts

## Objective

Introduce a monotonic session clock and a nanosecond presentation timestamp while preserving the legacy `timestamp: f64` compatibility field.

## Files changed by this batch

- `frontend/src-tauri/src/audio/session_clock.rs`
- `frontend/src-tauri/src/audio/recording_state.rs`
- `frontend/src-tauri/src/audio/pipeline.rs`
- `frontend/src-tauri/src/audio/incremental_saver.rs`
- `frontend/src-tauri/src/audio/mod.rs`

## Compatibility

- `AudioChunk::timestamp` remains available and continues to carry recording-relative seconds.
- `AudioChunk::pts_ns` is populated at the capture boundary from `SessionClock` and is reused by mixed recording chunks.
- Existing UI/database/transcript event shapes are unchanged.
- Pause/resume and stop behavior are covered in `SessionClock`; the legacy duration methods remain intact for compatibility.

## Verification

- `cargo test --lib audio::session_clock`: 3 passed, 0 failed.
- `cargo test --lib audio::recording_state`: 3 passed, 0 failed.
- Full `cargo test --lib audio` after the main clock/PTS changes: 91 passed, 2 failed, 3 ignored out of 96.
- The two failures are the same baseline failures; no new failure was introduced.
- `cargo fmt --all -- --check`: environment blocked because `cargo-fmt` is unavailable.

## Known gaps

- The current ring buffer is still configured at 600ms; the next batch extracts the configurable 20/40/50ms window without changing the default until its behavior is tested.
- Cross-stream timestamp alignment and one-hour drift remain unverified on real devices.
