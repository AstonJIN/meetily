# 阶段 0：基线与观测能力

## 执行边界

本阶段只增加聚合观测，不改变音频数据、队列类型、发送策略、UI 事件、数据库结构或录音文件格式。观测开关为 `MEETILY_AUDIO_PIPELINE_METRICS`，默认开启；设置为 `0`、`false` 或 `off` 可关闭。

## Git 与同步证据

- 远程：`origin = https://github.com/AstonJIN/meetily.git`
- 同步命令：`git fetch origin --prune`
- 同步结果：执行前后的 `origin/main` 与本地基线均为 `91b0c09`；远程现有可见分支只有 `main`。
- 实施分支：`codex/streaming-pipeline-phase1`，已推送为 `origin/codex/streaming-pipeline-phase1`。
- 既有未提交改动：未纳入本批 diff；根仓库和 `backend/whisper.cpp` 均保留了本地可恢复 stash 快照。

## 代码基线

已确认当前路径仍使用无界 Tokio channel：

- `frontend/src-tauri/src/audio/recording_state.rs`：采集入口持有音频无界 sender。
- `frontend/src-tauri/src/audio/pipeline.rs`：输入、转录输出和混音录音输出均使用无界 sender/receiver。
- `frontend/src-tauri/src/audio/transcription/worker.rs`：转录 dispatcher 使用无界 channel。
- `frontend/src-tauri/src/audio/recording_saver.rs`：录音保存任务使用无界 channel。

## 新增观测

`frontend/src-tauri/src/audio/pipeline_metrics.rs` 提供共享 session 聚合器，记录：

- 输入、转录、录音队列当前深度、峰值深度、入队/出队数和发送失败数。
- 管道处理 chunk 数、平均/最大处理耗时和最新录音相对时间戳。
- 当前进程 RSS 与本 session RSS 峰值。
- 每 5 秒最多输出一次 `audio_pipeline_metrics` 汇总日志；停止/flush 时强制输出一次。

观测器只保留计数器和每个队列一个等待起点，不保留逐 chunk 历史；它本身不会随会议时长线性增长。队列等待值是从当前非空队列 epoch 起点估算，阶段 1 将以有界队列和带时间戳的包重新定义精确语义。

## 测试证据

### 修改前基线

命令：`cargo test --lib audio`（目录：`frontend/src-tauri`）

- 86 tests：81 passed，2 failed，3 ignored。
- 已有失败：
  - `audio::device_detection::tests::test_calculate_buffer_timeout_bluetooth`：`159.999996ms` 与 `160ms` 的浮点精度断言。
  - `audio::vad::tests::test_vad_large_file_progress`：120 秒合成输入只得到 1 段，测试期望至少 6 段。

### 修改后

命令：`cargo test --lib audio::pipeline_metrics`

- 3 passed，0 failed，0 ignored。

命令：`cargo test --lib audio`

- 89 tests：84 passed，2 failed，3 ignored。
- 失败集合与基线一致；新增 3 个 metrics 测试全部通过，未发现本批新增失败。

### 环境阻断

- `cargo fmt --all -- --check`：阻断，当前 stable toolchain 未安装 `cargo-fmt` 组件。
- `pnpm exec tsc --noEmit`：阻断，当前 `frontend` 依赖中没有 `tsc` 可执行文件。
- 10 分钟素材三次重复、2 小时 soak、目标机器 RSS/CPU/队列水位和音视频漂移：当前仓库没有固定媒体素材或目标硬件记录，暂未宣称通过。

## 阶段 0 结论

代码级观测试点已完成，单元测试通过且现有失败未扩大；运行时性能基线仍为 `证据不足`，不能据此证明第一批验收阈值或后续优化收益。
