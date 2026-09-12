//! Original input custody through device initialization, including final GPU map.

use super::*;
use transitions::NativeTransitionProgressV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DeviceInitializationStageV1 {
    Source,
    Admission,
    Allocate,
    CpuCurrentness,
    CpuMap,
    CpuPrepare,
    CpuWrite,
    CpuVerify,
    CpuUnmap,
    CpuClosingCurrentness,
    GpuMap,
    Complete,
}

pub(super) enum InitializationSourceV1 {
    Unvalidated(Box<[u8]>, Gfx942DeviceContentDescriptorV1),
    Validated(ValidatedInitializationSourceV1),
    Repeated(Gfx942RepeatedByteContentV1),
}

pub(super) enum InitializationLeaseV1 {
    None,
    Unmapped(Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>),
    Complete(Gfx942InitializedDeviceMemoryV1),
}

pub(super) struct DeviceInitializationCustodyV1 {
    pub(super) source: Option<InitializationSourceV1>,
    pub(super) lease: InitializationLeaseV1,
    pub(super) stage: DeviceInitializationStageV1,
    pub(super) native_started: bool,
    pub(super) input_admitted: bool,
    pub(super) progress: NativeTransitionProgressV1,
    pub(super) failed: bool,
    started: bool,
}

// Reserve once before backend effects; retaining an admitted failure cannot allocate.
pub(super) struct TerminalInitializationSlotV1 {
    roots: Vec<DeviceInitializationCustodyV1>,
}

impl TerminalInitializationSlotV1 {
    pub(super) fn new() -> Result<Self, MemorySessionError> {
        let mut roots = Vec::new();
        roots
            .try_reserve_exact(1)
            .map_err(|_| MemorySessionError::DeviceMemoryAllocationCapacity { maximum: 1 })?;
        Ok(Self { roots })
    }

    pub(super) fn as_ref(&self) -> Option<&DeviceInitializationCustodyV1> {
        self.roots.first()
    }

    pub(super) fn is_some(&self) -> bool {
        self.as_ref().is_some()
    }

    #[cfg(test)]
    pub(super) fn is_none(&self) -> bool {
        self.as_ref().is_none()
    }

    pub(super) fn retain(&mut self, root: DeviceInitializationCustodyV1) {
        assert!(self.roots.is_empty(), "occupied initialization custody");
        assert!(
            self.roots.capacity() >= 1,
            "reserved initialization custody"
        );
        self.roots.push(root);
    }

    #[cfg(test)]
    pub(super) fn storage_for_test(&self) -> (usize, usize) {
        (self.roots.as_ptr() as usize, self.roots.capacity())
    }
}

#[derive(Clone, Copy)]
pub(super) struct AllocationRequestV1 {
    pub(super) device: DeviceKeyV1,
    pub(super) vm: VmKeyV1,
    pub(super) alignment: u64,
}

impl DeviceInitializationCustodyV1 {
    pub(super) fn new(source: InitializationSourceV1, lease: InitializationLeaseV1) -> Self {
        Self {
            source: Some(source),
            lease,
            stage: DeviceInitializationStageV1::Source,
            native_started: false,
            input_admitted: false,
            progress: NativeTransitionProgressV1::default(),
            failed: false,
            started: false,
        }
    }

    #[cfg(test)]
    pub(super) fn from_validated(
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>,
        source: ValidatedInitializationSourceV1,
    ) -> Self {
        Self::new(
            InitializationSourceV1::Validated(source),
            InitializationLeaseV1::Unmapped(lease),
        )
    }

    #[cfg(test)]
    pub(super) fn from_repeated_lease(
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>,
        source: Gfx942RepeatedByteContentV1,
    ) -> Self {
        Self::new(
            InitializationSourceV1::Repeated(source),
            InitializationLeaseV1::Unmapped(lease),
        )
    }

    fn source_metadata(
        &mut self,
    ) -> Result<(usize, Gfx942DeviceContentDescriptorV1), MemorySessionError> {
        if let Some(InitializationSourceV1::Unvalidated(bytes, content)) = &self.source {
            let byte_len = validate_initialization_source_bytes(bytes, *content)?;
            let Some(InitializationSourceV1::Unvalidated(bytes, content)) = self.source.take()
            else {
                unreachable!("borrowed original source");
            };
            self.source = Some(InitializationSourceV1::Validated(
                ValidatedInitializationSourceV1 {
                    bytes,
                    byte_len,
                    content,
                },
            ));
        }
        match self
            .source
            .as_ref()
            .expect("retained initialization source")
        {
            InitializationSourceV1::Validated(source) => {
                if u64::try_from(source.bytes().len()) != Ok(source.byte_len())
                    || source.byte_len() != source.content().byte_len()
                {
                    return Err(MemorySessionError::DeviceContentMismatch);
                }
                Ok((source.bytes().len(), source.content()))
            }
            InitializationSourceV1::Repeated(recipe) => Ok((
                usize::try_from(recipe.content().byte_len())
                    .map_err(|_| MemorySessionError::SizeOverflow)?,
                recipe.content(),
            )),
            InitializationSourceV1::Unvalidated(..) => unreachable!("validated original source"),
        }
    }

    fn admitted(&self) -> bool {
        self.native_started || self.input_admitted
    }

    // The outer live-lane owner can keep this root through its own retake.
    // This core never moves an owner into the standalone engine terminal slot.
    pub(super) fn prepare_in_place<B: MemoryBackend>(
        &mut self,
        engine: &mut SharedMemoryEngine<B>,
        request: Option<AllocationRequestV1>,
    ) -> Result<(), MemorySessionError> {
        if self.started {
            return Err(MemorySessionError::InvalidDeviceMemoryAuthority);
        }
        self.started = true;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.prepare_inner(engine, request)
        }));
        if !matches!(result, Ok(Ok(()))) {
            self.failed = true;
            if self.admitted() {
                engine.phase = SharedMemorySessionPhaseV1::Quarantined;
            }
        }
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn prepare_inner<B: MemoryBackend>(
        &mut self,
        engine: &mut SharedMemoryEngine<B>,
        request: Option<AllocationRequestV1>,
    ) -> Result<(), MemorySessionError> {
        // An existing native input needs custody even when source metadata rejects.
        // Public source validation still precedes allocation admission.
        if let InitializationLeaseV1::Unmapped(lease) = &self.lease {
            validate_unmapped_owner(engine, lease)?;
            self.input_admitted = true;
        }
        let (expected_len, content) = self.source_metadata()?;
        self.stage = DeviceInitializationStageV1::Admission;
        engine.require_active()?;
        if engine.terminal_device_initialization.is_some() {
            return engine.quarantine(MemorySessionError::SharedSessionQuarantined);
        }
        match (&self.lease, request) {
            (InitializationLeaseV1::None, Some(request)) => {
                self.stage = DeviceInitializationStageV1::Allocate;
                let lease = engine.with_device_backing_unwind_quarantine(|engine| {
                    engine.allocate_device_memory_with_flags_inner(
                        request.device,
                        request.vm,
                        content.byte_len(),
                        request.alignment,
                        KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
                        &mut self.native_started,
                    )
                })?;
                self.lease = InitializationLeaseV1::Unmapped(lease);
            }
            (InitializationLeaseV1::Unmapped(_), None) => {}
            _ => return Err(MemorySessionError::InvalidDeviceMemoryAuthority),
        }
        let InitializationLeaseV1::Unmapped(lease) = &self.lease else {
            unreachable!("retained unmapped initialization input");
        };
        if lease.layout.uapi_flags != KFD_ALLOC_MEMORY_FLAGS_DEVICE_LOCAL_PUBLIC
            || lease.layout.requested_bytes != content.byte_len()
        {
            return engine.quarantine(MemorySessionError::DeviceContentMismatch);
        }
        match self
            .source
            .as_ref()
            .expect("retained initialization source")
        {
            InitializationSourceV1::Validated(source) => engine
                .initialize_public_device_memory_after_preflight(
                    lease,
                    expected_len,
                    |mapped| copy_public_device_mapping(mapped, source.bytes()),
                    Some(source.bytes()),
                    &mut self.stage,
                )?,
            InitializationSourceV1::Repeated(recipe) => engine
                .initialize_public_device_memory_after_preflight(
                    lease,
                    expected_len,
                    |mapped| fill_public_device_mapping(mapped, recipe.repeated_byte()),
                    None,
                    &mut self.stage,
                )?,
            InitializationSourceV1::Unvalidated(..) => {
                unreachable!("authenticated initialization source")
            }
        }
        self.stage = DeviceInitializationStageV1::GpuMap;
        engine.map_device_memory_borrowed(lease, &mut self.progress)?;
        let InitializationLeaseV1::Unmapped(lease) =
            std::mem::replace(&mut self.lease, InitializationLeaseV1::None)
        else {
            unreachable!("borrowed original initialization lease");
        };
        self.lease = InitializationLeaseV1::Complete(Gfx942InitializedDeviceMemoryV1 {
            lease: lease.retag(),
            content,
        });
        self.stage = DeviceInitializationStageV1::Complete;
        Ok(())
    }

    pub(super) fn take_complete(
        &mut self,
    ) -> Result<Gfx942InitializedDeviceMemoryV1, MemorySessionError> {
        if self.failed
            || self.stage != DeviceInitializationStageV1::Complete
            || !matches!(self.lease, InitializationLeaseV1::Complete(_))
        {
            return Err(MemorySessionError::InvalidDeviceMemoryAuthority);
        }
        let InitializationLeaseV1::Complete(output) =
            std::mem::replace(&mut self.lease, InitializationLeaseV1::None)
        else {
            unreachable!("checked complete initialization");
        };
        Ok(output)
    }

    fn retain_failure<B: MemoryBackend>(self, engine: &mut SharedMemoryEngine<B>) {
        if self.admitted() {
            engine.phase = SharedMemorySessionPhaseV1::Quarantined;
            engine.terminal_device_initialization.retain(self);
        }
    }
}

fn validate_unmapped_owner<B: MemoryBackend>(
    engine: &SharedMemoryEngine<B>,
    lease: &Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1>,
) -> Result<(), MemorySessionError> {
    let index = engine.device_memory_index(lease, DeviceMemoryPhaseV1::Unmapped)?;
    let record = &engine.device_memory[index];
    let exact_charge = match (&engine.device_backing_account, &record.backing_charge) {
        (None, None) => true,
        (Some(account), Some(charge)) => charge.matches(
            account,
            engine.session_id,
            record.device,
            record.vm,
            record.id,
            record.generation,
            record.layout,
        ),
        _ => false,
    };
    if record.id == 0
        || record.generation == 0
        || record.vm.device != record.device
        || record.reservation.is_none()
        || record.handle.is_none()
        || record.mapping.is_some()
        || record.free_attempted
        || !exact_charge
    {
        return Err(MemorySessionError::InvalidDeviceMemoryAuthority);
    }
    Ok(())
}

pub(super) fn finish_v1<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    mut custody: DeviceInitializationCustodyV1,
    request: Option<AllocationRequestV1>,
) -> Result<Gfx942InitializedDeviceMemoryV1, MemorySessionError> {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        custody.prepare_in_place(engine, request)?;
        custody.take_complete()
    }));
    match result {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => {
            custody.retain_failure(engine);
            Err(error)
        }
        Err(payload) => {
            custody.retain_failure(engine);
            std::panic::resume_unwind(payload)
        }
    }
}

pub(super) fn initialize_bytes_v1<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    device: DeviceKeyV1,
    vm: VmKeyV1,
    bytes: Box<[u8]>,
    alignment: u64,
    content: Gfx942DeviceContentDescriptorV1,
) -> Result<Gfx942InitializedDeviceMemoryV1, MemorySessionError> {
    finish_v1(
        engine,
        DeviceInitializationCustodyV1::new(
            InitializationSourceV1::Unvalidated(bytes, content),
            InitializationLeaseV1::None,
        ),
        Some(AllocationRequestV1 {
            device,
            vm,
            alignment,
        }),
    )
}

pub(super) fn initialize_repeated_v1<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    device: DeviceKeyV1,
    vm: VmKeyV1,
    recipe: Gfx942RepeatedByteContentV1,
    alignment: u64,
) -> Result<Gfx942InitializedDeviceMemoryV1, MemorySessionError> {
    finish_v1(
        engine,
        DeviceInitializationCustodyV1::new(
            InitializationSourceV1::Repeated(recipe),
            InitializationLeaseV1::None,
        ),
        Some(AllocationRequestV1 {
            device,
            vm,
            alignment,
        }),
    )
}
