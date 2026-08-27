# 阶段 2d：固定素材回放与验证记录

## 真实模型素材

用户下载并解压的目录：

```text
/Users/jance/sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20
```

必需文件已验证存在且非空：int8 encoder、fp32 decoder、int8 joiner 和 `tokens.txt`。官方归档还包含 `test_wavs/0.wav`，用于本次 smoke replay。

## 回放命令

```bash
MEETILY_ZIPFORMER_V1=1 \
MEETILY_ZIPFORMER_MODEL_DIR=/Users/jance/sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20 \
CARGO_BUILD_JOBS=2 \
cargo test --manifest-path frontend/src-tauri/Cargo.toml \
  --lib audio::transcription::zipformer_provider::tests::replays_downloaded_bilingual_fixture \
  -- --ignored --nocapture
```

结果：1 passed / 0 failed；sherpa-onnx native runtime 成功加载模型，固定 WAV replay 产生至少一个 transcription update。

## 尚未宣称的验收项

本次只证明模型、native runtime、Rust provider 生命周期和固定素材 smoke replay。尚未宣称 CER/WER、partial/final P95、RTF、RSS soak、抢话/长静音端点，以及真实设备录音接受；这些需要固定中文/中英混合素材和目标 Mac 采集证据后才能决定是否默认切换。
