# 阶段 2a：Zipformer 运行时与模型接入

## 范围

- 为 `frontend/src-tauri` 增加 `sherpa-onnx 1.13.6` 静态 Rust 依赖。
- 新增双语 Zipformer 模型目录、文件完整性和非空校验。
- 增加开发环境开关，默认不改变现有录音行为。
- 增加可选热词文件配置入口。

## 配置

```text
MEETILY_ZIPFORMER_V1=1
MEETILY_ZIPFORMER_MODEL_DIR=/path/to/sherpa-onnx-streaming-zipformer-bilingual-zh-en-2023-02-20
MEETILY_ZIPFORMER_NUM_THREADS=1
MEETILY_ZIPFORMER_HOTWORDS_FILE=/path/to/hotwords.txt
```

未设置 `MEETILY_ZIPFORMER_MODEL_DIR` 时，试点会查找当前用户 home 目录下的官方归档目录名。未设置 `MEETILY_ZIPFORMER_V1` 时，Zipformer 不会被初始化。

## 验证

命令：

```bash
CARGO_BUILD_JOBS=2 cargo test --manifest-path frontend/src-tauri/Cargo.toml --lib audio::transcription
```

结果：6 个可运行测试通过，1 个真实模型回放测试按需忽略。首次依赖编译遇到工作区磁盘不足，清理可再生 `target` 构建产物后，以 2 并行任务重新编译通过；源文件、模型压缩包和解压目录未删除。

## 兼容与回退

Zipformer 只在显式 pilot 开关打开且模型/运行时初始化成功时被选择。预检或初始化失败会回到数据库当前配置的 Whisper/Parakeet 路径；没有删除或改写两条旧 provider 路径。
