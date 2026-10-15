# sl-device

StarLight 的 Virtual Device Interface（VDI）契约层，以稳定的 Rust traits 描述计算设备、队列、缓冲区和执行资源。

## 职责边界

- 定义后端实现所需的设备与资源接口。
- 为 CPU、Vulkan 及后续第三方后端提供统一契约。
- 不包含具体硬件驱动或调度策略。

接口与实现仍在基线阶段；具体契约以 RFC-0001 为准。

## 开发

在 StarLight workspace 根目录运行 `cargo check -p sl-device` 或 `cargo test -p sl-device`。
