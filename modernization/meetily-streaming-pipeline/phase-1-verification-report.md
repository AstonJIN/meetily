# Verification Report: Phase 1 Code Pilot

## Verdict

The phase-1 code pilot is implemented and pushed on `codex/streaming-pipeline-phase1`. Its deterministic Rust tests pass and the existing audio test suite has no new failures. The phase-1 runtime exit is **not proven** because fixed media fixtures, target hardware, real-device saturation tests, and long-duration soak evidence are absent.

## Implemented slices

| Slice | Evidence | Result |
|---|---|---|
| Aggregated pipeline observations | `phase-0-baseline.md`, metrics unit tests | Code gate met; runtime thresholds blocked |
| Input queue adapter | `8c5d54d`, bounded queue unit tests | Opt-in bounded `try_send`; legacy adapter retained |
| Session clock and audio PTS | `6c1a0ae`, SessionClock/AudioChunk tests | Monotonic recording-relative `pts_ns` at audio capture boundary |
| 20/40/50ms mix window | `00c47f8`, ring-buffer tests | Streaming pilot accepts only 20, 40, or 50ms; disabled path stays 600ms |
| Bounded ASR handoff | `b7aa445`, queue/metrics tests | Non-blocking Full path with degraded event and bounded recovery ranges |
| Bounded recording handoff | `68b0eae`, queue tests | Non-blocking Full/Closed path with explicit audio error reporting |

## Exact verification

- `cargo test --lib audio::bounded_queue`: 7 passed, 0 failed.
- `cargo test --lib audio::pipeline_metrics`: 4 passed, 0 failed.
- `cargo test --lib audio::session_clock`: 3 passed, 0 failed.
- `cargo test --lib audio::recording_state`: 3 passed, 0 failed.
- `cargo test --lib audio::pipeline::tests`: 2 passed, 0 failed.
- `MEETILY_STREAMING_PIPELINE_V1=1 cargo test --lib audio::bounded_queue`: 7 passed, 0 failed.
- Final `cargo test --lib audio`: 100 passed, 2 failed, 3 ignored out of 105.
- `git diff --check`: passed for each implementation slice.
- `cargo fmt --all -- --check`: blocked because `cargo-fmt` is unavailable in the active toolchain.

## Baseline failures preserved

- `audio::device_detection::tests::test_calculate_buffer_timeout_bluetooth`: exact floating-point duration mismatch (`159.999996ms` vs `160ms`).
- `audio::vad::tests::test_vad_large_file_progress`: the 120-second synthetic signal found 1 segment while the existing expectation requires at least 6.

These were present before the phase-1 changes and were not altered as part of the migration slice.

## Open gates

- No 10-minute repeated run or 2-hour soak with RSS/queue thresholds.
- No real microphone/system-device saturation injection, no-loss recording proof, or cross-device drift measurement.
- No video `pts_ns` integration; screen capture remains outside this audio-only slice.
- Phase 2 Zipformer is blocked pending a selected runtime, model/vocabulary, license, CPU fallback, and fixed benchmark fixtures; see `implementation/phase-2-zipformer/blocker-record.md`.
