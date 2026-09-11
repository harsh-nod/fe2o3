use super::*;
use crate::queue_linux::primary_fixture::{
    LocalCreationArmV1, LocalEventV1, LocalPublishedV1, LocalUnpublishedV1,
};
use fe2o3_kfd_uapi::{KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES, KfdGfx942CreateQueueOutputs};

#[path = "integration_platform_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum Role {
    Runtime,
    Control,
    CreationArm,
    Event,
    Shadow,
    Doorbell,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) struct OwnerIdentity {
    id: usize,
    role: Role,
    session: u64,
    pid: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShadowPhase {
    Unpublished,
    Published,
    Disposed,
}

pub(super) struct Owner {
    trace: Rc<RefCell<Trace>>,
    identity: OwnerIdentity,
    shadow: Option<(ShadowPhase, OwnerIdentity, SharedGttAllocationIdentityV1)>,
    doorbell: Option<KfdGfx942CreateQueueOutputs>,
    native_event_id: Option<u32>,
    local_arm: Option<LocalCreationArmV1>,
    local_event: Option<LocalEventV1>,
    local_unpublished: Option<LocalUnpublishedV1>,
    local_published: Option<LocalPublishedV1>,
}

impl Owner {
    pub(super) fn new(role: Role) -> Self {
        let trace = trace();
        let mut t = trace.borrow_mut();
        let identity = OwnerIdentity {
            id: t.minted.len() + 1,
            role,
            session: t.session,
            pid: std::process::id(),
        };
        t.minted.push(identity);
        if role == Role::Runtime
            && let Some(gate) = &t.local_gate
        {
            gate.admit(identity.pid).unwrap();
        }
        let local_event =
            (role == Role::Event && t.local_gate.is_some()).then(|| t.local_resources.event());
        let event_id = 10 + t.minted.iter().filter(|id| id.role == Role::Event).count() as u32;
        let native_event_id =
            (role == Role::Event).then(|| local_event.as_ref().map_or(event_id, LocalEventV1::id));
        drop(t);
        Self {
            trace,
            identity,
            shadow: None,
            doorbell: None,
            native_event_id,
            local_arm: None,
            local_event,
            local_unpublished: None,
            local_published: None,
        }
    }

    fn validate(&self, role: Role, memory: &Memory) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        if !Rc::ptr_eq(&self.trace, &trace())
            || self.identity.role != role
            || self.identity.session != memory.primary_session_id()
            || self.identity.pid != memory.opener_pid()
        {
            return Err(ComputeAqlQueueSessionErrorV1::Contract(
                "platform owner binding",
            ));
        }
        Ok(())
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        self.trace.borrow_mut().drops += 1;
    }
}

pub(super) struct Fixture;

fn observe(name: &'static str) {
    let occurrence = record(name);
    if let Some((at, nth, true)) = trace().borrow().fault
        && (at, nth) == (name, occurrence)
    {
        std::panic::panic_any((name, occurrence));
    }
}

impl PrimaryEnvironmentV1 for Fixture {
    type Memory = Memory;
    type Runtime = Owner;
    type RuntimeControl = Owner;
    type Event = Owner;
    type Unpublished = Owner;
    type Published = Owner;
    type Doorbell = Owner;
    type CreationArm = Owner;

    fn enable_runtime(_: &mut Memory) -> Result<Owner, ComputeAqlQueueSessionErrorV1> {
        step("runtime-enable")?;
        Ok(Owner::new(Role::Runtime))
    }
    fn validate_runtime(
        runtime: &Owner,
        control: Option<&Owner>,
        memory: &Memory,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        step("runtime-validate")?;
        runtime.validate(Role::Runtime, memory)?;
        if let Some(control) = control {
            control.validate(Role::Control, memory)?;
        }
        Ok(())
    }
    fn arm_creation(_: &Memory) -> Result<Owner, ComputeAqlQueueSessionErrorV1> {
        step("gate-arm")?;
        let arm = trace()
            .borrow()
            .local_gate
            .as_ref()
            .map(LocalGateV1::arm)
            .transpose()?;
        let mut owner = Owner::new(Role::CreationArm);
        owner.local_arm = arm;
        Ok(owner)
    }
    fn create_event(_: &mut Memory) -> Result<Owner, ComputeAqlQueueSessionErrorV1> {
        step("event")?;
        Ok(Owner::new(Role::Event))
    }
    fn install_shadows(
        memory: &mut Memory,
        context: &Cpu<ExecutableGttV1>,
        event: &Owner,
    ) -> Result<Owner, ComputeAqlQueueSessionErrorV1> {
        step("shadow-install")?;
        event.validate(Role::Event, memory)?;
        assert_eq!(
            Some(context.storage_identity()),
            memory.primary_identities().last().copied()
        );
        let local = event
            .local_event
            .as_ref()
            .map(LocalUnpublishedV1::install)
            .transpose()?;
        let mut owner = Owner::new(Role::Shadow);
        owner.local_unpublished = local;
        owner.shadow = Some((
            ShadowPhase::Unpublished,
            event.identity,
            context.storage_identity(),
        ));
        Ok(owner)
    }
    fn initialize_shadows(
        memory: &mut Memory,
        context: &mut Cpu<ExecutableGttV1>,
        shadows: &Owner,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        step("shadow-init")?;
        shadows.validate(Role::Shadow, memory)?;
        let (phase, _, identity) = shadows.shadow.unwrap();
        assert_eq!(phase, ShadowPhase::Unpublished);
        assert_eq!(identity, context.storage_identity());
        memory
            .primary_write(context, |bytes| {
                assert!(bytes.iter().all(|&b| b == 0));
                if let Some(local) = &shadows.local_unpublished {
                    local.initialize(bytes)
                } else {
                    Ok(())
                }
            })?
            .map_err(|_| {
                ComputeAqlQueueSessionErrorV1::Contract("gfx942 CWSR header initialization")
            })?;
        Ok(())
    }
    fn validate_event(
        memory: &Memory,
        event: &Owner,
        shadows: &Owner,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        step("event-validate")?;
        event.validate(Role::Event, memory)?;
        shadows.validate(Role::Shadow, memory)?;
        let (phase, parent, _) = shadows.shadow.unwrap();
        assert_eq!(phase, ShadowPhase::Unpublished);
        assert_eq!(parent, event.identity);
        if let Some(local) = &shadows.local_unpublished {
            local.validate(event.local_event.as_ref().unwrap())?;
        }
        Ok(())
    }
    fn restore_shadow_write(shadows: &Owner) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        step("shadow-restore")?;
        assert_eq!(shadows.shadow.unwrap().0, ShadowPhase::Unpublished);
        if let Some(local) = &shadows.local_unpublished {
            local.protect_read_only()?;
            local.restore_write()?;
        }
        Ok(())
    }
    fn publish_shadows(mut owner: Owner) -> Owner {
        record("publish");
        let phase = &mut owner.shadow.as_mut().unwrap().0;
        assert_eq!(*phase, ShadowPhase::Unpublished);
        *phase = ShadowPhase::Published;
        owner.local_published = owner
            .local_unpublished
            .take()
            .map(LocalUnpublishedV1::publish);
        owner
    }
    fn cleanup_unpublished(owner: &mut Owner) {
        record("cleanup");
        let trace = trace();
        let mut t = trace.borrow_mut();
        assert!(t.poison, "poison must precede cleanup");
        assert_eq!(owner.shadow.unwrap().0, ShadowPhase::Unpublished);
        owner.shadow.as_mut().unwrap().0 = ShadowPhase::Disposed;
        if let Some(local) = &mut owner.local_unpublished {
            local.cleanup_terminal();
        }
        t.cleanup += 1;
    }
    fn mark_queue_created(runtime: &mut Owner) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        assert_eq!(runtime.identity.role, Role::Runtime);
        step("runtime-created").map_err(|e| terminal_creation("runtime queue-live transition", e))
    }
    fn recover_create_outputs(
        engine: &NativeQueueEngineV1<PrimaryQueueBackendV1<Memory>>,
        key: QueueKeyV1,
    ) -> Option<KfdGfx942CreateQueueOutputs> {
        step("recover-outputs").ok()?;
        engine.create_outputs(key)
    }
    fn recover_native_queue_id(
        engine: &NativeQueueEngineV1<PrimaryQueueBackendV1<Memory>>,
        key: QueueKeyV1,
    ) -> Option<u32> {
        step("recover-id").ok()?;
        engine.native_queue_id(key)
    }
    fn create_dependency_owner(
        key: QueueKeyV1,
    ) -> Result<ComputeDependencySessionOwnerV1, ComputeDependencyTargetUseErrorV1> {
        if step("dependency").is_err() {
            return if trace().borrow().dependency_invalid {
                ComputeDependencySessionOwnerV1::new(0)
            } else {
                Err(ComputeDependencyTargetUseErrorV1::Allocation)
            };
        }
        ComputeDependencySessionOwnerV1::new(key.id.0)
    }
    fn event_id(event: &Owner) -> u32 {
        observe("event-id");
        assert_eq!(event.identity.role, Role::Event);
        event.native_event_id.unwrap()
    }
    fn map_doorbell(
        memory: &Memory,
        outputs: KfdGfx942CreateQueueOutputs,
        pid: u32,
    ) -> Result<Owner, ComputeAqlQueueSessionErrorV1> {
        step("doorbell").map_err(|e| terminal_creation("doorbell mapping", e))?;
        assert_eq!(
            pid,
            memory.opener_pid(),
            "exact opener PID passed to doorbell mapping"
        );
        assert_eq!(trace().borrow().session, memory.primary_session_id());
        assert_eq!(
            trace().borrow().create_return,
            Some((outputs.queue_id().value(), outputs.doorbell_offset().raw()))
        );
        let mut owner = Owner::new(Role::Doorbell);
        owner.doorbell = Some(outputs);
        Ok(owner)
    }
    fn doorbell_observation(doorbell: &Owner) -> (usize, u64) {
        observe("doorbell-observe");
        let outputs = doorbell.doorbell.unwrap();
        (
            KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES as usize,
            outputs.doorbell_offset().in_process_byte_offset(),
        )
    }
    fn finish_creation(arm: &mut Owner, pid: u32) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        assert_eq!(arm.identity.role, Role::CreationArm);
        assert_eq!(arm.identity.pid, pid);
        step("gate-finish")?;
        if let Some(local) = &mut arm.local_arm {
            let trace = trace();
            let t = trace.borrow();
            if t.local_finish_poison {
                t.local_gate.as_ref().unwrap().poison();
            }
            local.finish(pid)?;
        }
        Ok(())
    }
    fn poison() {
        record("poison");
        let trace = trace();
        let mut t = trace.borrow_mut();
        t.poison = true;
        if let Some(gate) = &t.local_gate {
            gate.poison();
        }
    }
}

pub(super) fn assert_platform<P>(root: &Root<P>, external: Option<&ExternalSlots>) {
    assert_platform_partition(platform_identities(root, external));
}

pub(super) fn platform_identities<P>(
    root: &Root<P>,
    external: Option<&ExternalSlots>,
) -> Vec<OwnerIdentity> {
    let trace = trace();
    let mut owners = Vec::new();
    let mut add = |owner: &Owner, role: Role, phase: Option<ShadowPhase>| {
        owner.validate(role, memory(root)).unwrap();
        if let Some(phase) = phase {
            let (actual, event, context) = owner.shadow.unwrap();
            assert_eq!(actual, phase);
            let event_owner = root
                .event
                .as_ref()
                .or_else(|| root.completed.as_ref().map(|c| &c.event))
                .unwrap();
            assert_eq!(event, event_owner.identity);
            assert!(memory(root).primary_identities().contains(&context));
        }
        if role == Role::Doorbell {
            assert_eq!(owner.doorbell, root.outputs);
        }
        owners.push(owner.identity);
    };
    for (slot, role) in [
        (&root.runtime, Role::Runtime),
        (&root.runtime_control, Role::Control),
        (&root.creation_arm, Role::CreationArm),
        (&root.event, Role::Event),
    ] {
        if let Some(owner) = slot {
            add(owner, role, None);
        }
    }
    if let Some(owner) = &root.unpublished {
        add(owner, Role::Shadow, Some(ShadowPhase::Disposed));
    }
    if let Some(owner) = &root.published {
        add(owner, Role::Shadow, Some(ShadowPhase::Published));
    }
    if let Some(c) = &root.completed {
        assert_eq!(Some(c.observation.event_id), c.event.native_event_id);
        if let Some(local) = &c.event.local_event {
            assert_eq!(c.observation.event_id, local.id());
        }
        add(&c.runtime, Role::Runtime, None);
        if let Some(owner) = &c.runtime_control {
            add(owner, Role::Control, None);
        }
        add(&c.event, Role::Event, None);
        add(&c.shadows, Role::Shadow, Some(ShadowPhase::Published));
        if let Some(owner) = &c.doorbell {
            add(owner, Role::Doorbell, None);
        }
    }
    if let Some(external) = external {
        for owner in external.runtime.iter().chain(&external.control) {
            if Rc::ptr_eq(&owner.trace, &trace) {
                owners.push(owner.identity);
            }
        }
    }
    owners
}

fn assert_platform_partition(owners: Vec<OwnerIdentity>) {
    let trace = trace();
    let t = trace.borrow();
    assert_eq!(t.drops, 0);
    assert_eq!(
        owners.len(),
        t.minted.len(),
        "every minted platform owner remains rooted"
    );
    let actual: std::collections::HashSet<_> = owners.into_iter().collect();
    let expected: std::collections::HashSet<_> = t.minted.iter().copied().collect();
    assert_eq!(actual.len(), t.minted.len(), "unique platform custody");
    assert_eq!(actual, expected);
}

pub(super) fn assert_auxiliary_platform(
    primary: &Root,
    auxiliary: &crate::queue::live::construction_auxiliary::AuxiliaryConstructionV1<3, Fixture>,
    installed: Option<&ComputeAqlQueueLaneStateV1<Fixture>>,
) {
    let mut owners = platform_identities(primary, None);
    let memory = memory(primary);
    let lane = installed.or(auxiliary.completed.as_ref());
    let exception = lane.and_then(|l| l.exception.as_ref());
    let event = exception.map(|e| &e.event).or(auxiliary.event.as_ref());
    let mut add = |owner: &Owner, role: Role| {
        owner.validate(role, memory).unwrap();
        owners.push(owner.identity);
    };
    for (owner, role) in [
        (
            exception.map(|e| &e.runtime).or(auxiliary.runtime.as_ref()),
            Role::Runtime,
        ),
        (event, Role::Event),
        (auxiliary.creation_arm.as_ref(), Role::CreationArm),
    ] {
        if let Some(owner) = owner {
            add(owner, role);
        }
    }
    if let Some(shadows) = exception
        .map(|e| &e.shadows)
        .or(auxiliary.published.as_ref())
        .or(auxiliary.unpublished.as_ref())
    {
        add(shadows, Role::Shadow);
        let (phase, parent, context) = shadows.shadow.unwrap();
        assert_eq!(
            phase,
            if auxiliary.unpublished.is_some() {
                ShadowPhase::Disposed
            } else {
                ShadowPhase::Published
            }
        );
        assert_eq!(parent, event.unwrap().identity);
        let engine = &primary.completed.as_ref().unwrap().engine;
        let authority = engine
            .resources
            .iter()
            .find(|record| {
                record
                    .authority
                    .as_ref()
                    .is_some_and(|a| a.view.plan.queue == auxiliary.key.unwrap())
            })
            .unwrap()
            .authority
            .as_ref()
            .unwrap();
        assert_eq!(
            context,
            Memory::primary_token_identity(&authority.context_save)
        );
    }
    if let Some(lane) = lane {
        assert_eq!(
            Some(lane.observation.event_id),
            event.unwrap().native_event_id
        );
        if let Some(doorbell) = &lane.doorbell {
            add(doorbell, Role::Doorbell);
            assert_eq!(doorbell.doorbell, auxiliary.outputs);
            let output = auxiliary.outputs.unwrap();
            assert_eq!(
                trace().borrow().create_returns[1],
                (output.queue_id().value(), output.doorbell_offset().raw())
            );
        }
    }
    assert_platform_partition(owners);
}
