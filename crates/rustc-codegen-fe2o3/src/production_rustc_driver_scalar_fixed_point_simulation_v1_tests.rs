//! Complete bounded component-SIM observations and independent source reference.
use super::*;
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Scenario {
    pub(super) root: String,
    length: usize,
    value: u32,
    choose: u32,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Effect {
    Allocation(usize),
    Begin(u64),
    PreparedWrite(u64, usize),
    Write(u64, usize, u32),
    Return(u64),
    End(u64, bool),
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SimRow {
    pub(super) scenario: Scenario,
    trapped: bool,
    pub(super) pairs: usize,
    effects: usize,
    backing_sha256: [u8; 32],
    effects_sha256: [u8; 32],
    before_trace: [u8; 32],
    after_trace: [u8; 32],
    before_events: usize,
    after_events: usize,
    before_debug: usize,
    after_debug: usize,
}
pub(super) fn scenarios(case: ScalarCase) -> Vec<Scenario> {
    if case == ScalarCase::NormalNoop {
        return vec![Scenario {
            root: case.roots()[0].into(),
            length: 0,
            value: 0,
            choose: 0,
        }];
    }
    let mut rows = Vec::new();
    for length in [0, 1, 63, 64, 65] {
        for value in [0, 1, u32::MAX - 1, u32::MAX] {
            for choose in [0, 1] {
                rows.push(Scenario {
                    root: case.roots()[0].into(),
                    length,
                    value,
                    choose,
                });
            }
            rows.push(Scenario {
                root: case.roots()[1].into(),
                length,
                value,
                choose: 0,
            });
        }
    }
    rows
}
fn initial(scenario: &Scenario) -> Vec<u8> {
    if scenario.root == "scalar_noop_control" {
        return Vec::new();
    }
    (0..scenario.length + 8)
        .flat_map(|i| {
            let value = if i < 4 || i >= scenario.length + 4 {
                0xcafe0000_u32 + i as u32
            } else {
                0xdeadbeef
            };
            value.to_le_bytes()
        })
        .collect()
}
fn reference(s: &Scenario) -> (Vec<u8>, Vec<Effect>, bool) {
    let mut bytes = initial(s);
    // Canonical cooperative launch opens all lifecycles before dispatch.
    let mut effects = (0..64).map(Effect::Begin).collect::<Vec<_>>();
    if !bytes.is_empty() {
        effects.push(Effect::Allocation(bytes.len()));
    }
    for lane in 0..64_u64 {
        if (lane as usize) < s.length {
            let offset = 16 + lane as usize * 4;
            let mut write = |value: u32| {
                effects.push(Effect::PreparedWrite(lane, offset));
                effects.push(Effect::Write(lane, offset, value));
                bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            };
            if s.root == "scalar_effect_then_overflow" {
                write(0x13579bdf);
                let Some(value) = s.value.checked_add(1) else {
                    effects.extend((0..64).map(|begun| Effect::End(begun, true)));
                    return (bytes, effects, true);
                };
                write(value);
            } else {
                write(s.value ^ if s.choose == 0 { 0 } else { 3 });
            }
        }
        effects.push(Effect::Return(lane));
        effects.push(Effect::End(lane, false));
    }
    (bytes, effects, false)
}
fn check_reference(
    s: &Scenario,
    bytes: &[u8],
    initialized: &[bool],
    effects: &[Effect],
    trapped: bool,
) -> ResultV1<()> {
    let (expected_bytes, expected_effects, expected_trap) = reference(s);
    require(
        bytes == expected_bytes
            && initialized == vec![true; bytes.len()]
            && effects == expected_effects
            && trapped == expected_trap,
        Phase::Sim,
        "independent reference/effects/canaries/initialization/outcome mismatch",
    )
}
pub(super) fn validate_row(row: &SimRow, expected: &Scenario) -> ResultV1<()> {
    let (bytes, effects, trapped) = reference(expected);
    require(
        &row.scenario == expected
            && row.pairs == 2
            && row.trapped == trapped
            && row.effects == effects.len()
            && row.backing_sha256 == digest(&bytes)
            && row.effects_sha256 == digest(&serde_json::to_vec(&effects).unwrap())
            && row.before_trace != [0; 32]
            && row.after_trace != [0; 32]
            && row.before_events >= effects.len() / 2
            && row.after_events >= effects.len() / 2
            && row.before_events <= OBSERVATION_CAP
            && row.after_events <= OBSERVATION_CAP
            && row.before_debug <= OBSERVATION_CAP
            && row.after_debug <= OBSERVATION_CAP
            && (expected.root == "scalar_noop_control"
                || row.before_debug > 0 && row.after_debug > 0),
        Phase::Sim,
        "bound full SIM observation",
    )
}
// Synthetic protocol payloads only. Never used by actual source callbacks.
pub(super) fn protocol_rows(case: ScalarCase) -> Vec<SimRow> {
    scenarios(case)
        .into_iter()
        .map(|scenario| {
            let (bytes, effects, trapped) = reference(&scenario);
            SimRow {
                scenario,
                trapped,
                pairs: 2,
                effects: effects.len(),
                backing_sha256: digest(&bytes),
                effects_sha256: digest(&serde_json::to_vec(&effects).unwrap()),
                before_trace: [1; 32],
                after_trace: [2; 32],
                before_events: effects.len(),
                after_events: effects.len(),
                before_debug: 1,
                after_debug: 1,
            }
        })
        .collect()
}
#[derive(Default)]
struct Trace {
    allocation: Option<u64>,
    bytes: Vec<u8>,
    initialized: Vec<bool>,
    effects: Vec<Effect>,
    event_count: usize,
    debug_count: usize,
    hash: Sha256,
    trap_checkpoint: Option<sim::SimulationSiteV1>,
    failure: Option<String>,
}
struct Events<'a> {
    module: &'a Module,
    trace: Rc<RefCell<Trace>>,
}
struct Debugger<'a> {
    module: &'a Module,
    trace: Rc<RefCell<Trace>>,
}
fn valid_invocation(invocation: sim::SimulationInvocationV1) -> bool {
    let x = invocation.global[0];
    x < 64
        && invocation.global == [x, 0, 0]
        && invocation.local == [x as u32, 0, 0]
        && invocation.workgroup == [0, 0, 0]
        && invocation.workgroup_size == [64, 1, 1]
        && invocation.workgroup_count == [1, 1, 1]
        && invocation.launch_extent == [64, 1, 1]
}
fn site(
    module: &Module,
    function: usize,
    block: fe2o3_kernel_ir::BlockId,
    operation: Option<u32>,
) -> Option<sim::SimulationSiteV1> {
    let function = module.functions.get(function)?;
    let block = function
        .body
        .as_ref()?
        .blocks
        .iter()
        .find(|b| b.id == block)?;
    if operation.is_some_and(|op| op as usize >= block.operations.len()) {
        return None;
    }
    Some(sim::SimulationSiteV1 {
        function: function.id.clone(),
        block: block.id,
        operation,
    })
}
impl sim::SimulationEventSinkV1 for Events<'_> {
    fn record(
        &mut self,
        event: &sim::SimulationEventV1,
    ) -> Result<(), sim::SimulationEventSinkErrorV1> {
        use sim::SimulationEventKindV1 as E;
        let bad = || sim::SimulationEventSinkErrorV1 {
            detail: "unexpected/incomplete scalar source event".into(),
        };
        let mut trace = self.trace.borrow_mut();
        if trace.event_count >= OBSERVATION_CAP || !valid_invocation(event.invocation) {
            return Err(bad());
        }
        trace.event_count += 1;
        trace.hash.update(format!("event:{event:?}\n").as_bytes());
        let lane = event.invocation.global[0];
        if !matches!(event.kind, E::AllocationPreexisting { .. })
            && site(
                self.module,
                event.site.function_ordinal,
                event.site.block,
                event.site.operation,
            )
            .is_none()
        {
            return Err(bad());
        }
        let effect = match &event.kind {
            E::AllocationPreexisting {
                allocation,
                address_space: AddressSpace::Global,
                bytes,
            } => {
                if trace.allocation.replace(*allocation).is_some() {
                    return Err(bad());
                }
                Some(Effect::Allocation(*bytes))
            }
            E::InvocationBegin => Some(Effect::Begin(lane)),
            E::InvocationEnd { outcome } => Some(Effect::End(
                lane,
                *outcome == sim::SimulationExecutionOutcomeV1::Failed,
            )),
            E::Return => Some(Effect::Return(lane)),
            E::MemoryWrite {
                allocation,
                offset,
                bytes,
            } if Some(*allocation) == trace.allocation && *bytes == 4 => {
                Some(Effect::PreparedWrite(lane, *offset))
            }
            E::OperationBegin
            | E::OperationEnd { .. }
            | E::BlockEnter
            | E::Terminator
            | E::Branch { .. } => None,
            _ => return Err(bad()),
        };
        if let Some(effect) = effect {
            trace.effects.push(effect);
        }
        Ok(())
    }
}
impl Debugger<'_> {
    fn check(&mut self, record: &sim::SimulationDebugRecordV1) -> Result<(), String> {
        use sim::{SimulationDebugCollectionV1::Captured, SimulationDebugRecordKindV1 as D};
        let bad = || "unexpected/incomplete scalar source debug record".to_string();
        let mut trace = self.trace.borrow_mut();
        if trace.debug_count >= OBSERVATION_CAP
            || record.ordinal != trace.debug_count as u64
            || !valid_invocation(record.invocation)
        {
            return Err(bad());
        }
        let actual_site = site(
            self.module,
            record.site.function_ordinal,
            record.site.block,
            Some(record.site.operation),
        )
        .ok_or_else(bad)?;
        trace.debug_count += 1;
        trace.hash.update(format!("debug:{record:?}\n").as_bytes());
        match &record.kind {
            D::Checkpoint {
                phase,
                stack: Captured(stack),
                memory: Captured(memory),
            } => {
                if stack.is_empty()
                    || stack.len() > 4
                    || stack
                        .iter()
                        .any(|frame| !matches!(frame.values, Captured(_)))
                {
                    return Err(bad());
                }
                match memory.as_slice() {
                    [] if trace.allocation.is_none() => {}
                    [allocation]
                        if Some(allocation.allocation) == trace.allocation
                            && allocation.address_space == AddressSpace::Global
                            && allocation.access == AccessMode::ReadWrite
                            && allocation.alignment == 4 =>
                    {
                        if allocation.bytes.len() > 4096
                            || allocation.initialized.len() != allocation.bytes.len()
                        {
                            return Err(bad());
                        }
                        trace.bytes.clone_from(&allocation.bytes);
                        trace.initialized.clone_from(&allocation.initialized);
                    }
                    _ => return Err(bad()),
                }
                if *phase == sim::SimulationDebugCheckpointPhaseV1::BeforeOperation
                    && operation_at(self.module, &actual_site).is_some_and(|op| is_trap(&op.kind))
                {
                    trace.trap_checkpoint = Some(actual_site);
                }
            }
            D::Memory {
                access: sim::SimulationDebugMemoryAccessV1::WriteCommitted,
                allocation,
                byte_offset,
                byte_len: 4,
                address_space: AddressSpace::Global,
                value: sim::SimulationDebugValueV1::Scalar(value),
            } if Some(*allocation) == trace.allocation && value.ty() == ScalarType::U32 => {
                trace.effects.push(Effect::Write(
                    record.invocation.global[0],
                    *byte_offset,
                    value.bits() as u32,
                ));
            }
            _ => return Err(bad()),
        }
        Ok(())
    }
}
impl sim::SimulationDebugSinkV1 for Debugger<'_> {
    fn record(
        &mut self,
        record: sim::SimulationDebugRecordV1,
    ) -> sim::SimulationDebugSinkControlV1 {
        if let Err(error) = self.check(&record) {
            self.trace.borrow_mut().failure.get_or_insert(error);
        }
        sim::SimulationDebugSinkControlV1::Continue
    }
}
fn operation_at<'a>(module: &'a Module, site: &sim::SimulationSiteV1) -> Option<&'a Operation> {
    module
        .function(&site.function)?
        .body
        .as_ref()?
        .blocks
        .iter()
        .find(|b| b.id == site.block)?
        .operations
        .get(site.operation? as usize)
}
#[derive(Debug, Eq, PartialEq)]
struct Run {
    bytes: Vec<u8>,
    effects: Vec<Effect>,
    trapped: bool,
    hash: [u8; 32],
    events: usize,
    debug: usize,
}
fn request_for(s: &Scenario) -> ResultV1<sim::SimulationRequestV1> {
    let mut request =
        sim::SimulationRequestV1::new(s.root.as_str(), [64, 1, 1], [64, 1, 1], Vec::new());
    request.events = sim::EventPolicyV1::Enabled;
    if s.root == "scalar_noop_control" {
        return Ok(request);
    }
    let target = sim::SimulationTargetV1::amdgpu_64();
    let initial = initial(s);
    let buffer = sim::BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        initial.clone(),
        vec![true; initial.len()],
        target,
    )
    .map_err(|e| failure(Phase::Sim, e))?;
    request.shared_buffers.push(sim::SharedBufferV1 {
        id: sim::BufferBackingIdV1(17),
        buffer,
    });
    request
        .arguments
        .push(sim::SimulationArgumentV1::BufferView(
            sim::BufferViewArgumentV1::new(
                sim::BufferBackingIdV1(17),
                ScalarType::U32,
                AccessMode::ReadWrite,
                4,
                16,
                s.length,
                target,
            )
            .map_err(|e| failure(Phase::Sim, e))?,
        ));
    request
        .arguments
        .push(sim::SimulationArgumentV1::Scalar(sim::ScalarBitsV1::u32(
            s.value,
        )));
    if s.root == "scalar_checked_identity" {
        request
            .arguments
            .push(sim::SimulationArgumentV1::Scalar(sim::ScalarBitsV1::u32(
                s.choose,
            )));
    }
    Ok(request)
}
fn simulate_one(
    owner: &Owner,
    admitted: &sim::AdmittedSimulationModuleV1,
    s: &Scenario,
) -> ResultV1<Run> {
    let request = request_for(s)?;
    let saved = request.clone();
    let trace = Rc::new(RefCell::new(Trace::default()));
    let mut events = Events {
        module: owner.module(),
        trace: Rc::clone(&trace),
    };
    let mut debugger = Debugger {
        module: owner.module(),
        trace: Rc::clone(&trace),
    };
    let limits = sim::SimulationLimitsV1 {
        max_invocations: 64,
        max_workgroups: 1,
        max_scheduled_slots: 64,
        max_steps: OBSERVATION_CAP as u64,
        max_events: OBSERVATION_CAP as u64,
        ..sim::SimulationLimitsV1::default()
    };
    let result = admitted.simulate_debugged_scheduled_with_sinks(
        &request,
        sim::SimulationTargetV1::amdgpu_64(),
        limits,
        sim::SimulationScheduleRequestV1::RecordCanonical {
            max_decisions: OBSERVATION_CAP,
        },
        sim::SimulationDebugCaptureLimitsV1::new(4, 4096, 1, 4096)
            .map_err(|e| failure(Phase::Sim, e))?,
        &mut events,
        &mut debugger,
    );
    drop(events);
    drop(debugger);
    let trace = Rc::try_unwrap(trace)
        .map_err(|_| failure(Phase::Sim, "live sink escaped"))?
        .into_inner();
    let (bytes, effects, trapped) = reference(s);
    require(
        trace.failure.is_none() && request == saved,
        Phase::Sim,
        "debug capture incomplete or request mutated",
    )?;
    check_reference(s, &trace.bytes, &trace.initialized, &trace.effects, trapped)?;
    match result {
        Ok(execution) => {
            require(
                !trapped
                    && execution.identity() == admitted.identity()
                    && execution.arguments() == request.arguments
                    && execution.invocations_executed() == 64
                    && execution.events_emitted() as usize == trace.event_count
                    && !execution.grants_execution_authority()
                    && matches!(
                        execution.conflict_assessment(),
                        sim::SimulationConflictAssessmentV1::NoConflictsObserved
                    )
                    && matches!(
                        execution.race_assessment(),
                        sim::SimulationRaceAssessmentV1::NoRacesObserved {
                            first_ordered_conflict: None
                        }
                    ),
                Phase::Sim,
                "successful SIM identity/arguments/lifecycle/conflict/authority",
            )?;
            if bytes.is_empty() {
                require(
                    execution.shared_buffers().is_empty(),
                    Phase::Sim,
                    "noop buffers",
                )?;
            } else {
                let buffer = execution
                    .shared_buffer(sim::BufferBackingIdV1(17))
                    .ok_or_else(|| failure(Phase::Sim, "missing full backing"))?;
                require(
                    execution.shared_buffers().len() == 1
                        && buffer.bytes() == bytes
                        && buffer.initialized() == vec![true; bytes.len()],
                    Phase::Sim,
                    "successful full backing/canaries",
                )?;
            }
        }
        Err(sim::SimulationErrorV1::Execution(error)) => {
            require(
                trapped
                    && error.kind == sim::SimulationExecutionErrorKindV1::ReachedUnreachable
                    && error.observation_failure.is_none()
                    && error
                        .invocation
                        .is_some_and(|i| valid_invocation(i) && i.global[0] == 0)
                    && error.site.as_ref().is_some_and(|site| {
                        operation_at(owner.module(), site).is_some_and(|op| is_trap(&op.kind))
                    })
                    && error.site == trace.trap_checkpoint,
                Phase::Sim,
                "expected typed actual assertion trap after committed effect",
            )?;
        }
        Err(error) => return Err(failure(Phase::Sim, error)),
    }
    Ok(Run {
        bytes,
        effects,
        trapped,
        hash: trace.hash.finalize().into(),
        events: trace.event_count,
        debug: trace.debug_count,
    })
}
pub(super) fn simulate_matrix(
    before: &Owner,
    after: &Owner,
    case: ScalarCase,
) -> ResultV1<Vec<SimRow>> {
    let admit = |owner: &Owner| -> ResultV1<sim::AdmittedSimulationModuleV1> {
        let canonical = VerifiedCanonicalKernelIrV12::from_canonical_bytes(
            owner.canonical().canonical_bytes().to_vec(),
        )
        .map_err(|e| failure(Phase::Sim, e))?;
        let admitted = sim::AdmittedSimulationModuleV1::admit_v12(
            canonical,
            sim::SimulationLimitsV1::default(),
        )
        .map_err(|e| failure(Phase::Sim, e))?;
        require(
            admitted.identity().digest() == owner.canonical().identity().digest()
                && admitted.identity().canonical_length()
                    == owner.canonical().identity().canonical_length(),
            Phase::Sim,
            "actual SIM subject identity",
        )?;
        Ok(admitted)
    };
    let (b, a) = (admit(before)?, admit(after)?);
    let mut rows = Vec::new();
    for scenario in scenarios(case) {
        let first = simulate_one(before, &b, &scenario)?;
        let second = simulate_one(after, &a, &scenario)?;
        require(
            first.bytes == second.bytes
                && first.effects == second.effects
                && first.trapped == second.trapped
                && first == simulate_one(before, &b, &scenario)?
                && second == simulate_one(after, &a, &scenario)?,
            Phase::Sim,
            "before/after effects or same-subject deterministic repetition",
        )?;
        rows.push(SimRow {
            scenario,
            trapped: first.trapped,
            pairs: 2,
            effects: first.effects.len(),
            backing_sha256: digest(&first.bytes),
            effects_sha256: digest(&serde_json::to_vec(&first.effects).unwrap()),
            before_trace: first.hash,
            after_trace: second.hash,
            before_events: first.events,
            after_events: second.events,
            before_debug: first.debug,
            after_debug: second.debug,
        });
    }
    Ok(rows)
}

#[test]
fn scalar_source_effect_oracle_rejects_order_value_canary_initialization_and_trap_changes() {
    let s = Scenario {
        root: "scalar_effect_then_overflow".into(),
        length: 65,
        value: u32::MAX,
        choose: 0,
    };
    let (bytes, effects, trapped) = reference(&s);
    let initialized = vec![true; bytes.len()];
    check_reference(&s, &bytes, &initialized, &effects, trapped).unwrap();
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Begin(_)))
            .count(),
        64
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::End(_, true)))
            .count(),
        64
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, Effect::Write(0, 16, 0x13579bdf)))
            .count(),
        1
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::Return(_) | Effect::End(_, false)))
    );
    for mutation in 0..6 {
        let (mut b, mut i, mut e, mut t) =
            (bytes.clone(), initialized.clone(), effects.clone(), trapped);
        match mutation {
            0 => e.swap(65, 66),
            1 => b[16] ^= 1,
            2 => b[0] ^= 1,
            3 => i[0] = false,
            4 => t = false,
            _ => e.push(Effect::End(0, false)),
        }
        assert!(check_reference(&s, &b, &i, &e, t).is_err());
    }
}
#[test]
fn scalar_source_observation_limits_reject_incomplete_or_stopped_capture() {
    let case = ScalarCase::CheckedOpt0;
    let expected = scenarios(case).remove(0);
    for mutation in 0..5 {
        let mut row = protocol_rows(case).remove(0);
        match mutation {
            0 => row.before_events = OBSERVATION_CAP + 1,
            1 => row.after_debug = OBSERVATION_CAP + 1,
            2 => row.before_trace = [0; 32],
            3 => row.pairs = 1,
            _ => row.after_debug = 0,
        }
        assert!(validate_row(&row, &expected).is_err());
    }
    use sim::{SimulationDebugSinkV1, SimulationEventSinkV1};
    let module = Module::new("synthetic-protocol-limit-test");
    let trace = Rc::new(RefCell::new(Trace {
        event_count: OBSERVATION_CAP,
        ..Trace::default()
    }));
    let invocation = sim::SimulationInvocationV1 {
        global: [0, 0, 0],
        workgroup: [0, 0, 0],
        local: [0, 0, 0],
        workgroup_size: [64, 1, 1],
        workgroup_count: [1, 1, 1],
        launch_extent: [64, 1, 1],
    };
    let mut events = Events {
        module: &module,
        trace: Rc::clone(&trace),
    };
    let event = sim::SimulationEventV1 {
        invocation,
        site: sim::SimulationEventSiteV1 {
            function_ordinal: 0,
            block: fe2o3_kernel_ir::BlockId(0),
            operation: None,
        },
        kind: sim::SimulationEventKindV1::InvocationBegin,
    };
    assert!(events.record(&event).is_err());
    let mut debugger = Debugger {
        module: &module,
        trace: Rc::clone(&trace),
    };
    let record = sim::SimulationDebugRecordV1 {
        ordinal: 0,
        invocation,
        site: sim::SimulationDebugSiteV1 {
            function_ordinal: 0,
            block: fe2o3_kernel_ir::BlockId(0),
            operation: 0,
        },
        schedule: sim::SimulationDebugScheduleV1 {
            identity: sim::SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1,
            decision_ordinal: 0,
        },
        kind: sim::SimulationDebugRecordKindV1::Checkpoint {
            phase: sim::SimulationDebugCheckpointPhaseV1::BeforeOperation,
            stack: sim::SimulationDebugCollectionV1::Unavailable {
                reason: sim::SimulationDebugUnavailableReasonV1::FrameLimit,
                required: 1,
            },
            memory: sim::SimulationDebugCollectionV1::Captured(Vec::new()),
        },
    };
    assert_eq!(
        debugger.record(record),
        sim::SimulationDebugSinkControlV1::Continue
    );
    assert!(
        trace.borrow().failure.is_some(),
        "incomplete delivery cannot be accepted as success"
    );
}
