//! Typed data and its metadata stay rooted through one-shot native disposal.

use super::*;
use crate::queue::dispatch_binding::{
    DispatchDataAuthorityV1, DispatchDataInputStorageV1, Gfx942FixedDispatchDataLayoutV1,
    Gfx942FixedDispatchDataV1, Gfx942FixedDispatchStorageIdentityV1,
};
use crate::sdma::{
    Gfx942SdmaBufferCleanupMetadataV1, Gfx942SdmaBufferStorageV1, Gfx942SdmaBufferV1,
};
use control_cleanup::{CleanupStageV1, NativeDisposalProgressV1};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use transitions::NativeTransitionProgressV1;

enum InputV1 {
    Fixed(Gfx942FixedDispatchDataV1),
    Authority(DispatchDataAuthorityV1),
    Sdma(Gfx942SdmaBufferV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DataCleanupMetadataV1 {
    Fixed {
        identity: Gfx942FixedDispatchStorageIdentityV1,
        layout: Gfx942FixedDispatchDataLayoutV1,
        fully_initialized: bool,
        initialized_content: Option<Gfx942DeviceContentDescriptorV1>,
    },
    HostDispatch(SharedGttMappedResourceFactsV1),
    DeviceDispatch(Gfx942DeviceMemoryDispatchFactsV1),
    Sdma(Gfx942SdmaBufferCleanupMetadataV1),
}

#[cfg(test)]
impl InputV1 {
    fn metadata(&self) -> DataCleanupMetadataV1 {
        match self {
            Self::Fixed(data) => DataCleanupMetadataV1::Fixed {
                identity: data.storage_identity(),
                layout: data.layout(),
                fully_initialized: data.is_fully_initialized(),
                initialized_content: data.initialized_content(),
            },
            Self::Authority(DispatchDataAuthorityV1::HostVisible(authority)) => {
                DataCleanupMetadataV1::HostDispatch(*authority.facts())
            }
            Self::Authority(DispatchDataAuthorityV1::Device(authority)) => {
                DataCleanupMetadataV1::DeviceDispatch(*authority.facts())
            }
            Self::Sdma(buffer) => DataCleanupMetadataV1::Sdma(buffer.cleanup_metadata()),
        }
    }
}

// Consumes the lease without retaining any operation that can revive authority.
#[allow(dead_code)]
struct DeviceReceiptV1 {
    identity: Gfx942DeviceMemoryIdentityV1,
    layout: Gfx942DeviceMemoryLayoutV1,
}

impl DeviceReceiptV1 {
    fn new(lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>) -> Self {
        Self {
            identity: lease.storage_identity(),
            layout: lease.layout(),
        }
    }
}

enum OwnerV1 {
    Input(InputV1),
    Host(ControlCleanupCustodyV1),
    MappedDevice(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>),
    UnmappedDevice(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>),
    DisposedDevice { _receipt: DeviceReceiptV1 },
}

pub(crate) struct DataCleanupCustodyV1 {
    owner: Option<OwnerV1>,
    metadata: Option<DataCleanupMetadataV1>,
    started: bool,
    failed: bool,
    complete: bool,
    stage: CleanupStageV1,
    unmap: NativeTransitionProgressV1,
    disposal: NativeDisposalProgressV1,
}

impl DataCleanupCustodyV1 {
    pub(crate) fn new(data: Gfx942FixedDispatchDataV1) -> Self {
        Self::input(InputV1::Fixed(data))
    }

    pub(crate) fn from_authority(data: DispatchDataAuthorityV1) -> Self {
        Self::input(InputV1::Authority(data))
    }

    pub(crate) fn from_sdma(buffer: Gfx942SdmaBufferV1) -> Self {
        Self::input(InputV1::Sdma(buffer))
    }

    fn input(input: InputV1) -> Self {
        Self {
            owner: Some(OwnerV1::Input(input)),
            metadata: None,
            started: false,
            failed: false,
            complete: false,
            stage: CleanupStageV1::UnmapPreflight,
            unmap: NativeTransitionProgressV1::default(),
            disposal: NativeDisposalProgressV1::default(),
        }
    }

    fn prepare(&mut self) {
        let Some(OwnerV1::Input(input)) = self.owner.take() else {
            unreachable!("unstarted data cleanup retains its original input")
        };
        // Decomposition is infallible and invokes neither allocation nor callbacks.
        let storage = match input {
            InputV1::Fixed(data) => {
                self.metadata = Some(DataCleanupMetadataV1::Fixed {
                    identity: data.storage_identity(),
                    layout: data.layout(),
                    fully_initialized: data.is_fully_initialized(),
                    initialized_content: data.initialized_content(),
                });
                data.into_parts().storage
            }
            InputV1::Authority(DispatchDataAuthorityV1::Device(authority)) => {
                let Gfx942DeviceMemoryDispatchAuthorityV1 { lease, facts } = authority;
                self.metadata = Some(DataCleanupMetadataV1::DeviceDispatch(facts));
                DispatchDataInputStorageV1::Device(lease)
            }
            InputV1::Authority(DispatchDataAuthorityV1::HostVisible(authority)) => {
                let SharedGttQueueResourceAuthorityV1 { token, facts, .. } = authority;
                self.metadata = Some(DataCleanupMetadataV1::HostDispatch(facts));
                DispatchDataInputStorageV1::HostVisible(token)
            }
            InputV1::Sdma(buffer) => {
                let (storage, metadata) = buffer.into_cleanup_parts();
                self.metadata = Some(DataCleanupMetadataV1::Sdma(metadata));
                match storage {
                    Gfx942SdmaBufferStorageV1::Host(token) => {
                        DispatchDataInputStorageV1::HostVisible(token)
                    }
                    Gfx942SdmaBufferStorageV1::Device(lease) => {
                        DispatchDataInputStorageV1::Device(lease)
                    }
                }
            }
        };
        self.owner = Some(match storage {
            DispatchDataInputStorageV1::Device(lease) => OwnerV1::MappedDevice(lease),
            DispatchDataInputStorageV1::HostVisible(token) => {
                OwnerV1::Host(ControlCleanupCustodyV1::host_data(token))
            }
        });
    }

    fn retain_disposed_receipt(&mut self) {
        if self.disposal.native_disposed
            && !matches!(self.owner, Some(OwnerV1::DisposedDevice { .. }))
        {
            let Some(OwnerV1::UnmappedDevice(lease)) = self.owner.take() else {
                unreachable!("only an unmapped device lease can finish disposal")
            };
            self.owner = Some(OwnerV1::DisposedDevice {
                _receipt: DeviceReceiptV1::new(lease),
            });
        }
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.complete
            && !self.failed
            && match &self.owner {
                Some(OwnerV1::Host(custody)) => custody.is_complete(),
                Some(OwnerV1::DisposedDevice { .. }) => self.disposal.native_disposed,
                _ => false,
            }
    }
}

pub(crate) trait DispatchDataReleaseV1 {
    fn release_data(
        &mut self,
        custody: &mut DataCleanupCustodyV1,
    ) -> Result<(), MemorySessionError>;
}

impl DispatchDataReleaseV1 for SharedGttMemorySessionV1 {
    fn release_data(
        &mut self,
        custody: &mut DataCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        release_v1(
            &mut self.engine,
            &mut control_cleanup::ProjectionV1::new(&mut self.foundation, self.vm),
            custody,
            crate::queue_linux::permanently_poison_process_global_kfd_runtime_gate_v1,
        )
    }
}

pub(super) fn release_v1<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    projection: &mut control_cleanup::ProjectionV1<'_>,
    custody: &mut DataCleanupCustodyV1,
    process_poison: impl FnMut(),
) -> Result<(), MemorySessionError> {
    if custody.started {
        return Err(MemorySessionError::InvalidAllocationAuthority);
    }
    custody.started = true;
    let result = catch_unwind(AssertUnwindSafe(|| {
        custody.prepare();
        if let Some(OwnerV1::Host(host)) = &mut custody.owner {
            control_cleanup::release_v1(engine, projection, host, process_poison)?;
            if !host.is_complete() {
                return Err(MemorySessionError::InvalidAllocationAuthority);
            }
        } else {
            custody.stage = CleanupStageV1::NativeUnmap;
            let Some(OwnerV1::MappedDevice(lease)) = &custody.owner else {
                unreachable!("prepared device cleanup retains its mapped lease")
            };
            engine.unmap_device_memory_borrowed(lease, &mut custody.unmap)?;
            let Some(OwnerV1::MappedDevice(lease)) = custody.owner.take() else {
                unreachable!("borrowed unmap preserves the device owner")
            };
            custody.owner = Some(OwnerV1::UnmappedDevice(lease.retag()));
            custody.stage = CleanupStageV1::NativeRelease;
            let Some(OwnerV1::UnmappedDevice(lease)) = &custody.owner else {
                unreachable!("successful unmap retains an unmapped device owner")
            };
            engine.release_device_memory_borrowed(lease, &mut custody.disposal)?;
            custody.retain_disposed_receipt();
        }
        custody.stage = CleanupStageV1::Complete;
        custody.complete = true;
        Ok(())
    }));
    // Disposal can finish before closing currentness/accounting returns or unwinds.
    custody.retain_disposed_receipt();
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => {
            custody.failed = true;
            if custody.unmap.attempted {
                engine.quarantine(error)
            } else {
                Err(error)
            }
        }
        Err(payload) => {
            custody.failed = true;
            engine.phase = SharedMemorySessionPhaseV1::Quarantined;
            resume_unwind(payload)
        }
    }
}

pub(super) fn release_owned_with_v1(
    mut custody: DataCleanupCustodyV1,
    memory: &mut impl DispatchDataReleaseV1,
    retain: impl FnOnce(DataCleanupCustodyV1),
) -> Result<(), MemorySessionError> {
    let result = catch_unwind(AssertUnwindSafe(|| {
        memory.release_data(&mut custody)?;
        if !custody.is_complete() {
            return Err(MemorySessionError::InvalidAllocationAuthority);
        }
        Ok(())
    }));
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => {
            retain(custody);
            Err(error)
        }
        Err(payload) => {
            retain(custody);
            resume_unwind(payload)
        }
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DataCleanupObservationV1 {
    pub(crate) metadata: DataCleanupMetadataV1,
    pub(crate) owner: &'static str,
    pub(crate) host: Option<ControlCleanupObservationV1>,
    pub(crate) device: Option<(Gfx942DeviceMemoryIdentityV1, Gfx942DeviceMemoryLayoutV1)>,
    pub(crate) stage: CleanupStageV1,
    pub(crate) started: bool,
    pub(crate) failed: bool,
    pub(crate) complete: bool,
    pub(crate) unmap: (bool, Option<bool>, Option<u32>),
    pub(crate) disposal: [(bool, Option<bool>); 3],
    pub(crate) native_disposed: bool,
}

#[cfg(test)]
impl DataCleanupCustodyV1 {
    pub(crate) fn sdma_metadata(&self) -> Option<&Gfx942SdmaBufferCleanupMetadataV1> {
        match self.metadata.as_ref()? {
            DataCleanupMetadataV1::Sdma(metadata) => Some(metadata),
            _ => None,
        }
    }

    pub(crate) fn observation(&self) -> DataCleanupObservationV1 {
        let (owner, host, device) = match self.owner.as_ref().unwrap() {
            OwnerV1::Input(_) => ("Original", None, None),
            OwnerV1::Host(host) => {
                let host = host.observation();
                (host.owner, Some(host), None)
            }
            OwnerV1::MappedDevice(lease) => (
                "Mapped",
                None,
                Some((lease.storage_identity(), lease.layout())),
            ),
            OwnerV1::UnmappedDevice(lease) => (
                "Unmapped",
                None,
                Some((lease.storage_identity(), lease.layout())),
            ),
            OwnerV1::DisposedDevice { _receipt: receipt } => (
                "NativeDisposed",
                None,
                Some((receipt.identity, receipt.layout)),
            ),
        };
        DataCleanupObservationV1 {
            metadata: match self.owner.as_ref().unwrap() {
                OwnerV1::Input(input) => input.metadata(),
                _ => self
                    .metadata
                    .clone()
                    .expect("retained original data metadata"),
            },
            owner,
            device,
            stage: host.as_ref().map_or(self.stage, |h| h.stage),
            started: self.started,
            failed: self.failed,
            complete: self.is_complete(),
            unmap: host.as_ref().map_or(
                (
                    self.unmap.attempted,
                    self.unmap.returned_success,
                    self.unmap.returned_map_prefix,
                ),
                |h| h.unmap,
            ),
            disposal: host.as_ref().map_or(
                [
                    self.disposal.cpu_unmap,
                    self.disposal.free,
                    self.disposal.va_release,
                ]
                .map(|p| (p.attempted, p.returned_success)),
                |h| h.disposal,
            ),
            native_disposed: host
                .as_ref()
                .map_or(self.disposal.native_disposed, |h| h.native_disposed),
            host,
        }
    }
}
