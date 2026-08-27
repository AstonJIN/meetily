# Blocker Record: phase-2-zipformer

## Decision

Do not connect a Zipformer provider to the live transcription path yet. The phase-2 entry contract requires an independent prototype with a locked model, vocabulary, runtime, and license before the main pipeline is changed.

## Repository evidence

- `frontend/src-tauri/Cargo.toml` declares `ort` for the existing Parakeet implementation, but no Zipformer or sherpa-onnx runtime dependency.
- `frontend/src-tauri/src/audio/transcription/parakeet_provider.rs` exposes only batch `transcribe(Vec<f32>)` behavior and sets `is_partial: false`.
- `frontend/src-tauri/src/parakeet_engine/` contains Parakeet model discovery/loading and existing ONNX files are not present in the repository.
- No Zipformer model, vocabulary, decoder configuration, or reproducible benchmark fixture exists in the workspace.

## Blocked checks

- Zipformer `accept_audio` / `partial` / `final` / `reset` / `drain` / `unload` behavior: not implemented because the concrete runtime contract is not selected.
- Chinese and mixed-language CER/WER, partial/final latency, RTF, model load/RSS, and fallback tests: blocked by missing model/runtime/fixtures.
- Default-path switch: intentionally not changed; Whisper and current Parakeet paths remain intact.

## Required unblock evidence

1. Select and provide the Zipformer runtime and exact version.
2. Confirm model, vocabulary, decoding configuration, license, and CPU-only fallback.
3. Add fixed audio fixtures plus expected partial/final semantics and benchmark thresholds.

The current phase-1 low-latency pilot remains independently rollback-safe and does not depend on this blocked provider.
