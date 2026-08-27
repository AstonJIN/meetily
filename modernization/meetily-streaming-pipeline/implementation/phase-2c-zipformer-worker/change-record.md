# 阶段 2c：Worker 路由与旧 provider 回退

## 实现

- `TranscriptionEngine` 增加显式 `Zipformer` 分支。
- Worker 支持一个音频 chunk 返回多个 streaming updates；Whisper、Parakeet 和现有 trait provider 仍保持最多一个结果的行为。
- 停止输入后调用 Zipformer `drain`，随后调用 `unload`，避免 final 假设滞留并释放运行时会话。
- pilot 预检优先于旧模型预加载，成功时避免同时加载第二个重 ASR 模型；失败时继续原有 Whisper/Parakeet 验证与初始化。

## 当前边界

该阶段保持现有 VAD → ASR 兼容入口，因此 Zipformer pilot 接收 VAD 产生的 16kHz speech ranges；固定素材回放直接按 20ms（320 samples）输入 streaming session。将原始混音帧旁路为真正逐帧 ASR 输入，以及 partial 的 UI 覆盖语义，留给阶段 2 后续验收批次，不切换默认 provider。

## 验证

```bash
CARGO_BUILD_JOBS=2 cargo test --manifest-path frontend/src-tauri/Cargo.toml --lib audio::transcription
```

结果：6 passed / 0 failed，1 ignored（真实模型回放）。
