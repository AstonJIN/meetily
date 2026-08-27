# Change Record: phase-0-observability

## Objective

为现有音频输入、转录和录音队列增加聚合观测，为第一批有界管道改造建立可对照的代码基线；保持现有行为不变。

## Traceability

- Migration plan: `migration-plan.md` §5 阶段 0。
- Protected compatibility: 音频数据、无界队列行为、转录事件、录音保存格式、UI/数据库接口均保持不变。
- Rollback: 设置 `MEETILY_AUDIO_PIPELINE_METRICS=0` 关闭观测；如需完全移除，回退本 batch 文件即可，不涉及数据迁移。

## Files changed by this batch

- `frontend/src-tauri/src/audio/pipeline_metrics.rs`
- `frontend/src-tauri/src/audio/mod.rs`
- `frontend/src-tauri/src/audio/recording_state.rs`
- `frontend/src-tauri/src/audio/pipeline.rs`
- `frontend/src-tauri/src/audio/recording_manager.rs`
- `frontend/src-tauri/src/audio/recording_saver.rs`
- `frontend/src-tauri/src/audio/transcription/worker.rs`
- `frontend/src-tauri/src/audio/recording_commands.rs`
- `modernization/meetily-streaming-pipeline/phase-0-baseline.md`

## Verification

- Baseline: `cargo test --lib audio` → 81 passed, 2 failed, 3 ignored。
- Focused: `cargo test --lib audio::pipeline_metrics` → 3 passed, 0 failed。
- Full audio: `cargo test --lib audio` → 84 passed, 2 failed, 3 ignored；失败集合与基线一致。
- `git diff --check`：待本 batch 最终复核时运行。
- `cargo fmt --all -- --check`：环境阻断，`cargo-fmt` 未安装。
- `pnpm exec tsc --noEmit`：环境阻断，`tsc` 不存在。

## Review notes

- 观测器不保存逐 chunk 数据，仅使用原子聚合值与每个队列一个等待起点。
- 观测调用不改变 send/receive 的成功路径；发送失败只增加计数并保留原错误处理。
- `start_transcription_task` 的新增参数已更新两个实际调用点；旧的 `.backup` 文件未参与编译。
- 未修改 `AudioChunk` 数据结构，因此没有增加序列化或跨模块数据兼容风险。

## Known gaps

- 真实 10 分钟重复样本、2 小时 soak、目标机器和音视频漂移数据缺失。
- 当前输入/输出仍是无界队列；本批只建立基线，不宣称已经满足内存硬上限。
- 队列等待值是非空队列 epoch 的估计值，阶段 1 需在统一 `pts_ns` 包中定义精确时间语义。
