# 阶段 2：Zipformer 旁路接入验证报告

## 结论

阶段 2 已完成“可编译、可加载、可固定素材回放”的旁路接入，但未达到默认 provider 切换出口。Whisper 和 Parakeet 仍是默认/回退路径；Zipformer 只有在 `MEETILY_ZIPFORMER_V1=1` 且模型运行时预检成功时才选择。

## 已验证

| 范围 | 结果 |
|---|---|
| sherpa-onnx Rust 依赖与现有 Parakeet `ort` 并存编译 | 通过 |
| 双语 Zipformer encoder/decoder/joiner/tokens 布局校验 | 通过 |
| StreamingAsrEngine 生命周期与 partial 去重契约 | 通过 |
| Worker Zipformer 分支、drain、unload 编译 | 通过 |
| 用户下载的双语 Zipformer + `test_wavs/0.wav` native replay | 1/1 通过 |
| 全 audio 单元回归 | 106 通过 / 2 基线失败 / 4 忽略 |

全 audio 回归中的两个失败与阶段 1 相同：

- `audio::device_detection::tests::test_calculate_buffer_timeout_bluetooth`：`159.999996ms` 与 `160ms` 的精确浮点断言。
- `audio::vad::tests::test_vad_large_file_progress`：120 秒合成素材只产生 1 个 speech segment，测试期待至少 6 个。

没有发现由 Zipformer 接入新增的测试失败。

## 未通过/未宣称

尚未有固定中文/中英混合评测集的 CER/WER、partial/final P95、RTF、模型加载/卸载 RSS、长静音/抢话端点和真实设备录音证据。当前 Worker 仍复用既有 VAD speech-range 入口；下一批应将 raw mixed 20–50ms frame 旁路给 Zipformer，并为前端实现稳定 utterance 的 partial 覆盖、final 追加，而不是把每个 partial 当作新片段。

## 回滚

不设置 `MEETILY_ZIPFORMER_V1` 即回到现有数据库 provider 选择；删除或清空 `MEETILY_ZIPFORMER_MODEL_DIR` 会触发预检失败并继续 Whisper/Parakeet 回退。旧模型、命令和事件格式均保留。
