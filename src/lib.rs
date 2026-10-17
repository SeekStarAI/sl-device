// SPDX-License-Identifier: MIT
// Copyright (c) 2026 SeekStarAI

//! Virtual device interface contracts for StarLight backends.
//!
//! A device owns its buffers and executes **single operators**. It does not
//! interpret dependencies, own requests or schedule commands: `sl-core`
//! resolves the command stream, offsets and scalar values and calls
//! [`Device::dispatch`] in order.
//!
//! The contract is frozen by `docs/ARCH-0007` §3.1. Two conventions complete
//! the parts the signature does not spell out:
//!
//! - Scalar operands (`ValueRole::Scalar`) have no buffer and are **not**
//!   included in `inputs`/`input_values`; they are passed through `scalars`, in
//!   input-port order. At most four scalar operands are passed per command.
//! - A command with a persistent-state effect (`KvAppend`, `CausalAttention`)
//!   receives that state buffer as an extra final input, with a `State` value
//!   in `input_values`. The device may read and write it. This is the only way
//!   a stateless `dispatch` signature can reach the KV cache.

use core::fmt;

use sl_ir::{BufferId, Op, OpCode, Value};

/// A typed slice of a device buffer.
///
/// `offset` and `len` are **element** counts. The element type follows the
/// dtype of the value the reference is bound to: `f32` for `F32` values and
/// `u32` for `U32` values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BufferRef {
    /// Backing buffer.
    pub buffer: BufferId,
    /// Offset in elements from the start of the buffer.
    pub offset: usize,
    /// Number of elements covered by the reference.
    pub len: usize,
}

/// Failure reported by a device.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceError {
    /// A buffer allocation could not be satisfied.
    OutOfMemory {
        /// Requested number of elements.
        requested: usize,
    },
    /// The buffer id is not known to the device.
    InvalidBuffer(BufferId),
    /// The requested element range is outside the buffer.
    InvalidRange {
        /// Buffer that was addressed.
        buffer: BufferId,
        /// Requested element offset.
        offset: usize,
        /// Requested element count.
        len: usize,
    },
    /// The device does not implement the operator.
    UnsupportedOp(OpCode),
    /// An element count or byte count overflowed.
    ShapeOverflow,
    /// The device rejected the command.
    Execution(&'static str),
    /// The device does not implement the requested feature yet.
    Unimplemented(&'static str),
}

impl fmt::Display for DeviceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfMemory { requested } => {
                write!(
                    formatter,
                    "device out of memory allocating {requested} elements"
                )
            }
            Self::InvalidBuffer(buffer) => write!(formatter, "unknown buffer {}", buffer.0),
            Self::InvalidRange {
                buffer,
                offset,
                len,
            } => write!(
                formatter,
                "range {offset}..{} is outside buffer {}",
                offset.saturating_add(*len),
                buffer.0
            ),
            Self::UnsupportedOp(op) => write!(formatter, "unsupported operator {op:?}"),
            Self::ShapeOverflow => formatter.write_str("element count overflows usize"),
            Self::Execution(reason) => write!(formatter, "device execution failed: {reason}"),
            Self::Unimplemented(feature) => {
                write!(formatter, "device feature is not implemented: {feature}")
            }
        }
    }
}

impl core::error::Error for DeviceError {}

/// A compute device: buffer storage plus single-operator execution.
///
/// Implementations must be `Send` because an engine may be moved between
/// threads. `dispatch` must not allocate and must not retain references to the
/// `Value` slices after it returns.
pub trait Device: Send {
    /// Allocates an `f32` buffer of `elements` elements.
    ///
    /// `align` is the requested alignment in bytes. A device that cannot
    /// honour it should document the strongest alignment it guarantees.
    fn allocate_f32(&mut self, elements: usize, align: usize) -> Result<BufferId, DeviceError>;

    /// Allocates a `u32` buffer of `elements` elements.
    fn allocate_u32(&mut self, elements: usize, align: usize) -> Result<BufferId, DeviceError>;

    /// Uploads `data` into the start of `buffer`.
    fn upload_f32(&mut self, buffer: BufferId, data: &[f32]) -> Result<(), DeviceError>;

    /// Uploads `data` into the start of `buffer`.
    fn upload_u32(&mut self, buffer: BufferId, data: &[u32]) -> Result<(), DeviceError>;

    /// Downloads the whole `buffer` into `out`.
    fn download_f32(&mut self, buffer: BufferId, out: &mut [f32]) -> Result<(), DeviceError>;

    /// Downloads the whole `buffer` into `out`.
    fn download_u32(&mut self, buffer: BufferId, out: &mut [u32]) -> Result<(), DeviceError>;

    /// Executes one operator.
    ///
    /// `inputs.len() == input_values.len()` and
    /// `outputs.len() == output_values.len()`. Every [`BufferRef::len`] equals
    /// the corresponding value's element count. Scalar operands are supplied
    /// through `scalars` in input-port order. Stateful operators receive the
    /// persistent state buffer as their final input (see the crate docs).
    #[allow(clippy::too_many_arguments)]
    fn dispatch(
        &mut self,
        op: &Op,
        inputs: &[BufferRef],
        input_values: &[&Value],
        outputs: &[BufferRef],
        output_values: &[&Value],
        scalars: &[u32],
    ) -> Result<(), DeviceError>;

    /// Blocks until every previously dispatched command has completed.
    fn synchronize(&mut self) -> Result<(), DeviceError>;
}

#[cfg(test)]
mod tests {
    use super::{BufferRef, DeviceError};
    use sl_ir::{BufferId, OpCode};

    fn assert_error<E: core::error::Error>() {}

    #[test]
    fn buffer_refs_are_plain_element_ranges() {
        let reference = BufferRef {
            buffer: BufferId(3),
            offset: 64,
            len: 16,
        };
        assert_eq!(reference.buffer, BufferId(3));
        assert_eq!(reference.offset, 64);
        assert_eq!(reference.len, 16);
        assert_eq!(reference, reference);
        assert_ne!(
            reference,
            BufferRef {
                buffer: BufferId(3),
                offset: 64,
                len: 15,
            }
        );
    }

    #[test]
    fn device_errors_are_diagnostic() {
        assert_eq!(
            DeviceError::OutOfMemory { requested: 128 }.to_string(),
            "device out of memory allocating 128 elements"
        );
        assert_eq!(
            DeviceError::InvalidRange {
                buffer: BufferId(7),
                offset: 4,
                len: 3,
            }
            .to_string(),
            "range 4..7 is outside buffer 7"
        );
        assert_eq!(
            DeviceError::UnsupportedOp(OpCode::CausalAttention).to_string(),
            "unsupported operator CausalAttention"
        );
        assert_eq!(
            DeviceError::Execution("bad shapes").to_string(),
            "device execution failed: bad shapes"
        );
        assert_eq!(
            DeviceError::Unimplemented("f16").to_string(),
            "device feature is not implemented: f16"
        );
        assert_eq!(
            DeviceError::InvalidBuffer(BufferId(1)).to_string(),
            "unknown buffer 1"
        );
        assert_eq!(
            DeviceError::ShapeOverflow.to_string(),
            "element count overflows usize"
        );
        assert_error::<DeviceError>();
    }

    #[test]
    fn dispatch_accepts_the_frozen_operator_vocabulary() {
        // The frozen interface refers to `sl_ir::Op`; make sure the re-export
        // path stays usable from a device implementation.
        let _op = sl_ir::Op::Silu;
        assert_eq!(sl_ir::Op::Silu.code(), OpCode::Silu);
    }
}
