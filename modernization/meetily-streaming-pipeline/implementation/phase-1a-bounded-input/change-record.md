# Change Record: phase-1a-bounded-input

## Objective

为采集到音频管道的输入边界提供可回退的有界队列适配器，确保音频回调使用非阻塞 `try_send`，队列满载时返回明确错误而不是静默等待或静默丢弃。

## Scope

- Added `frontend/src-tauri/src/audio/bounded_queue.rs`.
- Updated `audio/mod.rs`, `recording_state.rs`, `pipeline.rs`, `recording_manager.rs`, `recording_commands.rs` for the adapter boundary.
- Existing unbounded behavior remains the default.
- Bounded input is enabled only by `MEETILY_STREAMING_PIPELINE_V1=1`.
- Capacity is configurable with `MEETILY_AUDIO_INPUT_QUEUE_CAPACITY`; invalid or zero values fall back to 64.

## Compatibility and rollback

- `AudioChunk` and all frontend/database contracts remain unchanged.
- `AudioPipelineManager::force_flush_and_stop` uses the same non-blocking boundary; Full/Closed is logged explicitly.
- Rollback: unset `MEETILY_STREAMING_PIPELINE_V1`, or set it to `0`/`false`/`off`, to use the legacy unbounded adapter.

## Verification

- `cargo test --lib audio::bounded_queue`: 4 passed, 0 failed.
- `MEETILY_STREAMING_PIPELINE_V1=1 cargo test --lib audio::bounded_queue`: 4 passed, 0 failed.
- `cargo test --lib audio`: 88 passed, 2 failed, 3 ignored out of 93; the same two baseline failures remain.
- `git diff --check`: pending final batch check.
- `cargo fmt --all -- --check`: environment blocked because `cargo-fmt` is unavailable.

## Known gaps

- This pilot reports queue saturation but still drops the rejected item at the existing caller boundary after returning an explicit error; the no-audio-loss policy is not yet proven.
- Only the input queue is switched. Transcription and recording output queues remain legacy paths until their own non-blocking/backpressure contracts are reviewed.
- No real-device, 10-minute, 2-hour soak, or queue-saturation fault-injection evidence exists yet.
