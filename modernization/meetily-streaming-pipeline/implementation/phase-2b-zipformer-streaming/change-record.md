# 阶段 2b：Zipformer StreamingAsrEngine

## 实现

`zipformer_provider.rs` 提供 provider-neutral 的 `StreamingAsrEngine` 生命周期：

- `accept_audio`
- `partial`
- `final_result`
- `set_hotwords_file`
- `reset`
- `drain`
- `unload`

Zipformer 使用 sherpa-onnx 的 online transducer API，固定接收 16kHz mono `f32`。会话通过互斥锁串行化；每个会话只保留最近 partial/final 状态，不保存无界的逐帧历史。相同 partial 文本不会重复返回，endpoint 或 drain 会产生 final 结果并重置上下文。

## 验证

默认单元测试验证模型布局、pilot flag、线程数和热词配置契约：6 passed / 0 failed；真实模型回放另行执行。
