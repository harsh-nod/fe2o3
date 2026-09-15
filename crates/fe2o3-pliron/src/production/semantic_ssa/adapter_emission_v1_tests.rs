use super::super::adapter::emission_v1::{
    SemanticSsaEmissionObserverV1, SemanticSsaEmissionSiteV1 as Site,
    SemanticSsaEntryOriginV1 as Entry, SemanticSsaEventRoleV1 as EventRole,
    SemanticSsaOperandRoleV1 as Operand, SemanticSsaVisitV1 as Visit, emit_statement_events_v1,
    emit_terminator_events_v1,
};
use super::super::adapter::semantic_function_ssa_input_with_observer_v1;
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAssignmentV1, SemanticBinaryOpV1, SemanticConstantV1, SemanticConstantValueV1,
    SemanticScalarValueV1,
};
use fe2o3_mir_model::{
    SsaArgumentV1, SsaDefinitionIdV1, SsaEdgeIdV1, SsaResolvedEventV1, SsaValueV1,
};

#[derive(Debug, Eq, PartialEq)]
struct Event {
    site: Site,
    operand: Operand,
    role: EventRole,
    ordinal: usize,
    event: SsaEventV1,
}

#[derive(Debug, Eq, PartialEq)]
struct Constant {
    site: Site,
    operand: Operand,
    next_event: usize,
    value: SemanticConstantV1,
}

#[derive(Default)]
struct Trace {
    visits: Vec<(Visit, Site)>,
    events: Vec<Event>,
    constants: Vec<Constant>,
    successors: Vec<(usize, usize, SemanticControlFlowEdgeV1)>,
    edge_definitions: Vec<(
        usize,
        usize,
        SemanticControlFlowEdgeV1,
        usize,
        SsaVariableIdV1,
    )>,
    entries: Vec<(usize, SsaVariableIdV1, Entry)>,
    elided: Vec<Site>,
    elision_lookups: Vec<(Site, usize)>,
    blocks: Vec<(usize, usize, usize)>,
    input: Vec<(usize, usize)>,
    reject_event: Option<usize>,
    reject_hook: Option<RejectedHook>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RejectedHook {
    Visit(Visit),
    ElisionLookup(Site),
    Successor { block: usize, ordinal: usize },
    Entry(usize),
    BlockComplete(usize),
    InputComplete,
}

const HOOK_DENIED: usize = 97;

impl Trace {
    fn refuse(&self, hook: RejectedHook) -> Result<(), usize> {
        if self.reject_hook == Some(hook) {
            Err(HOOK_DENIED)
        } else {
            Ok(())
        }
    }
}

impl SemanticSsaEmissionObserverV1 for Trace {
    type Error = usize;

    fn visit(&mut self, kind: Visit, site: Site) -> Result<(), usize> {
        self.visits.push((kind, site));
        self.refuse(RejectedHook::Visit(kind))
    }

    fn event(
        &mut self,
        site: Site,
        operand: Operand,
        role: EventRole,
        ordinal: usize,
        event: SsaEventV1,
    ) -> Result<(), usize> {
        self.events.push(row(site, operand, role, ordinal, event));
        if self.reject_event == Some(ordinal) {
            Err(ordinal)
        } else {
            Ok(())
        }
    }

    fn constant(
        &mut self,
        site: Site,
        operand: Operand,
        next_event: usize,
        constant: &SemanticConstantV1,
    ) -> Result<(), usize> {
        self.constants.push(Constant {
            site,
            operand,
            next_event,
            value: constant.clone(),
        });
        Ok(())
    }

    fn successor(
        &mut self,
        block: usize,
        ordinal: usize,
        edge: SemanticControlFlowEdgeV1,
    ) -> Result<(), usize> {
        self.successors.push((block, ordinal, edge));
        self.refuse(RejectedHook::Successor { block, ordinal })
    }

    fn edge_definition(
        &mut self,
        block: usize,
        edge_ordinal: usize,
        edge: SemanticControlFlowEdgeV1,
        definition_ordinal: usize,
        variable: SsaVariableIdV1,
    ) -> Result<(), usize> {
        self.edge_definitions
            .push((block, edge_ordinal, edge, definition_ordinal, variable));
        Ok(())
    }

    fn entry_definition(
        &mut self,
        ordinal: usize,
        variable: SsaVariableIdV1,
        origin: Entry,
    ) -> Result<(), usize> {
        self.entries.push((ordinal, variable, origin));
        self.refuse(RejectedHook::Entry(ordinal))
    }

    fn elided_borrow(&mut self, site: Site) -> Result<(), usize> {
        self.elided.push(site);
        Ok(())
    }

    fn statement_elision_lookup(&mut self, site: Site, candidates: usize) -> Result<(), usize> {
        self.elision_lookups.push((site, candidates));
        self.refuse(RejectedHook::ElisionLookup(site))
    }

    fn block_complete(
        &mut self,
        block: usize,
        events: usize,
        successors: usize,
    ) -> Result<(), usize> {
        self.blocks.push((block, events, successors));
        self.refuse(RejectedHook::BlockComplete(block))
    }

    fn input_complete(&mut self, blocks: usize, entry_definitions: usize) -> Result<(), usize> {
        self.input.push((blocks, entry_definitions));
        self.refuse(RejectedHook::InputComplete)
    }
}

fn row(site: Site, operand: Operand, role: EventRole, ordinal: usize, event: SsaEventV1) -> Event {
    Event {
        site,
        operand,
        role,
        ordinal,
        event,
    }
}

fn variable(index: u32) -> SsaVariableIdV1 {
    SsaVariableIdV1::new(index)
}

fn used(index: u32) -> SsaEventV1 {
    SsaEventV1::Use(variable(index))
}
fn killed(index: u32) -> SsaEventV1 {
    SsaEventV1::Kill(variable(index))
}
fn defined(index: u32) -> SsaEventV1 {
    SsaEventV1::Define(variable(index))
}

// These are emission-grammar operands, not an independently admitted executable.
fn indexed(local: u32, index: u32) -> SemanticPlaceV1 {
    let ty = SemanticTypeIdV1::from_index(1);
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty).unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(index)),
                ty,
            )
            .unwrap(),
        ],
        ty,
    )
    .unwrap()
}

fn constant() -> SemanticConstantV1 {
    SemanticConstantV1::new(
        SemanticTypeIdV1::from_index(1),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(37, 4).unwrap()),
    )
}

fn observed_input(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    callables: &[SemanticCallableDeclV1],
) -> (SsaConstructionInputV1, Trace) {
    let transparent = transparent_borrow_sites_v1(function, callables);
    let mut trace = Trace::default();
    let observed = semantic_function_ssa_input_with_observer_v1(
        function,
        types,
        callables,
        &transparent,
        &mut trace,
    )
    .unwrap();
    let ordinary = semantic_function_ssa_input_v1(function, types, callables, &transparent);
    assert_eq!(observed, ordinary);
    assert!(
        trace
            .visits
            .iter()
            .all(|(_, site)| *site != Site::Auxiliary)
    );
    assert!(
        trace
            .events
            .iter()
            .all(|event| event.site != Site::Auxiliary)
    );
    (observed.0, trace)
}

#[test]
fn original_ordinals_include_move_kill_projection_and_repeated_arguments() {
    let statement = Site::Statement {
        block: 7,
        statement: 3,
    };
    let terminator = Site::Terminator { block: 7 };
    let assignment = SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        test_scalar_place(3),
        SemanticRvalueV1::new(
            SemanticTypeIdV1::from_index(1),
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: SemanticOperandV1::Move(test_scalar_place(1)),
                right: SemanticOperandV1::Copy(indexed(2, 4)),
            },
        ),
    ));
    let call = test_call(
        0,
        vec![
            SemanticOperandV1::Constant(constant()),
            SemanticOperandV1::Copy(test_scalar_place(3)),
            SemanticOperandV1::Move(test_scalar_place(3)),
            SemanticOperandV1::Copy(indexed(2, 4)),
        ],
        None,
    );
    let mut events = Vec::new();
    let mut trace = Trace::default();
    emit_statement_events_v1(&assignment, false, statement, &mut events, &mut trace).unwrap();
    emit_terminator_events_v1(&call, None, terminator, &mut events, &mut trace).unwrap();
    assert_eq!(
        events,
        [
            used(1),
            killed(1),
            used(2),
            used(4),
            defined(3),
            used(3),
            used(3),
            killed(3),
            used(2),
            used(4)
        ]
    );
    assert_eq!(
        trace.events,
        [
            row(
                statement,
                Operand::RvalueOperand(0),
                EventRole::BaseUse,
                0,
                used(1)
            ),
            row(
                statement,
                Operand::RvalueOperand(0),
                EventRole::MoveKill,
                1,
                killed(1)
            ),
            row(
                statement,
                Operand::RvalueOperand(1),
                EventRole::BaseUse,
                2,
                used(2)
            ),
            row(
                statement,
                Operand::RvalueOperand(1),
                EventRole::ProjectionIndexUse(1),
                3,
                used(4)
            ),
            row(
                statement,
                Operand::Destination,
                EventRole::DestinationDefine,
                4,
                defined(3)
            ),
            row(
                terminator,
                Operand::CallArgument(1),
                EventRole::BaseUse,
                5,
                used(3)
            ),
            row(
                terminator,
                Operand::CallArgument(2),
                EventRole::BaseUse,
                6,
                used(3)
            ),
            row(
                terminator,
                Operand::CallArgument(2),
                EventRole::MoveKill,
                7,
                killed(3)
            ),
            row(
                terminator,
                Operand::CallArgument(3),
                EventRole::BaseUse,
                8,
                used(2)
            ),
            row(
                terminator,
                Operand::CallArgument(3),
                EventRole::ProjectionIndexUse(1),
                9,
                used(4)
            ),
        ]
    );
    assert_eq!(
        trace.constants,
        [Constant {
            site: terminator,
            operand: Operand::CallArgument(0),
            next_event: 5,
            value: constant(),
        }]
    );
    assert_eq!(
        trace
            .visits
            .iter()
            .filter(|(visit, _)| *visit == Visit::Projection)
            .count(),
        4
    );
}

#[test]
fn projected_moves_and_destinations_do_not_invent_kills_or_definitions() {
    let statement = Site::Statement {
        block: 0,
        statement: 0,
    };
    let assignment = test_assign_to(indexed(3, 4), SemanticOperandV1::Move(indexed(1, 2)));
    let mut events = Vec::new();
    let mut trace = Trace::default();
    emit_statement_events_v1(assignment.kind(), false, statement, &mut events, &mut trace).unwrap();
    assert_eq!(events, [used(1), used(2), used(3), used(4)]);
    assert_eq!(
        trace.events,
        [
            row(
                statement,
                Operand::RvalueOperand(0),
                EventRole::BaseUse,
                0,
                used(1)
            ),
            row(
                statement,
                Operand::RvalueOperand(0),
                EventRole::ProjectionIndexUse(1),
                1,
                used(2)
            ),
            row(
                statement,
                Operand::Destination,
                EventRole::BaseUse,
                2,
                used(3)
            ),
            row(
                statement,
                Operand::Destination,
                EventRole::ProjectionIndexUse(1),
                3,
                used(4)
            ),
        ]
    );
    assert_eq!(
        trace
            .visits
            .iter()
            .filter(|(visit, _)| *visit == Visit::Place)
            .count(),
        2
    );
    for (ordinal, kind) in [
        SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(2)),
        SemanticStatementKindV1::Nop,
        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(2)),
    ]
    .into_iter()
    .enumerate()
    {
        emit_statement_events_v1(
            &kind,
            false,
            Site::Statement {
                block: 0,
                statement: ordinal + 1,
            },
            &mut events,
            &mut trace,
        )
        .unwrap();
    }
    assert_eq!(
        events,
        [used(1), used(2), used(3), used(4), killed(2), killed(2)]
    );
    assert_eq!(
        trace.events[4],
        row(
            Site::Statement {
                block: 0,
                statement: 1
            },
            Operand::StorageLive,
            EventRole::StorageKill,
            4,
            killed(2)
        )
    );
    assert_eq!(
        trace.events[5],
        row(
            Site::Statement {
                block: 0,
                statement: 3
            },
            Operand::StorageDead,
            EventRole::StorageKill,
            5,
            killed(2)
        )
    );
}

#[test]
fn assertion_message_and_return_events_keep_exact_operand_roles() {
    let site = Site::Terminator { block: 2 };
    let assertion = SemanticTerminatorKindV1::Assert {
        condition: SemanticOperandV1::Move(test_scalar_place(1)),
        expected: true,
        message: SemanticAssertMessageV1::BoundsCheck {
            length: SemanticOperandV1::Copy(test_scalar_place(2)),
            index: SemanticOperandV1::Constant(constant()),
        },
        target: test_edge(SemanticEdgeRoleV1::AssertSuccess, 3),
        unwind: SemanticUnwindActionV1::Unreachable,
    };
    let mut events = Vec::new();
    let mut trace = Trace::default();
    emit_terminator_events_v1(&assertion, None, site, &mut events, &mut trace).unwrap();
    assert_eq!(events, [used(1), killed(1), used(2)]);
    assert_eq!(
        trace.events,
        [
            row(
                site,
                Operand::AssertCondition,
                EventRole::BaseUse,
                0,
                used(1)
            ),
            row(
                site,
                Operand::AssertCondition,
                EventRole::MoveKill,
                1,
                killed(1)
            ),
            row(
                site,
                Operand::AssertMessage(0),
                EventRole::BaseUse,
                2,
                used(2)
            ),
        ]
    );
    assert_eq!(
        trace.constants,
        [Constant {
            site,
            operand: Operand::AssertMessage(1),
            next_event: 3,
            value: constant(),
        }]
    );
    emit_terminator_events_v1(
        &SemanticTerminatorKindV1::Return,
        None,
        site,
        &mut events,
        &mut trace,
    )
    .unwrap();
    assert_eq!(events.len(), 3);
    emit_terminator_events_v1(
        &SemanticTerminatorKindV1::Return,
        Some(0),
        site,
        &mut events,
        &mut trace,
    )
    .unwrap();
    assert_eq!(
        trace.events.last(),
        Some(&row(
            site,
            Operand::ReturnValue,
            EventRole::BaseUse,
            3,
            used(0)
        ))
    );
}

#[test]
fn actual_adapter_distinguishes_transparent_and_authenticated_elided_borrows() {
    let ordinary = test_function(vec![test_block(
        70,
        vec![test_borrow(2, 1)],
        test_call(0, vec![SemanticOperandV1::Copy(test_scalar_place(2))], None),
    )]);
    let callables = [test_intrinsic_callable(ordinary.abi().clone())];
    assert_eq!(transparent_borrow_sites_v1(&ordinary, &callables).len(), 1);
    let (input, trace) = observed_input(&ordinary, None, &callables);
    assert_eq!(input.blocks()[0].events(), [used(1), defined(2), used(2)]);
    assert!(trace.elided.is_empty());
    assert_eq!(
        trace.events[0],
        row(
            Site::Statement {
                block: 0,
                statement: 0
            },
            Operand::RvaluePlace,
            EventRole::BaseUse,
            0,
            used(1)
        )
    );
    assert_eq!(trace.entries, [(0, variable(1), Entry::Argument(0))]);

    let (types, function, callables) = test_elided_grid_leader_case(0, true, false, false, false);
    let (input, trace) = observed_input(&function, Some(&types), &callables);
    let statement = Site::Statement {
        block: 2,
        statement: 0,
    };
    let terminator = Site::Terminator { block: 2 };
    assert_eq!(input.blocks()[2].events(), [defined(4), used(2), used(4)]);
    assert_eq!(trace.elided, [statement]);
    assert!(trace.elision_lookups.contains(&(statement, 1)));
    let block_events = trace
        .events
        .iter()
        .filter(|event| event.site == statement || event.site == terminator)
        .collect::<Vec<_>>();
    assert_eq!(
        block_events,
        [
            &row(
                statement,
                Operand::ElidedBorrowDestination,
                EventRole::DestinationDefine,
                0,
                defined(4)
            ),
            &row(
                terminator,
                Operand::CallArgument(0),
                EventRole::BaseUse,
                1,
                used(2)
            ),
            &row(
                terminator,
                Operand::CallArgument(1),
                EventRole::BaseUse,
                2,
                used(4)
            ),
        ]
    );
    assert!(
        trace
            .entries
            .iter()
            .all(|(_, _, origin)| *origin != Entry::ImplicitCapability)
    );
}

#[test]
fn actual_entry_origin_distinguishes_implicit_capability_from_argument() {
    let types = implicit_scope_types();
    let function = test_implicit_scope_function(
        0,
        vec![test_block(
            104,
            vec![test_typed_borrow(2, 2, 1, 0)],
            test_call(
                0,
                vec![SemanticOperandV1::Copy(test_typed_place(2, 2))],
                None,
            ),
        )],
    );
    let callables = [test_intrinsic_callable(function.abi().clone())];
    let (input, trace) = observed_input(&function, Some(&types), &callables);
    assert_eq!(input.entry_definitions(), [variable(1)]);
    assert_eq!(trace.entries, [(0, variable(1), Entry::ImplicitCapability)]);
    assert_eq!(trace.input, [(1, 1)]);
    let plan = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
    assert_eq!(
        plan.entry_definitions(),
        [SsaArgumentV1::new(
            variable(1),
            SsaValueV1::Definition(SsaDefinitionIdV1::new(0))
        )]
    );
}

#[test]
fn repeated_target_edges_keep_successor_ordinals_and_exact_call_return_definition() {
    let returned = test_edge(SemanticEdgeRoleV1::CallReturn, 1);
    let unwound = test_edge(SemanticEdgeRoleV1::CallUnwind, 1);
    let function = test_function(vec![
        test_block(
            75,
            vec![],
            test_call_with_unwind(
                SemanticCallDestinationV1::new(test_scalar_place(2), returned),
                unwound,
            ),
        ),
        test_block(76, vec![], SemanticTerminatorKindV1::Return),
    ]);
    let (input, trace) = observed_input(&function, None, &[]);
    assert_eq!(trace.successors, [(0, 0, returned), (0, 1, unwound)]);
    assert_eq!(trace.edge_definitions, [(0, 0, returned, 0, variable(2))]);
    assert_eq!(trace.blocks, [(0, 0, 2), (1, 0, 0)]);
    let plan = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
    assert_eq!(
        plan.edge_definitions(SsaEdgeIdV1::new(SsaBlockIdV1::new(0), 0))
            .unwrap(),
        [SsaArgumentV1::new(
            variable(2),
            SsaValueV1::Definition(SsaDefinitionIdV1::new(1))
        )]
    );
    assert!(
        plan.edge_definitions(SsaEdgeIdV1::new(SsaBlockIdV1::new(0), 1))
            .unwrap()
            .is_empty()
    );

    let same = test_edge(SemanticEdgeRoleV1::SwitchValue, 1);
    let otherwise = test_edge(SemanticEdgeRoleV1::SwitchOtherwise, 1);
    let switch = SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(test_place(1, Some(0))),
        targets: SemanticSwitchTargetsV1::new(
            vec![
                SemanticSwitchTargetV1::new(0, same),
                SemanticSwitchTargetV1::new(1, same),
            ],
            otherwise,
        )
        .unwrap(),
    };
    let function = test_function(vec![
        test_block(77, vec![], switch),
        test_block(78, vec![], SemanticTerminatorKindV1::Return),
    ]);
    let (_, trace) = observed_input(&function, None, &[]);
    assert_eq!(
        trace.successors,
        [(0, 0, same), (0, 1, same), (0, 2, otherwise)]
    );
    assert!(trace.edge_definitions.is_empty());
}

#[test]
fn original_events_include_unpromoted_and_unreachable_occurrences() {
    let function = test_function(vec![
        test_block(
            80,
            vec![
                test_assign(2, SemanticOperandV1::Copy(test_place(1, Some(0)))),
                test_store(
                    test_place(1, Some(0)),
                    SemanticOperandV1::Move(test_scalar_place(2)),
                ),
                test_assign(3, SemanticOperandV1::Copy(test_place(1, Some(1)))),
            ],
            SemanticTerminatorKindV1::Return,
        ),
        test_block(
            81,
            vec![test_assign(
                3,
                SemanticOperandV1::Copy(test_place(1, Some(0))),
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ]);
    let (input, trace) = observed_input(&function, Some(&test_types(false)), &[]);
    assert!(!input.promotable()[1]);
    assert_eq!(
        input.blocks()[0].events(),
        [
            used(1),
            defined(2),
            used(2),
            killed(2),
            used(1),
            used(1),
            defined(3)
        ]
    );
    assert_eq!(input.blocks()[1].events(), [used(1), defined(3)]);
    assert_eq!(trace.blocks, [(0, 7, 0), (1, 2, 0)]);
    assert_eq!(
        trace
            .events
            .iter()
            .map(|event| event.ordinal)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5, 6, 0, 1]
    );
    let plan = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
    let first = SsaValueV1::Definition(SsaDefinitionIdV1::new(0));
    let second = SsaValueV1::Definition(SsaDefinitionIdV1::new(1));
    assert_eq!(
        plan.resolved_events(SsaBlockIdV1::new(0)).unwrap(),
        [
            (
                1,
                SsaResolvedEventV1::Define {
                    variable: variable(2),
                    value: first
                }
            ),
            (
                2,
                SsaResolvedEventV1::Use {
                    variable: variable(2),
                    value: first
                }
            ),
            (
                3,
                SsaResolvedEventV1::Kill {
                    variable: variable(2),
                    previous: Some(first)
                }
            ),
            (
                6,
                SsaResolvedEventV1::Define {
                    variable: variable(3),
                    value: second
                }
            ),
        ]
    );
    assert!(!plan.is_reachable(SsaBlockIdV1::new(1)));
    assert!(plan.resolved_events(SsaBlockIdV1::new(1)).is_none());
    assert!(plan.resolved_event(SsaBlockIdV1::new(0), 0).is_none());
}

#[test]
fn observer_denial_precedes_event_push_and_stops_following_emission() {
    let statement = test_assign(2, SemanticOperandV1::Move(test_scalar_place(1)));
    let site = Site::Statement {
        block: 0,
        statement: 0,
    };
    let mut events = vec![used(9)];
    let mut trace = Trace {
        reject_event: Some(2),
        ..Trace::default()
    };
    assert_eq!(
        emit_statement_events_v1(statement.kind(), false, site, &mut events, &mut trace),
        Err(2)
    );
    assert_eq!(events, [used(9), used(1)]);
    assert_eq!(
        trace.events,
        [
            row(
                site,
                Operand::RvalueOperand(0),
                EventRole::BaseUse,
                1,
                used(1)
            ),
            row(
                site,
                Operand::RvalueOperand(0),
                EventRole::MoveKill,
                2,
                killed(1)
            ),
        ]
    );
    assert!(
        !trace
            .visits
            .iter()
            .any(|(visit, _)| *visit == Visit::Projection)
    );
    assert!(trace.constants.is_empty());
    assert!(trace.blocks.is_empty());
    assert!(trace.input.is_empty());

    let function = test_function(vec![test_block(
        90,
        vec![
            statement,
            test_assign(3, SemanticOperandV1::Constant(constant())),
        ],
        SemanticTerminatorKindV1::Return,
    )]);
    let mut trace = Trace {
        reject_event: Some(1),
        ..Trace::default()
    };
    assert_eq!(
        semantic_function_ssa_input_with_observer_v1(
            &function,
            None,
            &[],
            &BTreeSet::new(),
            &mut trace,
        ),
        Err(1)
    );
    assert_eq!(
        trace.events,
        [
            row(
                site,
                Operand::RvalueOperand(0),
                EventRole::BaseUse,
                0,
                used(1)
            ),
            row(
                site,
                Operand::RvalueOperand(0),
                EventRole::MoveKill,
                1,
                killed(1)
            ),
        ]
    );
    assert!(trace.constants.is_empty());
    assert!(trace.successors.is_empty());
    assert!(trace.entries.is_empty());
    assert!(trace.blocks.is_empty());
    assert!(trace.input.is_empty());
    assert!(!trace.visits.iter().any(|(_, site)| *site
        == Site::Statement {
            block: 0,
            statement: 1
        }));
}

#[test]
fn denied_place_and_projection_visits_stop_before_dependent_events() {
    let statement = test_assign(2, SemanticOperandV1::Copy(indexed(1, 3)));
    let site = Site::Statement {
        block: 0,
        statement: 0,
    };
    for denied in [Visit::Place, Visit::Projection] {
        let mut trace = Trace {
            reject_hook: Some(RejectedHook::Visit(denied)),
            ..Trace::default()
        };
        let mut events = Vec::new();
        assert_eq!(
            emit_statement_events_v1(statement.kind(), false, site, &mut events, &mut trace,),
            Err(HOOK_DENIED)
        );
        let mut expected_visits = vec![
            (Visit::Statement, site),
            (Visit::Rvalue, site),
            (Visit::Operand, site),
            (Visit::Place, site),
        ];
        if denied == Visit::Projection {
            expected_visits.push((Visit::Projection, site));
            // The first projection is a Field, not the later Index. Even this
            // eventless projection's denied visit stops the next projection.
            assert_eq!(events, [used(1)]);
            assert_eq!(
                trace.events,
                [row(
                    site,
                    Operand::RvalueOperand(0),
                    EventRole::BaseUse,
                    0,
                    used(1)
                )]
            );
        } else {
            assert!(events.is_empty());
            assert!(trace.events.is_empty());
        }
        assert_eq!(trace.visits, expected_visits);
        assert!(trace.constants.is_empty());
    }
}

#[test]
fn denied_actual_elision_lookup_precedes_statement_dispatch() {
    let (types, function, callables) = test_elided_grid_leader_case(0, true, false, false, false);
    let transparent = transparent_borrow_sites_v1(&function, &callables);
    let site = Site::Statement {
        block: 2,
        statement: 0,
    };
    let mut trace = Trace {
        reject_hook: Some(RejectedHook::ElisionLookup(site)),
        ..Trace::default()
    };
    assert_eq!(
        semantic_function_ssa_input_with_observer_v1(
            &function,
            Some(&types),
            &callables,
            &transparent,
            &mut trace,
        ),
        Err(HOOK_DENIED)
    );
    assert_eq!(trace.elision_lookups.last(), Some(&(site, 1)));
    assert!(!trace.visits.contains(&(Visit::Statement, site)));
    assert!(
        trace
            .events
            .iter()
            .all(|event| event.site != site && event.site != Site::Terminator { block: 2 })
    );
    assert!(trace.elided.is_empty());
    assert_eq!(
        trace
            .blocks
            .iter()
            .map(|(block, _, _)| *block)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert!(trace.entries.is_empty());
    assert!(trace.input.is_empty());
}

#[test]
fn denied_successor_stops_before_its_edge_definition_and_later_edges() {
    let returned = test_edge(SemanticEdgeRoleV1::CallReturn, 1);
    let unwound = test_edge(SemanticEdgeRoleV1::CallUnwind, 1);
    let function = test_function(vec![
        test_block(
            92,
            vec![],
            test_call_with_unwind(
                SemanticCallDestinationV1::new(test_scalar_place(2), returned),
                unwound,
            ),
        ),
        test_block(93, vec![], SemanticTerminatorKindV1::Return),
    ]);
    for ordinal in [0, 1] {
        let mut trace = Trace {
            reject_hook: Some(RejectedHook::Successor { block: 0, ordinal }),
            ..Trace::default()
        };
        assert_eq!(
            semantic_function_ssa_input_with_observer_v1(
                &function,
                None,
                &[],
                &BTreeSet::new(),
                &mut trace,
            ),
            Err(HOOK_DENIED)
        );
        if ordinal == 0 {
            assert_eq!(trace.successors, [(0, 0, returned)]);
            assert!(trace.edge_definitions.is_empty());
        } else {
            assert_eq!(trace.successors, [(0, 0, returned), (0, 1, unwound)]);
            assert_eq!(trace.edge_definitions, [(0, 0, returned, 0, variable(2))]);
        }
        assert!(trace.blocks.is_empty());
        assert!(trace.entries.is_empty());
        assert!(trace.input.is_empty());
        assert!(!trace.visits.contains(&(Visit::Block, Site::Block(1))));
    }
}

#[test]
fn denied_entry_definition_stops_before_terminal_input_completion() {
    let function = test_function(vec![test_block(
        94,
        vec![],
        SemanticTerminatorKindV1::Return,
    )]);
    let mut trace = Trace {
        reject_hook: Some(RejectedHook::Entry(0)),
        ..Trace::default()
    };
    assert_eq!(
        semantic_function_ssa_input_with_observer_v1(
            &function,
            None,
            &[],
            &BTreeSet::new(),
            &mut trace,
        ),
        Err(HOOK_DENIED)
    );
    assert_eq!(trace.blocks, [(0, 0, 0)]);
    assert_eq!(trace.entries, [(0, variable(1), Entry::Argument(0))]);
    assert_eq!(
        trace.visits.last(),
        Some(&(Visit::EntryCandidate, Site::Local(1)))
    );
    assert!(trace.input.is_empty());
}

#[test]
fn denied_completion_hooks_do_not_return_a_partial_adapter_input() {
    let function = test_function(vec![test_block(
        95,
        vec![],
        SemanticTerminatorKindV1::Return,
    )]);
    for denied in [RejectedHook::BlockComplete(0), RejectedHook::InputComplete] {
        let mut trace = Trace {
            reject_hook: Some(denied),
            ..Trace::default()
        };
        assert_eq!(
            semantic_function_ssa_input_with_observer_v1(
                &function,
                None,
                &[],
                &BTreeSet::new(),
                &mut trace,
            ),
            Err(HOOK_DENIED)
        );
        assert_eq!(trace.blocks, [(0, 0, 0)]);
        if denied == RejectedHook::BlockComplete(0) {
            assert!(trace.entries.is_empty());
            assert!(trace.input.is_empty());
            assert!(
                !trace
                    .visits
                    .iter()
                    .any(|(visit, _)| *visit == Visit::EntryCandidate)
            );
        } else {
            assert_eq!(trace.entries, [(0, variable(1), Entry::Argument(0))]);
            assert_eq!(trace.input, [(1, 1)]);
        }
    }
}
