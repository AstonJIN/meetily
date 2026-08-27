# Change Record: phase-1c-mix-window

## Objective

Extract the hard-coded mixer window into a validated 20/40/50ms configuration while keeping the legacy 600ms window as the default when the streaming pilot is disabled.

## Files changed by this batch

- `frontend/src-tauri/src/audio/bounded_queue.rs`
- `frontend/src-tauri/src/audio/pipeline.rs`
- `modernization/meetily-streaming-pipeline/implementation/phase-1c-mix-window/change-record.md`

## Behavior and rollback

- `MEETILY_AUDIO_MIX_WINDOW_MS` accepts only 20, 40, or 50; invalid values use 40ms for the streaming pilot.
- `MEETILY_STREAMING_PIPELINE_V1` unset/false keeps the existing 600ms window.
- The ring buffer retains the existing eight-window jitter allowance, now derived from the selected window.
- No default recording path, VAD logic, or public event contract changed.

## Verification

- `cargo test --lib audio::pipeline::tests`: 2 passed, 0 failed.
- `cargo test --lib audio::bounded_queue`: 6 passed, 0 failed.
- `cargo test --lib audio`: 98 passed, 2 failed, 3 ignored out of 103; the same two baseline failures remain.
- `git diff --check`: pending final batch check.
- `cargo fmt --all -- --check`: blocked because `cargo-fmt` is unavailable in the active toolchain.

## Known gaps

- 20/40/50ms behavior is proven by deterministic ring-buffer tests, not by live microphone/system-device capture.
- RSS, queue saturation, audio loss, and cross-device drift remain unverified.
