//! The production session adapter over real fake-native records and foundation.

use super::*;
use crate::shared_memory::transitions::{
    self as adapter, ProjectionFaultV1 as Fault, TransitionStageV1 as Stage,
};
use std::any::TypeId;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn fixture(configured: bool) -> BackingConstructorFixture {
    let mut fixture = BackingConstructorFixture::new(
        configured.then(|| Gfx942DeviceBackingBudgetV1::new(1 << 20, 16).unwrap()),
    );
    if configured {
        fixture
            .engine
            .configure_host_visible_backing_budget_v1(
                fixture.device.model_key(),
                fixture.vm,
                Gfx942HostVisibleBackingBudgetV1::new(1 << 20, 256).unwrap(),
            )
            .unwrap();
    }
    fixture
}

fn allocate<P: GttProfileV1>(
    f: &mut BackingConstructorFixture,
) -> SharedGttAllocationV1<P, GttCpuWritableV1> {
    adapter::allocate_v1(
        &mut f.engine,
        &mut adapter::ProjectionV1::new(&mut f.foundation, f.device, f.vm),
        4096,
        || panic!("unexpected preflight poison"),
    )
    .unwrap()
}

fn map_mutable<P: MutableGpuGttProfileV1>(
    f: &mut BackingConstructorFixture,
    token: SharedGttAllocationV1<P, GttCpuWritableV1>,
) -> Result<SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>, MemorySessionError> {
    adapter::map_mutable_v1(
        &mut f.engine,
        &mut adapter::ProjectionV1::new(&mut f.foundation, f.device, f.vm),
        token,
        || panic!("unexpected preflight poison"),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TokenSnapshot {
    session: u64,
    id: u64,
    generation: u64,
    layout: SharedGttAllocationLayoutV1,
    profile: TypeId,
    state: TypeId,
    userptr: bool,
}

impl TokenSnapshot {
    fn new<P: GttProfileV1, S: GttAllocationStateV1>(token: &SharedGttAllocationV1<P, S>) -> Self {
        Self {
            session: token.session_id,
            id: token.id,
            generation: token.generation,
            layout: token.layout,
            profile: TypeId::of::<P>(),
            state: TypeId::of::<S>(),
            userptr: P::IS_USERPTR,
        }
    }

    fn assert_terminal(self, token: &adapter::TerminalTokenV1) {
        assert_eq!(token.session_id, self.session);
        assert_eq!(token.id, self.id);
        assert_eq!(token.generation, self.generation);
        assert_eq!(token.layout, self.layout);
        assert_eq!(token.profile_type, self.profile);
        assert_eq!(token.state_type, self.state);
        assert_eq!(token.profile, self.layout.profile);
        assert_eq!(token.flags, self.layout.uapi_flags);
        assert_eq!(token.userptr, self.userptr);
    }
}

fn calls(engine: &SharedMemoryEngine<FakeBackend>) -> [usize; 8] {
    let b = &engine.backend;
    [
        b.currentness_calls,
        b.reserve_va_calls,
        b.alloc_calls,
        b.map_cpu_calls,
        b.map_gpu_calls,
        b.unmap_gpu_calls,
        b.free_calls,
        b.release_va_calls,
    ]
}

fn assert_closed(f: &mut BackingConstructorFixture) {
    assert_eq!(f.engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    let before = calls(&f.engine);
    assert!(matches!(
        adapter::allocate_v1::<_, KernargGttV1>(
            &mut f.engine,
            &mut adapter::ProjectionV1::new(&mut f.foundation, f.device, f.vm),
            4096,
            || panic!("closed session must not preflight"),
        ),
        Err(MemorySessionError::SharedSessionQuarantined)
    ));
    assert_eq!(calls(&f.engine), before);
    assert_eq!(f.engine.backend.unmap_gpu_calls, 0);
    assert_eq!(f.engine.backend.free_calls, 0);
    assert_eq!(f.engine.backend.release_va_calls, 0);
}

fn assert_complete_record(f: &BackingConstructorFixture, id: u64) {
    let record = f.engine.allocations.iter().find(|r| r.id == id).unwrap();
    assert!(record.reservation.is_some());
    assert!(record.mapping.as_ref().unwrap().active);
    assert!(record.handle.is_some());
    assert!(!record.free_attempted);
    assert_eq!(
        f.engine.allocation_record_slots.get(&id).copied(),
        Some((id - 1) as usize)
    );
}

fn allocation_projection_failure<P: GttProfileV1>(configured: bool) {
    let mut f = fixture(configured);
    f.engine.backend.fixed_va = Some(0x30_0000);
    let before = f.foundation.memory().clone();
    let result = adapter::allocate_v1::<_, P>(
        &mut f.engine,
        &mut adapter::ProjectionV1::new(&mut f.foundation, f.device, f.vm),
        4096,
        || panic!("unexpected poison"),
    );
    assert!(matches!(
        result,
        Err(MemorySessionError::Model("shared allocation projection"))
    ));
    let terminal = f.engine.terminal_transition.as_ref().unwrap();
    assert_eq!(terminal.stage, Stage::AllocationProjection);
    assert!(terminal.input.is_none());
    let output = terminal.output.as_ref().unwrap();
    assert_eq!(output.profile_type, TypeId::of::<P>());
    assert_eq!(output.state_type, TypeId::of::<GttCpuWritableV1>());
    assert_eq!(output.layout, profile_layout::<P>(4096).unwrap());
    assert_eq!(
        (output.session_id, output.id, output.generation),
        (f.engine.session_id, 1, 1)
    );
    assert_eq!(output.userptr, P::IS_USERPTR);
    assert_complete_record(&f, output.id);
    assert_eq!(f.foundation.memory(), &before);
    assert!(f.engine.pending_allocation.is_none());
    if let Some(account) = &f.engine.host_backing_account {
        assert_eq!(
            account.usage().retained_records,
            usize::from(is_host_backing_profile::<P>())
        );
        assert_eq!(account.usage().quarantined_records, 0);
    }
    assert_closed(&mut f);
}

#[test]
fn session_allocation_projection_rejection_preserves_all_returned_profiles() {
    for configured in [false, true] {
        allocation_projection_failure::<HostVisibleCoherentGttV1>(configured);
        allocation_projection_failure::<KernargGttV1>(configured);
        allocation_projection_failure::<ExecutableGttV1>(configured);
        allocation_projection_failure::<AqlQueueGttV1>(configured);
        allocation_projection_failure::<ExecutableAqlQueueProbeGttV1>(configured);
        allocation_projection_failure::<UserptrAqlQueueProbeGttV1>(configured);
        allocation_projection_failure::<UserptrAqlControlGttV1>(configured);
    }
}

fn allocation_faults<P: GttProfileV1>() {
    for stage in [
        Stage::AllocationEvidence,
        Stage::AllocationProjection,
        Stage::AllocationCommit,
    ] {
        for fault in [Fault::Error, Fault::Panic] {
            let mut f = fixture(true);
            let mut projection = adapter::ProjectionV1::new(&mut f.foundation, f.device, f.vm);
            projection.fault = Some((stage, fault));
            let result = catch_unwind(AssertUnwindSafe(|| {
                adapter::allocate_v1::<_, P>(&mut f.engine, &mut projection, 4096, || {
                    panic!("unexpected poison")
                })
            }));
            match fault {
                Fault::Error => assert!(matches!(
                    result.unwrap(),
                    Err(MemorySessionError::Injected("session projection"))
                )),
                Fault::Panic => assert_eq!(
                    result.err().unwrap().downcast_ref::<(&str, Stage)>(),
                    Some(&("session projection", stage))
                ),
                Fault::ExhaustRevision => unreachable!(),
            }
            let terminal = f.engine.terminal_transition.as_ref().unwrap();
            assert_eq!(terminal.stage, stage);
            assert!(terminal.input.is_none());
            assert_eq!(
                terminal.output.as_ref().unwrap().state_type,
                TypeId::of::<GttCpuWritableV1>()
            );
            assert_complete_record(&f, 1);
            assert_closed(&mut f);
        }
    }
}

#[test]
fn session_allocation_projection_error_and_panic_keep_returned_token() {
    allocation_faults::<ExecutableGttV1>();
    allocation_faults::<KernargGttV1>();
    allocation_faults::<HostVisibleCoherentGttV1>();
}

#[test]
fn session_allocation_preflight_and_pending_native_failure_are_distinct() {
    let mut f = fixture(true);
    let before = calls(&f.engine);
    let result = adapter::allocate_v1::<_, ExecutableGttV1>(
        &mut f.engine,
        &mut adapter::ProjectionV1::new(&mut f.foundation, f.device, f.vm),
        0,
        || panic!("unexpected poison"),
    );
    assert!(result.is_err());
    assert_eq!(calls(&f.engine), before);
    assert!(f.engine.terminal_transition.is_none());
    assert!(f.engine.pending_allocation.is_none());
    assert_eq!(f.engine.phase(), SharedMemorySessionPhaseV1::Active);

    f.engine.backend.alloc_oom = true;
    let result = adapter::allocate_v1::<_, ExecutableGttV1>(
        &mut f.engine,
        &mut adapter::ProjectionV1::new(&mut f.foundation, f.device, f.vm),
        4096,
        || panic!("unexpected poison"),
    );
    assert!(result.is_err());
    assert!(f.engine.terminal_transition.is_none());
    assert!(
        f.engine
            .pending_allocation
            .as_ref()
            .unwrap()
            .allocation_output
            .is_some()
    );
    assert!(f.engine.allocations.is_empty());
    assert_closed(&mut f);
}

fn certify(f: &mut BackingConstructorFixture, revision: u64) {
    f.foundation
        .mint_invariant_certificate(f.engine.session_id, f.device, f.vm)
        .unwrap();
    f.foundation
        .set_certificate_revision_for_test(revision)
        .unwrap();
}

#[test]
fn session_native_transitions_preflight_the_complete_revision_budget() {
    let mut f = fixture(false);
    certify(&mut f, u64::MAX - 1);
    let before = calls(&f.engine);
    let poisoned = Cell::new(false);
    let result = adapter::allocate_v1::<_, ExecutableGttV1>(
        &mut f.engine,
        &mut adapter::ProjectionV1::new(&mut f.foundation, f.device, f.vm),
        4096,
        || poisoned.set(true),
    );
    assert!(matches!(
        result,
        Err(MemorySessionError::Model(
            "queue foundation certificate revision exhausted"
        ))
    ));
    assert!(poisoned.get());
    assert_eq!(calls(&f.engine), before);
    assert!(f.engine.terminal_transition.is_none());
    assert!(f.engine.allocations.is_empty());

    let mut f = fixture(false);
    let token = allocate::<KernargGttV1>(&mut f);
    let expected = TokenSnapshot::new(&token);
    certify(&mut f, u64::MAX);
    let before = calls(&f.engine);
    let poisoned = Cell::new(false);
    let result = adapter::map_mutable_v1(
        &mut f.engine,
        &mut adapter::ProjectionV1::new(&mut f.foundation, f.device, f.vm),
        token,
        || poisoned.set(true),
    );
    assert!(result.is_err());
    assert!(poisoned.get());
    assert_eq!(calls(&f.engine), before);
    let terminal = f.engine.terminal_transition.as_ref().unwrap();
    assert_eq!(terminal.stage, Stage::Preflight);
    expected.assert_terminal(terminal.input.as_ref().unwrap());
    assert!(!terminal.progress.attempted);
    assert_closed(&mut f);
}

#[test]
fn session_native_transitions_accept_exact_revision_headroom() {
    let mut f = fixture(false);
    certify(&mut f, u64::MAX - 2);
    let token = allocate::<ExecutableGttV1>(&mut f);
    assert_eq!(token.id, 1);
    assert!(
        f.foundation
            .preflight_memory_transition_revisions(1)
            .is_err()
    );
    assert!(f.engine.terminal_transition.is_none());

    let mut f = fixture(false);
    let token = allocate::<KernargGttV1>(&mut f);
    certify(&mut f, u64::MAX - 1);
    let token = map_mutable(&mut f, token).unwrap();
    assert_eq!(token.id, 1);
    assert!(
        f.foundation
            .preflight_memory_transition_revisions(1)
            .is_err()
    );
    assert!(f.engine.terminal_transition.is_none());
}

#[test]
fn session_late_allocation_commit_exhaustion_keeps_successful_native_token() {
    let mut f = fixture(true);
    certify(&mut f, 0);
    let mut projection = adapter::ProjectionV1::new(&mut f.foundation, f.device, f.vm);
    projection.fault = Some((Stage::AllocationCommit, Fault::ExhaustRevision));
    assert!(matches!(
        adapter::allocate_v1::<_, ExecutableGttV1>(
            &mut f.engine,
            &mut projection,
            4096,
            || panic!("unexpected poison"),
        ),
        Err(MemorySessionError::Model(
            "queue foundation certificate revision exhausted"
        ))
    ));
    let terminal = f.engine.terminal_transition.as_ref().unwrap();
    assert_eq!(terminal.stage, Stage::AllocationCommit);
    assert_eq!(terminal.output.as_ref().unwrap().id, 1);
    assert_complete_record(&f, 1);
    assert_closed(&mut f);
}

#[test]
fn session_seal_error_panic_and_currentness_preserve_precursor_custody() {
    for case in 0..6 {
        let mut f = fixture(false);
        let token = allocate::<ExecutableGttV1>(&mut f);
        let expected = TokenSnapshot::new(&token);
        let before = f.engine.backend.currentness_calls;
        match case {
            0 => f.engine.backend.fail_operation = Some("protect_cpu_read_only"),
            1 => f.engine.backend.panic_operation = Some("protect_cpu_read_only"),
            2 => f.engine.backend.fail_currentness_at = Some(before + 1),
            3 => f.engine.backend.panic_currentness_at = Some(before + 1),
            4 => f.engine.backend.fail_currentness_at = Some(before + 2),
            5 => f.engine.backend.panic_currentness_at = Some(before + 2),
            _ => unreachable!(),
        }
        let result = catch_unwind(AssertUnwindSafe(|| adapter::seal_v1(&mut f.engine, token)));
        if case % 2 == 0 {
            assert!(result.unwrap().is_err());
        } else {
            let operation = if case == 1 {
                "protect_cpu_read_only"
            } else {
                "currentness"
            };
            assert_eq!(
                result.err().unwrap().downcast_ref::<(&str, &str)>(),
                Some(&("N2 native panic", operation))
            );
        }
        let terminal = f.engine.terminal_transition.as_ref().unwrap();
        assert_eq!(terminal.stage, Stage::Seal);
        expected.assert_terminal(terminal.input.as_ref().unwrap());
        assert!(terminal.output.is_none());
        assert_eq!(terminal.progress.attempted, !matches!(case, 2 | 3));
        assert_eq!(
            terminal.progress.returned_success,
            match case {
                0 => Some(false),
                4 | 5 => Some(true),
                _ => None,
            }
        );
        assert_eq!(
            f.engine.allocations[0].phase,
            SharedAllocationPhaseV1::CpuWritable
        );
        assert_eq!(
            f.engine.allocations[0].mapping.as_ref().unwrap().writable,
            case < 4
        );
        assert_complete_record(&f, 1);
        assert_closed(&mut f);
    }
}

fn mutable_map_prefixes<P: MutableGpuGttProfileV1>() {
    for prefix in 0..=2 {
        for errno in [false, true] {
            if prefix == 1 && !errno {
                continue;
            }
            let mut f = fixture(true);
            let token = allocate::<P>(&mut f);
            let expected = TokenSnapshot::new(&token);
            let usage = f.engine.host_backing_account.as_ref().unwrap().usage();
            f.engine.backend.map_progress = prefix;
            f.engine.backend.map_errno = errno;
            assert!(map_mutable(&mut f, token).is_err());
            let terminal = f.engine.terminal_transition.as_ref().unwrap();
            assert_eq!(terminal.stage, Stage::Map);
            expected.assert_terminal(terminal.input.as_ref().unwrap());
            assert!(terminal.output.is_none());
            assert!(terminal.progress.attempted);
            assert_eq!(terminal.progress.returned_map_prefix, Some(prefix));
            assert_eq!(terminal.progress.returned_success, Some(!errno));
            assert_eq!(
                f.engine.host_backing_account.as_ref().unwrap().usage(),
                usage
            );
            assert_complete_record(&f, 1);
            assert_closed(&mut f);
        }
    }
}

#[test]
fn session_mutable_map_partial_and_errno_outputs_keep_exact_input() {
    mutable_map_prefixes::<HostVisibleCoherentGttV1>();
    mutable_map_prefixes::<KernargGttV1>();
    mutable_map_prefixes::<AqlQueueGttV1>();
    mutable_map_prefixes::<ExecutableAqlQueueProbeGttV1>();
    mutable_map_prefixes::<UserptrAqlQueueProbeGttV1>();
    mutable_map_prefixes::<UserptrAqlControlGttV1>();
}

enum ControlInput {
    Kernarg(SharedGttAllocationV1<KernargGttV1, GttCpuWritableV1>),
    Executable(SharedGttAllocationV1<ExecutableGttV1, GttExecutableImmutableV1>),
}

impl ControlInput {
    fn new(f: &mut BackingConstructorFixture, executable: bool) -> Self {
        if executable {
            let token = allocate::<ExecutableGttV1>(f);
            Self::Executable(adapter::seal_v1(&mut f.engine, token).unwrap())
        } else {
            Self::Kernarg(allocate(f))
        }
    }

    fn snapshot(&self) -> TokenSnapshot {
        match self {
            Self::Kernarg(token) => TokenSnapshot::new(token),
            Self::Executable(token) => TokenSnapshot::new(token),
        }
    }

    fn map(
        self,
        f: &mut BackingConstructorFixture,
        fault: Option<(Stage, Fault)>,
    ) -> Result<(), MemorySessionError> {
        let mut projection = adapter::ProjectionV1::new(&mut f.foundation, f.device, f.vm);
        projection.fault = fault;
        match self {
            Self::Kernarg(token) => {
                adapter::map_mutable_v1(&mut f.engine, &mut projection, token, || {
                    panic!("unexpected poison")
                })
                .map(|_| ())
            }
            Self::Executable(token) => {
                adapter::map_executable_v1(&mut f.engine, &mut projection, token, || {
                    panic!("unexpected poison")
                })
                .map(|_| ())
            }
        }
    }
}

#[test]
fn session_executable_map_partial_and_errno_outputs_keep_exact_input() {
    for prefix in 0..=2 {
        for errno in [false, true] {
            if prefix == 1 && !errno {
                continue;
            }
            let mut f = fixture(false);
            let input = ControlInput::new(&mut f, true);
            let expected = input.snapshot();
            f.engine.backend.map_progress = prefix;
            f.engine.backend.map_errno = errno;
            assert!(input.map(&mut f, None).is_err());
            let terminal = f.engine.terminal_transition.as_ref().unwrap();
            expected.assert_terminal(terminal.input.as_ref().unwrap());
            assert!(terminal.output.is_none());
            assert_eq!(terminal.progress.returned_map_prefix, Some(prefix));
            assert_eq!(terminal.progress.returned_success, Some(!errno));
            assert_closed(&mut f);
        }
    }
}

#[test]
fn session_mapping_native_and_currentness_panics_keep_original_token() {
    for executable in [false, true] {
        for case in 0..5 {
            let mut f = fixture(false);
            let input = ControlInput::new(&mut f, executable);
            let expected = input.snapshot();
            let before = f.engine.backend.currentness_calls;
            match case {
                0 => f.engine.backend.panic_operation = Some("map_gpu"),
                1 => f.engine.backend.fail_currentness_at = Some(before + 1),
                2 => f.engine.backend.panic_currentness_at = Some(before + 1),
                3 => f.engine.backend.fail_currentness_at = Some(before + 2),
                4 => f.engine.backend.panic_currentness_at = Some(before + 2),
                _ => unreachable!(),
            }
            let result = catch_unwind(AssertUnwindSafe(|| input.map(&mut f, None)));
            if matches!(case, 1 | 3) {
                assert!(result.unwrap().is_err());
            } else {
                let operation = if case == 0 { "map_gpu" } else { "currentness" };
                assert_eq!(
                    result.err().unwrap().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", operation))
                );
            }
            let terminal = f.engine.terminal_transition.as_ref().unwrap();
            expected.assert_terminal(terminal.input.as_ref().unwrap());
            assert!(terminal.output.is_none());
            assert_eq!(terminal.progress.attempted, matches!(case, 0 | 3 | 4));
            assert_eq!(
                terminal.progress.returned_map_prefix,
                if case >= 3 { Some(1) } else { None }
            );
            assert_complete_record(&f, expected.id);
            assert_closed(&mut f);
        }
    }
}

#[test]
fn session_map_projection_rejection_retains_mapped_successor() {
    for executable in [false, true] {
        let mut f = fixture(false);
        let input = ControlInput::new(&mut f, executable);
        let mut expected = input.snapshot();
        expected.state = if executable {
            TypeId::of::<GttGpuAccessibleExecutableV1>()
        } else {
            TypeId::of::<GttGpuAccessibleMutableV1>()
        };
        let (_, _, mapping) = model_keys(f.vm, expected.id, expected.generation);
        let model = f
            .foundation
            .memory()
            .next(MemoryTransitionV1::BeginMap {
                key: mapping,
                target_devices: vec![f.device.model_key()],
                access: MemoryAccessV1::ReadWrite,
            })
            .unwrap();
        f.foundation
            .replace_memory_after_sealed_transition(model)
            .unwrap();
        let before = f.foundation.memory().clone();
        assert!(matches!(
            input.map(&mut f, None),
            Err(MemorySessionError::Model("shared map projection"))
        ));
        let terminal = f.engine.terminal_transition.as_ref().unwrap();
        assert_eq!(terminal.stage, Stage::MapProjection);
        assert!(terminal.input.is_none());
        expected.assert_terminal(terminal.output.as_ref().unwrap());
        assert_eq!(terminal.progress.returned_map_prefix, Some(1));
        assert_eq!(f.foundation.memory(), &before);
        assert_closed(&mut f);
    }
}

#[test]
fn session_map_stage_faults_preserve_precursor_or_successor_exactly() {
    for executable in [false, true] {
        for (stage, fault) in [
            (Stage::MappingEvidence, Fault::Error),
            (Stage::MappingEvidence, Fault::Panic),
            (Stage::Map, Fault::Error),
            (Stage::Map, Fault::Panic),
            (Stage::MapProjection, Fault::Error),
            (Stage::MapProjection, Fault::Panic),
            (Stage::MapCommit, Fault::Error),
            (Stage::MapCommit, Fault::Panic),
            (Stage::MapCommit, Fault::ExhaustRevision),
        ] {
            let mut f = fixture(true);
            let input = ControlInput::new(&mut f, executable);
            let mut expected = input.snapshot();
            let mapped = matches!(stage, Stage::MapProjection | Stage::MapCommit);
            if mapped {
                expected.state = if executable {
                    TypeId::of::<GttGpuAccessibleExecutableV1>()
                } else {
                    TypeId::of::<GttGpuAccessibleMutableV1>()
                };
            }
            certify(&mut f, 0);
            let before = f.foundation.memory().clone();
            let calls_before = calls(&f.engine);
            let result = catch_unwind(AssertUnwindSafe(|| input.map(&mut f, Some((stage, fault)))));
            match fault {
                Fault::Panic => assert_eq!(
                    result.err().unwrap().downcast_ref::<(&str, Stage)>(),
                    Some(&("session projection", stage))
                ),
                Fault::Error | Fault::ExhaustRevision => assert!(result.unwrap().is_err()),
            }
            let terminal = f.engine.terminal_transition.as_ref().unwrap();
            assert_eq!(terminal.stage, stage);
            assert_eq!(terminal.progress.attempted, mapped);
            if mapped {
                assert!(terminal.input.is_none());
                expected.assert_terminal(terminal.output.as_ref().unwrap());
                assert_eq!(terminal.progress.returned_map_prefix, Some(1));
            } else {
                assert!(terminal.output.is_none());
                expected.assert_terminal(terminal.input.as_ref().unwrap());
                assert_eq!(terminal.progress.returned_map_prefix, None);
                assert_eq!(calls(&f.engine), calls_before);
            }
            assert_eq!(f.foundation.memory(), &before);
            assert_closed(&mut f);
        }
    }
}

#[test]
fn session_retention_validates_borrowed_input_before_consumption() {
    for field in 0..6 {
        let mut f = fixture(true);
        let token = allocate::<KernargGttV1>(&mut f);
        let mut token = map_mutable(&mut f, token).unwrap();
        match field {
            0 => token.session_id += 1,
            1 => token.generation += 1,
            2 => token.layout.requested_bytes += 1,
            3 => f.engine.allocations[0].profile = SharedGttProfileV1::Executable,
            4 => f.engine.allocations[0].phase = SharedAllocationPhaseV1::CpuWritable,
            5 => token.id += 1,
            _ => unreachable!(),
        }
        let expected = TokenSnapshot::new(&token);
        let before = calls(&f.engine);
        assert!(adapter::retained_facts_v1(&f.engine, f.vm, &token).is_err());
        assert_eq!(TokenSnapshot::new(&token), expected);
        assert!(matches!(
            adapter::retain_v1::<_, AqlDispatchKernargResourceRoleV1, _, _>(
                &mut f.engine,
                f.vm,
                token,
            ),
            Err(MemorySessionError::InvalidAllocationAuthority)
        ));
        assert_eq!(calls(&f.engine), before);
        assert!(f.engine.terminal_transition.is_none());
        assert_eq!(f.engine.phase(), SharedMemorySessionPhaseV1::Active);
    }
}

fn materialization_panic<P: GttProfileV1>() {
    for backend_panic in [false, true] {
        let mut f = fixture(true);
        let mut token = allocate::<P>(&mut f);
        let expected = TokenSnapshot::new(&token);
        let before = f.engine.backend.currentness_calls;
        f.engine.backend.panic_currentness_at = Some(before + 2);
        if backend_panic {
            f.engine.allocations[0]
                .mapping
                .as_mut()
                .unwrap()
                .panic_access = Some("with_bytes_mut");
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            f.engine.with_bytes_mut(&mut token, |bytes| {
                bytes[0] = 0x7b;
                std::panic::panic_any(("control callback", "materialize"));
            })
        }));
        let wanted = if backend_panic {
            ("N2 native panic", "with_bytes_mut")
        } else {
            ("control callback", "materialize")
        };
        assert_eq!(
            result.err().unwrap().downcast_ref::<(&str, &str)>(),
            Some(&wanted)
        );
        assert_eq!(TokenSnapshot::new(&token), expected);
        assert_eq!(f.engine.backend.currentness_calls, before + 1);
        assert_eq!(
            f.engine.allocations[0].mapping.as_ref().unwrap().bytes[0],
            if backend_panic { 0 } else { 0x7b }
        );
        assert_complete_record(&f, expected.id);
        assert_closed(&mut f);
    }
}

#[test]
fn session_control_materialization_panic_preserves_original_borrowed_token() {
    materialization_panic::<ExecutableGttV1>();
    materialization_panic::<KernargGttV1>();
}

#[test]
fn session_retention_rejects_actual_host_account_substitution_without_effects() {
    let mut f = fixture(true);
    let token = allocate::<HostVisibleCoherentGttV1>(&mut f);
    let token = map_mutable(&mut f, token).unwrap();
    let original = f.engine.host_backing_account.take().unwrap();
    let original_usage = original.usage();
    f.engine.host_backing_account = Some(
        HostBackingAccountV1::new(
            f.engine.session_id,
            f.device.model_key(),
            f.vm,
            Gfx942HostVisibleBackingBudgetV1::new(1 << 20, 256).unwrap(),
        )
        .unwrap(),
    );
    let before = calls(&f.engine);
    assert!(matches!(
        adapter::retain_v1::<_, AqlCompletionSignalResourceRoleV1, _, _>(
            &mut f.engine,
            f.vm,
            token,
        ),
        Err(MemorySessionError::InvalidAllocationAuthority)
    ));
    assert_eq!(calls(&f.engine), before);
    assert_eq!(original.usage(), original_usage);
    assert_eq!(
        f.engine
            .host_backing_account
            .as_ref()
            .unwrap()
            .usage()
            .retained_records,
        0
    );
    assert!(f.engine.allocations[0].host_backing_charge.is_some());
    assert!(f.engine.terminal_transition.is_none());
    assert_eq!(f.engine.phase(), SharedMemorySessionPhaseV1::Active);
}

#[test]
fn session_host_map_panic_and_currentness_keep_exact_n1_debit() {
    for case in 0..5 {
        let mut f = fixture(true);
        let token = allocate::<HostVisibleCoherentGttV1>(&mut f);
        let expected = TokenSnapshot::new(&token);
        let usage = f.engine.host_backing_account.as_ref().unwrap().usage();
        let before = f.engine.backend.currentness_calls;
        match case {
            0 => f.engine.backend.panic_operation = Some("map_gpu"),
            1 => f.engine.backend.fail_currentness_at = Some(before + 1),
            2 => f.engine.backend.panic_currentness_at = Some(before + 1),
            3 => f.engine.backend.fail_currentness_at = Some(before + 2),
            4 => f.engine.backend.panic_currentness_at = Some(before + 2),
            _ => unreachable!(),
        }
        let result = catch_unwind(AssertUnwindSafe(|| map_mutable(&mut f, token)));
        if matches!(case, 1 | 3) {
            assert!(result.unwrap().is_err());
        } else {
            let operation = if case == 0 { "map_gpu" } else { "currentness" };
            assert_eq!(
                result.err().unwrap().downcast_ref::<(&str, &str)>(),
                Some(&("N2 native panic", operation))
            );
        }
        let terminal = f.engine.terminal_transition.as_ref().unwrap();
        expected.assert_terminal(terminal.input.as_ref().unwrap());
        assert!(terminal.output.is_none());
        assert_eq!(
            f.engine.host_backing_account.as_ref().unwrap().usage(),
            usage
        );
        assert_complete_record(&f, expected.id);
        assert_closed(&mut f);
    }
}

fn materialization_currentness_panic<P: GttProfileV1>() {
    for offset in [1, 2] {
        let mut f = fixture(false);
        let mut token = allocate::<P>(&mut f);
        let expected = TokenSnapshot::new(&token);
        let before = f.engine.backend.currentness_calls;
        f.engine.backend.panic_currentness_at = Some(before + offset);
        let result = catch_unwind(AssertUnwindSafe(|| {
            f.engine.with_bytes_mut(&mut token, |bytes| {
                bytes.fill(0x5a);
            })
        }));
        assert_eq!(
            result.err().unwrap().downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", "currentness"))
        );
        assert_eq!(TokenSnapshot::new(&token), expected);
        assert_eq!(f.engine.backend.currentness_calls, before + offset);
        assert_eq!(
            f.engine.allocations[0].mapping.as_ref().unwrap().bytes,
            vec![if offset == 1 { 0 } else { 0x5a }; 4096]
        );
        assert_closed(&mut f);
    }
}

#[test]
fn session_control_materialization_currentness_panic_closes_with_borrowed_owner() {
    materialization_currentness_panic::<ExecutableGttV1>();
    materialization_currentness_panic::<KernargGttV1>();
}

#[test]
fn session_control_failure_preserves_existing_n1_n2_records_and_charges() {
    let mut f = fixture(true);
    let host = allocate::<HostVisibleCoherentGttV1>(&mut f);
    let host = map_mutable(&mut f, host).unwrap();
    let host_before = TokenSnapshot::new(&host);
    let device = f.mapped_device();
    let device_before = device.lease.storage_identity();
    let host_usage = f.engine.host_backing_account.as_ref().unwrap().usage();
    let device_usage = f.usage();
    let input = ControlInput::new(&mut f, true);
    f.engine.backend.map_errno = true;
    assert!(input.map(&mut f, None).is_err());
    assert_eq!(TokenSnapshot::new(&host), host_before);
    assert_eq!(device.lease.storage_identity(), device_before);
    assert_eq!(
        f.engine.host_backing_account.as_ref().unwrap().usage(),
        host_usage
    );
    assert_eq!(f.usage(), device_usage);
    assert!(f.engine.allocations[0].host_backing_charge.is_some());
    assert!(f.engine.allocations[1].host_backing_charge.is_none());
    assert!(f.engine.device_memory[0].backing_charge.is_some());
    assert_complete_record(&f, host.id);
    assert_closed(&mut f);
}

#[test]
fn session_terminal_transition_cannot_be_overwritten_or_retried() {
    let mut f = fixture(false);
    let token = allocate::<KernargGttV1>(&mut f);
    let expected = TokenSnapshot::new(&token);
    f.engine.backend.map_errno = true;
    assert!(map_mutable(&mut f, token).is_err());
    assert_closed(&mut f);
    // Even inconsistent phase state cannot admit another attempt over custody.
    f.engine.phase = SharedMemorySessionPhaseV1::Active;
    let before = calls(&f.engine);
    assert!(matches!(
        adapter::allocate_v1::<_, KernargGttV1>(
            &mut f.engine,
            &mut adapter::ProjectionV1::new(&mut f.foundation, f.device, f.vm),
            4096,
            || panic!("occupied transition must reject before preflight"),
        ),
        Err(MemorySessionError::SharedSessionQuarantined)
    ));
    assert_eq!(calls(&f.engine), before);
    expected.assert_terminal(
        f.engine
            .terminal_transition
            .as_ref()
            .unwrap()
            .input
            .as_ref()
            .unwrap(),
    );
    assert_closed(&mut f);
}

#[test]
fn session_control_success_preserves_exact_foundation_loan_and_authority() {
    let mut f = fixture(true);
    let mut queue = f.transfer(&[]).unwrap();
    let loan = f
        .ownership
        .loan_foundation(
            f.engine.session_id,
            &mut f.foundation,
            &mut queue,
            f.device,
            f.vm,
        )
        .unwrap();
    let mut code = allocate::<ExecutableGttV1>(&mut f);
    f.engine
        .with_bytes_mut(&mut code, |bytes| bytes.fill(0x31))
        .unwrap();
    let code_identity = code.storage_identity();
    let code = adapter::seal_v1(&mut f.engine, code).unwrap();
    let code = adapter::map_executable_v1(
        &mut f.engine,
        &mut adapter::ProjectionV1::new(&mut f.foundation, f.device, f.vm),
        code,
        || panic!("unexpected poison"),
    )
    .unwrap();
    let code =
        adapter::retain_v1::<_, AqlDispatchCodeResourceRoleV1, _, _>(&mut f.engine, f.vm, code)
            .unwrap();
    assert_eq!(code.token.storage_identity(), code_identity);
    assert_eq!(
        code.facts.mapping,
        model_keys(f.vm, code_identity.id, code_identity.generation).2
    );
    let mut kernarg = allocate::<KernargGttV1>(&mut f);
    f.engine
        .with_bytes_mut(&mut kernarg, |bytes| bytes.fill(0x42))
        .unwrap();
    let kernarg_identity = kernarg.storage_identity();
    let kernarg = map_mutable(&mut f, kernarg).unwrap();
    let kernarg = adapter::retain_v1::<_, AqlDispatchKernargResourceRoleV1, _, _>(
        &mut f.engine,
        f.vm,
        kernarg,
    )
    .unwrap();
    assert_eq!(kernarg.token.storage_identity(), kernarg_identity);
    assert_eq!(
        f.engine.allocations[0].mapping.as_ref().unwrap().bytes,
        vec![0x31; 4096]
    );
    assert_eq!(
        f.engine.allocations[1].mapping.as_ref().unwrap().bytes,
        vec![0x42; 4096]
    );
    f.ownership
        .reclaim_foundation(
            f.engine.session_id,
            &mut f.foundation,
            &mut queue,
            f.device,
            f.vm,
            loan,
        )
        .unwrap();
    assert_eq!(queue.memory().validate_global_invariants(), Ok(()));
    assert_eq!(queue.memory().mappings().len(), 2);
    assert!(f.engine.terminal_transition.is_none());
    assert!(f.engine.pending_allocation.is_none());
    assert_eq!(f.engine.backend.map_gpu_calls, 2);
    assert_eq!(f.engine.phase(), SharedMemorySessionPhaseV1::Active);
}
