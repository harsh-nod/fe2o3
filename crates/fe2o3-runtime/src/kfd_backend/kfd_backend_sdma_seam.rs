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

use super::initialized_storage::{InitializedStorageAttemptV1, InitializedStorageOwnerV1};
#[cfg(test)]
use fe2o3_kfd::Gfx942PersistentComputeStorageIneligibilityV1;
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
    Gfx942DirectionalPersistentSdmaSubmissionV1,
    Gfx942DirectionalPersistentSdmaSynchronousExecutionFailureV1,
    Gfx942DirectionalPersistentSdmaTerminalCustodyV1,
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
    Gfx942SameDevicePersistentSdmaWindowTerminalCustodyV1, Gfx942SdmaAllocationDispositionV1,
    Gfx942SdmaAllocationFailureV1, Gfx942SdmaBufferV1, Gfx942SdmaErrorV1,
};
use fe2o3_kfd::{
    Gfx942PersistentComputeStorageAttemptV1, Gfx942PersistentComputeStoragePromotionCustodyV1,
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

#[cfg(test)]
impl SdmaBufferOwnerV1 {
    pub(crate) fn scripted_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Scripted(host) => Some(&host.bytes),
            Self::Native(_) => None,
        }
    }

    pub(crate) fn scripted_bytes_mut(&mut self) -> Option<&mut [u8]> {
        match self {
            Self::Scripted(host) => Some(&mut host.bytes),
            Self::Native(_) => None,
        }
    }
}

#[derive(Debug)]
pub(super) enum DirectionalSdmaDeviceOwnerV1 {
    Native(Gfx942DirectionalQueuePersistentAllocationV1),
    #[cfg(test)]
    Scripted(ScriptedDeviceOwnerV1),
}

#[cfg(test)]
impl DirectionalSdmaDeviceOwnerV1 {
    pub(crate) fn scripted_owner_id(&self) -> Option<u64> {
        match self {
            Self::Scripted(device) => Some(device.owner_id()),
            Self::Native(_) => None,
        }
    }

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

    #[cfg(test)]
    pub(crate) fn scripted_owner_id(&self) -> Option<u64> {
        match self {
            Self::Scripted { device, .. } => Some(device.owner_id()),
            Self::Native(_) => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn scripted_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Scripted { device, .. } => Some(&device.bytes),
            Self::Native(_) => None,
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
    StoragePromotion(fe2o3_kfd::Gfx942PersistentComputeStoragePromotionTerminalCustodyV1),
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

pub(super) enum SdmaTransitionFailureV1<R, D = String> {
    Retryable {
        detail: D,
        custody: R,
    },
    ProcessTeardown {
        detail: D,
        custody: SdmaTerminalCustodyV1,
    },
}

#[derive(Debug)]
pub(super) enum SdmaOwnerDiagnosticV1 {
    Native(ComputeAqlQueueSessionErrorV1),
    Message(String),
    Static(&'static str),
    UnexpectedRetryable(ComputeAqlQueueSessionErrorV1),
    #[cfg(test)]
    Scripted(String),
    #[cfg(test)]
    UnexpectedScriptedRetryable(String),
}

impl core::fmt::Display for SdmaOwnerDiagnosticV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Native(error) => core::fmt::Display::fmt(error, f),
            Self::Message(detail) => f.write_str(detail),
            Self::Static(detail) => f.write_str(detail),
            Self::UnexpectedRetryable(error) => write!(
                f,
                "directional SDMA synchronous wait returned non-timeout retryable custody: {error}"
            ),
            #[cfg(test)]
            Self::Scripted(detail) => f.write_str(detail),
            #[cfg(test)]
            Self::UnexpectedScriptedRetryable(detail) => write!(
                f,
                "directional SDMA synchronous wait returned non-timeout retryable custody: {detail}"
            ),
        }
    }
}

#[derive(Debug)]
pub(crate) struct SdmaAllocationFailureV1 {
    pub(super) detail: SdmaOwnerDiagnosticV1,
    pub(super) disposition: Gfx942SdmaAllocationDispositionV1,
}

impl From<Gfx942SdmaAllocationFailureV1> for SdmaAllocationFailureV1 {
    fn from(failure: Gfx942SdmaAllocationFailureV1) -> Self {
        Self {
            disposition: failure.disposition(),
            detail: SdmaOwnerDiagnosticV1::Native(failure.into_error()),
        }
    }
}

#[cfg(test)]
impl SdmaAllocationFailureV1 {
    fn scripted_protocol(detail: String) -> Self {
        Self {
            detail: SdmaOwnerDiagnosticV1::Scripted(detail),
            disposition: Gfx942SdmaAllocationDispositionV1::ProcessTeardown,
        }
    }
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

pub(super) enum DirectionalSdmaSynchronousExecutionFailureV1 {
    RetryableBeforePublication {
        detail: SdmaOwnerDiagnosticV1,
        pair: DirectionalSdmaPairOwnerV1,
    },
    RetryableTimeout {
        detail: SdmaOwnerDiagnosticV1,
        submission: DirectionalSdmaSubmissionOwnerV1,
    },
    ProcessTeardown {
        detail: SdmaOwnerDiagnosticV1,
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
        detail: SdmaOwnerDiagnosticV1,
        buffer: SdmaBufferOwnerV1,
    },
    Ambiguous {
        detail: SdmaOwnerDiagnosticV1,
    },
    #[cfg(test)]
    ProcessTeardown {
        detail: SdmaOwnerDiagnosticV1,
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

pub(super) enum DirectionalSdmaWaitV1 {
    Timeout(DirectionalSdmaSubmissionOwnerV1),
    Completed(DirectionalSdmaCompletedOwnerV1),
}

pub(super) enum SameDeviceSdmaWaitV1 {
    Timeout(SameDeviceSdmaSubmissionOwnerV1),
    Completed(SameDeviceSdmaCompletedOwnerV1),
}

fn is_exact_sdma_timeout_v1(error: &ComputeAqlQueueSessionErrorV1) -> bool {
    matches!(
        error,
        ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Timeout)
    )
}

pub(super) enum DirectionalSdmaOpsV1<'a> {
    Native(&'a mut ComputeAqlQueueSessionV1),
    #[cfg(test)]
    Scripted(&'a mut ScriptedSdmaDriverV1),
}

pub(super) fn retire_native_directional_completed_v1(
    (device, host, frontier): fe2o3_kfd::Gfx942PersistentComputeReadyPartsV1,
) -> Result<
    DirectionalSdmaPairOwnerV1,
    SdmaTransitionFailureV1<DirectionalSdmaCompletedOwnerV1, SdmaOwnerDiagnosticV1>,
> {
    match device.retire_settled_frontier_v1(frontier) {
        Ok(device) => Ok(DirectionalSdmaPairOwnerV1 {
            device: DirectionalSdmaDeviceOwnerV1::Native(device),
            host: SdmaBufferOwnerV1::Native(host),
        }),
        Err(failure) => Err(SdmaTransitionFailureV1::ProcessTeardown {
            detail: SdmaOwnerDiagnosticV1::Static("frontier retirement failed"),
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
                            detail: SdmaOwnerDiagnosticV1::Native(error),
                            buffer: SdmaBufferOwnerV1::Native(buffer),
                        },
                        None => SdmaRecycleFailureV1::Ambiguous {
                            detail: SdmaOwnerDiagnosticV1::Native(error),
                        },
                    }
                })
            }
            #[cfg(test)]
            (Self::Scripted(driver), SdmaBufferOwnerV1::Scripted(buffer)) => driver.recycle(buffer),
            #[cfg(test)]
            (_, buffer) => Err(SdmaRecycleFailureV1::ProcessTeardown {
                detail: SdmaOwnerDiagnosticV1::Scripted(
                    "directional SDMA owner/driver mismatch during recycle".to_owned(),
                ),
                custody: scripted_mismatch_buffer(buffer, "recycle"),
            }),
        }
    }
}

#[cfg(test)]
mod scripted;

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
mod window_tests;

mod directional;

mod observation;

mod same_device;
