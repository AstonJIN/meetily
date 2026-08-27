# 阶段 2f：Zipformer 配置路由与回退

## 目标

将设置页保存的 `provider=zipformer` 和模型目录接入 native transcription engine，并保留 Parakeet/Whisper 的安全回退路径。

## 改动

- `frontend/src-tauri/src/audio/transcription/zipformer_provider.rs`
  - 增加按指定目录构造 runtime 的入口。
  - 增加 `zipformer_validate_model` Tauri command，只做必需模型文件校验。
- `frontend/src-tauri/src/audio/transcription/engine.rs`
  - 根据保存的 Zipformer 模型目录初始化 provider。
  - Zipformer 初始化失败时依次尝试 Parakeet，再回退到 Whisper。
  - Zipformer 成功时先卸载旧 ASR，避免同时持有重模型。
- `frontend/src-tauri/src/database/repositories/setting.rs`
  - 将 Zipformer 作为无需 API key 的本地 provider。
- `frontend/src-tauri/src/lib.rs`
  - 注册 `zipformer_validate_model` command。

## 验证

- `CARGO_BUILD_JOBS=2 cargo test --manifest-path frontend/src-tauri/Cargo.toml --lib audio::transcription`: 8 passed / 0 failed / 1 ignored。
- 真实已下载双语模型 replay：1 passed / 0 failed；测试使用按指定模型目录构造的 Zipformer provider，并验证 partial/final、drain、unload 生命周期。
- `target/release/bundle/macos/meetily.app` 内容检查：包含 `zipformer_validate_model`、`MEETILY_ZIPFORMER_*` 和 sherpa Zipformer 符号；`codesign --verify --deep --strict` 通过。
- `git diff --check`: 通过。

## 已知边界

- 回退分支已接入并有编译覆盖；在没有可用 Parakeet/Whisper 模型的故障注入环境中，尚未做 GUI 级回退演练。
- `pnpm run tauri:build` 生成 `.app`、DMG 和 updater archive；最后因本机缺少 `TAURI_SIGNING_PRIVATE_KEY` 返回 1，未完成 updater 签名，不影响 `.app` 产物本身。
- CER/WER、partial/final P95、RTF、RSS soak 和真实麦克风/系统音频仍属于阶段 2 的后续验收项。
