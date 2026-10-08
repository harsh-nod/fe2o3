//! Opt-in primary-queue storage, with the original promotion return lineage.

use super::*;
use fe2o3_kfd::{
    Gfx942DeviceContentDescriptorV1, Gfx942DeviceContentRoleV1, Gfx942SdmaBufferV1,
    Gfx942SdmaDispatchDataBridgeV1,
};

pub(super) type NativeCustody =
    Custody<Gfx942SdmaBufferV1, Gfx942FixedDispatchDataV1, Gfx942SdmaDispatchDataBridgeV1>;

pub(super) enum Custody<B, D, R> {
    Empty,
    Input(B),
    Promoting,
    Output(D, R),
    Bound(R),
    Transferred,
    Disposed,
}

type PromotionResult<D, R, E, B> = Result<(D, R), (E, Option<B>)>;

trait Operations {
    type Buffer;
    type Data;
    type Bridge;
    type Error;

    fn allocate(&mut self, bytes: usize) -> Result<Self::Buffer, Self::Error>;
    fn write(&mut self, buffer: &mut Self::Buffer, bytes: &[u8]) -> Result<(), Self::Error>;
    fn promote(
        &mut self,
        buffer: Self::Buffer,
    ) -> PromotionResult<Self::Data, Self::Bridge, Self::Error, Self::Buffer>;
}

impl<B, D, R> Custody<B, D, R> {
    pub(super) fn empty() -> Self {
        Self::Empty
    }

    pub(super) fn is_disposed_or_unentered(&self) -> bool {
        matches!(self, Self::Empty | Self::Disposed)
    }

    pub(super) fn is_bound(&self) -> bool {
        matches!(self, Self::Bound(_))
    }

    pub(super) fn is_transferred(&self) -> bool {
        matches!(self, Self::Transferred)
    }

    pub(super) fn take_bridge_for_copy(&mut self) -> Option<R> {
        if !self.is_bound() {
            return None;
        }
        let Self::Bound(bridge) = core::mem::replace(self, Self::Transferred) else {
            std::process::abort()
        };
        // Transferred is deliberately not Disposed. Only the exact copy's
        // physical source release can retire this remaining shell obligation.
        Some(bridge)
    }

    fn prepare<O: Operations<Buffer = B, Data = D, Bridge = R>>(
        &mut self,
        ops: &mut O,
        bytes: &[u8],
    ) -> Result<(), O::Error> {
        if !matches!(self, Self::Empty) {
            std::process::abort();
        }
        *self = Self::Input(ops.allocate(bytes.len())?);
        let Self::Input(input) = self else {
            std::process::abort();
        };
        ops.write(input, bytes)?;
        let Self::Input(input) = core::mem::replace(self, Self::Promoting) else {
            std::process::abort();
        };
        match ops.promote(input) {
            Ok((data, bridge)) => {
                *self = Self::Output(data, bridge);
                Ok(())
            }
            Err((error, recovered)) => {
                if let Some(input) = recovered {
                    *self = Self::Input(input);
                }
                // None means the lower original queue retains terminal input.
                Err(error)
            }
        }
    }

    fn take_data_for_binding(&mut self) -> D {
        let Self::Output(data, bridge) = core::mem::replace(self, Self::Promoting) else {
            std::process::abort();
        };
        *self = Self::Bound(bridge);
        data
    }

    pub(super) fn dispose_after_data_release(&mut self) {
        match self {
            Self::Empty => (),
            Self::Bound(_) => *self = Self::Disposed,
            _ => std::process::abort(),
        }
    }
}

struct NativeOperations<'a> {
    queue: &'a mut ComputeAqlQueueSessionV1,
    content: Gfx942DeviceContentDescriptorV1,
}

impl Operations for NativeOperations<'_> {
    type Buffer = Gfx942SdmaBufferV1;
    type Data = Gfx942FixedDispatchDataV1;
    type Bridge = Gfx942SdmaDispatchDataBridgeV1;
    type Error = fe2o3_kfd::ComputeAqlQueueSessionErrorV1;

    fn allocate(&mut self, bytes: usize) -> Result<Self::Buffer, Self::Error> {
        self.queue.allocate_sdma_host_buffer(bytes)
    }

    fn write(&mut self, buffer: &mut Self::Buffer, bytes: &[u8]) -> Result<(), Self::Error> {
        self.queue.write_sdma_host_buffer(buffer, 0, bytes)
    }

    fn promote(
        &mut self,
        buffer: Self::Buffer,
    ) -> Result<(Self::Data, Self::Bridge), (Self::Error, Option<Self::Buffer>)> {
        self.queue
            .promote_sdma_host_buffer_to_fixed_dispatch_data(buffer, self.content)
            .map_err(|error| error.into_parts())
    }
}

pub(super) fn content(
    source: [u8; 32],
    bytes: &[u8],
) -> Result<Gfx942DeviceContentDescriptorV1, fe2o3_kfd::Gfx942DeviceContentDescriptorErrorV1> {
    // This checks initialization bytes only. It cannot authenticate a generated
    // output, a new consumer profile, or any completion/version authority.
    Gfx942DeviceContentDescriptorV1::from_bytes(Gfx942DeviceContentRoleV1::new(source, 0)?, bytes)
}

impl KfdRuntimeBackendV1 {
    pub(crate) fn generated_sdma_lane_ready_v1(
        &self,
    ) -> Result<bool, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        self.require_default_dispatch_capacity_v1()?;
        if !self.native_available {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "SDMA-backed generated DATA requires the native primary queue",
            ));
        }
        Ok(!self.persistent_compute_is_active_v1()
            && self.free_compute_lane_v1() == Some(0)
            && !self.any_compute_active_v1()
            && self.stream_compute_lanes.is_empty())
    }

    pub(super) fn preflight_generated_sdma_v1(
        &self,
        plan: &GeneratedShellPlanV1,
        buffers: &[crate::Gfx942KfdDispatchBufferV1],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if plan.profile != crate::generated_source::GeneratedProfileV1::Singleton
            || plan.count != 1
            || buffers.len() != 1
            || buffers[0].bytes().is_empty()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "SDMA-backed generated DATA requires one exact Singleton extent",
            ));
        }
        if !self.generated_sdma_lane_ready_v1()? {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "SDMA-backed generated primary lane unavailable",
            ));
        }
        Ok(())
    }

    pub(super) fn bind_generated_sdma_v1(
        &mut self,
        key: u64,
        programs: Vec<ValidatedKernelEnvelope<'_>>,
        bytes: &[u8],
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.ensure_sdma_queue_v1()?;
        let record = self
            .generated_shells
            .get_mut(&key)
            .expect("rooted generated shell");
        let native = record.native.as_mut().expect("native entry");
        let queue = self.queue.as_mut().expect("rooted original SDMA queue");
        native
            .sdma
            .prepare(&mut NativeOperations { queue, content }, bytes)
            .map_err(|error| {
                Self::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    error.to_string(),
                )
            })?;
        let created = native.native_lane.is_none();
        let result = if let Some(handle) = native.native_lane {
            queue
                .with_compute_lane_v1(handle, |lane| {
                    // The vector was prepaid before native entry. Taking DATA only
                    // inside the admitted lane callback preserves it on preflight refusal.
                    native.data.push(native.sdma.take_data_for_binding());
                    lane.bind_fixed_dispatch(
                        programs,
                        [record.control.take().expect("original one-shot packet")],
                        core::mem::take(&mut native.data),
                    )
                })
                .and_then(core::convert::identity)
        } else {
            queue.bind_initial_fixed_dispatch_v1(
                programs,
                [record.control.take().expect("original one-shot packet")],
                1,
                |_, _| Ok(native.sdma.take_data_for_binding()),
            )
        };
        result.map_err(|error| self.generated_native_error_v1("SDMA DATA binding", error))?;
        let handle = self
            .queue
            .as_ref()
            .expect("retained queue")
            .primary_compute_lane_v1();
        self.native_compute_lanes[0] = Some(handle);
        self.generated_shells
            .get_mut(&key)
            .expect("rooted shell")
            .native
            .as_mut()
            .expect("native entry")
            .native_lane = Some(handle);
        if created {
            self.observe_generated_queue_creation_v1(0);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
