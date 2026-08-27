# Change Record: phase-1d-bounded-asr

## Objective

Add a rollback-safe, non-blocking bounded queue at the VAD-to-transcription boundary. Queue saturation must be observable and must retain bounded recording-relative ranges for later retranscription.

## Files changed by this batch

- `frontend/src-tauri/src/audio/bounded_queue.rs`
- `frontend/src-tauri/src/audio/pipeline.rs`
- `frontend/src-tauri/src/audio/recording_manager.rs`
- `frontend/src-tauri/src/audio/transcription/worker.rs`
- `frontend/src-tauri/src/audio/pipeline_metrics.rs`
- `modernization/meetily-streaming-pipeline/implementation/phase-1d-bounded-asr/change-record.md`

## Behavior and rollback

- When `MEETILY_STREAMING_PIPELINE_V1` is enabled, the transcription queue is bounded to 8 items by default and can be changed with `MEETILY_TRANSCRIPTION_QUEUE_CAPACITY`.
- `try_send` is used at the audio-pipeline boundary; a full queue never makes the audio pipeline await.
- Full/closed transcription sends emit a `transcription_degraded` warning and increment the aggregated failure/degraded counters.
- Up to 32 recording-relative ranges are retained for later retranscription; older ranges are counted as dropped once this bounded diagnostic buffer is full.
- With the feature switch disabled, the adapter continues to use the legacy unbounded queue.
- The recording queue remains unchanged in this subsection and is still covered by its own follow-up slice.

## Verification

- `cargo test --lib audio::bounded_queue`: 6 passed, 0 failed.
- `cargo test --lib audio::pipeline_metrics`: 4 passed, 0 failed.
- `cargo test --lib audio`: 99 passed, 2 failed, 3 ignored out of 104.
- `git diff --check`: passed.
- `cargo fmt --all -- --check`: remains blocked because `cargo-fmt` is unavailable in the active toolchain.

## Known gaps

- Queue saturation was proven at the adapter/metrics unit level; no live ASR overload injection or real-device run was available.
- The retained ranges are an observable recovery contract; automatic retranscription is not implemented in this slice.
- Runtime queue latency, RSS, no-loss recording behavior, and long-duration soak thresholds remain unverified.

## Baseline failure preservation

The two failing audio tests are unchanged baseline failures:

- Bluetooth buffer timeout exact-float assertion (`159.999996ms` vs `160ms`).
- VAD large-file synthetic segmentation expectation (1 found vs at least 6 expected).
