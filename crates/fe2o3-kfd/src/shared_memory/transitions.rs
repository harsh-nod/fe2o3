//! Typed owners across native construction and its model projection.

use super::*;
use std::any::TypeId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TransitionStageV1 {
    Preflight,
    Checkpoint,
    Allocate,
    AllocationEvidence,
    AllocationProjection,
    AllocationCommit,
    Seal,
    MappingEvidence,
    Map,
    MapProjection,
    MapCommit,
    Retain,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct NativeTransitionProgressV1 {
    pub(super) attempted: bool,
    pub(super) returned_success: Option<bool>,
    pub(super) returned_map_prefix: Option<u32>,
}

// This consumes the actual token. There is deliberately no reconstruction,
// retag, disposal or extraction operation on terminal custody.
#[allow(dead_code)]
pub(super) struct TerminalTokenV1 {
    pub(super) session_id: u64,
    pub(super) id: u64,
    pub(super) generation: u64,
    pub(super) layout: SharedGttAllocationLayoutV1,
    pub(super) profile_type: TypeId,
    pub(super) state_type: TypeId,
    pub(super) profile: SharedGttProfileV1,
    pub(super) flags: u32,
    pub(super) userptr: bool,
}

impl TerminalTokenV1 {
    fn from_token<P: GttProfileV1, S: GttAllocationStateV1>(
        token: SharedGttAllocationV1<P, S>,
    ) -> Self {
        let SharedGttAllocationV1 {
            session_id,
            id,
            generation,
            layout,
            marker: _,
        } = token;
        Self {
            session_id,
            id,
            generation,
            layout,
            profile_type: TypeId::of::<P>(),
            state_type: TypeId::of::<S>(),
            profile: P::PROFILE,
            flags: P::FLAGS.bits(),
            userptr: P::IS_USERPTR,
        }
    }
}

#[allow(dead_code)]
pub(super) struct TerminalTransitionV1 {
    pub(super) stage: TransitionStageV1,
    pub(super) progress: NativeTransitionProgressV1,
    pub(super) input: Option<TerminalTokenV1>,
    pub(super) output: Option<TerminalTokenV1>,
}

struct TransitionOwnersV1<P: GttProfileV1, I: GttAllocationStateV1, O: GttAllocationStateV1> {
    input: Option<SharedGttAllocationV1<P, I>>,
    output: Option<SharedGttAllocationV1<P, O>>,
    admitted: bool,
    stage: TransitionStageV1,
    progress: NativeTransitionProgressV1,
}

impl<P: GttProfileV1, I: GttAllocationStateV1, O: GttAllocationStateV1>
    TransitionOwnersV1<P, I, O>
{
    fn promote(&mut self) {
        assert!(self.output.is_none(), "transition output occupied");
        let input = self.input.take().expect("retained transition input");
        self.output = Some(input.retag());
    }

    fn retain_failure<B: MemoryBackend>(self, engine: &mut SharedMemoryEngine<B>) {
        if self.admitted {
            engine.phase = SharedMemorySessionPhaseV1::Quarantined;
            engine.terminal_transition = Some(TerminalTransitionV1 {
                stage: self.stage,
                progress: self.progress,
                input: self.input.map(TerminalTokenV1::from_token),
                output: self.output.map(TerminalTokenV1::from_token),
            });
        }
    }
}

// R is non-owning result metadata. Usable tokens stay in owners until the
// operation, including model projection, has returned successfully.
fn run_v1<B, P, I, O, R>(
    engine: &mut SharedMemoryEngine<B>,
    input: Option<SharedGttAllocationV1<P, I>>,
    expected: Option<SharedAllocationPhaseV1>,
    operation: impl FnOnce(
        &mut SharedMemoryEngine<B>,
        &mut TransitionOwnersV1<P, I, O>,
    ) -> Result<R, MemorySessionError>,
) -> Result<(SharedGttAllocationV1<P, O>, R), MemorySessionError>
where
    B: MemoryBackend,
    P: GttProfileV1,
    I: GttAllocationStateV1,
    O: GttAllocationStateV1,
{
    engine.require_active()?;
    if engine.terminal_transition.is_some() {
        return engine.quarantine(MemorySessionError::SharedSessionQuarantined);
    }
    let mut owners = TransitionOwnersV1 {
        input,
        output: None,
        admitted: false,
        stage: TransitionStageV1::Preflight,
        progress: NativeTransitionProgressV1::default(),
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let (Some(input), Some(expected)) = (&owners.input, expected) {
            owners.admitted = engine.index(input, expected).is_ok();
        }
        let result = operation(engine, &mut owners)?;
        assert!(owners.output.is_some(), "completed transition output");
        Ok(result)
    }));
    match result {
        Ok(Ok(result)) => Ok((
            owners.output.take().expect("checked transition output"),
            result,
        )),
        Ok(Err(error)) => {
            owners.retain_failure(engine);
            Err(error)
        }
        Err(payload) => {
            owners.retain_failure(engine);
            std::panic::resume_unwind(payload)
        }
    }
}

pub(super) struct ProjectionV1<'a> {
    foundation: &'a mut QueueModelFoundationV1,
    device: ModelDeviceAdmissionV1,
    vm: VmKeyV1,
    #[cfg(test)]
    pub(super) fault: Option<(TransitionStageV1, ProjectionFaultV1)>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(super) enum ProjectionFaultV1 {
    Error,
    Panic,
    ExhaustRevision,
}

impl<'a> ProjectionV1<'a> {
    pub(super) fn new(
        foundation: &'a mut QueueModelFoundationV1,
        device: ModelDeviceAdmissionV1,
        vm: VmKeyV1,
    ) -> Self {
        Self {
            foundation,
            device,
            vm,
            #[cfg(test)]
            fault: None,
        }
    }

    fn stage(
        &mut self,
        stage: &mut TransitionStageV1,
        next: TransitionStageV1,
    ) -> Result<(), MemorySessionError> {
        *stage = next;
        #[cfg(test)]
        if let Some((at, fault)) = self.fault
            && at == next
        {
            match fault {
                ProjectionFaultV1::Error => {
                    return Err(MemorySessionError::Injected("session projection"));
                }
                ProjectionFaultV1::Panic => std::panic::panic_any(("session projection", next)),
                ProjectionFaultV1::ExhaustRevision => self
                    .foundation
                    .set_certificate_revision_for_test(u64::MAX)
                    .expect("certified projection fault fixture"),
            }
        }
        Ok(())
    }
}

pub(super) fn allocate_v1<B: MemoryBackend, P: GttProfileV1>(
    engine: &mut SharedMemoryEngine<B>,
    projection: &mut ProjectionV1<'_>,
    requested_bytes: usize,
    process_poison: impl FnOnce(),
) -> Result<SharedGttAllocationV1<P, GttCpuWritableV1>, MemorySessionError> {
    let (token, ()) = run_v1::<B, P, GttCpuWritableV1, GttCpuWritableV1, _>(
        engine,
        None,
        None,
        |engine, owners| {
            preflight_queue_foundation_native_memory_transition_v1(
                projection.foundation,
                engine,
                2,
                process_poison,
            )?;
            projection.stage(&mut owners.stage, TransitionStageV1::Checkpoint)?;
            let checkpoint = projection
                .foundation
                .memory()
                .checkpoint_released()
                .map_err(|_| MemorySessionError::Model("shared memory journal checkpoint"))?;
            projection
                .foundation
                .replace_memory_after_sealed_transition(checkpoint)
                .map_err(MemorySessionError::Model)?;
            projection.stage(&mut owners.stage, TransitionStageV1::Allocate)?;
            owners.output = Some(engine.allocate::<P>(requested_bytes)?);
            owners.admitted = true;
            projection.stage(&mut owners.stage, TransitionStageV1::AllocationEvidence)?;
            let (id, generation, layout, base, handle) =
                engine.evidence(owners.output.as_ref().expect("returned allocation"))?;
            let (reservation, allocation, _) = model_keys(projection.vm, id, generation);
            projection.stage(&mut owners.stage, TransitionStageV1::AllocationProjection)?;
            let model = project_allocation(
                projection.foundation.memory(),
                reservation,
                allocation,
                base,
                layout,
                handle,
                P::KIND,
            )
            .map_err(|_| MemorySessionError::Model("shared allocation projection"))?;
            projection.stage(&mut owners.stage, TransitionStageV1::AllocationCommit)?;
            projection
                .foundation
                .replace_memory_after_sealed_transition(model)
                .map_err(MemorySessionError::Model)
        },
    )?;
    Ok(token)
}

pub(super) fn seal_v1<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    token: SharedGttAllocationV1<ExecutableGttV1, GttCpuWritableV1>,
) -> Result<SharedGttAllocationV1<ExecutableGttV1, GttExecutableImmutableV1>, MemorySessionError> {
    let (token, ()) = run_v1(
        engine,
        Some(token),
        Some(SharedAllocationPhaseV1::CpuWritable),
        |engine, owners| {
            owners.stage = TransitionStageV1::Seal;
            engine.seal_executable_borrowed(
                owners.input.as_ref().expect("retained executable input"),
                &mut owners.progress,
            )?;
            owners.promote();
            Ok(())
        },
    )?;
    Ok(token)
}

type MapTransitionFnV1<B, P, I> = fn(
    &mut SharedMemoryEngine<B>,
    &SharedGttAllocationV1<P, I>,
    &mut NativeTransitionProgressV1,
) -> Result<(), MemorySessionError>;

fn map_v1<B, P, I, O>(
    engine: &mut SharedMemoryEngine<B>,
    projection: &mut ProjectionV1<'_>,
    token: SharedGttAllocationV1<P, I>,
    expected: SharedAllocationPhaseV1,
    map: MapTransitionFnV1<B, P, I>,
    process_poison: impl FnOnce(),
) -> Result<SharedGttAllocationV1<P, O>, MemorySessionError>
where
    B: MemoryBackend,
    P: GttProfileV1,
    I: GttAllocationStateV1,
    O: GttAllocationStateV1,
{
    let (token, ()) = run_v1(engine, Some(token), Some(expected), |engine, owners| {
        preflight_queue_foundation_native_memory_transition_v1(
            projection.foundation,
            engine,
            1,
            process_poison,
        )?;
        projection.stage(&mut owners.stage, TransitionStageV1::MappingEvidence)?;
        let input = owners.input.as_ref().expect("retained mapping input");
        let (id, generation, _, _, _) = engine.evidence(input)?;
        let (_, _, mapping) = model_keys(projection.vm, id, generation);
        projection.stage(&mut owners.stage, TransitionStageV1::Map)?;
        map(engine, input, &mut owners.progress)?;
        owners.promote();
        projection.stage(&mut owners.stage, TransitionStageV1::MapProjection)?;
        let model = project_map(projection.foundation.memory(), mapping, projection.device)
            .map_err(|_| MemorySessionError::Model("shared map projection"))?;
        projection.stage(&mut owners.stage, TransitionStageV1::MapCommit)?;
        projection
            .foundation
            .replace_memory_after_sealed_transition(model)
            .map_err(MemorySessionError::Model)
    })?;
    Ok(token)
}

pub(super) fn map_mutable_v1<B: MemoryBackend, P: MutableGpuGttProfileV1>(
    engine: &mut SharedMemoryEngine<B>,
    projection: &mut ProjectionV1<'_>,
    token: SharedGttAllocationV1<P, GttCpuWritableV1>,
    process_poison: impl FnOnce(),
) -> Result<SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>, MemorySessionError> {
    map_v1(
        engine,
        projection,
        token,
        SharedAllocationPhaseV1::CpuWritable,
        SharedMemoryEngine::map_mutable_borrowed,
        process_poison,
    )
}

pub(super) fn map_executable_v1<B: MemoryBackend>(
    engine: &mut SharedMemoryEngine<B>,
    projection: &mut ProjectionV1<'_>,
    token: SharedGttAllocationV1<ExecutableGttV1, GttExecutableImmutableV1>,
    process_poison: impl FnOnce(),
) -> Result<SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>, MemorySessionError>
{
    map_v1(
        engine,
        projection,
        token,
        SharedAllocationPhaseV1::ExecutableImmutable,
        SharedMemoryEngine::map_executable_borrowed,
        process_poison,
    )
}

pub(super) fn retain_v1<B, R, P, S>(
    engine: &mut SharedMemoryEngine<B>,
    vm: VmKeyV1,
    token: SharedGttAllocationV1<P, S>,
) -> Result<SharedGttQueueResourceAuthorityV1<R, P, S>, MemorySessionError>
where
    B: MemoryBackend,
    R: SharedGttQueueResourceRoleV1,
    P: GttProfileV1,
    S: GpuMappedGttStateV1,
{
    let (token, facts) = run_v1(engine, Some(token), Some(S::PHASE), |engine, owners| {
        owners.stage = TransitionStageV1::Retain;
        let input = owners.input.as_ref().expect("retained resource input");
        let facts = retained_facts_v1(engine, vm, input)?;
        owners.promote();
        Ok(facts)
    })?;
    Ok(SharedGttQueueResourceAuthorityV1 {
        token,
        facts,
        role: PhantomData,
    })
}

pub(super) fn retained_facts_v1<B, P, S>(
    engine: &SharedMemoryEngine<B>,
    vm: VmKeyV1,
    token: &SharedGttAllocationV1<P, S>,
) -> Result<SharedGttMappedResourceFactsV1, MemorySessionError>
where
    B: MemoryBackend,
    P: GttProfileV1,
    S: GpuMappedGttStateV1,
{
    let index = engine.index(token, S::PHASE)?;
    let record = &engine.allocations[index];
    let (_, _, mapping) = model_keys(vm, record.id, record.generation);
    Ok(SharedGttMappedResourceFactsV1 {
        gpu_va: record.gpu_va,
        logical_bytes: record.layout.requested_bytes,
        cpu_mapping_bytes: record.layout.cpu_mapping_bytes,
        gpu_va_bytes: record.layout.gpu_va_bytes,
        mapping,
        publication: MemoryPublicationKeyV1 {
            mapping,
            id: MemoryPublicationIdV1(record.id),
        },
    })
}
