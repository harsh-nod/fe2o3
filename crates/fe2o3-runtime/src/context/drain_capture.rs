//! Sealed logical registration and post-drain coherent host capture.

use super::*;
use fe2o3_runtime_model::r69_host_capture_range_v1;

/// Fixed capture failures. These values carry no native custody or retry authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeHostCaptureErrorV1 {
    UnsupportedBackend,
    InvalidRange,
    UnknownAllocation,
    ForeignContext,
    DeviceLocal,
    SourceUnavailable,
    ContextTerminal,
    Pending,
    NativeRejected,
    NativeUncertain,
    CaptureIncomplete,
}

impl fmt::Display for RuntimeHostCaptureErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "runtime host capture: {self:?}")
    }
}

impl Error for RuntimeHostCaptureErrorV1 {}

/// A Context-issued logical allocation range, not native read or completion authority.
///
/// Accepted work may still change the bytes or create native backing after this
/// registration. The owner checks the exact live backing only after drain.
/// Releasing the allocation invalidates this registration.
///
/// ```compile_fail
/// use fe2o3_runtime::RuntimeHostCaptureSourceV1;
/// let source = RuntimeHostCaptureSourceV1 {};
/// ```
pub struct RuntimeHostCaptureSourceV1 {
    allocation: RuntimeAllocationIdV1,
    device: RuntimeDeviceIdV1,
    backend_allocation: u64,
    allocation_bytes: u64,
    byte_offset: u64,
    byte_len: usize,
}

impl RuntimeHostCaptureSourceV1 {
    pub const fn allocation(&self) -> RuntimeAllocationIdV1 {
        self.allocation
    }

    pub const fn device(&self) -> RuntimeDeviceIdV1 {
        self.device
    }

    pub const fn byte_offset(&self) -> u64 {
        self.byte_offset
    }

    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    pub(crate) const fn belongs_to_context(&self, generation: u64) -> bool {
        self.allocation.context_generation == generation
    }
}

impl fmt::Debug for RuntimeHostCaptureSourceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeHostCaptureSourceV1")
            .field("allocation", &self.allocation)
            .field("device", &self.device)
            .field("byte_offset", &self.byte_offset)
            .field("byte_len", &self.byte_len)
            .finish_non_exhaustive()
    }
}

/// Owner-created coherent-read request. Only the Context can construct one after
/// validating a private drain witness and its registered logical allocation.
/// Backend implementers must additionally validate their exact native storage.
///
/// ```compile_fail
/// use fe2o3_runtime::BackendHostCaptureV1;
/// let request = BackendHostCaptureV1 {};
/// ```
pub struct BackendHostCaptureV1<'a> {
    device: u64,
    allocation: u64,
    byte_offset: u64,
    destination: &'a mut [u8],
}

impl BackendHostCaptureV1<'_> {
    pub const fn device(&self) -> u64 {
        self.device
    }

    pub const fn allocation(&self) -> u64 {
        self.allocation
    }

    pub const fn byte_offset(&self) -> u64 {
        self.byte_offset
    }

    pub fn destination_mut(&mut self) -> &mut [u8] {
        self.destination
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    pub(crate) const fn capture_context_generation_v1(&self) -> u64 {
        self.context_generation
    }

    /// Registers a positive HostVisible range without touching native storage.
    /// The source is usable only by this Context's eventual progress owner.
    pub fn prepare_host_drain_capture_v1(
        &self,
        allocation: RuntimeAllocationIdV1,
        byte_offset: u64,
        byte_len: usize,
    ) -> Result<RuntimeHostCaptureSourceV1, RuntimeHostCaptureErrorV1> {
        if self.terminal {
            return Err(RuntimeHostCaptureErrorV1::ContextTerminal);
        }
        if allocation.context_generation != self.context_generation {
            return Err(RuntimeHostCaptureErrorV1::ForeignContext);
        }
        let record = self
            .allocations
            .get(&allocation)
            .ok_or(RuntimeHostCaptureErrorV1::UnknownAllocation)?;
        if record.kind != RuntimeMemoryKindV1::HostVisible {
            return Err(RuntimeHostCaptureErrorV1::DeviceLocal);
        }
        let length =
            u64::try_from(byte_len).map_err(|_| RuntimeHostCaptureErrorV1::InvalidRange)?;
        if !r69_host_capture_range_v1(byte_offset, length, record.byte_len, length) {
            return Err(RuntimeHostCaptureErrorV1::InvalidRange);
        }
        Ok(RuntimeHostCaptureSourceV1 {
            allocation,
            device: record.device,
            backend_allocation: record.backend_allocation,
            allocation_bytes: record.byte_len,
            byte_offset,
            byte_len,
        })
    }

    fn validate_host_capture_source_v1(
        &self,
        source: &RuntimeHostCaptureSourceV1,
        destination_len: usize,
    ) -> Result<u64, RuntimeHostCaptureErrorV1> {
        if self.terminal {
            return Err(RuntimeHostCaptureErrorV1::ContextTerminal);
        }
        if !source.belongs_to_context(self.context_generation) {
            return Err(RuntimeHostCaptureErrorV1::ForeignContext);
        }
        let current = self.prepare_host_drain_capture_v1(
            source.allocation,
            source.byte_offset,
            source.byte_len,
        )?;
        if current.backend_allocation != source.backend_allocation
            || current.device != source.device
            || current.allocation_bytes != source.allocation_bytes
        {
            return Err(RuntimeHostCaptureErrorV1::SourceUnavailable);
        }
        let length =
            u64::try_from(source.byte_len).map_err(|_| RuntimeHostCaptureErrorV1::InvalidRange)?;
        let destination_bytes =
            u64::try_from(destination_len).map_err(|_| RuntimeHostCaptureErrorV1::InvalidRange)?;
        if !r69_host_capture_range_v1(
            source.byte_offset,
            length,
            source.allocation_bytes,
            destination_bytes,
        ) {
            return Err(RuntimeHostCaptureErrorV1::InvalidRange);
        }
        if self.graph_reservation.is_some()
            || self
                .submissions
                .values()
                .any(|record| !record.quiescent || !record.status.is_terminal())
        {
            return Err(RuntimeHostCaptureErrorV1::Pending);
        }
        self.devices
            .iter()
            .find(|device| device.id == source.device)
            .map(|device| device.backend_device)
            .ok_or(RuntimeHostCaptureErrorV1::SourceUnavailable)
    }

    pub(crate) fn capture_host_drain_v1(
        &mut self,
        source: &RuntimeHostCaptureSourceV1,
        destination: &mut [u8],
        _quiescence: crate::async_engine::DrainQuiescenceV1,
    ) -> Result<(), RuntimeHostCaptureErrorV1> {
        let device = self.validate_host_capture_source_v1(source, destination.len())?;
        match self
            .backend
            .capture_coherent_host_range_v1(BackendHostCaptureV1 {
                device,
                allocation: source.backend_allocation,
                byte_offset: source.byte_offset,
                destination,
            }) {
            Ok(()) => Ok(()),
            Err(
                RuntimeBackendFailureV1::Rejected(error)
                | RuntimeBackendFailureV1::Quiescent(error),
            ) => Err(error),
            Err(RuntimeBackendFailureV1::Terminal(error)) => {
                self.terminal = true;
                Err(error)
            }
        }
    }
}
