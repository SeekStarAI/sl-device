# sl-device

StarLight 的 Virtual Device Interface（VDI）契约层：定义后端必须实现的缓冲区、上传/下载与单算子分发接口。

## 当前能力

- `BufferRef { buffer, offset, len }`：元素级缓冲区视图；
- `DeviceError`：内存、缓冲区、范围、算子、溢出与执行错误；
- `trait Device: Send`：
  - `allocate_f32` / `allocate_u32`
  - `upload_f32` / `upload_u32`
  - `download_f32` / `download_u32`
  - `dispatch(op, inputs, input_values, outputs, output_values, scalars)`
  - `synchronize`

v1 只覆盖 `F32` 与 `U32`，同步执行，不包含队列、命令录制或异步完成。

## 契约

- 设备只执行**单算子**，不解释命令依赖、不持有请求、不做调度；
- 命令顺序、scratchpad 偏移与绑定解析由 `sl-core` 负责；
- `dispatch` 不得分配堆内存；
- `inputs.len() == input_values.len()`，`outputs.len() == output_values.len()`；
- `BufferRef.len` 必须等于对应 value 的元素数；
- 后端通过 `dispatch` 匹配 `sl_ir::Op` 并调用自己的内核。

参考实现：`starlight-backend-cpu::CpuDevice`。

契约依据：[ARCH-0007](<../../docs/ARCH-0007: Graph Core Runtime 三线施工方案.md>)。

## 开发

在 StarLight workspace 根目录运行：

```sh
cargo test -p sl-device
cargo clippy -p sl-device --all-targets -- -D warnings
```

当前 3 个单元测试，覆盖错误格式化与 trait 对象的静态约束。
