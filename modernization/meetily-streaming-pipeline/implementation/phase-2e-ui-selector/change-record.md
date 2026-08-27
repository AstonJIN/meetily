# 阶段 2e：设置页 Zipformer 选择器

## 目标

让测试人员可以在 Meetily 的转录设置中直接选择 Zipformer，并在录音启动前验证已解压模型目录；Whisper/Parakeet 的既有设置入口保持不变。

## 改动

- `frontend/src/components/TranscriptSettings.tsx`
  - 增加 `Zipformer (sherpa-onnx streaming test)` provider。
  - 增加模型目录输入、验证并保存按钮。
  - 空目录使用 native 默认目录解析；成功验证后保存解析后的目录。
- `frontend/src/components/LanguageSelection.tsx`
  - 接受新的 `zipformer` provider 类型。
- `frontend/src/hooks/useRecordingStart.ts`
  - 启动前读取当前 transcript provider。
  - 选择 Zipformer 时调用 native 模型目录验证，不再误用 Parakeet 就绪检查。

## 验证

- `pnpm install --frozen-lockfile`: 完成，安装锁定依赖。
- `pnpm exec tsc --noEmit`: 通过。
- `pnpm build`: 通过。
- 首次类型检查发现 `LanguageSelection` provider 联合类型缺少 Zipformer，补齐后复跑通过。
- 新 bundle 的 GUI 检查：设置页显示 Zipformer，输入实际模型目录后“验证并保存”成功；重启 bundle 后 provider 和目录仍恢复。

## 已知边界

- 本批次验证的是设置页到 native command 的 GUI 可调用链路；尚未进行真实设备录音验收。
- Zipformer 仍是开发测试入口，不改变默认 provider。
