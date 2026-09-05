//! Private move-only adapter around the persistent SDMA owner APIs.
//!
//! The native branch is a type-preserving forwarding layer. Tests use an exact
//! FIFO script with opaque owners, so facade failure paths can be exercised
//! without a KFD device or constructors for native custody types.

// Error transitions return large native owners inline. Boxing after a native
// failure would add an allocation failure exactly where custody cannot be lost.
#![allow(
    clippy::items_after_test_module,
    clippy::large_enum_variant,
    clippy::result_large_err
)]

use fe2o3_kfd::{
    ComputeAqlQueueSessionErrorV1, ComputeAqlQueueSessionV1,
    GFX942_PERSISTENT_DIRECTIONAL_SDMA_MAX_WINDOW_PACKETS_V1,
    GFX942_SAME_DEVICE_PERSISTENT_SDMA_MAX_WINDOW_PACKETS_V1, GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1,
    Gfx942DirectionalPersistentSdmaCompletedV1, Gfx942DirectionalPersistentSdmaCopyPollV1,
    Gfx942DirectionalPersistentSdmaDemotionCustodyV1,
    Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1,
    Gfx942DirectionalPersistentSdmaExecutionCustodyV1,
    Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1,
    Gfx942DirectionalPersistentSdmaPromotionCustodyV1,
    Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1,
    Gfx942DirectionalPersistentSdmaSubmissionCustodyV1,
    Gfx942DirectionalPersistentSdmaSubmissionV1, Gfx942DirectionalPersistentSdmaTerminalCustodyV1,
    Gfx942DirectionalPersistentSdmaWindowCompletedV1,
    Gfx942DirectionalPersistentSdmaWindowCopyPollV1,
    Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1,
    Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1,
    Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
    Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1,
    Gfx942DirectionalQueuePersistentAllocationV1, Gfx942DispatchBindingErrorV1,
    Gfx942PersistentComputeReadyFailureCustodyV1, Gfx942PersistentComputeReadyTerminalCustodyV1,
    Gfx942PersistentComputeReadyV1, Gfx942PersistentSdmaDirectionV1,
    Gfx942SameDevicePersistentSdmaWindowCompletedV1,
    Gfx942SameDevicePersistentSdmaWindowCopyPollV1,
    Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1,
    Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1,
    Gfx942SameDevicePersistentSdmaWindowSubmissionV1,
    Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1, Gfx942SdmaBufferV1,
};
#[cfg(test)]
use sha2::{Digest, Sha256};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DirectionalSdmaCopyRequestV1 {
    pub(super) host_offset: u64,
    pub(super) device_offset: u64,
    pub(super) copy_bytes: u32,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum DirectionalSdmaRequestPlanV1 {
    Single(DirectionalSdmaCopyRequestV1),
    Window(Box<[DirectionalSdmaCopyRequestV1]>),
}

impl DirectionalSdmaRequestPlanV1 {
    fn as_slice(&self) -> &[DirectionalSdmaCopyRequestV1] {
        match self {
            Self::Single(request) => core::slice::from_ref(request),
            Self::Window(requests) => requests,
        }
    }

    fn packet_count(&self) -> usize {
        self.as_slice().len()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SameDeviceSdmaCopyRequestV1 {
    pub(super) source_offset: u64,
    pub(super) destination_offset: u64,
    pub(super) copy_bytes: u32,
}

fn validate_same_device_window_requests_v1(
    requests: &[SameDeviceSdmaCopyRequestV1],
) -> Result<(u64, u64, u32), String> {
    if requests.is_empty()
        || requests.len() > GFX942_SAME_DEVICE_PERSISTENT_SDMA_MAX_WINDOW_PACKETS_V1
    {
        return Err("same-device SDMA window packet count is outside 1..=63".to_owned());
    }
    let first = requests[0];
    let mut next_source_offset = first.source_offset;
    let mut next_destination_offset = first.destination_offset;
    let mut total_bytes = 0_u64;
    for (index, request) in requests.iter().enumerate() {
        if request.source_offset != next_source_offset
            || request.destination_offset != next_destination_offset
        {
            return Err(
                "same-device SDMA window requests are not ordered and contiguous".to_owned(),
            );
        }
        if request.copy_bytes == 0
            || request.copy_bytes > GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1
            || (index + 1 != requests.len()
                && request.copy_bytes != GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1)
        {
            return Err("same-device SDMA window packetization is not canonical".to_owned());
        }
        let copy_bytes = u64::from(request.copy_bytes);
        next_source_offset = next_source_offset
            .checked_add(copy_bytes)
            .ok_or_else(|| "same-device SDMA source window offset overflow".to_owned())?;
        next_destination_offset = next_destination_offset
            .checked_add(copy_bytes)
            .ok_or_else(|| "same-device SDMA destination window offset overflow".to_owned())?;
        total_bytes = total_bytes
            .checked_add(copy_bytes)
            .ok_or_else(|| "same-device SDMA window length overflow".to_owned())?;
    }
    let total_bytes = u32::try_from(total_bytes)
        .map_err(|_| "same-device SDMA window length exceeds u32".to_owned())?;
    Ok((first.source_offset, first.destination_offset, total_bytes))
}

fn validate_window_requests_v1(
    requests: &[DirectionalSdmaCopyRequestV1],
) -> Result<(u64, u64, u32), String> {
    if requests.is_empty()
        || requests.len() > GFX942_PERSISTENT_DIRECTIONAL_SDMA_MAX_WINDOW_PACKETS_V1
    {
        return Err("directional SDMA window packet count is outside 1..=63".to_owned());
    }
    let first = requests[0];
    let mut next_host_offset = first.host_offset;
    let mut next_device_offset = first.device_offset;
    let mut total_bytes = 0_u64;
    for (index, request) in requests.iter().enumerate() {
        if request.host_offset != next_host_offset || request.device_offset != next_device_offset {
            return Err(
                "directional SDMA window requests are not ordered and contiguous".to_owned(),
            );
        }
        if request.copy_bytes == 0
            || request.copy_bytes > GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1
            || (index + 1 != requests.len()
                && request.copy_bytes != GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1)
        {
            return Err("directional SDMA window packetization is not canonical".to_owned());
        }
        let copy_bytes = u64::from(request.copy_bytes);
        next_host_offset = next_host_offset
            .checked_add(copy_bytes)
            .ok_or_else(|| "directional SDMA host window offset overflow".to_owned())?;
        next_device_offset = next_device_offset
            .checked_add(copy_bytes)
            .ok_or_else(|| "directional SDMA device window offset overflow".to_owned())?;
        total_bytes = total_bytes
            .checked_add(copy_bytes)
            .ok_or_else(|| "directional SDMA window length overflow".to_owned())?;
    }
    let total_bytes = u32::try_from(total_bytes)
        .map_err(|_| "directional SDMA window length exceeds u32".to_owned())?;
    Ok((first.host_offset, first.device_offset, total_bytes))
}

#[derive(Debug)]
pub(super) enum SdmaBufferOwnerV1 {
    Native(Gfx942SdmaBufferV1),
    #[cfg(test)]
    Scripted(ScriptedBufferOwnerV1),
}

#[derive(Debug)]
pub(super) enum DirectionalSdmaDeviceOwnerV1 {
    Native(Gfx942DirectionalQueuePersistentAllocationV1),
    #[cfg(test)]
    Scripted(ScriptedDeviceOwnerV1),
}

#[cfg(test)]
impl DirectionalSdmaDeviceOwnerV1 {
    pub(crate) fn scripted_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Scripted(device) => Some(&device.bytes),
            Self::Native(_) => None,
        }
    }

    pub(crate) fn scripted_bytes_mut(&mut self) -> Option<&mut [u8]> {
        match self {
            Self::Scripted(device) => Some(&mut device.bytes),
            Self::Native(_) => None,
        }
    }
}

#[derive(Debug)]
pub(super) struct DirectionalSdmaPairOwnerV1 {
    pub(super) device: DirectionalSdmaDeviceOwnerV1,
    pub(super) host: SdmaBufferOwnerV1,
}

#[derive(Debug)]
pub(super) struct SameDeviceSdmaPairOwnerV1 {
    pub(super) source: DirectionalSdmaDeviceOwnerV1,
    pub(super) destination: DirectionalSdmaDeviceOwnerV1,
}

#[derive(Debug)]
pub(super) enum DirectionalSdmaSubmissionOwnerV1 {
    NativeSingle {
        submission: Gfx942DirectionalPersistentSdmaSubmissionV1,
        host_offset: u64,
        device_offset: u64,
    },
    NativeWindow {
        submission: Gfx942DirectionalPersistentSdmaWindowSubmissionV1,
    },
    #[cfg(test)]
    Scripted(ScriptedSubmissionOwnerV1),
}

pub(super) enum DirectionalSdmaCompletedOwnerV1 {
    NativeSingle {
        completed: Gfx942DirectionalPersistentSdmaCompletedV1,
        host_offset: u64,
        device_offset: u64,
    },
    NativeWindow {
        completed: Gfx942DirectionalPersistentSdmaWindowCompletedV1,
    },
    #[cfg(test)]
    Scripted(ScriptedCompletedOwnerV1),
}

pub(super) enum PersistentComputeReadyOwnerV1 {
    Native(Gfx942PersistentComputeReadyV1),
    #[cfg(test)]
    Scripted {
        device: ScriptedDeviceOwnerV1,
        authenticated_sha256: [u8; 32],
    },
}

impl core::fmt::Debug for PersistentComputeReadyOwnerV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PersistentComputeReadyOwnerV1")
            .field("byte_len", &self.byte_len())
            .field("physical_byte_len", &self.physical_byte_len())
            .finish_non_exhaustive()
    }
}

impl PersistentComputeReadyOwnerV1 {
    pub(super) fn byte_len(&self) -> u64 {
        match self {
            Self::Native(ready) => ready.byte_len(),
            #[cfg(test)]
            Self::Scripted { device, .. } => {
                u64::try_from(device.bytes.len()).expect("scripted device length fits u64")
            }
        }
    }

    pub(super) fn physical_byte_len(&self) -> u64 {
        match self {
            Self::Native(ready) => ready.physical_byte_len(),
            #[cfg(test)]
            Self::Scripted { device, .. } => {
                u64::try_from(device.bytes.len()).expect("scripted device length fits u64")
            }
        }
    }

    pub(super) fn authenticated_sha256(&self) -> [u8; 32] {
        match self {
            Self::Native(ready) => ready.authenticated_sha256(),
            #[cfg(test)]
            Self::Scripted {
                authenticated_sha256,
                ..
            } => *authenticated_sha256,
        }
    }

    pub(super) fn normalize(self) -> DirectionalSdmaDeviceOwnerV1 {
        match self {
            Self::Native(ready) => DirectionalSdmaDeviceOwnerV1::Native(
                fe2o3_kfd::normalize_persistent_compute_ready_v1(ready),
            ),
            #[cfg(test)]
            Self::Scripted { device, .. } => DirectionalSdmaDeviceOwnerV1::Scripted(device),
        }
    }

    pub(super) const fn from_native(ready: Gfx942PersistentComputeReadyV1) -> Self {
        Self::Native(ready)
    }
}

pub(super) enum PersistentComputeReadyTransitionFailureV1 {
    Recovered {
        pair: DirectionalSdmaPairOwnerV1,
    },
    ForeignQueue {
        detail: String,
        terminal_receiver: bool,
        completed: DirectionalSdmaCompletedOwnerV1,
    },
    ProcessTeardown {
        detail: String,
        custody: Option<SdmaTerminalCustodyV1>,
    },
}

impl core::fmt::Debug for DirectionalSdmaCompletedOwnerV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DirectionalSdmaCompletedOwnerV1")
            .field("direction", &self.direction())
            .field("host_offset", &self.host_offset())
            .field("device_offset", &self.device_offset())
            .field("copy_bytes", &self.copy_bytes())
            .field("packet_count", &self.packet_count())
            .finish_non_exhaustive()
    }
}

impl DirectionalSdmaCompletedOwnerV1 {
    pub(super) fn direction(&self) -> Gfx942PersistentSdmaDirectionV1 {
        match self {
            Self::NativeSingle { completed, .. } => completed.direction(),
            Self::NativeWindow { completed } => completed.direction(),
            #[cfg(test)]
            Self::Scripted(completed) => completed.direction,
        }
    }

    pub(super) fn copy_bytes(&self) -> u32 {
        match self {
            Self::NativeSingle { completed, .. } => completed.copy_bytes(),
            Self::NativeWindow { completed } => completed.copy_bytes(),
            #[cfg(test)]
            Self::Scripted(completed) => completed.copy_bytes,
        }
    }

    pub(super) fn host_offset(&self) -> u64 {
        match self {
            Self::NativeSingle { host_offset, .. } => *host_offset,
            Self::NativeWindow { completed } => completed.host_offset(),
            #[cfg(test)]
            Self::Scripted(completed) => completed.host_offset,
        }
    }

    pub(super) fn device_offset(&self) -> u64 {
        match self {
            Self::NativeSingle { device_offset, .. } => *device_offset,
            Self::NativeWindow { completed } => completed.device_offset(),
            #[cfg(test)]
            Self::Scripted(completed) => completed.device_offset,
        }
    }

    pub(super) fn packet_count(&self) -> usize {
        match self {
            Self::NativeSingle { .. } => 1,
            Self::NativeWindow { completed } => completed.packet_count(),
            #[cfg(test)]
            Self::Scripted(completed) => completed.packet_count,
        }
    }
}

#[derive(Debug)]
pub(super) enum SameDeviceSdmaSubmissionOwnerV1 {
    Native {
        submission: Gfx942SameDevicePersistentSdmaWindowSubmissionV1,
    },
    #[cfg(test)]
    Scripted(ScriptedSameDeviceSubmissionOwnerV1),
}

pub(super) enum SameDeviceSdmaCompletedOwnerV1 {
    Native {
        completed: Gfx942SameDevicePersistentSdmaWindowCompletedV1,
    },
    #[cfg(test)]
    Scripted(ScriptedSameDeviceCompletedOwnerV1),
}

impl core::fmt::Debug for SameDeviceSdmaCompletedOwnerV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SameDeviceSdmaCompletedOwnerV1")
            .field("source_offset", &self.source_offset())
            .field("destination_offset", &self.destination_offset())
            .field("copy_bytes", &self.copy_bytes())
            .field("packet_count", &self.packet_count())
            .finish_non_exhaustive()
    }
}

impl SameDeviceSdmaCompletedOwnerV1 {
    pub(super) fn source_offset(&self) -> u64 {
        match self {
            Self::Native { completed } => completed.source_offset(),
            #[cfg(test)]
            Self::Scripted(completed) => completed.source_offset,
        }
    }

    pub(super) fn destination_offset(&self) -> u64 {
        match self {
            Self::Native { completed } => completed.destination_offset(),
            #[cfg(test)]
            Self::Scripted(completed) => completed.destination_offset,
        }
    }

    pub(super) fn copy_bytes(&self) -> u32 {
        match self {
            Self::Native { completed } => completed.copy_bytes(),
            #[cfg(test)]
            Self::Scripted(completed) => completed.copy_bytes,
        }
    }

    pub(super) fn packet_count(&self) -> usize {
        match self {
            Self::Native { completed } => completed.packet_count(),
            #[cfg(test)]
            Self::Scripted(completed) => completed.packet_count,
        }
    }
}

pub(super) enum NativeDirectionalSdmaTerminalCustodyV1 {
    Promotion(Gfx942DirectionalPersistentSdmaPromotionTerminalCustodyV1),
    Demotion(Gfx942DirectionalPersistentSdmaDemotionTerminalCustodyV1),
    SingleSubmission(Gfx942DirectionalPersistentSdmaTerminalCustodyV1),
    WindowSubmission(Gfx942DirectionalPersistentSdmaWindowTerminalCustodyV1),
    Published(DirectionalSdmaSubmissionOwnerV1),
    Retirement {
        failure: Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1,
        host: Gfx942SdmaBufferV1,
    },
    ReadyPromotion(Gfx942PersistentComputeReadyTerminalCustodyV1),
}

#[allow(dead_code)]
pub(super) enum NativeSameDeviceSdmaTerminalCustodyV1 {
    Submission(Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1),
    PublishedWindow(SameDeviceSdmaSubmissionOwnerV1),
    Completed(SameDeviceSdmaCompletedOwnerV1),
}

pub(super) enum SdmaTerminalCustodyV1 {
    Native(NativeDirectionalSdmaTerminalCustodyV1),
    NativeSameDevice(NativeSameDeviceSdmaTerminalCustodyV1),
    #[cfg(test)]
    Scripted(ScriptedTerminalCustodyV1),
}

pub(super) enum SdmaTransitionFailureV1<R> {
    Retryable {
        detail: String,
        custody: R,
    },
    ProcessTeardown {
        detail: String,
        custody: SdmaTerminalCustodyV1,
    },
}

pub(super) enum DirectionalSdmaExecutionFailureV1 {
    Retryable {
        detail: String,
        submission: DirectionalSdmaSubmissionOwnerV1,
    },
    ProcessTeardown {
        detail: String,
        custody: SdmaTerminalCustodyV1,
    },
}

pub(super) enum SameDeviceSdmaExecutionFailureV1 {
    Retryable {
        detail: String,
        submission: SameDeviceSdmaSubmissionOwnerV1,
    },
    ProcessTeardown {
        detail: String,
        custody: SdmaTerminalCustodyV1,
    },
}

pub(super) enum SdmaRecycleFailureV1 {
    Recovered {
        detail: String,
        buffer: SdmaBufferOwnerV1,
    },
    Ambiguous {
        detail: String,
    },
    #[cfg(test)]
    ProcessTeardown {
        detail: String,
        custody: SdmaTerminalCustodyV1,
    },
}

pub(super) enum DirectionalSdmaPollV1 {
    Pending(DirectionalSdmaSubmissionOwnerV1),
    Completed(DirectionalSdmaCompletedOwnerV1),
}

pub(super) enum SameDeviceSdmaPollV1 {
    Pending(SameDeviceSdmaSubmissionOwnerV1),
    Completed(SameDeviceSdmaCompletedOwnerV1),
}

pub(super) enum DirectionalSdmaOpsV1<'a> {
    Native(&'a mut ComputeAqlQueueSessionV1),
    #[cfg(test)]
    Scripted(&'a mut ScriptedSdmaDriverV1),
}

fn retire_native_directional_completed_v1(
    (device, host, frontier): fe2o3_kfd::Gfx942PersistentComputeReadyPartsV1,
) -> Result<DirectionalSdmaPairOwnerV1, SdmaTransitionFailureV1<DirectionalSdmaCompletedOwnerV1>> {
    match device.retire_settled_frontier_v1(frontier) {
        Ok(device) => Ok(DirectionalSdmaPairOwnerV1 {
            device: DirectionalSdmaDeviceOwnerV1::Native(device),
            host: SdmaBufferOwnerV1::Native(host),
        }),
        Err(failure) => Err(SdmaTransitionFailureV1::ProcessTeardown {
            detail: "frontier retirement failed".to_owned(),
            custody: SdmaTerminalCustodyV1::Native(
                NativeDirectionalSdmaTerminalCustodyV1::Retirement { failure, host },
            ),
        }),
    }
}

fn map_native_ready_promotion_v1(
    promotion: Result<
        (
            fe2o3_kfd::Gfx942PersistentComputeReadyV1,
            Gfx942SdmaBufferV1,
        ),
        fe2o3_kfd::Gfx942PersistentComputeReadyFailureV1,
    >,
) -> Result<
    (PersistentComputeReadyOwnerV1, SdmaBufferOwnerV1),
    PersistentComputeReadyTransitionFailureV1,
> {
    match promotion {
        Ok((ready, host)) => Ok((
            PersistentComputeReadyOwnerV1::Native(ready),
            SdmaBufferOwnerV1::Native(host),
        )),
        Err(failure) => {
            let (error, custody) = failure.into_parts();
            let terminal_receiver = matches!(
                error,
                ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::Poisoned
                )
            );
            let detail = error.to_string();
            let (device, host, frontier) = match custody {
                Gfx942PersistentComputeReadyFailureCustodyV1::Retryable(parts) => parts,
                Gfx942PersistentComputeReadyFailureCustodyV1::ForeignQueue(completed) => {
                    return Err(PersistentComputeReadyTransitionFailureV1::ForeignQueue {
                        detail,
                        terminal_receiver,
                        completed: DirectionalSdmaCompletedOwnerV1::NativeWindow { completed },
                    });
                }
                Gfx942PersistentComputeReadyFailureCustodyV1::ProcessTeardown(custody) => {
                    return Err(PersistentComputeReadyTransitionFailureV1::ProcessTeardown {
                        detail,
                        custody: Some(SdmaTerminalCustodyV1::Native(
                            NativeDirectionalSdmaTerminalCustodyV1::ReadyPromotion(custody),
                        )),
                    });
                }
            };
            match device.retire_settled_frontier_v1(frontier) {
                Ok(device) => Err(PersistentComputeReadyTransitionFailureV1::Recovered {
                    pair: DirectionalSdmaPairOwnerV1 {
                        device: DirectionalSdmaDeviceOwnerV1::Native(device),
                        host: SdmaBufferOwnerV1::Native(host),
                    },
                }),
                Err(failure) => Err(PersistentComputeReadyTransitionFailureV1::ProcessTeardown {
                    detail,
                    custody: Some(SdmaTerminalCustodyV1::Native(
                        NativeDirectionalSdmaTerminalCustodyV1::Retirement { failure, host },
                    )),
                }),
            }
        }
    }
}

impl<'a> DirectionalSdmaOpsV1<'a> {
    pub(super) fn allocate_host(&mut self, byte_len: usize) -> Result<SdmaBufferOwnerV1, String> {
        match self {
            Self::Native(queue) => queue
                .allocate_sdma_pooled_host_buffer(byte_len)
                .map(SdmaBufferOwnerV1::Native)
                .map_err(|error| error.to_string()),
            #[cfg(test)]
            Self::Scripted(driver) => driver.allocate_buffer(byte_len, ScriptedBufferKindV1::Host),
        }
    }

    pub(super) fn allocate_device_buffer(
        &mut self,
        byte_len: u64,
        alignment: u64,
    ) -> Result<SdmaBufferOwnerV1, String> {
        match self {
            Self::Native(queue) => queue
                .allocate_sdma_pooled_device_buffer(byte_len, alignment)
                .map(SdmaBufferOwnerV1::Native)
                .map_err(|error| error.to_string()),
            #[cfg(test)]
            Self::Scripted(driver) => {
                let len = usize::try_from(byte_len)
                    .map_err(|_| "scripted device allocation length overflow".to_owned())?;
                driver.allocate_buffer(len, ScriptedBufferKindV1::Device)
            }
        }
    }

    pub(super) fn write_host(
        &mut self,
        buffer: &mut SdmaBufferOwnerV1,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), String> {
        match (self, buffer) {
            (Self::Native(queue), SdmaBufferOwnerV1::Native(buffer)) => queue
                .write_sdma_host_buffer(buffer, offset, bytes)
                .map_err(|error| error.to_string()),
            #[cfg(test)]
            (Self::Scripted(driver), SdmaBufferOwnerV1::Scripted(buffer)) => {
                driver.write_host(buffer, offset, bytes)
            }
            #[cfg(test)]
            (_, buffer) => Err(format!(
                "directional SDMA owner/driver mismatch while writing {:?}",
                buffer
            )),
        }
    }

    pub(super) fn write_full_host_authenticated(
        &mut self,
        buffer: &mut SdmaBufferOwnerV1,
        bytes: &[u8],
    ) -> Result<Option<[u8; 32]>, String> {
        match (self, buffer) {
            (Self::Native(queue), SdmaBufferOwnerV1::Native(buffer)) => queue
                .write_full_sdma_host_buffer_authenticated_v1(buffer, bytes)
                .map_err(|error| error.to_string()),
            #[cfg(test)]
            (Self::Scripted(driver), SdmaBufferOwnerV1::Scripted(buffer)) => driver
                .write_full_host_authenticated(buffer, bytes)
                .map(Some),
            #[cfg(test)]
            (_, buffer) => Err(format!(
                "directional SDMA owner/driver mismatch while writing authenticated content to {:?}",
                buffer
            )),
        }
    }

    pub(super) fn read_host(
        &mut self,
        buffer: &SdmaBufferOwnerV1,
        offset: u64,
        byte_len: u64,
    ) -> Result<Box<[u8]>, String> {
        match (self, buffer) {
            (Self::Native(queue), SdmaBufferOwnerV1::Native(buffer)) => queue
                .read_sdma_host_buffer(buffer, offset, byte_len)
                .map_err(|error| error.to_string()),
            #[cfg(test)]
            (Self::Scripted(driver), SdmaBufferOwnerV1::Scripted(buffer)) => {
                driver.read_host(buffer, offset, byte_len)
            }
            #[cfg(test)]
            (_, buffer) => Err(format!(
                "directional SDMA owner/driver mismatch while reading {:?}",
                buffer
            )),
        }
    }

    pub(super) fn promote(
        &mut self,
        buffer: SdmaBufferOwnerV1,
    ) -> Result<DirectionalSdmaDeviceOwnerV1, SdmaTransitionFailureV1<SdmaBufferOwnerV1>> {
        match (self, buffer) {
            (Self::Native(queue), SdmaBufferOwnerV1::Native(buffer)) => queue
                .promote_sdma_device_buffer_to_directional_persistent_allocation_v1(buffer)
                .map(DirectionalSdmaDeviceOwnerV1::Native)
                .map_err(|failure| {
                    let (error, custody) = failure.into_parts();
                    match custody {
                        Gfx942DirectionalPersistentSdmaPromotionCustodyV1::Retryable(buffer) => {
                            SdmaTransitionFailureV1::Retryable {
                                detail: error.to_string(),
                                custody: SdmaBufferOwnerV1::Native(buffer),
                            }
                        }
                        Gfx942DirectionalPersistentSdmaPromotionCustodyV1::ProcessTeardown(
                            custody,
                        ) => SdmaTransitionFailureV1::ProcessTeardown {
                            detail: error.to_string(),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::Promotion(custody),
                            ),
                        },
                    }
                }),
            #[cfg(test)]
            (Self::Scripted(driver), SdmaBufferOwnerV1::Scripted(buffer)) => driver.promote(buffer),
            #[cfg(test)]
            (_, buffer) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during promotion".to_owned(),
                custody: scripted_mismatch_buffer(buffer, "promotion"),
            }),
        }
    }

    pub(super) fn demote(
        &mut self,
        device: DirectionalSdmaDeviceOwnerV1,
    ) -> Result<SdmaBufferOwnerV1, SdmaTransitionFailureV1<DirectionalSdmaDeviceOwnerV1>> {
        match (self, device) {
            (Self::Native(queue), DirectionalSdmaDeviceOwnerV1::Native(device)) => queue
                .demote_directional_persistent_allocation_to_sdma_device_buffer_v1(device)
                .map(SdmaBufferOwnerV1::Native)
                .map_err(|failure| {
                    let (error, custody) = failure.into_parts();
                    match custody {
                        Gfx942DirectionalPersistentSdmaDemotionCustodyV1::Retryable(device) => {
                            SdmaTransitionFailureV1::Retryable {
                                detail: error.to_string(),
                                custody: DirectionalSdmaDeviceOwnerV1::Native(device),
                            }
                        }
                        Gfx942DirectionalPersistentSdmaDemotionCustodyV1::ProcessTeardown(
                            custody,
                        ) => SdmaTransitionFailureV1::ProcessTeardown {
                            detail: error.to_string(),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::Demotion(custody),
                            ),
                        },
                    }
                }),
            #[cfg(test)]
            (Self::Scripted(driver), DirectionalSdmaDeviceOwnerV1::Scripted(device)) => {
                driver.demote(device)
            }
            #[cfg(test)]
            (_, device) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during demotion".to_owned(),
                custody: scripted_mismatch_device(device, "demotion"),
            }),
        }
    }

    pub(super) fn submit(
        &mut self,
        pair: DirectionalSdmaPairOwnerV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        requests: DirectionalSdmaRequestPlanV1,
    ) -> Result<DirectionalSdmaSubmissionOwnerV1, SdmaTransitionFailureV1<DirectionalSdmaPairOwnerV1>>
    {
        let request_slice = requests.as_slice();
        let (host_offset, device_offset, copy_bytes) =
            match validate_window_requests_v1(request_slice) {
                Ok(window) => window,
                Err(detail) => {
                    return Err(SdmaTransitionFailureV1::Retryable {
                        detail,
                        custody: pair,
                    });
                }
            };
        if matches!(&requests, DirectionalSdmaRequestPlanV1::Window(requests) if requests.len() == 1)
        {
            return Err(SdmaTransitionFailureV1::Retryable {
                detail: "one directional SDMA packet must use single-request custody".to_owned(),
                custody: pair,
            });
        }
        let packet_count = requests.packet_count();
        match (self, pair.device, pair.host) {
            (
                Self::Native(queue),
                DirectionalSdmaDeviceOwnerV1::Native(device),
                SdmaBufferOwnerV1::Native(host),
            ) => {
                match requests {
                    DirectionalSdmaRequestPlanV1::Single(_) => {
                        match queue.submit_directional_persistent_sdma_copy_v1(
                        device,
                        direction,
                        host,
                        host_offset,
                        device_offset,
                        copy_bytes,
                    ) {
                        Ok(submission)
                            if submission.direction() == direction
                                && submission.copy_bytes() == copy_bytes =>
                        {
                            Ok(DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                submission,
                                host_offset,
                                device_offset,
                            })
                        }
                        Ok(submission) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                            detail: "directional SDMA published-single metadata changed unexpectedly"
                                .to_owned(),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::Published(
                                    DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                        submission,
                                        host_offset,
                                        device_offset,
                                    },
                                ),
                            ),
                        }),
                        Err(failure) => {
                            let (error, custody) = failure.into_parts();
                            Err(match custody {
                                Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::Retryable {
                                    allocation,
                                    host,
                                } => SdmaTransitionFailureV1::Retryable {
                                    detail: error.to_string(),
                                    custody: DirectionalSdmaPairOwnerV1 {
                                        device: DirectionalSdmaDeviceOwnerV1::Native(allocation),
                                        host: SdmaBufferOwnerV1::Native(host),
                                    },
                                },
                                Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::ProcessTeardown(
                                    custody,
                                ) => SdmaTransitionFailureV1::ProcessTeardown {
                                    detail: error.to_string(),
                                    custody: SdmaTerminalCustodyV1::Native(
                                        NativeDirectionalSdmaTerminalCustodyV1::SingleSubmission(
                                            custody,
                                        ),
                                    ),
                                },
                            })
                        }
                    }
                    }
                    DirectionalSdmaRequestPlanV1::Window(_) => {
                        match queue.submit_directional_persistent_sdma_window_v1(
                        device,
                        direction,
                        host,
                        host_offset,
                        device_offset,
                        copy_bytes,
                    ) {
                        Ok(submission)
                            if submission.direction() == direction
                                && submission.host_offset() == host_offset
                                && submission.device_offset() == device_offset
                                && submission.copy_bytes() == copy_bytes
                                && submission.packet_count() == packet_count =>
                        {
                            Ok(DirectionalSdmaSubmissionOwnerV1::NativeWindow { submission })
                        }
                        Ok(submission) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                            detail: "directional SDMA published-window metadata changed unexpectedly"
                                .to_owned(),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::Published(
                                    DirectionalSdmaSubmissionOwnerV1::NativeWindow { submission },
                                ),
                            ),
                        }),
                        Err(failure) => {
                            let (error, custody) = failure.into_parts();
                            Err(match custody {
                                Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::Retryable {
                                    allocation,
                                    host,
                                } => SdmaTransitionFailureV1::Retryable {
                                    detail: error.to_string(),
                                    custody: DirectionalSdmaPairOwnerV1 {
                                        device: DirectionalSdmaDeviceOwnerV1::Native(allocation),
                                        host: SdmaBufferOwnerV1::Native(host),
                                    },
                                },
                                Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(
                                    custody,
                                ) => SdmaTransitionFailureV1::ProcessTeardown {
                                    detail: error.to_string(),
                                    custody: SdmaTerminalCustodyV1::Native(
                                        NativeDirectionalSdmaTerminalCustodyV1::WindowSubmission(
                                            custody,
                                        ),
                                    ),
                                },
                            })
                        }
                    }
                    }
                }
            }
            #[cfg(test)]
            (
                Self::Scripted(driver),
                DirectionalSdmaDeviceOwnerV1::Scripted(device),
                SdmaBufferOwnerV1::Scripted(host),
            ) => driver.submit(device, host, direction, requests),
            #[cfg(test)]
            (_, device, host) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during publication".to_owned(),
                custody: scripted_mismatch_pair(device, host, "submission"),
            }),
        }
    }

    pub(super) fn poll(
        &mut self,
        submission: DirectionalSdmaSubmissionOwnerV1,
    ) -> Result<DirectionalSdmaPollV1, DirectionalSdmaExecutionFailureV1> {
        match (self, submission) {
            (
                Self::Native(queue),
                DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                    submission,
                    host_offset,
                    device_offset,
                },
            ) => {
                let expected_direction = submission.direction();
                let expected_copy_bytes = submission.copy_bytes();
                match queue.poll_directional_persistent_sdma_copy_v1(submission) {
                    Ok(Gfx942DirectionalPersistentSdmaCopyPollV1::Pending(submission))
                        if submission.direction() == expected_direction
                            && submission.copy_bytes() == expected_copy_bytes =>
                    {
                        Ok(DirectionalSdmaPollV1::Pending(
                            DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                submission,
                                host_offset,
                                device_offset,
                            },
                        ))
                    }
                    Ok(Gfx942DirectionalPersistentSdmaCopyPollV1::Pending(submission)) => {
                        Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                            detail: "directional SDMA pending-single metadata changed unexpectedly"
                                .to_owned(),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::Published(
                                    DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                        submission,
                                        host_offset,
                                        device_offset,
                                    },
                                ),
                            ),
                        })
                    }
                    Ok(Gfx942DirectionalPersistentSdmaCopyPollV1::Completed(completed)) => {
                        Ok(DirectionalSdmaPollV1::Completed(
                            DirectionalSdmaCompletedOwnerV1::NativeSingle {
                                completed,
                                host_offset,
                                device_offset,
                            },
                        ))
                    }
                    Err(failure) => {
                        let (error, custody) = failure.into_parts();
                        Err(match custody {
                            Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(submission)
                                if submission.direction() == expected_direction
                                    && submission.copy_bytes() == expected_copy_bytes =>
                            {
                                DirectionalSdmaExecutionFailureV1::Retryable {
                                    detail: error.to_string(),
                                    submission: DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                        submission,
                                        host_offset,
                                        device_offset,
                                    },
                                }
                            }
                            Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(
                                submission,
                            ) => DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                                detail:
                                    "directional SDMA retryable-single metadata changed unexpectedly"
                                        .to_owned(),
                                custody: SdmaTerminalCustodyV1::Native(
                                    NativeDirectionalSdmaTerminalCustodyV1::Published(
                                        DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                            submission,
                                            host_offset,
                                            device_offset,
                                        },
                                    ),
                                ),
                            },
                            Gfx942DirectionalPersistentSdmaExecutionCustodyV1::ProcessTeardown(
                                custody,
                            ) => DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                                detail: error.to_string(),
                                custody: SdmaTerminalCustodyV1::Native(
                                    NativeDirectionalSdmaTerminalCustodyV1::SingleSubmission(
                                        custody,
                                    ),
                                ),
                            },
                        })
                    }
                }
            }
            (
                Self::Native(queue),
                DirectionalSdmaSubmissionOwnerV1::NativeWindow { submission },
            ) => {
                let expected_direction = submission.direction();
                let expected_host_offset = submission.host_offset();
                let expected_device_offset = submission.device_offset();
                let expected_copy_bytes = submission.copy_bytes();
                let expected_packet_count = submission.packet_count();
                match queue.poll_directional_persistent_sdma_window_v1(submission) {
                    Ok(poll) => Ok(match poll {
                        Gfx942DirectionalPersistentSdmaWindowCopyPollV1::Pending(submission)
                            if submission.direction() == expected_direction
                                && submission.host_offset() == expected_host_offset
                                && submission.device_offset() == expected_device_offset
                                && submission.copy_bytes() == expected_copy_bytes
                                && submission.packet_count() == expected_packet_count =>
                        {
                            DirectionalSdmaPollV1::Pending(
                                DirectionalSdmaSubmissionOwnerV1::NativeWindow { submission },
                            )
                        }
                        Gfx942DirectionalPersistentSdmaWindowCopyPollV1::Pending(submission) => {
                            return Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                                detail:
                                    "directional SDMA pending-window metadata changed unexpectedly"
                                        .to_owned(),
                                custody: SdmaTerminalCustodyV1::Native(
                                    NativeDirectionalSdmaTerminalCustodyV1::Published(
                                        DirectionalSdmaSubmissionOwnerV1::NativeWindow {
                                            submission,
                                        },
                                    ),
                                ),
                            });
                        }
                        Gfx942DirectionalPersistentSdmaWindowCopyPollV1::Completed(completed) => {
                            DirectionalSdmaPollV1::Completed(
                                DirectionalSdmaCompletedOwnerV1::NativeWindow { completed },
                            )
                        }
                    }),
                    Err(failure) => {
                        let (error, custody) = failure.into_parts();
                        Err(match custody {
                        Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::Pending(
                            submission,
                        ) if submission.direction() == expected_direction
                            && submission.host_offset() == expected_host_offset
                            && submission.device_offset() == expected_device_offset
                            && submission.copy_bytes() == expected_copy_bytes
                            && submission.packet_count() == expected_packet_count => {
                            DirectionalSdmaExecutionFailureV1::Retryable {
                                detail: error.to_string(),
                                submission: DirectionalSdmaSubmissionOwnerV1::NativeWindow { submission },
                            }
                        }
                        Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::Pending(
                            submission,
                        ) => DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                            detail: "directional SDMA retryable-window metadata changed unexpectedly"
                                .to_owned(),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::Published(
                                    DirectionalSdmaSubmissionOwnerV1::NativeWindow { submission },
                                ),
                            ),
                        },
                        Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::ProcessTeardown(
                            custody,
                        ) => DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                            detail: error.to_string(),
                            custody: SdmaTerminalCustodyV1::Native(
                                NativeDirectionalSdmaTerminalCustodyV1::WindowSubmission(custody),
                            ),
                        },
                    })
                    }
                }
            }
            #[cfg(test)]
            (Self::Scripted(driver), DirectionalSdmaSubmissionOwnerV1::Scripted(submission)) => {
                driver.poll(submission)
            }
            #[cfg(test)]
            (_, submission) => Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during poll".to_owned(),
                custody: scripted_mismatch_submission(submission, "poll"),
            }),
        }
    }

    pub(super) fn wait(
        &mut self,
        submission: DirectionalSdmaSubmissionOwnerV1,
        timeout: Duration,
    ) -> Result<DirectionalSdmaCompletedOwnerV1, DirectionalSdmaExecutionFailureV1> {
        match (self, submission) {
            (
                Self::Native(queue),
                DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                    submission,
                    host_offset,
                    device_offset,
                },
            ) => {
                let expected_direction = submission.direction();
                let expected_copy_bytes = submission.copy_bytes();
                match queue.wait_directional_persistent_sdma_copy_for_v1(submission, timeout) {
                    Ok(completed) => Ok(DirectionalSdmaCompletedOwnerV1::NativeSingle {
                        completed,
                        host_offset,
                        device_offset,
                    }),
                    Err(failure) => {
                        let (error, custody) = failure.into_parts();
                        Err(match custody {
                            Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(submission)
                                if submission.direction() == expected_direction
                                    && submission.copy_bytes() == expected_copy_bytes =>
                            {
                                DirectionalSdmaExecutionFailureV1::Retryable {
                                    detail: error.to_string(),
                                    submission: DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                        submission,
                                        host_offset,
                                        device_offset,
                                    },
                                }
                            }
                            Gfx942DirectionalPersistentSdmaExecutionCustodyV1::Pending(
                                submission,
                            ) => DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                                detail:
                                    "directional SDMA retryable-single metadata changed unexpectedly"
                                        .to_owned(),
                                custody: SdmaTerminalCustodyV1::Native(
                                    NativeDirectionalSdmaTerminalCustodyV1::Published(
                                        DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                                            submission,
                                            host_offset,
                                            device_offset,
                                        },
                                    ),
                                ),
                            },
                            Gfx942DirectionalPersistentSdmaExecutionCustodyV1::ProcessTeardown(
                                custody,
                            ) => DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                                detail: error.to_string(),
                                custody: SdmaTerminalCustodyV1::Native(
                                    NativeDirectionalSdmaTerminalCustodyV1::SingleSubmission(
                                        custody,
                                    ),
                                ),
                            },
                        })
                    }
                }
            }
            (
                Self::Native(queue),
                DirectionalSdmaSubmissionOwnerV1::NativeWindow { submission },
            ) => {
                let expected_direction = submission.direction();
                let expected_host_offset = submission.host_offset();
                let expected_device_offset = submission.device_offset();
                let expected_copy_bytes = submission.copy_bytes();
                let expected_packet_count = submission.packet_count();
                match queue.wait_directional_persistent_sdma_window_for_v1(submission, timeout) {
                    Ok(completed) => {
                        Ok(DirectionalSdmaCompletedOwnerV1::NativeWindow { completed })
                    }
                    Err(failure) => {
                        let (error, custody) = failure.into_parts();
                        Err(match custody {
                            Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::Pending(
                                submission,
                            ) if submission.direction() == expected_direction
                                && submission.host_offset() == expected_host_offset
                                && submission.device_offset() == expected_device_offset
                                && submission.copy_bytes() == expected_copy_bytes
                                && submission.packet_count() == expected_packet_count => {
                                DirectionalSdmaExecutionFailureV1::Retryable {
                                    detail: error.to_string(),
                                    submission: DirectionalSdmaSubmissionOwnerV1::NativeWindow {
                                        submission,
                                    },
                                }
                            }
                            Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::Pending(
                                submission,
                            ) => DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                                detail:
                                    "directional SDMA retryable-window metadata changed unexpectedly"
                                        .to_owned(),
                                custody: SdmaTerminalCustodyV1::Native(
                                    NativeDirectionalSdmaTerminalCustodyV1::Published(
                                        DirectionalSdmaSubmissionOwnerV1::NativeWindow {
                                            submission,
                                        },
                                    ),
                                ),
                            },
                            Gfx942DirectionalPersistentSdmaWindowExecutionCustodyV1::ProcessTeardown(
                                custody,
                            ) => DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                                detail: error.to_string(),
                                custody: SdmaTerminalCustodyV1::Native(
                                    NativeDirectionalSdmaTerminalCustodyV1::WindowSubmission(custody),
                                ),
                            },
                        })
                    }
                }
            }
            #[cfg(test)]
            (Self::Scripted(driver), DirectionalSdmaSubmissionOwnerV1::Scripted(submission)) => {
                driver.wait(submission)
            }
            #[cfg(test)]
            (_, submission) => Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during wait".to_owned(),
                custody: scripted_mismatch_submission(submission, "wait"),
            }),
        }
    }

    pub(super) fn retire(
        &mut self,
        completed: DirectionalSdmaCompletedOwnerV1,
    ) -> Result<DirectionalSdmaPairOwnerV1, SdmaTransitionFailureV1<DirectionalSdmaCompletedOwnerV1>>
    {
        match (self, completed) {
            (Self::Native(_), DirectionalSdmaCompletedOwnerV1::NativeSingle { completed, .. }) => {
                retire_native_directional_completed_v1(completed.into_parts())
            }
            (Self::Native(_), DirectionalSdmaCompletedOwnerV1::NativeWindow { completed }) => {
                retire_native_directional_completed_v1(completed.into_parts())
            }
            #[cfg(test)]
            (Self::Scripted(driver), DirectionalSdmaCompletedOwnerV1::Scripted(completed)) => {
                driver.retire(completed)
            }
            #[cfg(test)]
            (_, completed) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during retirement".to_owned(),
                custody: scripted_mismatch_completed(completed, "retirement"),
            }),
        }
    }

    pub(super) fn promote_full_h2d_to_compute_ready(
        &mut self,
        completed: DirectionalSdmaCompletedOwnerV1,
        content: fe2o3_kfd::Gfx942DeviceContentDescriptorV1,
    ) -> Result<
        (PersistentComputeReadyOwnerV1, SdmaBufferOwnerV1),
        PersistentComputeReadyTransitionFailureV1,
    > {
        match (self, completed) {
            (
                Self::Native(queue),
                DirectionalSdmaCompletedOwnerV1::NativeSingle { completed, .. },
            ) => map_native_ready_promotion_v1(
                queue.promote_full_single_h2d_to_persistent_compute_ready_v1(completed, content),
            ),
            (Self::Native(queue), DirectionalSdmaCompletedOwnerV1::NativeWindow { completed }) => {
                map_native_ready_promotion_v1(
                    queue.promote_full_h2d_to_persistent_compute_ready_v1(completed, content),
                )
            }
            #[cfg(test)]
            (Self::Scripted(driver), DirectionalSdmaCompletedOwnerV1::Scripted(completed)) => {
                match driver.promote_full_h2d_to_compute_ready(completed, content) {
                    Ok((device, host)) => Ok((
                        PersistentComputeReadyOwnerV1::Scripted {
                            device,
                            authenticated_sha256: content.sha256(),
                        },
                        SdmaBufferOwnerV1::Scripted(host),
                    )),
                    Err(scripted::ScriptedPersistentComputeReadyFailureV1::Recovered(pair)) => {
                        Err(PersistentComputeReadyTransitionFailureV1::Recovered { pair })
                    }
                    Err(scripted::ScriptedPersistentComputeReadyFailureV1::ForeignQueue {
                        completed,
                        terminal_receiver,
                    }) => Err(PersistentComputeReadyTransitionFailureV1::ForeignQueue {
                        detail: "scripted persistent-compute ready promotion foreign queue"
                            .to_owned(),
                        terminal_receiver,
                        completed: DirectionalSdmaCompletedOwnerV1::Scripted(completed),
                    }),
                    Err(scripted::ScriptedPersistentComputeReadyFailureV1::ProcessTeardown(
                        completed,
                    )) => Err(PersistentComputeReadyTransitionFailureV1::ProcessTeardown {
                        detail: "scripted persistent-compute ready promotion teardown".to_owned(),
                        custody: Some(SdmaTerminalCustodyV1::Scripted(
                            ScriptedTerminalCustodyV1::Completed(
                                DirectionalSdmaCompletedOwnerV1::Scripted(completed),
                            ),
                        )),
                    }),
                }
            }
            #[cfg(test)]
            (_, completed) => Err(PersistentComputeReadyTransitionFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during H2D promotion".to_owned(),
                custody: Some(scripted_mismatch_completed(completed, "H2D promotion")),
            }),
        }
    }

    pub(super) fn submit_same_device(
        &mut self,
        pair: SameDeviceSdmaPairOwnerV1,
        requests: Box<[SameDeviceSdmaCopyRequestV1]>,
    ) -> Result<SameDeviceSdmaSubmissionOwnerV1, SdmaTransitionFailureV1<SameDeviceSdmaPairOwnerV1>>
    {
        let (source_offset, destination_offset, copy_bytes) =
            match validate_same_device_window_requests_v1(&requests) {
                Ok(window) => window,
                Err(detail) => {
                    return Err(SdmaTransitionFailureV1::Retryable {
                        detail,
                        custody: pair,
                    });
                }
            };
        match (self, pair.source, pair.destination) {
            (
                Self::Native(queue),
                DirectionalSdmaDeviceOwnerV1::Native(source),
                DirectionalSdmaDeviceOwnerV1::Native(destination),
            ) => match queue.submit_same_device_persistent_sdma_window_v1(
                source,
                source_offset,
                destination,
                destination_offset,
                copy_bytes,
            ) {
                Ok(submission)
                    if submission.source_offset() == source_offset
                        && submission.destination_offset() == destination_offset
                        && submission.copy_bytes() == copy_bytes
                        && submission.packet_count() == requests.len() =>
                {
                    Ok(SameDeviceSdmaSubmissionOwnerV1::Native { submission })
                }
                Ok(submission) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                    detail: "same-device SDMA published-window metadata changed unexpectedly"
                        .to_owned(),
                    custody: SdmaTerminalCustodyV1::NativeSameDevice(
                        NativeSameDeviceSdmaTerminalCustodyV1::PublishedWindow(
                            SameDeviceSdmaSubmissionOwnerV1::Native { submission },
                        ),
                    ),
                }),
                Err(failure) => {
                    let (error, custody) = failure.into_parts();
                    Err(match custody {
                        Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::Retryable {
                            source,
                            destination,
                        } => SdmaTransitionFailureV1::Retryable {
                            detail: error.to_string(),
                            custody: SameDeviceSdmaPairOwnerV1 {
                                source: DirectionalSdmaDeviceOwnerV1::Native(source),
                                destination: DirectionalSdmaDeviceOwnerV1::Native(destination),
                            },
                        },
                        Gfx942SameDevicePersistentSdmaWindowSubmissionCustodyV1::ProcessTeardown(
                            custody,
                        ) => SdmaTransitionFailureV1::ProcessTeardown {
                            detail: error.to_string(),
                            custody: SdmaTerminalCustodyV1::NativeSameDevice(
                                NativeSameDeviceSdmaTerminalCustodyV1::Submission(custody),
                            ),
                        },
                    })
                }
            },
            #[cfg(test)]
            (
                Self::Scripted(driver),
                DirectionalSdmaDeviceOwnerV1::Scripted(source),
                DirectionalSdmaDeviceOwnerV1::Scripted(destination),
            ) => driver.submit_same_device(
                SameDeviceSdmaPairOwnerV1 {
                    source: DirectionalSdmaDeviceOwnerV1::Scripted(source),
                    destination: DirectionalSdmaDeviceOwnerV1::Scripted(destination),
                },
                requests,
            ),
            #[cfg(test)]
            (_, source, destination) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: "same-device SDMA owner/driver mismatch during publication".to_owned(),
                custody: scripted_mismatch_same_device_pair(
                    SameDeviceSdmaPairOwnerV1 {
                        source,
                        destination,
                    },
                    "submission",
                ),
            }),
        }
    }

    pub(super) fn poll_same_device(
        &mut self,
        submission: SameDeviceSdmaSubmissionOwnerV1,
    ) -> Result<SameDeviceSdmaPollV1, SameDeviceSdmaExecutionFailureV1> {
        match (self, submission) {
            (Self::Native(queue), SameDeviceSdmaSubmissionOwnerV1::Native { submission }) => {
                let expected_source_request = submission.source_request();
                let expected_destination_request = submission.destination_request();
                let expected_descriptor = submission.descriptor();
                match queue.poll_same_device_persistent_sdma_window_v1(submission) {
                    Ok(poll) => Ok(match poll {
                        Gfx942SameDevicePersistentSdmaWindowCopyPollV1::Pending(submission)
                            if submission.source_request() == expected_source_request
                                && submission.destination_request()
                                    == expected_destination_request
                                && submission.descriptor() == expected_descriptor =>
                        {
                            SameDeviceSdmaPollV1::Pending(SameDeviceSdmaSubmissionOwnerV1::Native {
                                submission,
                            })
                        }
                        Gfx942SameDevicePersistentSdmaWindowCopyPollV1::Pending(submission) => {
                            return Err(SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
                                detail:
                                    "same-device SDMA pending-window identity changed unexpectedly"
                                        .to_owned(),
                                custody: SdmaTerminalCustodyV1::NativeSameDevice(
                                    NativeSameDeviceSdmaTerminalCustodyV1::PublishedWindow(
                                        SameDeviceSdmaSubmissionOwnerV1::Native { submission },
                                    ),
                                ),
                            });
                        }
                        Gfx942SameDevicePersistentSdmaWindowCopyPollV1::Completed(completed) => {
                            SameDeviceSdmaPollV1::Completed(
                                SameDeviceSdmaCompletedOwnerV1::Native { completed },
                            )
                        }
                    }),
                    Err(failure) => {
                        let (error, custody) = failure.into_parts();
                        Err(match custody {
                            Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::Pending(
                                submission,
                            ) if submission.source_request() == expected_source_request
                                && submission.destination_request()
                                    == expected_destination_request
                                && submission.descriptor() == expected_descriptor =>
                            {
                                SameDeviceSdmaExecutionFailureV1::Retryable {
                                    detail: error.to_string(),
                                    submission: SameDeviceSdmaSubmissionOwnerV1::Native {
                                        submission,
                                    },
                                }
                            }
                            Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::Pending(
                                submission,
                            ) => SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
                                detail:
                                    "same-device SDMA retryable-window identity changed unexpectedly"
                                        .to_owned(),
                                custody: SdmaTerminalCustodyV1::NativeSameDevice(
                                    NativeSameDeviceSdmaTerminalCustodyV1::PublishedWindow(
                                        SameDeviceSdmaSubmissionOwnerV1::Native { submission },
                                    ),
                                ),
                            },
                            Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::ProcessTeardown(
                                custody,
                            ) => SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
                                detail: error.to_string(),
                                custody: SdmaTerminalCustodyV1::NativeSameDevice(
                                    NativeSameDeviceSdmaTerminalCustodyV1::Submission(custody),
                                ),
                            },
                        })
                    }
                }
            }
            #[cfg(test)]
            (Self::Scripted(driver), SameDeviceSdmaSubmissionOwnerV1::Scripted(submission)) => {
                driver.poll_same_device(submission)
            }
            #[cfg(test)]
            (_, submission) => Err(SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
                detail: "same-device SDMA owner/driver mismatch during poll".to_owned(),
                custody: scripted_mismatch_same_device_submission(submission, "poll"),
            }),
        }
    }

    #[allow(dead_code)]
    pub(super) fn wait_same_device(
        &mut self,
        submission: SameDeviceSdmaSubmissionOwnerV1,
        timeout: Duration,
    ) -> Result<SameDeviceSdmaCompletedOwnerV1, SameDeviceSdmaExecutionFailureV1> {
        match (self, submission) {
            (Self::Native(queue), SameDeviceSdmaSubmissionOwnerV1::Native { submission }) => {
                let expected_source_request = submission.source_request();
                let expected_destination_request = submission.destination_request();
                let expected_descriptor = submission.descriptor();
                match queue.wait_same_device_persistent_sdma_window_for_v1(submission, timeout) {
                    Ok(completed) => Ok(SameDeviceSdmaCompletedOwnerV1::Native { completed }),
                    Err(failure) => {
                        let (error, custody) = failure.into_parts();
                        Err(match custody {
                            Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::Pending(
                                submission,
                            ) if submission.source_request() == expected_source_request
                                && submission.destination_request()
                                    == expected_destination_request
                                && submission.descriptor() == expected_descriptor =>
                            {
                                SameDeviceSdmaExecutionFailureV1::Retryable {
                                    detail: error.to_string(),
                                    submission: SameDeviceSdmaSubmissionOwnerV1::Native {
                                        submission,
                                    },
                                }
                            }
                            Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::Pending(
                                submission,
                            ) => SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
                                detail:
                                    "same-device SDMA retryable-window identity changed unexpectedly"
                                        .to_owned(),
                                custody: SdmaTerminalCustodyV1::NativeSameDevice(
                                    NativeSameDeviceSdmaTerminalCustodyV1::PublishedWindow(
                                        SameDeviceSdmaSubmissionOwnerV1::Native { submission },
                                    ),
                                ),
                            },
                            Gfx942SameDevicePersistentSdmaWindowExecutionCustodyV1::ProcessTeardown(
                                custody,
                            ) => SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
                                detail: error.to_string(),
                                custody: SdmaTerminalCustodyV1::NativeSameDevice(
                                    NativeSameDeviceSdmaTerminalCustodyV1::Submission(custody),
                                ),
                            },
                        })
                    }
                }
            }
            #[cfg(test)]
            (Self::Scripted(driver), SameDeviceSdmaSubmissionOwnerV1::Scripted(submission)) => {
                driver.wait_same_device(submission)
            }
            #[cfg(test)]
            (_, submission) => Err(SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
                detail: "same-device SDMA owner/driver mismatch during wait".to_owned(),
                custody: scripted_mismatch_same_device_submission(submission, "wait"),
            }),
        }
    }

    pub(super) fn retire_same_device(
        &mut self,
        completed: SameDeviceSdmaCompletedOwnerV1,
    ) -> Result<SameDeviceSdmaPairOwnerV1, SdmaTransitionFailureV1<SameDeviceSdmaCompletedOwnerV1>>
    {
        match (self, completed) {
            (Self::Native(_), SameDeviceSdmaCompletedOwnerV1::Native { completed }) => {
                match completed.retire_settled_frontiers_v1() {
                    Ok(pair) => {
                        let (source, destination) = pair.into_parts();
                        Ok(SameDeviceSdmaPairOwnerV1 {
                            source: DirectionalSdmaDeviceOwnerV1::Native(source),
                            destination: DirectionalSdmaDeviceOwnerV1::Native(destination),
                        })
                    }
                    Err(failure) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                        detail: "paired same-device frontier retirement failed".to_owned(),
                        custody: SdmaTerminalCustodyV1::NativeSameDevice(
                            NativeSameDeviceSdmaTerminalCustodyV1::Completed(
                                SameDeviceSdmaCompletedOwnerV1::Native {
                                    completed: failure.into_completed(),
                                },
                            ),
                        ),
                    }),
                }
            }
            #[cfg(test)]
            (Self::Scripted(driver), SameDeviceSdmaCompletedOwnerV1::Scripted(completed)) => {
                driver.retire_same_device(completed)
            }
            #[cfg(test)]
            (_, completed) => Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: "same-device SDMA owner/driver mismatch during retirement".to_owned(),
                custody: scripted_mismatch_same_device_completed(completed, "retirement"),
            }),
        }
    }

    pub(super) fn recycle(
        &mut self,
        buffer: SdmaBufferOwnerV1,
    ) -> Result<(), SdmaRecycleFailureV1> {
        match (self, buffer) {
            (Self::Native(queue), SdmaBufferOwnerV1::Native(buffer)) => {
                queue.recycle_sdma_buffer(buffer).map_err(|failure| {
                    let (error, recovered) = failure.into_parts();
                    match recovered {
                        Some(buffer) => SdmaRecycleFailureV1::Recovered {
                            detail: error.to_string(),
                            buffer: SdmaBufferOwnerV1::Native(buffer),
                        },
                        None => SdmaRecycleFailureV1::Ambiguous {
                            detail: error.to_string(),
                        },
                    }
                })
            }
            #[cfg(test)]
            (Self::Scripted(driver), SdmaBufferOwnerV1::Scripted(buffer)) => driver.recycle(buffer),
            #[cfg(test)]
            (_, buffer) => Err(SdmaRecycleFailureV1::ProcessTeardown {
                detail: "directional SDMA owner/driver mismatch during recycle".to_owned(),
                custody: scripted_mismatch_buffer(buffer, "recycle"),
            }),
        }
    }
}

#[cfg(test)]
mod scripted {
    use super::*;
    use std::cell::RefCell;
    use std::collections::{HashMap, VecDeque};
    use std::rc::Rc;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum ScriptedFailureModeV1 {
        Success,
        Retryable,
        ProcessTeardown,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub(crate) enum ScriptedExecutionOutcomeV1 {
        Pending,
        Completed {
            direction: Option<Gfx942PersistentSdmaDirectionV1>,
            copy_bytes: Option<u32>,
        },
        CompletedWindow {
            direction: Option<Gfx942PersistentSdmaDirectionV1>,
            copy_bytes: Option<u32>,
            requests: Option<Vec<DirectionalSdmaCopyRequestV1>>,
        },
        Retryable,
        ProcessTeardown,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub(crate) enum ScriptedSameDeviceExecutionOutcomeV1 {
        Pending,
        Completed {
            copy_bytes: Option<u32>,
            requests: Option<Vec<SameDeviceSdmaCopyRequestV1>>,
            swap_allocations: bool,
        },
        Retryable,
        ProcessTeardown,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum ScriptedRecycleOutcomeV1 {
        Success,
        Recovered,
        Ambiguous,
    }

    #[derive(Debug)]
    pub(crate) enum ScriptedSdmaStepV1 {
        Allocate {
            kind: ScriptedBufferKindV1,
            byte_len: usize,
        },
        Write {
            offset: u64,
            byte_len: usize,
        },
        Read {
            offset: u64,
            byte_len: u64,
        },
        Promote(ScriptedFailureModeV1),
        PromoteComputeReady(ScriptedFailureModeV1),
        PromoteComputeReadyForeignQueue,
        PromoteComputeReadyForeignQueueTerminal,
        Demote(ScriptedFailureModeV1),
        Submit {
            direction: Gfx942PersistentSdmaDirectionV1,
            host_offset: u64,
            device_offset: u64,
            copy_bytes: u32,
            outcome: ScriptedFailureModeV1,
        },
        SubmitWindow {
            direction: Gfx942PersistentSdmaDirectionV1,
            requests: Vec<DirectionalSdmaCopyRequestV1>,
            outcome: ScriptedFailureModeV1,
        },
        SubmitSameDeviceWindow {
            requests: Vec<SameDeviceSdmaCopyRequestV1>,
            outcome: ScriptedFailureModeV1,
        },
        Poll(ScriptedExecutionOutcomeV1),
        Wait(ScriptedExecutionOutcomeV1),
        Retire(ScriptedFailureModeV1),
        PollSameDevice(ScriptedSameDeviceExecutionOutcomeV1),
        WaitSameDevice(ScriptedSameDeviceExecutionOutcomeV1),
        RetireSameDevice(ScriptedFailureModeV1),
        Recycle(ScriptedRecycleOutcomeV1),
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) enum ScriptedBufferKindV1 {
        Host,
        Device,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ScriptedOwnerRoleV1 {
        Buffer(ScriptedBufferKindV1),
        Device,
    }

    #[derive(Debug, Default)]
    struct ScriptedCustodyLedgerV1 {
        next_id: u64,
        owners: HashMap<u64, ScriptedOwnerRoleV1>,
        unexpected_drops: usize,
    }

    #[derive(Debug)]
    struct ScriptedOwnerTokenV1 {
        id: u64,
        role: ScriptedOwnerRoleV1,
        ledger: Rc<RefCell<ScriptedCustodyLedgerV1>>,
        armed: bool,
    }

    impl ScriptedOwnerTokenV1 {
        fn new(role: ScriptedOwnerRoleV1, ledger: Rc<RefCell<ScriptedCustodyLedgerV1>>) -> Self {
            let id = {
                let mut ledger = ledger.borrow_mut();
                ledger.next_id += 1;
                let id = ledger.next_id;
                assert!(ledger.owners.insert(id, role).is_none());
                id
            };
            Self {
                id,
                role,
                ledger,
                armed: true,
            }
        }

        fn transition(mut self, role: ScriptedOwnerRoleV1) -> Self {
            let prior = self.ledger.borrow_mut().owners.insert(self.id, role);
            assert_eq!(prior, Some(self.role));
            self.role = role;
            self
        }

        fn release(mut self) {
            let prior = self.ledger.borrow_mut().owners.remove(&self.id);
            assert_eq!(prior, Some(self.role));
            self.armed = false;
        }
    }

    impl Drop for ScriptedOwnerTokenV1 {
        fn drop(&mut self) {
            if self.armed {
                let _ = self.ledger.borrow_mut().owners.remove(&self.id);
                self.ledger.borrow_mut().unexpected_drops += 1;
            }
        }
    }

    #[derive(Debug)]
    pub(crate) struct ScriptedBufferOwnerV1 {
        token: ScriptedOwnerTokenV1,
        kind: ScriptedBufferKindV1,
        bytes: Vec<u8>,
        full_content_certificate: Option<ScriptedHostContentCertificateV1>,
    }

    #[derive(Debug, Eq, PartialEq)]
    struct ScriptedHostContentCertificateV1 {
        owner_id: u64,
        byte_len: usize,
        sha256: [u8; 32],
    }

    #[derive(Debug)]
    pub(crate) struct ScriptedDeviceOwnerV1 {
        token: ScriptedOwnerTokenV1,
        pub(super) bytes: Vec<u8>,
    }

    #[derive(Debug)]
    pub(crate) struct ScriptedSubmissionOwnerV1 {
        pair: DirectionalSdmaPairOwnerV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        requests: DirectionalSdmaRequestPlanV1,
        copy_bytes: u32,
    }

    #[derive(Debug)]
    pub(crate) struct ScriptedCompletedOwnerV1 {
        pair: DirectionalSdmaPairOwnerV1,
        pub(super) direction: Gfx942PersistentSdmaDirectionV1,
        pub(super) host_offset: u64,
        pub(super) device_offset: u64,
        pub(super) copy_bytes: u32,
        pub(super) packet_count: usize,
    }

    pub(super) enum ScriptedPersistentComputeReadyFailureV1 {
        Recovered(DirectionalSdmaPairOwnerV1),
        ForeignQueue {
            completed: ScriptedCompletedOwnerV1,
            terminal_receiver: bool,
        },
        ProcessTeardown(ScriptedCompletedOwnerV1),
    }

    #[derive(Debug)]
    pub(crate) struct ScriptedSameDeviceSubmissionOwnerV1 {
        pair: SameDeviceSdmaPairOwnerV1,
        requests: Box<[SameDeviceSdmaCopyRequestV1]>,
        copy_bytes: u32,
        source_owner_id: u64,
        destination_owner_id: u64,
    }

    #[derive(Debug)]
    pub(crate) struct ScriptedSameDeviceCompletedOwnerV1 {
        pair: SameDeviceSdmaPairOwnerV1,
        pub(super) source_offset: u64,
        pub(super) destination_offset: u64,
        pub(super) copy_bytes: u32,
        pub(super) packet_count: usize,
    }

    #[allow(dead_code)]
    pub(crate) enum ScriptedTerminalCustodyV1 {
        Buffer(SdmaBufferOwnerV1),
        Device(DirectionalSdmaDeviceOwnerV1),
        Pair(DirectionalSdmaPairOwnerV1),
        Submission(DirectionalSdmaSubmissionOwnerV1),
        Completed(DirectionalSdmaCompletedOwnerV1),
        SameDevicePair(SameDeviceSdmaPairOwnerV1),
        SameDeviceSubmission(SameDeviceSdmaSubmissionOwnerV1),
        SameDeviceCompleted(SameDeviceSdmaCompletedOwnerV1),
    }

    pub(crate) struct ScriptedSdmaDriverV1 {
        steps: VecDeque<ScriptedSdmaStepV1>,
        ledger: Rc<RefCell<ScriptedCustodyLedgerV1>>,
    }

    impl core::fmt::Debug for ScriptedSdmaDriverV1 {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter
                .debug_struct("ScriptedSdmaDriverV1")
                .field("steps", &self.steps)
                .field("owners", &self.ledger.borrow().owners)
                .field("unexpected_drops", &self.ledger.borrow().unexpected_drops)
                .finish()
        }
    }

    impl ScriptedSdmaDriverV1 {
        pub(crate) fn new(steps: impl IntoIterator<Item = ScriptedSdmaStepV1>) -> Self {
            Self {
                steps: steps.into_iter().collect(),
                ledger: Rc::new(RefCell::new(ScriptedCustodyLedgerV1::default())),
            }
        }

        fn pop(&mut self) -> Result<ScriptedSdmaStepV1, String> {
            self.steps
                .pop_front()
                .ok_or_else(|| "scripted directional SDMA operation was not expected".to_owned())
        }

        pub(crate) fn is_exhausted(&self) -> bool {
            self.steps.is_empty()
        }

        pub(crate) fn remaining_steps(&self) -> usize {
            self.steps.len()
        }

        pub(crate) fn live_owner_count(&self) -> usize {
            self.ledger.borrow().owners.len()
        }

        pub(crate) fn unexpected_drops(&self) -> usize {
            self.ledger.borrow().unexpected_drops
        }

        pub(crate) fn test_host_owner(&self, byte_len: usize) -> SdmaBufferOwnerV1 {
            SdmaBufferOwnerV1::Scripted(ScriptedBufferOwnerV1 {
                token: ScriptedOwnerTokenV1::new(
                    ScriptedOwnerRoleV1::Buffer(ScriptedBufferKindV1::Host),
                    Rc::clone(&self.ledger),
                ),
                kind: ScriptedBufferKindV1::Host,
                bytes: vec![0; byte_len],
                full_content_certificate: None,
            })
        }

        pub(crate) fn test_device_owner(&self, byte_len: usize) -> DirectionalSdmaDeviceOwnerV1 {
            DirectionalSdmaDeviceOwnerV1::Scripted(ScriptedDeviceOwnerV1 {
                token: ScriptedOwnerTokenV1::new(
                    ScriptedOwnerRoleV1::Device,
                    Rc::clone(&self.ledger),
                ),
                bytes: vec![0; byte_len],
            })
        }

        fn owns_token(&self, token: &ScriptedOwnerTokenV1) -> bool {
            Rc::ptr_eq(&self.ledger, &token.ledger)
        }

        fn owns_buffer(&self, buffer: &ScriptedBufferOwnerV1) -> bool {
            self.owns_token(&buffer.token)
        }

        fn owns_device(&self, device: &ScriptedDeviceOwnerV1) -> bool {
            self.owns_token(&device.token)
        }

        fn owns_pair(&self, pair: &DirectionalSdmaPairOwnerV1) -> bool {
            matches!(
                (&pair.device, &pair.host),
                (
                    DirectionalSdmaDeviceOwnerV1::Scripted(device),
                    SdmaBufferOwnerV1::Scripted(host)
                ) if self.owns_device(device) && self.owns_buffer(host)
            )
        }

        fn owns_submission(&self, submission: &ScriptedSubmissionOwnerV1) -> bool {
            self.owns_pair(&submission.pair)
        }

        fn owns_completed(&self, completed: &ScriptedCompletedOwnerV1) -> bool {
            self.owns_pair(&completed.pair)
        }

        fn same_device_owner_ids(pair: &SameDeviceSdmaPairOwnerV1) -> Option<(u64, u64)> {
            match (&pair.source, &pair.destination) {
                (
                    DirectionalSdmaDeviceOwnerV1::Scripted(source),
                    DirectionalSdmaDeviceOwnerV1::Scripted(destination),
                ) => Some((source.token.id, destination.token.id)),
                #[allow(unreachable_patterns)]
                _ => None,
            }
        }

        fn owns_same_device_pair(&self, pair: &SameDeviceSdmaPairOwnerV1) -> bool {
            match (&pair.source, &pair.destination) {
                (
                    DirectionalSdmaDeviceOwnerV1::Scripted(source),
                    DirectionalSdmaDeviceOwnerV1::Scripted(destination),
                ) => self.owns_device(source) && self.owns_device(destination),
                #[allow(unreachable_patterns)]
                _ => false,
            }
        }

        fn owns_same_device_submission(
            &self,
            submission: &ScriptedSameDeviceSubmissionOwnerV1,
        ) -> bool {
            self.owns_same_device_pair(&submission.pair)
                && Self::same_device_owner_ids(&submission.pair)
                    == Some((submission.source_owner_id, submission.destination_owner_id))
        }

        fn owns_same_device_completed(
            &self,
            completed: &ScriptedSameDeviceCompletedOwnerV1,
        ) -> bool {
            self.owns_same_device_pair(&completed.pair)
        }

        pub(super) fn allocate_buffer(
            &mut self,
            byte_len: usize,
            kind: ScriptedBufferKindV1,
        ) -> Result<SdmaBufferOwnerV1, String> {
            match self.pop()? {
                ScriptedSdmaStepV1::Allocate {
                    kind: expected_kind,
                    byte_len: expected_len,
                } if expected_kind == kind && expected_len == byte_len => {}
                step => return Err(format!("scripted SDMA allocation mismatch: {step:?}")),
            }
            Ok(SdmaBufferOwnerV1::Scripted(ScriptedBufferOwnerV1 {
                token: ScriptedOwnerTokenV1::new(
                    ScriptedOwnerRoleV1::Buffer(kind),
                    Rc::clone(&self.ledger),
                ),
                kind,
                bytes: vec![0; byte_len],
                full_content_certificate: None,
            }))
        }

        pub(super) fn write_host(
            &mut self,
            buffer: &mut ScriptedBufferOwnerV1,
            offset: u64,
            bytes: &[u8],
        ) -> Result<(), String> {
            if !self.owns_buffer(buffer) {
                return Err("scripted SDMA write owner belongs to another driver".to_owned());
            }
            match self.pop()? {
                ScriptedSdmaStepV1::Write {
                    offset: expected_offset,
                    byte_len,
                } if expected_offset == offset && byte_len == bytes.len() => {}
                step => return Err(format!("scripted SDMA write mismatch: {step:?}")),
            }
            if buffer.kind != ScriptedBufferKindV1::Host {
                return Err("scripted SDMA write requires host storage".to_owned());
            }
            let start = usize::try_from(offset).map_err(|_| "scripted write offset overflow")?;
            let end = start
                .checked_add(bytes.len())
                .filter(|end| *end <= buffer.bytes.len())
                .ok_or("scripted write exceeds buffer")?;
            buffer.full_content_certificate = None;
            buffer.bytes[start..end].copy_from_slice(bytes);
            Ok(())
        }

        pub(super) fn write_full_host_authenticated(
            &mut self,
            buffer: &mut ScriptedBufferOwnerV1,
            bytes: &[u8],
        ) -> Result<[u8; 32], String> {
            if !self.owns_buffer(buffer) {
                return Err(
                    "scripted authenticated SDMA write owner belongs to another driver".to_owned(),
                );
            }
            if buffer.kind != ScriptedBufferKindV1::Host || buffer.bytes.len() != bytes.len() {
                return Err(
                    "scripted authenticated SDMA write requires one exact full host extent"
                        .to_owned(),
                );
            }
            buffer.full_content_certificate = None;
            let mut hasher = Sha256::new();
            for (index, chunk) in bytes
                .chunks(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize)
                .enumerate()
            {
                let offset = (index as u64)
                    .checked_mul(u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1))
                    .ok_or("scripted authenticated write offset overflow")?;
                match self.pop()? {
                    ScriptedSdmaStepV1::Write {
                        offset: expected_offset,
                        byte_len,
                    } if expected_offset == offset && byte_len == chunk.len() => {}
                    step => {
                        return Err(format!(
                            "scripted authenticated SDMA write mismatch: {step:?}"
                        ));
                    }
                }
                let start = usize::try_from(offset)
                    .map_err(|_| "scripted authenticated write offset overflow")?;
                let end = start
                    .checked_add(chunk.len())
                    .ok_or("scripted authenticated write range overflow")?;
                hasher.update(chunk);
                buffer.bytes[start..end].copy_from_slice(chunk);
            }
            let digest = hasher.finalize().into();
            buffer.full_content_certificate = Some(ScriptedHostContentCertificateV1 {
                owner_id: buffer.token.id,
                byte_len: buffer.bytes.len(),
                sha256: digest,
            });
            Ok(digest)
        }

        pub(super) fn read_host(
            &mut self,
            buffer: &ScriptedBufferOwnerV1,
            offset: u64,
            byte_len: u64,
        ) -> Result<Box<[u8]>, String> {
            if !self.owns_buffer(buffer) {
                return Err("scripted SDMA read owner belongs to another driver".to_owned());
            }
            match self.pop()? {
                ScriptedSdmaStepV1::Read {
                    offset: expected_offset,
                    byte_len: expected_len,
                } if expected_offset == offset && expected_len == byte_len => {}
                step => return Err(format!("scripted SDMA read mismatch: {step:?}")),
            }
            if buffer.kind != ScriptedBufferKindV1::Host {
                return Err("scripted SDMA read requires host storage".to_owned());
            }
            let start = usize::try_from(offset).map_err(|_| "scripted read offset overflow")?;
            let len = usize::try_from(byte_len).map_err(|_| "scripted read length overflow")?;
            let end = start
                .checked_add(len)
                .filter(|end| *end <= buffer.bytes.len())
                .ok_or("scripted read exceeds buffer")?;
            Ok(buffer.bytes[start..end].into())
        }

        pub(super) fn promote(
            &mut self,
            buffer: ScriptedBufferOwnerV1,
        ) -> Result<DirectionalSdmaDeviceOwnerV1, SdmaTransitionFailureV1<SdmaBufferOwnerV1>>
        {
            if !self.owns_buffer(&buffer) {
                return Err(scripted_buffer_mismatch(
                    buffer,
                    "promotion owner belongs to another driver".to_owned(),
                ));
            }
            let outcome = match self.pop() {
                Ok(ScriptedSdmaStepV1::Promote(outcome)) => outcome,
                Ok(step) => {
                    return Err(scripted_buffer_mismatch(
                        buffer,
                        format!("promotion mismatch: {step:?}"),
                    ));
                }
                Err(detail) => return Err(scripted_buffer_mismatch(buffer, detail)),
            };
            match outcome {
                ScriptedFailureModeV1::Success => Ok(DirectionalSdmaDeviceOwnerV1::Scripted(
                    ScriptedDeviceOwnerV1 {
                        token: buffer.token.transition(ScriptedOwnerRoleV1::Device),
                        bytes: buffer.bytes,
                    },
                )),
                ScriptedFailureModeV1::Retryable => Err(SdmaTransitionFailureV1::Retryable {
                    detail: "scripted promotion retryable".to_owned(),
                    custody: SdmaBufferOwnerV1::Scripted(buffer),
                }),
                ScriptedFailureModeV1::ProcessTeardown => {
                    Err(SdmaTransitionFailureV1::ProcessTeardown {
                        detail: "scripted promotion teardown".to_owned(),
                        custody: SdmaTerminalCustodyV1::Scripted(
                            ScriptedTerminalCustodyV1::Buffer(SdmaBufferOwnerV1::Scripted(buffer)),
                        ),
                    })
                }
            }
        }

        pub(super) fn demote(
            &mut self,
            device: ScriptedDeviceOwnerV1,
        ) -> Result<SdmaBufferOwnerV1, SdmaTransitionFailureV1<DirectionalSdmaDeviceOwnerV1>>
        {
            if !self.owns_device(&device) {
                return Err(scripted_device_mismatch(
                    device,
                    "demotion owner belongs to another driver".to_owned(),
                ));
            }
            let outcome = match self.pop() {
                Ok(ScriptedSdmaStepV1::Demote(outcome)) => outcome,
                Ok(step) => {
                    return Err(scripted_device_mismatch(
                        device,
                        format!("demotion mismatch: {step:?}"),
                    ));
                }
                Err(detail) => return Err(scripted_device_mismatch(device, detail)),
            };
            match outcome {
                ScriptedFailureModeV1::Success => {
                    Ok(SdmaBufferOwnerV1::Scripted(ScriptedBufferOwnerV1 {
                        token: device
                            .token
                            .transition(ScriptedOwnerRoleV1::Buffer(ScriptedBufferKindV1::Device)),
                        kind: ScriptedBufferKindV1::Device,
                        bytes: device.bytes,
                        full_content_certificate: None,
                    }))
                }
                ScriptedFailureModeV1::Retryable => Err(SdmaTransitionFailureV1::Retryable {
                    detail: "scripted demotion retryable".to_owned(),
                    custody: DirectionalSdmaDeviceOwnerV1::Scripted(device),
                }),
                ScriptedFailureModeV1::ProcessTeardown => {
                    Err(SdmaTransitionFailureV1::ProcessTeardown {
                        detail: "scripted demotion teardown".to_owned(),
                        custody: SdmaTerminalCustodyV1::Scripted(
                            ScriptedTerminalCustodyV1::Device(
                                DirectionalSdmaDeviceOwnerV1::Scripted(device),
                            ),
                        ),
                    })
                }
            }
        }

        pub(super) fn submit(
            &mut self,
            device: ScriptedDeviceOwnerV1,
            mut host: ScriptedBufferOwnerV1,
            direction: Gfx942PersistentSdmaDirectionV1,
            requests: DirectionalSdmaRequestPlanV1,
        ) -> Result<
            DirectionalSdmaSubmissionOwnerV1,
            SdmaTransitionFailureV1<DirectionalSdmaPairOwnerV1>,
        > {
            if direction == Gfx942PersistentSdmaDirectionV1::DeviceToHost {
                host.full_content_certificate = None;
            }
            let pair = DirectionalSdmaPairOwnerV1 {
                device: DirectionalSdmaDeviceOwnerV1::Scripted(device),
                host: SdmaBufferOwnerV1::Scripted(host),
            };
            if !self.owns_pair(&pair) {
                return Err(scripted_pair_mismatch(
                    pair,
                    "submission owners belong to another driver".to_owned(),
                ));
            }
            let copy_bytes = match validate_window_requests_v1(requests.as_slice()) {
                Ok((_, _, copy_bytes)) => copy_bytes,
                Err(detail) => return Err(scripted_pair_mismatch(pair, detail)),
            };
            let outcome = match (self.pop(), &requests) {
                (
                    Ok(ScriptedSdmaStepV1::Submit {
                        direction: expected_direction,
                        host_offset: expected_host_offset,
                        device_offset: expected_device_offset,
                        copy_bytes: expected_copy_bytes,
                        outcome,
                    }),
                    DirectionalSdmaRequestPlanV1::Single(request),
                ) if expected_direction == direction
                    && expected_host_offset == request.host_offset
                    && expected_device_offset == request.device_offset
                    && expected_copy_bytes == request.copy_bytes =>
                {
                    outcome
                }
                (
                    Ok(ScriptedSdmaStepV1::SubmitWindow {
                        direction: expected_direction,
                        requests: expected_requests,
                        outcome,
                    }),
                    DirectionalSdmaRequestPlanV1::Window(requests),
                ) if expected_direction == direction
                    && expected_requests.as_slice() == requests.as_ref() =>
                {
                    outcome
                }
                (Ok(step), _) => {
                    return Err(scripted_pair_mismatch(
                        pair,
                        format!("submission mismatch: {step:?}"),
                    ));
                }
                (Err(detail), _) => return Err(scripted_pair_mismatch(pair, detail)),
            };
            match outcome {
                ScriptedFailureModeV1::Success => Ok(DirectionalSdmaSubmissionOwnerV1::Scripted(
                    ScriptedSubmissionOwnerV1 {
                        pair,
                        direction,
                        requests,
                        copy_bytes,
                    },
                )),
                ScriptedFailureModeV1::Retryable => Err(SdmaTransitionFailureV1::Retryable {
                    detail: "scripted submission retryable".to_owned(),
                    custody: pair,
                }),
                ScriptedFailureModeV1::ProcessTeardown => {
                    Err(SdmaTransitionFailureV1::ProcessTeardown {
                        detail: "scripted submission teardown".to_owned(),
                        custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Pair(
                            pair,
                        )),
                    })
                }
            }
        }

        pub(super) fn submit_same_device(
            &mut self,
            pair: SameDeviceSdmaPairOwnerV1,
            requests: Box<[SameDeviceSdmaCopyRequestV1]>,
        ) -> Result<
            SameDeviceSdmaSubmissionOwnerV1,
            SdmaTransitionFailureV1<SameDeviceSdmaPairOwnerV1>,
        > {
            if !self.owns_same_device_pair(&pair) {
                return Err(scripted_same_device_pair_mismatch(
                    pair,
                    "same-device submission owners belong to another driver".to_owned(),
                ));
            }
            let Some((source_owner_id, destination_owner_id)) = Self::same_device_owner_ids(&pair)
            else {
                return Err(scripted_same_device_pair_mismatch(
                    pair,
                    "same-device submission owners are not scripted device allocations".to_owned(),
                ));
            };
            if source_owner_id == destination_owner_id {
                return Err(scripted_same_device_pair_mismatch(
                    pair,
                    "same-device submission requires distinct allocation owners".to_owned(),
                ));
            }
            let copy_bytes = match validate_same_device_window_requests_v1(&requests) {
                Ok((_, _, copy_bytes)) => copy_bytes,
                Err(detail) => {
                    return Err(SdmaTransitionFailureV1::Retryable {
                        detail,
                        custody: pair,
                    });
                }
            };
            let outcome = match self.pop() {
                Ok(ScriptedSdmaStepV1::SubmitSameDeviceWindow {
                    requests: expected_requests,
                    outcome,
                }) if expected_requests.as_slice() == requests.as_ref() => outcome,
                Ok(step) => {
                    return Err(scripted_same_device_pair_mismatch(
                        pair,
                        format!("same-device submission mismatch: {step:?}"),
                    ));
                }
                Err(detail) => return Err(scripted_same_device_pair_mismatch(pair, detail)),
            };
            match outcome {
                ScriptedFailureModeV1::Success => Ok(SameDeviceSdmaSubmissionOwnerV1::Scripted(
                    ScriptedSameDeviceSubmissionOwnerV1 {
                        pair,
                        requests,
                        copy_bytes,
                        source_owner_id,
                        destination_owner_id,
                    },
                )),
                ScriptedFailureModeV1::Retryable => Err(SdmaTransitionFailureV1::Retryable {
                    detail: "scripted same-device submission retryable".to_owned(),
                    custody: pair,
                }),
                ScriptedFailureModeV1::ProcessTeardown => {
                    Err(SdmaTransitionFailureV1::ProcessTeardown {
                        detail: "scripted same-device submission teardown".to_owned(),
                        custody: SdmaTerminalCustodyV1::Scripted(
                            ScriptedTerminalCustodyV1::SameDevicePair(pair),
                        ),
                    })
                }
            }
        }

        pub(super) fn poll(
            &mut self,
            submission: ScriptedSubmissionOwnerV1,
        ) -> Result<DirectionalSdmaPollV1, DirectionalSdmaExecutionFailureV1> {
            if !self.owns_submission(&submission) {
                return Err(scripted_submission_mismatch(
                    submission,
                    "poll owner belongs to another driver".to_owned(),
                ));
            }
            let outcome = match self.pop() {
                Ok(ScriptedSdmaStepV1::Poll(outcome)) => outcome,
                Ok(step) => {
                    return Err(scripted_submission_mismatch(
                        submission,
                        format!("poll mismatch: {step:?}"),
                    ));
                }
                Err(detail) => return Err(scripted_submission_mismatch(submission, detail)),
            };
            execute_outcome(submission, outcome, "poll")
        }

        pub(super) fn poll_same_device(
            &mut self,
            submission: ScriptedSameDeviceSubmissionOwnerV1,
        ) -> Result<SameDeviceSdmaPollV1, SameDeviceSdmaExecutionFailureV1> {
            if !self.owns_same_device_submission(&submission) {
                return Err(scripted_same_device_submission_mismatch(
                    submission,
                    "same-device poll owner or allocation roles changed".to_owned(),
                ));
            }
            let outcome = match self.pop() {
                Ok(ScriptedSdmaStepV1::PollSameDevice(outcome)) => outcome,
                Ok(step) => {
                    return Err(scripted_same_device_submission_mismatch(
                        submission,
                        format!("same-device poll mismatch: {step:?}"),
                    ));
                }
                Err(detail) => {
                    return Err(scripted_same_device_submission_mismatch(submission, detail));
                }
            };
            execute_same_device_outcome(submission, outcome, "poll")
        }

        pub(super) fn wait(
            &mut self,
            submission: ScriptedSubmissionOwnerV1,
        ) -> Result<DirectionalSdmaCompletedOwnerV1, DirectionalSdmaExecutionFailureV1> {
            if !self.owns_submission(&submission) {
                return Err(scripted_submission_mismatch(
                    submission,
                    "wait owner belongs to another driver".to_owned(),
                ));
            }
            let outcome = match self.pop() {
                Ok(ScriptedSdmaStepV1::Wait(outcome)) => outcome,
                Ok(step) => {
                    return Err(scripted_submission_mismatch(
                        submission,
                        format!("wait mismatch: {step:?}"),
                    ));
                }
                Err(detail) => return Err(scripted_submission_mismatch(submission, detail)),
            };
            match execute_outcome(submission, outcome, "wait")? {
                DirectionalSdmaPollV1::Completed(completed) => Ok(completed),
                DirectionalSdmaPollV1::Pending(submission) => {
                    Err(DirectionalSdmaExecutionFailureV1::Retryable {
                        detail: "scripted wait timed out".to_owned(),
                        submission,
                    })
                }
            }
        }

        pub(super) fn wait_same_device(
            &mut self,
            submission: ScriptedSameDeviceSubmissionOwnerV1,
        ) -> Result<SameDeviceSdmaCompletedOwnerV1, SameDeviceSdmaExecutionFailureV1> {
            if !self.owns_same_device_submission(&submission) {
                return Err(scripted_same_device_submission_mismatch(
                    submission,
                    "same-device wait owner or allocation roles changed".to_owned(),
                ));
            }
            let outcome = match self.pop() {
                Ok(ScriptedSdmaStepV1::WaitSameDevice(outcome)) => outcome,
                Ok(step) => {
                    return Err(scripted_same_device_submission_mismatch(
                        submission,
                        format!("same-device wait mismatch: {step:?}"),
                    ));
                }
                Err(detail) => {
                    return Err(scripted_same_device_submission_mismatch(submission, detail));
                }
            };
            match execute_same_device_outcome(submission, outcome, "wait")? {
                SameDeviceSdmaPollV1::Completed(completed) => Ok(completed),
                SameDeviceSdmaPollV1::Pending(submission) => {
                    Err(SameDeviceSdmaExecutionFailureV1::Retryable {
                        detail: "scripted same-device wait timed out".to_owned(),
                        submission,
                    })
                }
            }
        }

        pub(super) fn retire(
            &mut self,
            completed: ScriptedCompletedOwnerV1,
        ) -> Result<
            DirectionalSdmaPairOwnerV1,
            SdmaTransitionFailureV1<DirectionalSdmaCompletedOwnerV1>,
        > {
            if !self.owns_completed(&completed) {
                return Err(scripted_completed_mismatch(
                    completed,
                    "retirement owner belongs to another driver".to_owned(),
                ));
            }
            let outcome = match self.pop() {
                Ok(ScriptedSdmaStepV1::Retire(outcome)) => outcome,
                Ok(step) => {
                    return Err(scripted_completed_mismatch(
                        completed,
                        format!("retirement mismatch: {step:?}"),
                    ));
                }
                Err(detail) => return Err(scripted_completed_mismatch(completed, detail)),
            };
            match outcome {
                ScriptedFailureModeV1::Success => Ok(completed.pair),
                ScriptedFailureModeV1::Retryable | ScriptedFailureModeV1::ProcessTeardown => {
                    Err(SdmaTransitionFailureV1::ProcessTeardown {
                        detail: "scripted retirement failure".to_owned(),
                        custody: SdmaTerminalCustodyV1::Scripted(
                            ScriptedTerminalCustodyV1::Completed(
                                DirectionalSdmaCompletedOwnerV1::Scripted(completed),
                            ),
                        ),
                    })
                }
            }
        }

        pub(super) fn promote_full_h2d_to_compute_ready(
            &mut self,
            completed: ScriptedCompletedOwnerV1,
            content: fe2o3_kfd::Gfx942DeviceContentDescriptorV1,
        ) -> Result<
            (ScriptedDeviceOwnerV1, ScriptedBufferOwnerV1),
            ScriptedPersistentComputeReadyFailureV1,
        > {
            if !self.owns_completed(&completed) {
                return Err(ScriptedPersistentComputeReadyFailureV1::ProcessTeardown(
                    completed,
                ));
            }
            let promotion = match self.steps.front() {
                Some(ScriptedSdmaStepV1::PromoteComputeReadyForeignQueue) => {
                    let _ = self.pop();
                    return Err(ScriptedPersistentComputeReadyFailureV1::ForeignQueue {
                        completed,
                        terminal_receiver: false,
                    });
                }
                Some(ScriptedSdmaStepV1::PromoteComputeReadyForeignQueueTerminal) => {
                    let _ = self.pop();
                    return Err(ScriptedPersistentComputeReadyFailureV1::ForeignQueue {
                        completed,
                        terminal_receiver: true,
                    });
                }
                Some(ScriptedSdmaStepV1::PromoteComputeReady(_)) => match self.pop() {
                    Ok(ScriptedSdmaStepV1::PromoteComputeReady(outcome)) => outcome,
                    _ => {
                        unreachable!("peeked scripted ready-promotion step remains ready promotion")
                    }
                },
                _ => ScriptedFailureModeV1::Success,
            };
            match promotion {
                ScriptedFailureModeV1::Success => {}
                ScriptedFailureModeV1::Retryable => {
                    return Err(ScriptedPersistentComputeReadyFailureV1::Recovered(
                        completed.pair,
                    ));
                }
                ScriptedFailureModeV1::ProcessTeardown => {
                    return Err(ScriptedPersistentComputeReadyFailureV1::ProcessTeardown(
                        completed,
                    ));
                }
            }
            let ScriptedCompletedOwnerV1 {
                pair,
                direction,
                host_offset,
                device_offset,
                copy_bytes,
                ..
            } = completed;
            let DirectionalSdmaPairOwnerV1 {
                device: DirectionalSdmaDeviceOwnerV1::Scripted(device),
                host: SdmaBufferOwnerV1::Scripted(host),
            } = pair
            else {
                unreachable!("scripted completion retains a scripted pair")
            };
            let observed_sha256 = host
                .full_content_certificate
                .as_ref()
                .filter(|certificate| {
                    certificate.owner_id == host.token.id
                        && certificate.byte_len == host.bytes.len()
                })
                .map(|certificate| certificate.sha256);
            let exact = direction == Gfx942PersistentSdmaDirectionV1::HostToDevice
                && host_offset == 0
                && device_offset == 0
                && u64::from(copy_bytes) == content.byte_len()
                && device.bytes.len() == host.bytes.len()
                && u64::try_from(device.bytes.len()).ok() == Some(content.byte_len())
                && observed_sha256 == Some(content.sha256());
            if exact {
                Ok((device, host))
            } else {
                Err(ScriptedPersistentComputeReadyFailureV1::Recovered(
                    DirectionalSdmaPairOwnerV1 {
                        device: DirectionalSdmaDeviceOwnerV1::Scripted(device),
                        host: SdmaBufferOwnerV1::Scripted(host),
                    },
                ))
            }
        }

        pub(super) fn retire_same_device(
            &mut self,
            completed: ScriptedSameDeviceCompletedOwnerV1,
        ) -> Result<
            SameDeviceSdmaPairOwnerV1,
            SdmaTransitionFailureV1<SameDeviceSdmaCompletedOwnerV1>,
        > {
            if !self.owns_same_device_completed(&completed) {
                return Err(scripted_same_device_completed_mismatch(
                    completed,
                    "same-device retirement owners belong to another driver".to_owned(),
                ));
            }
            let outcome = match self.pop() {
                Ok(ScriptedSdmaStepV1::RetireSameDevice(outcome)) => outcome,
                Ok(step) => {
                    return Err(scripted_same_device_completed_mismatch(
                        completed,
                        format!("same-device retirement mismatch: {step:?}"),
                    ));
                }
                Err(detail) => {
                    return Err(scripted_same_device_completed_mismatch(completed, detail));
                }
            };
            match outcome {
                ScriptedFailureModeV1::Success => Ok(completed.pair),
                ScriptedFailureModeV1::Retryable | ScriptedFailureModeV1::ProcessTeardown => {
                    Err(SdmaTransitionFailureV1::ProcessTeardown {
                        detail: "scripted same-device retirement failure".to_owned(),
                        custody: SdmaTerminalCustodyV1::Scripted(
                            ScriptedTerminalCustodyV1::SameDeviceCompleted(
                                SameDeviceSdmaCompletedOwnerV1::Scripted(completed),
                            ),
                        ),
                    })
                }
            }
        }

        pub(super) fn recycle(
            &mut self,
            buffer: ScriptedBufferOwnerV1,
        ) -> Result<(), SdmaRecycleFailureV1> {
            if !self.owns_buffer(&buffer) {
                return Err(SdmaRecycleFailureV1::ProcessTeardown {
                    detail: "recycle owner belongs to another driver".to_owned(),
                    custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Buffer(
                        SdmaBufferOwnerV1::Scripted(buffer),
                    )),
                });
            }
            let outcome = match self.pop() {
                Ok(ScriptedSdmaStepV1::Recycle(outcome)) => outcome,
                Ok(step) => {
                    return Err(SdmaRecycleFailureV1::ProcessTeardown {
                        detail: format!("recycle mismatch: {step:?}"),
                        custody: SdmaTerminalCustodyV1::Scripted(
                            ScriptedTerminalCustodyV1::Buffer(SdmaBufferOwnerV1::Scripted(buffer)),
                        ),
                    });
                }
                Err(detail) => {
                    return Err(SdmaRecycleFailureV1::ProcessTeardown {
                        detail,
                        custody: SdmaTerminalCustodyV1::Scripted(
                            ScriptedTerminalCustodyV1::Buffer(SdmaBufferOwnerV1::Scripted(buffer)),
                        ),
                    });
                }
            };
            match outcome {
                ScriptedRecycleOutcomeV1::Success => {
                    buffer.token.release();
                    Ok(())
                }
                ScriptedRecycleOutcomeV1::Recovered => Err(SdmaRecycleFailureV1::Recovered {
                    detail: "scripted recycle recovered".to_owned(),
                    buffer: SdmaBufferOwnerV1::Scripted(buffer),
                }),
                ScriptedRecycleOutcomeV1::Ambiguous => {
                    buffer.token.release();
                    Err(SdmaRecycleFailureV1::Ambiguous {
                        detail: "scripted recycle ambiguous".to_owned(),
                    })
                }
            }
        }
    }

    fn execute_outcome(
        submission: ScriptedSubmissionOwnerV1,
        outcome: ScriptedExecutionOutcomeV1,
        operation: &'static str,
    ) -> Result<DirectionalSdmaPollV1, DirectionalSdmaExecutionFailureV1> {
        match outcome {
            ScriptedExecutionOutcomeV1::Pending => Ok(DirectionalSdmaPollV1::Pending(
                DirectionalSdmaSubmissionOwnerV1::Scripted(submission),
            )),
            ScriptedExecutionOutcomeV1::Retryable => {
                Err(DirectionalSdmaExecutionFailureV1::Retryable {
                    detail: format!("scripted {operation} retryable"),
                    submission: DirectionalSdmaSubmissionOwnerV1::Scripted(submission),
                })
            }
            ScriptedExecutionOutcomeV1::ProcessTeardown => {
                Err(DirectionalSdmaExecutionFailureV1::ProcessTeardown {
                    detail: format!("scripted {operation} teardown"),
                    custody: SdmaTerminalCustodyV1::Scripted(
                        ScriptedTerminalCustodyV1::Submission(
                            DirectionalSdmaSubmissionOwnerV1::Scripted(submission),
                        ),
                    ),
                })
            }
            ScriptedExecutionOutcomeV1::Completed {
                direction,
                copy_bytes,
            } => complete_scripted_submission(submission, direction, copy_bytes, None),
            ScriptedExecutionOutcomeV1::CompletedWindow {
                direction,
                copy_bytes,
                requests,
            } => complete_scripted_submission(submission, direction, copy_bytes, requests),
        }
    }

    fn complete_scripted_submission(
        mut submission: ScriptedSubmissionOwnerV1,
        direction: Option<Gfx942PersistentSdmaDirectionV1>,
        copy_bytes: Option<u32>,
        reported_requests: Option<Vec<DirectionalSdmaCopyRequestV1>>,
    ) -> Result<DirectionalSdmaPollV1, DirectionalSdmaExecutionFailureV1> {
        for request in submission.requests.as_slice() {
            let len = usize::try_from(request.copy_bytes).expect("u32 fits usize");
            let host_start = usize::try_from(request.host_offset)
                .expect("admitted scripted host offset fits usize");
            let device_start = usize::try_from(request.device_offset)
                .expect("admitted scripted device offset fits usize");
            let (device, host) = match (&mut submission.pair.device, &mut submission.pair.host) {
                (
                    DirectionalSdmaDeviceOwnerV1::Scripted(device),
                    SdmaBufferOwnerV1::Scripted(host),
                ) => (device, host),
                _ => unreachable!("scripted submission retains scripted pair"),
            };
            match submission.direction {
                Gfx942PersistentSdmaDirectionV1::HostToDevice => device.bytes
                    [device_start..device_start + len]
                    .copy_from_slice(&host.bytes[host_start..host_start + len]),
                Gfx942PersistentSdmaDirectionV1::DeviceToHost => host.bytes
                    [host_start..host_start + len]
                    .copy_from_slice(&device.bytes[device_start..device_start + len]),
            }
        }
        let packet_count = reported_requests
            .as_ref()
            .map_or_else(|| submission.requests.packet_count(), Vec::len);
        let (host_offset, device_offset) = reported_requests
            .as_deref()
            .unwrap_or_else(|| submission.requests.as_slice())
            .first()
            .map(|request| (request.host_offset, request.device_offset))
            .unwrap_or((0, 0));
        Ok(DirectionalSdmaPollV1::Completed(
            DirectionalSdmaCompletedOwnerV1::Scripted(ScriptedCompletedOwnerV1 {
                pair: submission.pair,
                direction: direction.unwrap_or(submission.direction),
                host_offset,
                device_offset,
                copy_bytes: copy_bytes.unwrap_or(submission.copy_bytes),
                packet_count,
            }),
        ))
    }

    fn execute_same_device_outcome(
        submission: ScriptedSameDeviceSubmissionOwnerV1,
        outcome: ScriptedSameDeviceExecutionOutcomeV1,
        operation: &'static str,
    ) -> Result<SameDeviceSdmaPollV1, SameDeviceSdmaExecutionFailureV1> {
        match outcome {
            ScriptedSameDeviceExecutionOutcomeV1::Pending => Ok(SameDeviceSdmaPollV1::Pending(
                SameDeviceSdmaSubmissionOwnerV1::Scripted(submission),
            )),
            ScriptedSameDeviceExecutionOutcomeV1::Retryable => {
                Err(SameDeviceSdmaExecutionFailureV1::Retryable {
                    detail: format!("scripted same-device {operation} retryable"),
                    submission: SameDeviceSdmaSubmissionOwnerV1::Scripted(submission),
                })
            }
            ScriptedSameDeviceExecutionOutcomeV1::ProcessTeardown => {
                Err(SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
                    detail: format!("scripted same-device {operation} teardown"),
                    custody: SdmaTerminalCustodyV1::Scripted(
                        ScriptedTerminalCustodyV1::SameDeviceSubmission(
                            SameDeviceSdmaSubmissionOwnerV1::Scripted(submission),
                        ),
                    ),
                })
            }
            ScriptedSameDeviceExecutionOutcomeV1::Completed {
                copy_bytes,
                requests,
                swap_allocations,
            } => complete_scripted_same_device_submission(
                submission,
                copy_bytes,
                requests,
                swap_allocations,
            ),
        }
    }

    fn complete_scripted_same_device_submission(
        mut submission: ScriptedSameDeviceSubmissionOwnerV1,
        copy_bytes: Option<u32>,
        reported_requests: Option<Vec<SameDeviceSdmaCopyRequestV1>>,
        swap_allocations: bool,
    ) -> Result<SameDeviceSdmaPollV1, SameDeviceSdmaExecutionFailureV1> {
        for request in submission.requests.iter() {
            let len = usize::try_from(request.copy_bytes).expect("u32 fits usize");
            let source_start = usize::try_from(request.source_offset)
                .expect("admitted scripted source offset fits usize");
            let destination_start = usize::try_from(request.destination_offset)
                .expect("admitted scripted destination offset fits usize");
            let (
                DirectionalSdmaDeviceOwnerV1::Scripted(source),
                DirectionalSdmaDeviceOwnerV1::Scripted(destination),
            ) = (&submission.pair.source, &mut submission.pair.destination)
            else {
                unreachable!("scripted same-device submission retains scripted owners")
            };
            destination.bytes[destination_start..destination_start + len]
                .copy_from_slice(&source.bytes[source_start..source_start + len]);
        }
        if swap_allocations {
            core::mem::swap(
                &mut submission.pair.source,
                &mut submission.pair.destination,
            );
        }
        let reported_requests = reported_requests
            .map(Vec::into_boxed_slice)
            .unwrap_or_else(|| submission.requests.clone());
        let pair_ids = ScriptedSdmaDriverV1::same_device_owner_ids(&submission.pair);
        if pair_ids != Some((submission.source_owner_id, submission.destination_owner_id))
            || reported_requests.as_ref() != submission.requests.as_ref()
        {
            return Err(SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
                detail: "scripted same-device completion identity or request roster changed"
                    .to_owned(),
                custody: SdmaTerminalCustodyV1::Scripted(
                    ScriptedTerminalCustodyV1::SameDevicePair(submission.pair),
                ),
            });
        }
        let packet_count = reported_requests.len();
        let (source_offset, destination_offset) = reported_requests
            .first()
            .map(|request| (request.source_offset, request.destination_offset))
            .unwrap_or((0, 0));
        Ok(SameDeviceSdmaPollV1::Completed(
            SameDeviceSdmaCompletedOwnerV1::Scripted(ScriptedSameDeviceCompletedOwnerV1 {
                pair: submission.pair,
                source_offset,
                destination_offset,
                copy_bytes: copy_bytes.unwrap_or(submission.copy_bytes),
                packet_count,
            }),
        ))
    }

    fn scripted_buffer_mismatch(
        buffer: ScriptedBufferOwnerV1,
        detail: String,
    ) -> SdmaTransitionFailureV1<SdmaBufferOwnerV1> {
        SdmaTransitionFailureV1::ProcessTeardown {
            detail,
            custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Buffer(
                SdmaBufferOwnerV1::Scripted(buffer),
            )),
        }
    }

    fn scripted_same_device_pair_mismatch(
        pair: SameDeviceSdmaPairOwnerV1,
        detail: String,
    ) -> SdmaTransitionFailureV1<SameDeviceSdmaPairOwnerV1> {
        SdmaTransitionFailureV1::ProcessTeardown {
            detail,
            custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::SameDevicePair(
                pair,
            )),
        }
    }

    fn scripted_same_device_submission_mismatch(
        submission: ScriptedSameDeviceSubmissionOwnerV1,
        detail: String,
    ) -> SameDeviceSdmaExecutionFailureV1 {
        SameDeviceSdmaExecutionFailureV1::ProcessTeardown {
            detail,
            custody: SdmaTerminalCustodyV1::Scripted(
                ScriptedTerminalCustodyV1::SameDeviceSubmission(
                    SameDeviceSdmaSubmissionOwnerV1::Scripted(submission),
                ),
            ),
        }
    }

    fn scripted_same_device_completed_mismatch(
        completed: ScriptedSameDeviceCompletedOwnerV1,
        detail: String,
    ) -> SdmaTransitionFailureV1<SameDeviceSdmaCompletedOwnerV1> {
        SdmaTransitionFailureV1::ProcessTeardown {
            detail,
            custody: SdmaTerminalCustodyV1::Scripted(
                ScriptedTerminalCustodyV1::SameDeviceCompleted(
                    SameDeviceSdmaCompletedOwnerV1::Scripted(completed),
                ),
            ),
        }
    }

    fn scripted_device_mismatch(
        device: ScriptedDeviceOwnerV1,
        detail: String,
    ) -> SdmaTransitionFailureV1<DirectionalSdmaDeviceOwnerV1> {
        SdmaTransitionFailureV1::ProcessTeardown {
            detail,
            custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Device(
                DirectionalSdmaDeviceOwnerV1::Scripted(device),
            )),
        }
    }

    fn scripted_pair_mismatch(
        pair: DirectionalSdmaPairOwnerV1,
        detail: String,
    ) -> SdmaTransitionFailureV1<DirectionalSdmaPairOwnerV1> {
        SdmaTransitionFailureV1::ProcessTeardown {
            detail,
            custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Pair(pair)),
        }
    }

    fn scripted_submission_mismatch(
        submission: ScriptedSubmissionOwnerV1,
        detail: String,
    ) -> DirectionalSdmaExecutionFailureV1 {
        DirectionalSdmaExecutionFailureV1::ProcessTeardown {
            detail,
            custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Submission(
                DirectionalSdmaSubmissionOwnerV1::Scripted(submission),
            )),
        }
    }

    fn scripted_completed_mismatch(
        completed: ScriptedCompletedOwnerV1,
        detail: String,
    ) -> SdmaTransitionFailureV1<DirectionalSdmaCompletedOwnerV1> {
        SdmaTransitionFailureV1::ProcessTeardown {
            detail,
            custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Completed(
                DirectionalSdmaCompletedOwnerV1::Scripted(completed),
            )),
        }
    }

    #[test]
    fn scripted_cpu_write_invalidates_authenticated_full_content() {
        let mut driver = ScriptedSdmaDriverV1::new([
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len: 8,
            },
            ScriptedSdmaStepV1::Write {
                offset: 3,
                byte_len: 1,
            },
        ]);
        let SdmaBufferOwnerV1::Scripted(mut host) = driver.test_host_owner(8) else {
            unreachable!()
        };
        let digest = driver
            .write_full_host_authenticated(&mut host, &[0x5a; 8])
            .unwrap();
        assert_eq!(
            host.full_content_certificate
                .as_ref()
                .map(|certificate| certificate.sha256),
            Some(digest)
        );
        driver.write_host(&mut host, 3, &[0xa5]).unwrap();
        assert!(host.full_content_certificate.is_none());
    }

    #[test]
    fn scripted_authenticated_write_preserves_max_linear_chunk_schedule() {
        let first_len = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize;
        let byte_len = first_len + 1;
        let mut driver = ScriptedSdmaDriverV1::new([
            ScriptedSdmaStepV1::Write {
                offset: 0,
                byte_len: first_len,
            },
            ScriptedSdmaStepV1::Write {
                offset: u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1),
                byte_len: 1,
            },
        ]);
        let SdmaBufferOwnerV1::Scripted(mut host) = driver.test_host_owner(byte_len) else {
            unreachable!()
        };
        let bytes = vec![0x6b; byte_len];
        let observed = driver
            .write_full_host_authenticated(&mut host, &bytes)
            .unwrap();
        assert_eq!(observed, <[u8; 32]>::from(Sha256::digest(&bytes)));
        assert!(driver.is_exhausted());
        assert_eq!(host.bytes, bytes);
        assert!(host.full_content_certificate.is_some());
    }

    #[test]
    fn scripted_request_direction_preserves_h2d_source_and_invalidates_d2h_destination() {
        for (direction, expect_certificate) in [
            (Gfx942PersistentSdmaDirectionV1::HostToDevice, true),
            (Gfx942PersistentSdmaDirectionV1::DeviceToHost, false),
        ] {
            let mut driver = ScriptedSdmaDriverV1::new([
                ScriptedSdmaStepV1::Write {
                    offset: 0,
                    byte_len: 8,
                },
                ScriptedSdmaStepV1::Submit {
                    direction,
                    host_offset: 0,
                    device_offset: 0,
                    copy_bytes: 8,
                    outcome: ScriptedFailureModeV1::Retryable,
                },
            ]);
            let SdmaBufferOwnerV1::Scripted(mut host) = driver.test_host_owner(8) else {
                unreachable!()
            };
            driver
                .write_full_host_authenticated(&mut host, &[0x5a; 8])
                .unwrap();
            let device = match driver.test_device_owner(8) {
                DirectionalSdmaDeviceOwnerV1::Scripted(device) => device,
                DirectionalSdmaDeviceOwnerV1::Native(_) => unreachable!(),
            };
            let request = DirectionalSdmaCopyRequestV1 {
                host_offset: 0,
                device_offset: 0,
                copy_bytes: 8,
            };
            let Err(SdmaTransitionFailureV1::Retryable { custody: pair, .. }) = driver.submit(
                device,
                host,
                direction,
                DirectionalSdmaRequestPlanV1::Single(request),
            ) else {
                panic!("scripted request must return recoverable prepublication custody")
            };
            let SdmaBufferOwnerV1::Scripted(host) = pair.host else {
                unreachable!()
            };
            assert_eq!(host.full_content_certificate.is_some(), expect_certificate);
        }
    }
}

#[cfg(test)]
#[allow(unused_imports)]
pub(super) use scripted::{
    ScriptedBufferKindV1, ScriptedBufferOwnerV1, ScriptedCompletedOwnerV1, ScriptedDeviceOwnerV1,
    ScriptedExecutionOutcomeV1, ScriptedFailureModeV1, ScriptedRecycleOutcomeV1,
    ScriptedSameDeviceCompletedOwnerV1, ScriptedSameDeviceExecutionOutcomeV1,
    ScriptedSameDeviceSubmissionOwnerV1, ScriptedSdmaDriverV1, ScriptedSdmaStepV1,
    ScriptedSubmissionOwnerV1, ScriptedTerminalCustodyV1,
};

#[cfg(test)]
fn scripted_mismatch_buffer(
    buffer: SdmaBufferOwnerV1,
    _operation: &'static str,
) -> SdmaTerminalCustodyV1 {
    SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Buffer(buffer))
}

#[cfg(test)]
fn scripted_mismatch_device(
    device: DirectionalSdmaDeviceOwnerV1,
    _operation: &'static str,
) -> SdmaTerminalCustodyV1 {
    SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Device(device))
}

#[cfg(test)]
fn scripted_mismatch_pair(
    device: DirectionalSdmaDeviceOwnerV1,
    host: SdmaBufferOwnerV1,
    _operation: &'static str,
) -> SdmaTerminalCustodyV1 {
    SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Pair(
        DirectionalSdmaPairOwnerV1 { device, host },
    ))
}

#[cfg(test)]
fn scripted_mismatch_submission(
    submission: DirectionalSdmaSubmissionOwnerV1,
    _operation: &'static str,
) -> SdmaTerminalCustodyV1 {
    SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Submission(submission))
}

#[cfg(test)]
fn scripted_mismatch_completed(
    completed: DirectionalSdmaCompletedOwnerV1,
    _operation: &'static str,
) -> SdmaTerminalCustodyV1 {
    SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Completed(completed))
}

#[cfg(test)]
fn scripted_mismatch_same_device_pair(
    pair: SameDeviceSdmaPairOwnerV1,
    _operation: &'static str,
) -> SdmaTerminalCustodyV1 {
    SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::SameDevicePair(pair))
}

#[cfg(test)]
fn scripted_mismatch_same_device_submission(
    submission: SameDeviceSdmaSubmissionOwnerV1,
    _operation: &'static str,
) -> SdmaTerminalCustodyV1 {
    SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::SameDeviceSubmission(submission))
}

#[cfg(test)]
fn scripted_mismatch_same_device_completed(
    completed: SameDeviceSdmaCompletedOwnerV1,
    _operation: &'static str,
) -> SdmaTerminalCustodyV1 {
    SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::SameDeviceCompleted(completed))
}

#[cfg(test)]
mod window_tests {
    use super::*;

    #[test]
    fn scripted_ready_promotion_does_not_rehash_host_bytes() {
        let source = include_str!("kfd_backend_sdma_seam.rs");
        let production = source
            .split_once("#[cfg(test)]\nmod window_tests")
            .expect("window tests remain after scripted implementation")
            .0;
        let promotion = production
            .rsplit_once("pub(super) fn promote_full_h2d_to_compute_ready(")
            .expect("scripted promotion method remains present")
            .1
            .split_once("pub(super) fn retire_same_device(")
            .expect("scripted retirement method follows promotion")
            .0;
        assert!(!promotion.contains("Sha256::digest"));
        assert!(promotion.contains(".full_content_certificate\n"));
    }

    #[test]
    fn ordered_window_validation_rejects_reorder_duplicates_and_noncanonical_packets() {
        let first = DirectionalSdmaCopyRequestV1 {
            host_offset: 11,
            device_offset: 29,
            copy_bytes: GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1,
        };
        let second = DirectionalSdmaCopyRequestV1 {
            host_offset: 11 + u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1),
            device_offset: 29 + u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1),
            copy_bytes: 1,
        };
        assert_eq!(
            validate_window_requests_v1(&[first, second]),
            Ok((11, 29, GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1))
        );
        assert!(validate_window_requests_v1(&[second, first]).is_err());
        assert!(validate_window_requests_v1(&[first, first]).is_err());
        assert!(
            validate_window_requests_v1(&[
                DirectionalSdmaCopyRequestV1 {
                    copy_bytes: 1,
                    ..first
                },
                second
            ])
            .is_err()
        );

        let first = SameDeviceSdmaCopyRequestV1 {
            source_offset: 7,
            destination_offset: 41,
            copy_bytes: GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1,
        };
        let second = SameDeviceSdmaCopyRequestV1 {
            source_offset: 7 + u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1),
            destination_offset: 41 + u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1),
            copy_bytes: 1,
        };
        assert_eq!(
            validate_same_device_window_requests_v1(&[first, second]),
            Ok((7, 41, GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1))
        );
        assert!(validate_same_device_window_requests_v1(&[second, first]).is_err());
        assert!(validate_same_device_window_requests_v1(&[first, first]).is_err());
        assert!(
            validate_same_device_window_requests_v1(&[
                SameDeviceSdmaCopyRequestV1 {
                    copy_bytes: 1,
                    ..first
                },
                second
            ])
            .is_err()
        );
    }

    #[test]
    fn one_packet_window_rejection_returns_the_exact_pair_without_consuming_publication() {
        let request = DirectionalSdmaCopyRequestV1 {
            host_offset: 0,
            device_offset: 0,
            copy_bytes: 8,
        };
        let direction = Gfx942PersistentSdmaDirectionV1::HostToDevice;
        let mut driver = ScriptedSdmaDriverV1::new([
            ScriptedSdmaStepV1::Submit {
                direction,
                host_offset: 0,
                device_offset: 0,
                copy_bytes: 8,
                outcome: ScriptedFailureModeV1::Retryable,
            },
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]);
        let pair = DirectionalSdmaPairOwnerV1 {
            device: driver.test_device_owner(8),
            host: driver.test_host_owner(8),
        };
        let mut ops = DirectionalSdmaOpsV1::Scripted(&mut driver);
        let pair = match ops.submit(
            pair,
            direction,
            DirectionalSdmaRequestPlanV1::Window(vec![request].into_boxed_slice()),
        ) {
            Err(SdmaTransitionFailureV1::Retryable { custody, .. }) => custody,
            _ => panic!("one packet must reject window custody before publication"),
        };
        let pair = match ops.submit(
            pair,
            direction,
            DirectionalSdmaRequestPlanV1::Single(request),
        ) {
            Err(SdmaTransitionFailureV1::Retryable { custody, .. }) => custody,
            _ => panic!("the restored pair must reach the still-pending single publication"),
        };
        let Ok(device) = ops.demote(pair.device) else {
            panic!("restored device custody must demote")
        };
        assert!(ops.recycle(device).is_ok());
        assert!(ops.recycle(pair.host).is_ok());
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 0);
        assert_eq!(driver.unexpected_drops(), 0);
    }

    #[test]
    fn same_device_wait_timeout_returns_the_exact_pair_for_later_completion() {
        let request = SameDeviceSdmaCopyRequestV1 {
            source_offset: 0,
            destination_offset: 0,
            copy_bytes: 8,
        };
        let mut driver = ScriptedSdmaDriverV1::new([
            ScriptedSdmaStepV1::SubmitSameDeviceWindow {
                requests: vec![request],
                outcome: ScriptedFailureModeV1::Success,
            },
            ScriptedSdmaStepV1::WaitSameDevice(ScriptedSameDeviceExecutionOutcomeV1::Pending),
            ScriptedSdmaStepV1::WaitSameDevice(ScriptedSameDeviceExecutionOutcomeV1::Completed {
                copy_bytes: None,
                requests: None,
                swap_allocations: false,
            }),
            ScriptedSdmaStepV1::RetireSameDevice(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]);
        let pair = SameDeviceSdmaPairOwnerV1 {
            source: driver.test_device_owner(8),
            destination: driver.test_device_owner(8),
        };
        let mut ops = DirectionalSdmaOpsV1::Scripted(&mut driver);
        let Ok(submission) = ops.submit_same_device(pair, vec![request].into_boxed_slice()) else {
            panic!("scripted same-device publication must succeed");
        };
        let submission = match ops.wait_same_device(submission, Duration::ZERO) {
            Err(SameDeviceSdmaExecutionFailureV1::Retryable { submission, .. }) => submission,
            _ => panic!("scripted same-device timeout must return exact pending custody"),
        };
        let Ok(completed) = ops.wait_same_device(submission, Duration::from_millis(1)) else {
            panic!("second scripted same-device wait must complete");
        };
        let Ok(pair) = ops.retire_same_device(completed) else {
            panic!("scripted paired retirement must succeed");
        };
        for device in [pair.source, pair.destination] {
            let Ok(buffer) = ops.demote(device) else {
                panic!("scripted device demotion must succeed");
            };
            assert!(ops.recycle(buffer).is_ok());
        }
        assert!(driver.is_exhausted());
        assert_eq!(driver.live_owner_count(), 0);
        assert_eq!(driver.unexpected_drops(), 0);
    }
}
