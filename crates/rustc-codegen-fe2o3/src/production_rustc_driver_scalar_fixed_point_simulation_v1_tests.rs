//! Complete bounded component-SIM observations and independent source reference.
use super::*;
use component_assertion::{AssertionRoute, AssertionRow, assertion_route};
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
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TrapSiteRow {
    pub(super) function: String,
    pub(super) block: u32,
    pub(super) operation: u32,
}
impl TrapSiteRow {
    pub(super) fn from_site(site: &sim::SimulationSiteV1) -> ResultV1<Self> {
        Ok(Self {
            function: site.function.as_str().into(),
            block: site.block.0,
            operation: site
                .operation
                .ok_or_else(|| failure(Phase::Sim, "trap operation absent"))?,
        })
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BranchRow {
    lane: u64,
    ordinal: usize,
    function: usize,
    block: u32,
    target: u32,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FailureEdgeRow {
    route: AssertionRoute,
    observed: BranchRow,
    trap_event: usize,
}
impl FailureEdgeRow {
    fn valid(&self, trap: &TrapSiteRow) -> bool {
        self.route.valid()
            && &self.route.trap == trap
            && self.observed.lane == 0
            && self.observed.function == self.route.failure_edge[0] as usize
            && self.observed.block == self.route.block
            && self.observed.target == self.route.failure_target
            && self.observed.ordinal > 0
            && self.observed.ordinal < self.trap_event
            && self.trap_event <= OBSERVATION_CAP
    }
}
fn check_failure_edge(
    route: &AssertionRoute,
    branches: &[BranchRow],
    trap: &TrapSiteRow,
    trap_event: usize,
    trap_lane: u64,
) -> ResultV1<FailureEdgeRow> {
    require(
        trap_lane == 0
            && branches.len() <= OBSERVATION_CAP
            && branches
                .iter()
                .all(|event| event.ordinal > 0 && event.ordinal < trap_event)
            && branches
                .windows(2)
                .all(|pair| pair[0].ordinal < pair[1].ordinal),
        Phase::Sim,
        "selected assertion failure lane/trace bound",
    )?;
    let mut selected = branches.iter().filter(|event| {
        event.lane == trap_lane
            && event.function == route.failure_edge[0] as usize
            && event.block == route.block
    });
    let observed = selected
        .next()
        .ok_or_else(|| failure(Phase::Sim, "selected failure branch absent"))?;
    require(
        selected.next().is_none(),
        Phase::Sim,
        "duplicate selected assertion branch",
    )?;
    let row = FailureEdgeRow {
        route: route.clone(),
        observed: observed.clone(),
        trap_event,
    };
    require(
        row.valid(trap),
        Phase::Sim,
        "actual selected failure edge must precede exact trap",
    )?;
    Ok(row)
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
    before_trap: Option<TrapSiteRow>,
    after_trap: Option<TrapSiteRow>,
    before_failure: Option<FailureEdgeRow>,
    after_failure: Option<FailureEdgeRow>,
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
            if case == ScalarCase::PreRankedCheckedOpt0 {
                rows.push(Scenario {
                    root: case.roots()[1].into(),
                    length,
                    value,
                    choose: 0,
                });
            }
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
            && row.before_trap.is_some() == trapped
            && row.after_trap.is_some() == trapped
            && row.before_failure.is_some() == trapped
            && row.after_failure.is_some() == trapped
            && row
                .before_failure
                .as_ref()
                .is_none_or(|edge| edge.trap_event <= row.before_events)
            && row
                .after_failure
                .as_ref()
                .is_none_or(|edge| edge.trap_event <= row.after_events)
            && [&row.before_failure, &row.after_failure]
                .iter()
                .zip([&row.before_trap, &row.after_trap])
                .all(|(edge, trap)| match (edge.as_ref(), trap.as_ref()) {
                    (Some(edge), Some(trap)) => edge.valid(trap),
                    (None, None) => true,
                    _ => false,
                })
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
                before_trap: trapped.then(|| TrapSiteRow {
                    function: "synthetic".into(),
                    block: 0,
                    operation: 0,
                }),
                after_trap: trapped.then(|| TrapSiteRow {
                    function: "synthetic".into(),
                    block: 0,
                    operation: 0,
                }),
                before_failure: None,
                after_failure: None,
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
    trap_event: Option<(usize, u64)>,
    branches: Vec<BranchRow>,
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
            E::Branch { target } => {
                let function = &self.module.functions[event.site.function_ordinal];
                let block = function
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks
                    .iter()
                    .find(|block| block.id == event.site.block)
                    .ok_or_else(bad)?;
                if event.site.operation.is_some()
                    || !block
                        .terminator
                        .as_ref()
                        .is_some_and(|term| term.successors().contains(target))
                {
                    return Err(bad());
                }
                let ordinal = trace.event_count;
                trace.branches.push(BranchRow {
                    lane,
                    ordinal,
                    function: event.site.function_ordinal,
                    block: event.site.block.0,
                    target: target.0,
                });
                None
            }
            E::OperationBegin | E::OperationEnd { .. } | E::BlockEnter | E::Terminator => None,
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
                    if trace.trap_checkpoint.is_some() {
                        return Err(bad());
                    }
                    trace.trap_event = Some((trace.event_count, record.invocation.global[0]));
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
    trap: Option<TrapSiteRow>,
    failure_edge: Option<FailureEdgeRow>,
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
    expected_route: Option<&AssertionRoute>,
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
            require(
                error
                    .site
                    .as_ref()
                    .map(TrapSiteRow::from_site)
                    .transpose()?
                    .as_ref()
                    == expected_route.map(|route| &route.trap)
                    && expected_route.is_some(),
                Phase::Sim,
                "trap did not reach exact selected assertion failure continuation",
            )?;
        }
        Err(error) => return Err(failure(Phase::Sim, error)),
    }
    let trap = trace
        .trap_checkpoint
        .as_ref()
        .map(TrapSiteRow::from_site)
        .transpose()?;
    require(
        trap.is_some() == trapped,
        Phase::Sim,
        "unexpected or missing actual trap checkpoint",
    )?;
    let failure_edge = if let Some(trap) = &trap {
        let route = expected_route.ok_or_else(|| failure(Phase::Sim, "selected route absent"))?;
        let (event, lane) = trace
            .trap_event
            .ok_or_else(|| failure(Phase::Sim, "trap event absent"))?;
        Some(check_failure_edge(
            route,
            &trace.branches,
            trap,
            event,
            lane,
        )?)
    } else {
        None
    };
    Ok(Run {
        bytes,
        effects,
        trapped,
        hash: trace.hash.finalize().into(),
        events: trace.event_count,
        debug: trace.debug_count,
        trap,
        failure_edge,
    })
}
pub(super) fn validate_assertion_row(
    row: &SimRow,
    assertion: &AssertionRow,
    before: &Subject,
    after: &Subject,
) -> ResultV1<()> {
    for trap in [&row.before_trap, &row.after_trap].into_iter().flatten() {
        require(
            row.scenario.root == assertion.root && trap.function == assertion.entry,
            Phase::Sim,
            "reported trap is not selected source root/function",
        )?;
    }
    for (edge, subject) in [(&row.before_failure, before), (&row.after_failure, after)] {
        if let Some(edge) = edge {
            require(
                &edge.route.subject == subject
                    && edge.route.function == assertion.entry
                    && row.scenario.root == assertion.root,
                Phase::Sim,
                "reported selected branch actual subject/root/function",
            )?;
        }
    }
    if let Some(edge) = &row.before_failure {
        require(
            edge.route.failure_edge == assertion.failure_edge
                && edge.route.success_successor == assertion.success_edge[2],
            Phase::Sim,
            "reported original branch sealed assertion coordinates",
        )?;
    }
    Ok(())
}
pub(super) fn simulate_matrix(
    before: &Owner,
    after: &Owner,
    case: ScalarCase,
    assertion: Option<&AssertionRow>,
) -> ResultV1<Vec<SimRow>> {
    let traps = match (case, assertion) {
        (ScalarCase::PreRankedCheckedOpt0, Some(assertion)) => Some((
            assertion_route(before, assertion, true)?,
            assertion_route(after, assertion, false)?,
        )),
        (ScalarCase::NormalNoop | ScalarCase::AdmittedCheckedOpt0, None) => None,
        _ => {
            return Err(failure(
                Phase::Sim,
                "source case/assertion custody mismatch",
            ));
        }
    };
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
        let first = simulate_one(before, &b, &scenario, traps.as_ref().map(|p| &p.0))?;
        let second = simulate_one(after, &a, &scenario, traps.as_ref().map(|p| &p.1))?;
        require(
            first.bytes == second.bytes
                && first.effects == second.effects
                && first.trapped == second.trapped
                && first == simulate_one(before, &b, &scenario, traps.as_ref().map(|p| &p.0))?
                && second == simulate_one(after, &a, &scenario, traps.as_ref().map(|p| &p.1))?,
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
            before_trap: first.trap,
            after_trap: second.trap,
            before_failure: first.failure_edge,
            after_failure: second.failure_edge,
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
    let case = ScalarCase::PreRankedCheckedOpt0;
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

#[test]
fn scalar_dynamic_assertion_edge_rejects_unrelated_edge_to_same_shared_trap() {
    use fe2o3_kernel_ir::{BasicBlock, BlockId, Signature};
    use sim::SimulationEventSinkV1;
    // A graph/event oracle fixture only; never a fabricated source qualification.
    let branch = |id, condition, failure, success| {
        let mut block = BasicBlock::new(BlockId(id));
        block.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(condition),
            then_target: BlockId(failure),
            then_arguments: vec![],
            else_target: BlockId(success),
            else_arguments: vec![],
        });
        block
    };
    let mut trap = BasicBlock::new(BlockId(2));
    trap.operations.push(Operation::new(
        vec![],
        Kind::Call {
            callee: AmdGpuDiagnosticOperation::Trap.intrinsic_function_id(),
            arguments: vec![],
        },
    ));
    trap.terminator = Some(Terminator::Unreachable);
    let mut success = BasicBlock::new(BlockId(3));
    success.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("shared-trap-oracle-only");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![Type::Scalar(ScalarType::Bool); 2], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![branch(0, 0, 2, 1), branch(1, 1, 2, 3), trap, success],
    ));
    module.required_capabilities = AmdGpuDiagnosticOperation::Trap.required_capabilities();
    module.functions[0].required_capabilities = module.required_capabilities.clone();
    module
        .functions
        .push(AmdGpuDiagnosticOperation::Trap.declaration());
    let mut kernel = fe2o3_kernel_ir::Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities = module.required_capabilities.clone();
    module.kernels.push(kernel);
    let mut work = Work::new(100_000_000);
    let mut budget = Budget::new(&mut work, 1024 * 1024 * 1024);
    let (owner, _) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    let module = owner.module();
    let trap = TrapSiteRow {
        function: "entry".into(),
        block: 2,
        operation: 0,
    };
    let route = AssertionRoute {
        subject: subject(&owner),
        function: "entry".into(),
        failure_edge: [0, 0, 0],
        success_successor: 1,
        block: 0,
        success_target: 1,
        failure_target: 2,
        trap: trap.clone(),
    };
    let record_branch = |block| {
        let trace = Rc::new(RefCell::new(Trace::default()));
        let mut events = Events {
            module,
            trace: Rc::clone(&trace),
        };
        events
            .record(&sim::SimulationEventV1 {
                invocation: sim::SimulationInvocationV1 {
                    global: [0, 0, 0],
                    workgroup: [0, 0, 0],
                    local: [0, 0, 0],
                    workgroup_size: [64, 1, 1],
                    workgroup_count: [1, 1, 1],
                    launch_extent: [64, 1, 1],
                },
                site: sim::SimulationEventSiteV1 {
                    function_ordinal: 0,
                    block: BlockId(block),
                    operation: None,
                },
                kind: sim::SimulationEventKindV1::Branch { target: BlockId(2) },
            })
            .unwrap();
        drop(events);
        Rc::try_unwrap(trace).ok().unwrap().into_inner().branches
    };
    let selected = record_branch(0);
    let unrelated = record_branch(1);
    assert_eq!(selected[0].target, unrelated[0].target);
    let observed = check_failure_edge(&route, &selected, &trap, 2, 0).unwrap();
    assert!(observed.valid(&trap));
    assert!(check_failure_edge(&route, &unrelated, &trap, 2, 0).is_err());
    for mutation in 0..9 {
        let mut changed = selected.clone();
        match mutation {
            0 => changed.clear(),
            1 => changed.push(changed[0].clone()),
            2 => changed[0].lane = 1,
            3 => changed[0].function = 1,
            4 => changed[0].block = 1,
            5 => changed[0].target = route.success_target,
            6 => changed[0].ordinal = 2,
            7 => changed[0].ordinal = 3,
            _ => changed[0].ordinal = 0,
        }
        assert!(check_failure_edge(&route, &changed, &trap, 2, 0).is_err());
    }
    let mut foreign = trap.clone();
    foreign.function = "foreign".into();
    assert!(check_failure_edge(&route, &selected, &foreign, 2, 0).is_err());
    assert!(check_failure_edge(&route, &selected, &trap, 2, 1).is_err());
    let origin =
        serde_json::json!({ "file": ([7; 32]), "bytes": [0, 9], "start": [1, 1], "end": [1, 10] });
    let assertion: AssertionRow = serde_json::from_value(serde_json::json!({
        "input": route.subject, "root": "scalar_effect_then_overflow", "entry": "entry",
        "source_function": ([1; 32]), "source_body": ([2; 32]), "semantic_block": 0,
        "expected": false, "condition_local": null, "expansion": origin, "call_site": origin,
        "fixture_file": { "file": ([7; 32]), "bytes": [0, 10], "start": [1, 1], "end": [2, 1] },
        "success_edge": [0, 0, 1], "failure_edge": [0, 0, 0]
    }))
    .unwrap();
    let mut row = protocol_rows(ScalarCase::PreRankedCheckedOpt0)
        .into_iter()
        .find(|row| row.trapped)
        .unwrap();
    row.before_trap = Some(trap.clone());
    row.after_trap = Some(trap.clone());
    row.before_failure = Some(observed.clone());
    row.after_failure = Some(observed);
    validate_row(&row, &row.scenario).unwrap();
    validate_assertion_row(&row, &assertion, &route.subject, &route.subject).unwrap();
    row.after_failure.as_mut().unwrap().route.subject.digest[0] ^= 1;
    assert!(validate_assertion_row(&row, &assertion, &route.subject, &route.subject).is_err());
    row.after_failure.as_mut().unwrap().route.subject = route.subject.clone();
    row.before_failure.as_mut().unwrap().route.failure_edge[1] += 1;
    assert!(validate_assertion_row(&row, &assertion, &route.subject, &route.subject).is_err());
    row.before_failure.as_mut().unwrap().trap_event = row.before_events + 1;
    assert!(validate_row(&row, &row.scenario).is_err());
}
