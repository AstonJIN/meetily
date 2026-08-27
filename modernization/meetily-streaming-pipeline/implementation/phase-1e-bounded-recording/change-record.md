# Change Record: phase-1e-bounded-recording

## Objective

Put the mixed-audio recording handoff behind the same rollback-safe queue adapter and make queue saturation explicit. Recording data must never be counted as successfully enqueued when the saver cannot accept it.

## Files changed by this batch

- `frontend/src-tauri/src/audio/bounded_queue.rs`
- `frontend/src-tauri/src/audio/pipeline.rs`
- `frontend/src-tauri/src/audio/recording_manager.rs`
- `frontend/src-tauri/src/audio/recording_saver.rs`
- `modernization/meetily-streaming-pipeline/implementation/phase-1e-bounded-recording/change-record.md`

## Behavior and rollback

- When `MEETILY_STREAMING_PIPELINE_V1` is enabled, the recording queue is bounded to 256 chunks by default and can be changed with `MEETILY_RECORDING_QUEUE_CAPACITY`.
- The pipeline uses non-blocking `try_send` for mixed recording chunks.
- A full recording queue increments the recording queue failure metric, reports `AudioError::BufferOverflow`, and emits a `recording_degraded` warning; it is not treated as a successful send.
- A closed recording queue reports `AudioError::ChannelClosed` and is also observable.
- With the feature switch disabled, the recording adapter remains backed by the legacy unbounded queue.

## Verification

- `cargo test --lib audio::bounded_queue`: 7 passed, 0 failed.
- `cargo test --lib audio`: 100 passed, 2 failed, 3 ignored out of 105.
- `git diff --check`: passed.
- `cargo fmt --all -- --check`: remains blocked because `cargo-fmt` is unavailable in the active toolchain.

## Known gaps

- Saturation behavior is covered by the generic queue adapter test; no real disk-I/O slowdown or live-device saturation injection was available.
- The bounded queue prevents unbounded growth but cannot guarantee zero physical loss if the saver remains unable to consume; the explicit error/degraded path is the safety behavior for that condition.
- Stop-time draining still uses the existing saver lifecycle and fixed wait; replacing that with `StopCoordinator` belongs to phase 4.

## Baseline failure preservation

The two failing audio tests are unchanged baseline failures:

- Bluetooth buffer timeout exact-float assertion (`159.999996ms` vs `160ms`).
- VAD large-file synthetic segmentation expectation (1 found vs at least 6 expected).
