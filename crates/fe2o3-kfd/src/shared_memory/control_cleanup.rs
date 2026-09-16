//! Borrowed, one-shot unmap and disposal of controls and coherent data.

use super::*;
use transitions::{NativeTransitionProgressV1, TerminalTokenV1};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct NativeCallProgressV1 {
    pub(super) attempted: bool,
    pub(super) returned_success: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct NativeDisposalProgressV1 {
    pub(super) cpu_unmap: NativeCallProgressV1,
    pub(super) free: NativeCallProgressV1,
    pub(super) va_release: NativeCallProgressV1,
    pub(super) native_disposed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CleanupStageV1 {
    UnmapPreflight,
    UnmapEvidence,
    NativeUnmap,
    UnmapProjection,
    UnmapCommit,
    Unmapped,
    ReleasePreflight,
    ReleaseEvidence,
    ReleaseProjection,
    NativeRelease,
    ReleaseCommit,
    Complete,
}

enum ControlTokenV1 {
    MappedKernarg(SharedGttAllocationV1<KernargGttV1, GttGpuAccessibleMutableV1>),
    UnmappedKernarg(SharedGttAllocationV1<KernargGttV1, GttCpuWritableV1>),
    MappedCode(SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>),
    UnmappedCode(SharedGttAllocationV1<ExecutableGttV1, GttExecutableImmutableV1>),
    MappedHostData(SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>),
    UnmappedHostData(SharedGttAllocationV1<HostVisibleCoherentGttV1, GttCpuWritableV1>),
    Disposed { _receipt: TerminalTokenV1 },
}

/// The caller retains this root across cleanup, including failed model commits.
/// No operation extracts or reconstructs a usable token after cleanup starts.
pub(crate) struct ControlCleanupCustodyV1 {
    token: Option<ControlTokenV1>,
    started: bool,
    failed: bool,
    stage: CleanupStageV1,
    unmap: NativeTransitionProgressV1,
    disposal: NativeDisposalProgressV1,
}

impl ControlCleanupCustodyV1 {
    pub(crate) fn kernarg(
        token: SharedGttAllocationV1<KernargGttV1, GttGpuAccessibleMutableV1>,
    ) -> Self {
        Self::new(ControlTokenV1::MappedKernarg(token))
    }

    pub(crate) fn code(
        token: SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>,
    ) -> Self {
        Self::new(ControlTokenV1::MappedCode(token))
    }

    pub(super) fn host_data(
        token: SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>,
    ) -> Self {
        Self::new(ControlTokenV1::MappedHostData(token))
    }

    fn new(token: ControlTokenV1) -> Self {
        Self {
            token: Some(token),
            started: false,
            failed: false,
            stage: CleanupStageV1::UnmapPreflight,
            unmap: NativeTransitionProgressV1::default(),
            disposal: NativeDisposalProgressV1::default(),
        }
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.stage == CleanupStageV1::Complete
            && !self.failed
            && matches!(self.token, Some(ControlTokenV1::Disposed { .. }))
    }

    fn evidence<B: MemoryBackend>(
        &self,
        engine: &SharedMemoryEngine<B>,
    ) -> Result<(u64, u64), MemorySessionError> {
        let (id, generation, _, _, _) = match self.token.as_ref() {
            Some(ControlTokenV1::MappedKernarg(token)) => engine.evidence(token)?,
            Some(ControlTokenV1::UnmappedKernarg(token)) => engine.evidence(token)?,
            Some(ControlTokenV1::MappedCode(token)) => engine.evidence(token)?,
            Some(ControlTokenV1::UnmappedCode(token)) => engine.evidence(token)?,
            Some(ControlTokenV1::MappedHostData(token)) => engine.evidence(token)?,
            Some(ControlTokenV1::UnmappedHostData(token)) => engine.evidence(token)?,
            _ => return Err(MemorySessionError::InvalidAllocationAuthority),
        };
        Ok((id, generation))
    }

    fn unmap<B: MemoryBackend>(
        &mut self,
        engine: &mut SharedMemoryEngine<B>,
    ) -> Result<(), MemorySessionError> {
        match self.token.as_ref() {
            Some(ControlTokenV1::MappedKernarg(token)) => {
                engine.unmap_mutable_borrowed(token, &mut self.unmap)?;
            }
            Some(ControlTokenV1::MappedCode(token)) => {
                engine.unmap_executable_borrowed(token, &mut self.unmap)?;
            }
            Some(ControlTokenV1::MappedHostData(token)) => {
                engine.unmap_mutable_borrowed(token, &mut self.unmap)?;
            }
            _ => return Err(MemorySessionError::InvalidAllocationAuthority),
        }
        self.token = Some(match self.token.take().expect("retained mapped control") {
            ControlTokenV1::MappedKernarg(token) => ControlTokenV1::UnmappedKernarg(token.retag()),
            ControlTokenV1::MappedCode(token) => ControlTokenV1::UnmappedCode(token.retag()),
            ControlTokenV1::MappedHostData(token) => {
                ControlTokenV1::UnmappedHostData(token.retag())
            }
            _ => unreachable!("successful unmap retains its original token"),
        });
        Ok(())
    }

    fn dispose<B: MemoryBackend>(
        &mut self,
        engine: &mut SharedMemoryEngine<B>,
    ) -> Result<(), MemorySessionError> {
        match self.token.as_ref() {
            Some(ControlTokenV1::UnmappedKernarg(token)) => engine.release_borrowed(
                token,
                SharedAllocationPhaseV1::CpuWritable,
                &mut self.disposal,
            ),
            Some(ControlTokenV1::UnmappedCode(token)) => engine.release_borrowed(
                token,
                SharedAllocationPhaseV1::ExecutableImmutable,
                &mut self.disposal,
            ),
            Some(ControlTokenV1::UnmappedHostData(token)) => engine.release_borrowed(
                token,
                SharedAllocationPhaseV1::CpuWritable,
                &mut self.disposal,
            ),
            _ => Err(MemorySessionError::InvalidAllocationAuthority),
        }
    }

    fn retain_disposed_receipt(&mut self) {
        if !self.disposal.native_disposed
            || matches!(self.token, Some(ControlTokenV1::Disposed { .. }))
        {
            return;
        }
        let receipt = match self.token.take().expect("retained disposal input") {
            ControlTokenV1::UnmappedKernarg(token) => TerminalTokenV1::from_token(token),
            ControlTokenV1::UnmappedCode(token) => TerminalTokenV1::from_token(token),
            ControlTokenV1::UnmappedHostData(token) => TerminalTokenV1::from_token(token),
            _ => unreachable!("only an unmapped control can finish native disposal"),
        };
        self.token = Some(ControlTokenV1::Disposed { _receipt: receipt });
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ControlCleanupObservationV1 {
    pub(crate) identity: SharedGttAllocationIdentityV1,
    pub(crate) layout: SharedGttAllocationLayoutV1,
    pub(crate) profile_type: std::any::TypeId,
    pub(crate) state_type: std::any::TypeId,
    pub(crate) owner: &'static str,
    pub(crate) stage: CleanupStageV1,
    pub(crate) started: bool,
    pub(crate) failed: bool,
    pub(crate) unmap: (bool, Option<bool>, Option<u32>),
    pub(crate) disposal: [(bool, Option<bool>); 3],
    pub(crate) native_disposed: bool,
}

#[cfg(test)]
impl ControlCleanupCustodyV1 {
    pub(crate) fn observation(&self) -> ControlCleanupObservationV1 {
        fn token<P: GttProfileV1, S: GttAllocationStateV1>(
            token: &SharedGttAllocationV1<P, S>,
        ) -> (
            SharedGttAllocationIdentityV1,
            SharedGttAllocationLayoutV1,
            std::any::TypeId,
            std::any::TypeId,
        ) {
            (
                token.storage_identity(),
                token.layout,
                std::any::TypeId::of::<P>(),
                std::any::TypeId::of::<S>(),
            )
        }
        let (owner, (identity, layout, profile_type, state_type)) =
            match self.token.as_ref().unwrap() {
                ControlTokenV1::MappedKernarg(t) => ("Mapped", token(t)),
                ControlTokenV1::MappedCode(t) => ("Mapped", token(t)),
                ControlTokenV1::UnmappedKernarg(t) => ("Unmapped", token(t)),
                ControlTokenV1::UnmappedCode(t) => ("Unmapped", token(t)),
                ControlTokenV1::MappedHostData(t) => ("Mapped", token(t)),
                ControlTokenV1::UnmappedHostData(t) => ("Unmapped", token(t)),
                ControlTokenV1::Disposed { _receipt: t } => (
                    "NativeDisposed",
                    (
                        SharedGttAllocationIdentityV1 {
                            session_id: t.session_id,
                            id: t.id,
                            generation: t.generation,
                        },
                        t.layout,
                        t.profile_type,
                        t.state_type,
                    ),
                ),
            };
        ControlCleanupObservationV1 {
            identity,
            layout,
            profile_type,
            state_type,
            owner,
            stage: self.stage,
            started: self.started,
            failed: self.failed,
            unmap: (
                self.unmap.attempted,
                self.unmap.returned_success,
                self.unmap.returned_map_prefix,
            ),
            disposal: [
                self.disposal.cpu_unmap,
                self.disposal.free,
                self.disposal.va_release,
            ]
            .map(|p| (p.attempted, p.returned_success)),
            native_disposed: self.disposal.native_disposed,
        }
    }
}

pub(super) struct ProjectionV1<'a> {
    foundation: &'a mut QueueModelFoundationV1,
    vm: VmKeyV1,
    #[cfg(test)]
    pub(super) fault: Option<(CleanupStageV1, transitions::ProjectionFaultV1)>,
}

impl<'a> ProjectionV1<'a> {
    pub(super) fn new(foundation: &'a mut QueueModelFoundationV1, vm: VmKeyV1) -> Self {
        Self {
            foundation,
            vm,
            #[cfg(test)]
            fault: None,
        }
    }

    fn stage(
        &mut self,
        custody: &mut ControlCleanupCustodyV1,
        stage: CleanupStageV1,
    ) -> Result<(), MemorySessionError> {
        custody.stage = stage;
        #[cfg(test)]
        if let Some((at, fault)) = self.fault
            && at == stage
        {
            match fault {
                transitions::ProjectionFaultV1::Error => {
                    return Err(MemorySessionError::Injected("control cleanup projection"));
                }
                transitions::ProjectionFaultV1::Panic => {
                    std::panic::panic_any(("control cleanup projection", stage));
                }
                transitions::ProjectionFaultV1::ExhaustRevision => self
                    .foundation
                    .set_certificate_revision_for_test(u64::MAX)
                    .expect("certified cleanup fixture"),
            }
        }
        Ok(())
    }
}

pub(super) fn release_v1<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    projection: &mut ProjectionV1<'_>,
    custody: &mut ControlCleanupCustodyV1,
    mut process_poison: impl FnMut(),
) -> Result<(), MemorySessionError> {
    unmap_v1(engine, projection, custody, &mut process_poison)?;
    finish_release_v1(engine, projection, custody, process_poison)
}

pub(super) fn unmap_v1<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    projection: &mut ProjectionV1<'_>,
    custody: &mut ControlCleanupCustodyV1,
    mut process_poison: impl FnMut(),
) -> Result<(), MemorySessionError> {
    if custody.started {
        return Err(MemorySessionError::InvalidAllocationAuthority);
    }
    custody.started = true;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        projection.stage(custody, CleanupStageV1::UnmapPreflight)?;
        preflight_queue_foundation_native_memory_transition_v1(
            projection.foundation,
            engine,
            1,
            &mut process_poison,
        )?;
        projection.stage(custody, CleanupStageV1::UnmapEvidence)?;
        let (id, generation) = custody.evidence(engine)?;
        let (_, _, mapping) = model_keys(projection.vm, id, generation);
        projection.stage(custody, CleanupStageV1::NativeUnmap)?;
        custody.unmap(engine)?;
        projection.stage(custody, CleanupStageV1::UnmapProjection)?;
        let unmapped = project_unmap(projection.foundation.memory(), mapping)
            .map_err(|_| MemorySessionError::Model("shared unmap projection"))?;
        projection.stage(custody, CleanupStageV1::UnmapCommit)?;
        projection
            .foundation
            .replace_memory_after_sealed_transition(unmapped)
            .map_err(MemorySessionError::Model)?;
        custody.stage = CleanupStageV1::Unmapped;
        Ok(())
    }));
    settle_result(engine, custody, result)
}

pub(super) fn finish_release_v1<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    projection: &mut ProjectionV1<'_>,
    custody: &mut ControlCleanupCustodyV1,
    mut process_poison: impl FnMut(),
) -> Result<(), MemorySessionError> {
    if !custody.started || custody.failed || custody.stage != CleanupStageV1::Unmapped {
        return Err(MemorySessionError::InvalidAllocationAuthority);
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        projection.stage(custody, CleanupStageV1::ReleasePreflight)?;
        preflight_queue_foundation_native_memory_transition_v1(
            projection.foundation,
            engine,
            1,
            &mut process_poison,
        )?;
        projection.stage(custody, CleanupStageV1::ReleaseEvidence)?;
        let (id, generation) = custody.evidence(engine)?;
        let (reservation, allocation, mapping) = model_keys(projection.vm, id, generation);
        projection.stage(custody, CleanupStageV1::ReleaseProjection)?;
        let released = project_release(
            projection.foundation.memory(),
            reservation,
            allocation,
            mapping,
        )
        .map_err(|_| MemorySessionError::Model("shared release projection"))?;
        projection.stage(custody, CleanupStageV1::NativeRelease)?;
        custody.dispose(engine)?;
        custody.retain_disposed_receipt();
        projection.stage(custody, CleanupStageV1::ReleaseCommit)?;
        projection
            .foundation
            .replace_memory_after_sealed_transition(released)
            .map_err(MemorySessionError::Model)?;
        custody.stage = CleanupStageV1::Complete;
        Ok(())
    }));
    settle_result(engine, custody, result)
}

fn settle_result<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    custody: &mut ControlCleanupCustodyV1,
    result: std::thread::Result<Result<(), MemorySessionError>>,
) -> Result<(), MemorySessionError> {
    // Native disposal may have completed before a closing currentness/accounting
    // error or panic. Retain a receipt, never a newly usable release authority.
    custody.retain_disposed_receipt();
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => {
            custody.failed = true;
            if matches!(custody.token, Some(ControlTokenV1::MappedHostData(_)))
                && !custody.unmap.attempted
            {
                Err(error)
            } else {
                engine.quarantine(error)
            }
        }
        Err(payload) => {
            custody.failed = true;
            engine.phase = SharedMemorySessionPhaseV1::Quarantined;
            std::panic::resume_unwind(payload)
        }
    }
}
