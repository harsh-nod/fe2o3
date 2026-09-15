use super::super::super::adapter::emission_v1::{
    SemanticSsaEmissionErrorV1 as EmissionError, SemanticSsaEventBufferV1,
    emit_statement_events_with_buffer_v1,
};
use super::super::super::adapter::prepared_v1::{
    SemanticSsaBlockOutputV1, SemanticSsaEntryOutputV1,
    prepare_semantic_ssa_adapter_with_observer_v1,
};
use super::*;

mod capture_prepass_v1_tests {
    include!("capture_prepass_v1_tests.rs");
}

mod call_address_output_tests {
    include!("call_address_output_v1_tests.rs");
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Overflow {
    Blocks,
    Events,
    Successors,
    Definitions,
    Entries,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Counts {
    blocks: usize,
    events: usize,
    successors: usize,
    definitions: usize,
    entries: usize,
}

impl Counts {
    fn tuple(self) -> (usize, usize, usize, usize, usize) {
        (
            self.blocks,
            self.events,
            self.successors,
            self.definitions,
            self.entries,
        )
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
struct CountEvents(usize);

impl SemanticSsaEventBufferV1 for CountEvents {
    type Error = Overflow;

    fn event_count(&self) -> usize {
        self.0
    }

    fn push_event(&mut self, _: SsaEventV1) -> Result<(), Overflow> {
        let next = self.0.checked_add(1).ok_or(Overflow::Events)?;
        self.0 = next;
        Ok(())
    }
}

// Only numeric state: no event, edge, block, entry or planner-input vectors.
// Seeds make overflow reachable without constructing an enormous source.
#[derive(Debug, Default, Eq, PartialEq)]
struct CountOutput {
    counts: Counts,
    pending_successors: usize,
    pending_definitions: usize,
    event_seed: usize,
    successor_seed: usize,
    definition_seed: usize,
}

impl SemanticSsaBlockOutputV1 for CountOutput {
    type Error = Overflow;
    type Events = CountEvents;

    fn begin_block(&mut self) -> Result<CountEvents, Overflow> {
        Ok(CountEvents(self.event_seed))
    }

    fn begin_successors(&mut self, _: usize) -> Result<(), Overflow> {
        self.pending_successors = self.successor_seed;
        self.pending_definitions = self.definition_seed;
        Ok(())
    }

    fn successor_count(&self) -> usize {
        self.pending_successors
    }

    fn push_successor(
        &mut self,
        _: SsaEdgeRoleV1,
        _: SsaBlockIdV1,
        definition: Option<SsaVariableIdV1>,
    ) -> Result<(), Overflow> {
        let successors = self
            .pending_successors
            .checked_add(1)
            .ok_or(Overflow::Successors)?;
        let definitions = self
            .pending_definitions
            .checked_add(usize::from(definition.is_some()))
            .ok_or(Overflow::Definitions)?;
        self.pending_successors = successors;
        self.pending_definitions = definitions;
        Ok(())
    }

    fn finish_block(&mut self, events: CountEvents) -> Result<(), Overflow> {
        let blocks = self.counts.blocks.checked_add(1).ok_or(Overflow::Blocks)?;
        let events = self
            .counts
            .events
            .checked_add(events.0)
            .ok_or(Overflow::Events)?;
        let successors = self
            .counts
            .successors
            .checked_add(self.pending_successors)
            .ok_or(Overflow::Successors)?;
        let definitions = self
            .counts
            .definitions
            .checked_add(self.pending_definitions)
            .ok_or(Overflow::Definitions)?;
        self.counts = Counts {
            blocks,
            events,
            successors,
            definitions,
            entries: self.counts.entries,
        };
        self.pending_successors = 0;
        self.pending_definitions = 0;
        Ok(())
    }
}

impl SemanticSsaEntryOutputV1 for CountOutput {
    type Error = Overflow;

    fn entry_count(&self) -> usize {
        self.counts.entries
    }

    fn push_entry(&mut self, _: SsaVariableIdV1) -> Result<(), Overflow> {
        let entries = self
            .counts
            .entries
            .checked_add(1)
            .ok_or(Overflow::Entries)?;
        self.counts.entries = entries;
        Ok(())
    }
}

struct Outcome {
    input: SsaConstructionInputV1,
    implicit: Vec<SsaVariableIdV1>,
    counts: Counts,
    trace: Trace,
}

fn run_prepared(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    callables: &[SemanticCallableDeclV1],
) -> Outcome {
    let transparent = transparent_borrow_sites_v1(function, callables);
    let mut preparation = Trace::default();
    let prepared = prepare_semantic_ssa_adapter_with_observer_v1(
        function,
        types,
        callables,
        &transparent,
        &mut preparation,
    )
    .unwrap();
    assert_eq!(preparation.visits, [(Visit::Function, Site::Function)]);
    assert!(preparation.events.is_empty());
    assert!(preparation.blocks.is_empty());
    assert!(preparation.entries.is_empty());
    assert!(!std::mem::needs_drop::<CountOutput>());
    assert!(!std::mem::needs_drop::<CountEvents>());
    let mut counts = CountOutput::default();
    let mut counted = Trace::default();
    let mut actual = Trace::default();
    prepared.emit_blocks(&mut counts, &mut counted).unwrap();
    let entries = prepared.into_entries(&mut actual).unwrap();
    entries.emit_entries(&mut counts, &mut counted).unwrap();
    let (input, implicit, _) = entries.finish(&mut actual).unwrap();
    assert_same_trace(&counted, &actual);
    Outcome {
        input,
        implicit,
        counts: counts.counts,
        trace: actual,
    }
}

fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1::unavailable(),
        kind,
    )
}

fn replace_source_parts(
    function: &SemanticFunctionDeclV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    assert!(function.export().is_none());
    SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        locals,
        function.entry(),
        blocks,
    )
    .unwrap()
}

fn call_with_duplicate_target(
    arguments: Vec<SemanticOperandV1>,
    destination: SemanticPlaceV1,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            arguments,
            Some(SemanticCallDestinationV1::new(
                destination,
                test_edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Cleanup(test_edge(SemanticEdgeRoleV1::CallUnwind, 1)),
        )
        .unwrap(),
    )
}

fn mixed_source() -> SemanticFunctionDeclV1 {
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
    test_function_with_reference_locals(
        vec![
            test_block(
                180,
                vec![
                    statement(SemanticStatementKindV1::Nop),
                    statement(SemanticStatementKindV1::Assume(
                        SemanticOperandV1::Constant(constant()),
                    )),
                    statement(assignment),
                ],
                call_with_duplicate_target(
                    vec![
                        SemanticOperandV1::Constant(constant()),
                        SemanticOperandV1::Copy(test_scalar_place(3)),
                        SemanticOperandV1::Move(test_scalar_place(3)),
                        SemanticOperandV1::Copy(indexed(2, 4)),
                    ],
                    test_scalar_place(5),
                ),
            ),
            test_block(181, vec![], SemanticTerminatorKindV1::Return),
            test_block(
                182,
                vec![test_assign(2, SemanticOperandV1::Constant(constant()))],
                SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
        ],
        6,
    )
}

fn repeated_elisions(
    prefix: bool,
    gap: bool,
    suffix: bool,
) -> (
    Vec<SemanticTypeDeclV1>,
    SemanticFunctionDeclV1,
    Vec<SemanticCallableDeclV1>,
) {
    let (types, source, callables) = test_elided_grid_leader_case(0, true, false, false, false);
    let mut statements = Vec::new();
    if prefix {
        statements.push(statement(SemanticStatementKindV1::Nop));
    }
    statements.push(test_typed_borrow(4, 2, 1, 0));
    if gap {
        statements.push(statement(SemanticStatementKindV1::Nop));
    }
    statements.push(test_typed_borrow(5, 2, 1, 0));
    if suffix {
        statements.push(statement(SemanticStatementKindV1::Nop));
    }
    let mut blocks = source.blocks().to_vec();
    blocks[2] = test_block(
        120,
        statements,
        source.blocks()[2].terminator().kind().clone(),
    );
    blocks[5] = test_block(
        123,
        vec![],
        test_call(
            1,
            vec![
                SemanticOperandV1::Copy(test_typed_place(2, 2)),
                SemanticOperandV1::Copy(test_typed_place(5, 2)),
            ],
            Some(SemanticCallDestinationV1::new(
                test_typed_place(6, 2),
                test_edge(SemanticEdgeRoleV1::CallReturn, 7),
            )),
        ),
    );
    blocks.push(test_block(
        125,
        vec![test_discriminant(10, 6)],
        test_option_switch(10, 8, 6),
    ));
    blocks.push(test_block(126, vec![], SemanticTerminatorKindV1::Return));
    let mut locals = source.locals().to_vec();
    locals.push(test_local(199, 2, SemanticLocalRoleV1::Temporary));
    let function = replace_source_parts(&source, locals, blocks);
    (types, function, callables)
}

fn two_implicit_scopes() -> (
    Vec<SemanticTypeDeclV1>,
    SemanticFunctionDeclV1,
    Vec<SemanticCallableDeclV1>,
) {
    let source = test_implicit_scope_function(
        0,
        vec![test_block(190, vec![], SemanticTerminatorKindV1::Return)],
    );
    let callables = vec![test_intrinsic_callable(source.abi().clone())];
    let function = replace_source_parts(
        &source,
        vec![
            test_local(191, 2, SemanticLocalRoleV1::Return),
            test_local(192, 0, SemanticLocalRoleV1::Temporary),
            test_local(193, 2, SemanticLocalRoleV1::Temporary),
            test_local(194, 2, SemanticLocalRoleV1::Argument(0)),
            test_local(195, 1, SemanticLocalRoleV1::Temporary),
            test_local(196, 2, SemanticLocalRoleV1::Temporary),
            test_local(197, 0, SemanticLocalRoleV1::Temporary),
            test_local(198, 2, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            test_block(
                199,
                vec![test_typed_borrow(2, 2, 1, 0)],
                test_call(
                    0,
                    vec![SemanticOperandV1::Copy(test_typed_place(2, 2))],
                    Some(SemanticCallDestinationV1::new(
                        test_typed_place(7, 2),
                        test_edge(SemanticEdgeRoleV1::CallReturn, 1),
                    )),
                ),
            ),
            test_block(
                200,
                vec![test_typed_borrow(5, 2, 6, 0)],
                test_call(
                    0,
                    vec![SemanticOperandV1::Copy(test_typed_place(5, 2))],
                    None,
                ),
            ),
        ],
    );
    (implicit_scope_types(), function, callables)
}

fn assert_same_trace(left: &Trace, right: &Trace) {
    assert_eq!(left.block_passes, right.block_passes);
    assert_eq!(left.entry_passes, right.entry_passes);
    assert_eq!(left.visits, right.visits);
    assert_eq!(left.events, right.events);
    assert_eq!(left.constants, right.constants);
    assert_eq!(left.successors, right.successors);
    assert_eq!(left.edge_definitions, right.edge_definitions);
    assert_eq!(left.entries, right.entries);
    assert_eq!(left.elided, right.elided);
    assert_eq!(left.elision_lookups, right.elision_lookups);
    assert_eq!(left.blocks, right.blocks);
    assert_eq!(left.input, right.input);
}

fn assert_edge(edge: &SsaEdgeInputV1, role: SemanticEdgeRoleV1, target: u32, defs: &[u32]) {
    assert_eq!(edge.role(), SsaEdgeRoleV1::new(semantic_edge_role_v1(role)));
    assert_eq!(edge.target(), SsaBlockIdV1::new(target));
    assert_eq!(
        edge.definitions()
            .iter()
            .map(|var| var.get())
            .collect::<Vec<_>>(),
        defs
    );
}

#[test]
fn eventless_and_literal_inputs_have_independent_counts() {
    for (statements, events, expected_events, constant_site, constant_role) in [
        (
            vec![
                statement(SemanticStatementKindV1::Nop),
                statement(SemanticStatementKindV1::Assume(
                    SemanticOperandV1::Constant(constant()),
                )),
            ],
            vec![],
            0,
            Site::Statement {
                block: 0,
                statement: 1,
            },
            Operand::Assume,
        ),
        (
            vec![test_assign(2, SemanticOperandV1::Constant(constant()))],
            vec![defined(2)],
            1,
            Site::Statement {
                block: 0,
                statement: 0,
            },
            Operand::RvalueOperand(0),
        ),
    ] {
        let function = test_function(vec![test_block(
            201,
            statements,
            SemanticTerminatorKindV1::Return,
        )]);
        let outcome = run_prepared(&function, None, &[]);
        assert_eq!(outcome.counts.tuple(), (1, expected_events, 0, 0, 1));
        assert_eq!(outcome.input.entry(), SsaBlockIdV1::new(0));
        assert_eq!(outcome.input.variable_count(), 4);
        assert_eq!(outcome.input.promotable(), [true; 4]);
        assert_eq!(outcome.input.entry_definitions(), [variable(1)]);
        assert_eq!(outcome.input.blocks().len(), 1);
        assert_eq!(outcome.input.blocks()[0].events(), events);
        assert!(outcome.input.blocks()[0].edges().is_empty());
        assert!(outcome.implicit.is_empty());
        assert_eq!(
            outcome.trace.entries,
            [(0, variable(1), Entry::Argument(0))]
        );
        assert_eq!(
            outcome.trace.constants,
            [Constant {
                site: constant_site,
                operand: constant_role,
                next_event: 0,
                value: constant(),
            }]
        );
        assert_eq!(outcome.trace.input, [(1, 1)]);
    }
}

#[test]
fn count_and_fill_preserve_move_projection_unreachable_and_return_edges() {
    let function = mixed_source();
    let outcome = run_prepared(&function, None, &[]);
    // Ten events in block0 plus the unreachable block2's one Define; two
    // same-target call successors plus block2's Goto; only CallReturn defines.
    assert_eq!(outcome.counts.tuple(), (3, 11, 3, 1, 1));
    assert_eq!(outcome.input.entry(), SsaBlockIdV1::new(0));
    assert_eq!(outcome.input.variable_count(), 6);
    assert_eq!(outcome.input.promotable(), [true; 6]);
    assert_eq!(outcome.input.entry_definitions(), [variable(1)]);
    let blocks = outcome.input.blocks();
    assert_eq!(blocks.len(), 3);
    assert_eq!(
        blocks[0].events(),
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
            used(4),
        ]
    );
    assert!(blocks[1].events().is_empty());
    assert_eq!(blocks[2].events(), [defined(2)]);
    assert_eq!(blocks[0].edges().len(), 2);
    assert_edge(
        &blocks[0].edges()[0],
        SemanticEdgeRoleV1::CallReturn,
        1,
        &[5],
    );
    assert_edge(
        &blocks[0].edges()[1],
        SemanticEdgeRoleV1::CallUnwind,
        1,
        &[],
    );
    assert!(blocks[1].edges().is_empty());
    assert_eq!(blocks[2].edges().len(), 1);
    assert_edge(&blocks[2].edges()[0], SemanticEdgeRoleV1::Goto, 1, &[]);
    assert_eq!(outcome.trace.blocks, [(0, 10, 2), (1, 0, 0), (2, 1, 1)]);
    assert_eq!(
        outcome
            .trace
            .events
            .iter()
            .map(|row| row.ordinal)
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0],
    );
    assert_eq!(
        outcome.trace.events[3].role,
        EventRole::ProjectionIndexUse(1)
    );
    assert_eq!(
        outcome.trace.events[9].role,
        EventRole::ProjectionIndexUse(1)
    );
    assert_eq!(
        outcome.trace.constants,
        [
            Constant {
                site: Site::Statement {
                    block: 0,
                    statement: 1
                },
                operand: Operand::Assume,
                next_event: 0,
                value: constant(),
            },
            Constant {
                site: Site::Terminator { block: 0 },
                operand: Operand::CallArgument(0),
                next_event: 5,
                value: constant(),
            },
            Constant {
                site: Site::Statement {
                    block: 2,
                    statement: 0
                },
                operand: Operand::RvalueOperand(0),
                next_event: 0,
                value: constant(),
            },
        ]
    );
}

#[test]
fn duplicate_switch_occurrences_are_counted_individually() {
    let function = test_function(vec![
        test_block(
            202,
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Constant(constant()),
                targets: SemanticSwitchTargetsV1::new(
                    vec![
                        SemanticSwitchTargetV1::new(
                            0,
                            test_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        ),
                        SemanticSwitchTargetV1::new(
                            1,
                            test_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        ),
                    ],
                    test_edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                )
                .unwrap(),
            },
        ),
        test_block(203, vec![], SemanticTerminatorKindV1::Return),
    ]);
    let outcome = run_prepared(&function, None, &[]);
    assert_eq!(outcome.counts.tuple(), (2, 0, 3, 0, 1));
    assert_eq!(
        outcome.trace.successors,
        [
            (0, 0, test_edge(SemanticEdgeRoleV1::SwitchValue, 1)),
            (0, 1, test_edge(SemanticEdgeRoleV1::SwitchValue, 1)),
            (0, 2, test_edge(SemanticEdgeRoleV1::SwitchOtherwise, 1)),
        ]
    );
    let edges = outcome.input.blocks()[0].edges();
    assert_eq!(edges.len(), 3);
    assert_edge(&edges[0], SemanticEdgeRoleV1::SwitchValue, 1, &[]);
    assert_edge(&edges[1], SemanticEdgeRoleV1::SwitchValue, 1, &[]);
    assert_edge(&edges[2], SemanticEdgeRoleV1::SwitchOtherwise, 1, &[]);
    assert!(outcome.trace.edge_definitions.is_empty());
    assert!(outcome.trace.events.is_empty());
}

#[test]
fn projected_call_destination_counts_only_address_inputs() {
    let function = test_function(vec![
        test_block(
            204,
            vec![],
            call_with_duplicate_target(
                vec![SemanticOperandV1::Constant(constant())],
                indexed(1, 2),
            ),
        ),
        test_block(205, vec![], SemanticTerminatorKindV1::Return),
    ]);
    let outcome = run_prepared(&function, None, &[]);
    assert_eq!(outcome.counts.tuple(), (2, 1, 2, 0, 1));
    assert_eq!(outcome.input.promotable(), [true, false, true, true]);
    assert_eq!(outcome.input.blocks()[0].events(), [used(2)]);
    assert_eq!(
        outcome.trace.events,
        [row(
            Site::Terminator { block: 0 },
            Operand::CallDestinationAddress,
            EventRole::ProjectionIndexUse(1),
            0,
            used(2),
        )]
    );
    assert!(outcome.trace.edge_definitions.is_empty());
    assert_eq!(
        outcome
            .trace
            .visits
            .iter()
            .filter(|(visit, _)| *visit == Visit::Projection)
            .count(),
        4
    );
    assert_eq!(
        outcome
            .trace
            .visits
            .iter()
            .filter(|(visit, _)| *visit == Visit::Place)
            .count(),
        1
    );
    assert_eq!(outcome.input.blocks()[0].edges().len(), 2);
    assert_edge(
        &outcome.input.blocks()[0].edges()[0],
        SemanticEdgeRoleV1::CallReturn,
        1,
        &[],
    );
    assert_edge(
        &outcome.input.blocks()[0].edges()[1],
        SemanticEdgeRoleV1::CallUnwind,
        1,
        &[],
    );
}

#[test]
fn actual_elision_classifier_selects_exact_sites_in_both_passes() {
    let (types, function, callables) = test_elided_grid_leader_case(0, true, false, false, false);
    let outcome = run_prepared(&function, Some(&types), &callables);
    assert_eq!(outcome.counts.tuple(), (7, 9, 6, 2, 0));
    assert_eq!(
        outcome.trace.elided,
        [Site::Statement {
            block: 2,
            statement: 0
        }]
    );
    assert_eq!(
        outcome.input.blocks()[2].events(),
        [defined(4), used(2), used(4)]
    );
    assert!(outcome.implicit.is_empty());
    assert!(outcome.input.entry_definitions().is_empty());

    for (prefix, gap, suffix) in [
        (false, false, false),
        (false, true, false),
        (true, true, true),
    ] {
        let (types, function, callables) = repeated_elisions(prefix, gap, suffix);
        let outcome = run_prepared(&function, Some(&types), &callables);
        // Events in blocks1/2/4/5/7 are3+4+3+2+3. Both Option results retain
        // their own discriminator/switch. Nops add no events.
        assert_eq!(outcome.counts.tuple(), (9, 15, 9, 3, 0));
        assert_eq!(
            outcome.trace.elided,
            [
                Site::Statement {
                    block: 2,
                    statement: usize::from(prefix)
                },
                Site::Statement {
                    block: 2,
                    statement: usize::from(prefix) + 1 + usize::from(gap)
                },
            ]
        );
        assert_eq!(
            outcome.input.blocks()[2].events(),
            [defined(4), defined(5), used(2), used(4)]
        );
        assert_eq!(
            outcome.input.blocks()[4].events(),
            [used(8), defined(9), used(9)]
        );
        assert_eq!(outcome.input.blocks()[5].events(), [used(2), used(5)]);
        assert_eq!(
            outcome.input.blocks()[7].events(),
            [used(6), defined(10), used(10)]
        );
        assert!(outcome.implicit.is_empty());
    }
}

#[test]
fn actual_implicit_classifier_orders_capabilities_and_arguments() {
    let types = implicit_scope_types();
    let function = test_implicit_scope_function(
        0,
        vec![test_block(
            206,
            vec![test_typed_borrow(2, 2, 1, 0)],
            test_call(
                0,
                vec![SemanticOperandV1::Copy(test_typed_place(2, 2))],
                None,
            ),
        )],
    );
    let callables = vec![test_intrinsic_callable(function.abi().clone())];
    let outcome = run_prepared(&function, Some(&types), &callables);
    assert_eq!(outcome.counts.tuple(), (1, 3, 0, 0, 1));
    assert_eq!(outcome.implicit, [variable(1)]);
    assert_eq!(
        outcome.trace.entries,
        [(0, variable(1), Entry::ImplicitCapability)]
    );
    assert_eq!(
        outcome.input.blocks()[0].events(),
        [used(1), defined(2), used(2)]
    );

    let (types, function, callables) = two_implicit_scopes();
    let outcome = run_prepared(&function, Some(&types), &callables);
    assert_eq!(outcome.counts.tuple(), (2, 6, 1, 1, 3));
    assert_eq!(outcome.implicit, [variable(1), variable(6)]);
    assert_eq!(
        outcome.input.entry_definitions(),
        [variable(1), variable(3), variable(6)]
    );
    assert_eq!(
        outcome.trace.entries,
        [
            (0, variable(1), Entry::ImplicitCapability),
            (1, variable(3), Entry::Argument(0)),
            (2, variable(6), Entry::ImplicitCapability),
        ]
    );
    assert_eq!(
        outcome.input.blocks()[0].events(),
        [used(1), defined(2), used(2)]
    );
    assert_eq!(
        outcome.input.blocks()[1].events(),
        [used(6), defined(5), used(5)]
    );
    assert_eq!(outcome.input.blocks()[0].edges().len(), 1);
    assert_edge(
        &outcome.input.blocks()[0].edges()[0],
        SemanticEdgeRoleV1::CallReturn,
        1,
        &[7],
    );
}

#[test]
fn event_count_overflow_and_observer_refusal_precede_the_failed_push() {
    let site = Site::Statement {
        block: 0,
        statement: 0,
    };
    let assignment = test_assign(2, SemanticOperandV1::Move(test_scalar_place(1)));
    for reject in [false, true] {
        let mut events = CountEvents(usize::MAX);
        let mut trace = Trace {
            reject_event: reject.then_some(usize::MAX),
            ..Trace::default()
        };
        let result = emit_statement_events_with_buffer_v1(
            assignment.kind(),
            false,
            site,
            &mut events,
            &mut trace,
        );
        let expected = if reject {
            EmissionError::Observer(usize::MAX)
        } else {
            EmissionError::Output(Overflow::Events)
        };
        assert_eq!(result, Err(expected));
        assert_eq!(events, CountEvents(usize::MAX));
        assert_eq!(
            trace.events,
            [row(
                site,
                Operand::RvalueOperand(0),
                EventRole::BaseUse,
                usize::MAX,
                used(1),
            )]
        );
    }
    let mut events = CountEvents(usize::MAX - 1);
    let mut trace = Trace::default();
    assert_eq!(
        emit_statement_events_with_buffer_v1(
            assignment.kind(),
            false,
            site,
            &mut events,
            &mut trace,
        ),
        Err(EmissionError::Output(Overflow::Events))
    );
    assert_eq!(events, CountEvents(usize::MAX));
    assert_eq!(
        trace.events,
        [
            row(
                site,
                Operand::RvalueOperand(0),
                EventRole::BaseUse,
                usize::MAX - 1,
                used(1)
            ),
            row(
                site,
                Operand::RvalueOperand(0),
                EventRole::MoveKill,
                usize::MAX,
                killed(1)
            ),
        ]
    );
    let mut trace = Trace::default();
    emit_statement_events_with_buffer_v1(
        &SemanticStatementKindV1::Assume(SemanticOperandV1::Constant(constant())),
        false,
        site,
        &mut events,
        &mut trace,
    )
    .unwrap();
    assert_eq!(events, CountEvents(usize::MAX));
    assert!(trace.events.is_empty());
    assert_eq!(trace.constants[0].next_event, usize::MAX);
}

#[test]
fn count_buffer_stops_at_place_and_projection_previsits() {
    let assignment = test_assign(2, SemanticOperandV1::Move(indexed(1, 3)));
    let site = Site::Statement {
        block: 0,
        statement: 0,
    };
    for (denied, prefix) in [(Visit::Place, 0), (Visit::Projection, 1)] {
        let mut events = CountEvents::default();
        let mut trace = Trace {
            reject_hook: Some(RejectedHook::Visit(denied)),
            ..Trace::default()
        };
        assert_eq!(
            emit_statement_events_with_buffer_v1(
                assignment.kind(),
                false,
                site,
                &mut events,
                &mut trace,
            ),
            Err(EmissionError::Observer(HOOK_DENIED))
        );
        assert_eq!(events, CountEvents(prefix));
        assert_eq!(trace.visits.last(), Some(&(denied, site)));
        if denied == Visit::Place {
            assert!(trace.events.is_empty());
        } else {
            assert_eq!(
                trace.events,
                [row(
                    site,
                    Operand::RvalueOperand(0),
                    EventRole::BaseUse,
                    0,
                    used(1),
                )]
            );
        }
    }
}

#[test]
fn checked_successor_counts_commit_neither_half_of_an_overflowing_row() {
    let function = mixed_source();
    let transparent = transparent_borrow_sites_v1(&function, &[]);
    let prepared = prepare_semantic_ssa_adapter_with_observer_v1(
        &function,
        None,
        &[],
        &transparent,
        &mut Trace::default(),
    )
    .unwrap();
    for (successor_seed, definition_seed, expected) in [
        (usize::MAX, 0, Overflow::Successors),
        (0, usize::MAX, Overflow::Definitions),
    ] {
        let mut counts = CountOutput {
            successor_seed,
            definition_seed,
            ..CountOutput::default()
        };
        let mut trace = Trace::default();
        assert_eq!(
            prepared.emit_blocks(&mut counts, &mut trace),
            Err(EmissionError::Output(expected))
        );
        assert_eq!(counts.counts, Counts::default());
        assert_eq!(counts.pending_successors, successor_seed);
        assert_eq!(counts.pending_definitions, definition_seed);
        assert_eq!(
            trace.successors,
            [(
                0,
                successor_seed,
                test_edge(SemanticEdgeRoleV1::CallReturn, 1)
            )]
        );
        assert_eq!(
            trace.edge_definitions,
            [(
                0,
                successor_seed,
                test_edge(SemanticEdgeRoleV1::CallReturn, 1),
                0,
                variable(5)
            )]
        );
        assert_eq!(trace.events.len(), 10);
        assert!(trace.blocks.is_empty());
        assert!(trace.input.is_empty());
    }
    let mut counts = CountOutput {
        successor_seed: usize::MAX,
        ..CountOutput::default()
    };
    let mut trace = Trace {
        reject_hook: Some(RejectedHook::Successor {
            block: 0,
            ordinal: usize::MAX,
        }),
        ..Trace::default()
    };
    assert_eq!(
        prepared.emit_blocks(&mut counts, &mut trace),
        Err(EmissionError::Observer(HOOK_DENIED))
    );
    assert_eq!(counts.pending_successors, usize::MAX);
    assert_eq!(counts.pending_definitions, 0);
    assert_eq!(counts.counts, Counts::default());
    assert!(trace.edge_definitions.is_empty());

    // A successor with no definition must not add one to this counter.
    let mut counts = CountOutput {
        pending_definitions: usize::MAX,
        ..CountOutput::default()
    };
    counts
        .push_successor(SsaEdgeRoleV1::new(0), SsaBlockIdV1::new(0), None)
        .unwrap();
    assert_eq!(counts.pending_successors, 1);
    assert_eq!(counts.pending_definitions, usize::MAX);
}

#[test]
fn checked_block_totals_fail_atomically_after_the_observed_prefix() {
    let function = mixed_source();
    let transparent = transparent_borrow_sites_v1(&function, &[]);
    let prepared = prepare_semantic_ssa_adapter_with_observer_v1(
        &function,
        None,
        &[],
        &transparent,
        &mut Trace::default(),
    )
    .unwrap();
    for (initial, expected) in [
        (
            Counts {
                blocks: usize::MAX,
                ..Counts::default()
            },
            Overflow::Blocks,
        ),
        (
            Counts {
                events: usize::MAX,
                ..Counts::default()
            },
            Overflow::Events,
        ),
        (
            Counts {
                successors: usize::MAX,
                ..Counts::default()
            },
            Overflow::Successors,
        ),
        (
            Counts {
                definitions: usize::MAX,
                ..Counts::default()
            },
            Overflow::Definitions,
        ),
    ] {
        let mut counts = CountOutput {
            counts: initial,
            ..CountOutput::default()
        };
        let mut trace = Trace::default();
        assert_eq!(
            prepared.emit_blocks(&mut counts, &mut trace),
            Err(EmissionError::Output(expected))
        );
        assert_eq!(counts.counts, initial);
        assert_eq!(counts.pending_successors, 2);
        assert_eq!(counts.pending_definitions, 1);
        assert_eq!(trace.blocks, [(0, 10, 2)]);
        assert!(!trace.visits.contains(&(Visit::Block, Site::Block(1))));
        assert!(trace.entries.is_empty());
        assert!(trace.input.is_empty());
    }
    let mut counts = CountOutput {
        event_seed: usize::MAX,
        ..CountOutput::default()
    };
    let mut trace = Trace::default();
    assert_eq!(
        prepared.emit_blocks(&mut counts, &mut trace),
        Err(EmissionError::Output(Overflow::Events))
    );
    assert_eq!(counts.counts, Counts::default());
    assert_eq!(trace.events.len(), 1);
    assert_eq!(trace.events[0].ordinal, usize::MAX);
    assert!(trace.successors.is_empty());
    assert!(trace.blocks.is_empty());
}

#[test]
fn denied_elision_lookup_does_not_consume_a_site_in_a_later_pass() {
    let (types, function, callables) = repeated_elisions(false, false, false);
    let transparent = transparent_borrow_sites_v1(&function, &callables);
    let prepared = prepare_semantic_ssa_adapter_with_observer_v1(
        &function,
        Some(&types),
        &callables,
        &transparent,
        &mut Trace::default(),
    )
    .unwrap();
    for denied_statement in [0, 1] {
        let site = Site::Statement {
            block: 2,
            statement: denied_statement,
        };
        let mut counts = CountOutput::default();
        let mut trace = Trace {
            reject_hook: Some(RejectedHook::ElisionLookup(site)),
            ..Trace::default()
        };
        assert_eq!(
            prepared.emit_blocks(&mut counts, &mut trace),
            Err(EmissionError::Observer(HOOK_DENIED))
        );
        assert_eq!(counts.counts.tuple(), (2, 3, 3, 1, 0));
        assert_eq!(trace.elision_lookups.last(), Some(&(site, 2)));
        assert!(!trace.visits.contains(&(Visit::Statement, site)));
        assert_eq!(trace.elided.len(), denied_statement);
        assert_eq!(trace.blocks, [(0, 0, 1), (1, 3, 2)]);
        assert!(trace.entries.is_empty());
        assert!(trace.input.is_empty());
    }
    let mut counts = CountOutput::default();
    let mut counted = Trace::default();
    prepared.emit_blocks(&mut counts, &mut counted).unwrap();
    let mut actual = Trace::default();
    let entries = prepared.into_entries(&mut actual).unwrap();
    entries.emit_entries(&mut counts, &mut counted).unwrap();
    let (input, implicit, _) = entries.finish(&mut actual).unwrap();
    assert_same_trace(&counted, &actual);
    assert_eq!(counts.counts.tuple(), (9, 15, 9, 3, 0));
    assert_eq!(
        input.blocks()[2].events(),
        [defined(4), defined(5), used(2), used(4)]
    );
    assert!(implicit.is_empty());
}

#[test]
fn entry_refusal_and_overflow_preserve_the_counter_and_pending_capability() {
    let (types, function, callables) = two_implicit_scopes();
    let transparent = transparent_borrow_sites_v1(&function, &callables);
    let prepared = prepare_semantic_ssa_adapter_with_observer_v1(
        &function,
        Some(&types),
        &callables,
        &transparent,
        &mut Trace::default(),
    )
    .unwrap();
    let mut actual = Trace::default();
    let entries = prepared.into_entries(&mut actual).unwrap();
    for reject in [false, true] {
        let mut counts = CountOutput {
            counts: Counts {
                entries: usize::MAX,
                ..Counts::default()
            },
            ..CountOutput::default()
        };
        let initial = counts.counts;
        let mut trace = Trace {
            reject_hook: reject.then_some(RejectedHook::Entry(usize::MAX)),
            ..Trace::default()
        };
        let expected = if reject {
            EmissionError::Observer(HOOK_DENIED)
        } else {
            EmissionError::Output(Overflow::Entries)
        };
        assert_eq!(entries.emit_entries(&mut counts, &mut trace), Err(expected));
        assert_eq!(counts.counts, initial);
        assert_eq!(
            trace.entries,
            [(usize::MAX, variable(1), Entry::ImplicitCapability)]
        );
        assert_eq!(
            trace.visits.last(),
            Some(&(Visit::EntryCandidate, Site::Local(1)))
        );
        assert!(trace.input.is_empty());
    }
    let mut counts = CountOutput::default();
    let mut trace = Trace {
        reject_hook: Some(RejectedHook::Entry(1)),
        ..Trace::default()
    };
    assert_eq!(
        entries.emit_entries(&mut counts, &mut trace),
        Err(EmissionError::Observer(HOOK_DENIED))
    );
    assert_eq!(counts.counts.entries, 1);
    assert_eq!(
        trace.entries,
        [
            (0, variable(1), Entry::ImplicitCapability),
            (1, variable(3), Entry::Argument(0)),
        ]
    );
    assert!(
        !trace
            .visits
            .contains(&(Visit::EntryCandidate, Site::Local(6)))
    );
    let mut counts = CountOutput::default();
    let mut counted = Trace::default();
    entries.emit_entries(&mut counts, &mut counted).unwrap();
    let (input, implicit, _) = entries.finish(&mut actual).unwrap();
    assert_eq!(counts.counts.entries, 3);
    assert_eq!(
        input.entry_definitions(),
        [variable(1), variable(3), variable(6)]
    );
    assert_eq!(implicit, [variable(1), variable(6)]);
    assert_eq!(counted.entries, actual.entries);
}

#[test]
fn denied_terminal_hooks_do_not_commit_the_enclosing_output() {
    let function = mixed_source();
    let transparent = transparent_borrow_sites_v1(&function, &[]);
    let prepared = prepare_semantic_ssa_adapter_with_observer_v1(
        &function,
        None,
        &[],
        &transparent,
        &mut Trace::default(),
    )
    .unwrap();
    let mut counts = CountOutput::default();
    let mut trace = Trace {
        reject_hook: Some(RejectedHook::BlockComplete(0)),
        ..Trace::default()
    };
    assert_eq!(
        prepared.emit_blocks(&mut counts, &mut trace),
        Err(EmissionError::Observer(HOOK_DENIED))
    );
    assert_eq!(counts.counts, Counts::default());
    assert_eq!(counts.pending_successors, 2);
    assert_eq!(counts.pending_definitions, 1);
    assert_eq!(trace.blocks, [(0, 10, 2)]);
    assert!(trace.entries.is_empty());

    let entries = prepared.into_entries(&mut Trace::default()).unwrap();
    let mut counts = CountOutput::default();
    let mut trace = Trace {
        reject_hook: Some(RejectedHook::InputComplete),
        ..Trace::default()
    };
    assert_eq!(
        entries.emit_entries(&mut counts, &mut trace),
        Err(EmissionError::Observer(HOOK_DENIED))
    );
    assert_eq!(counts.counts.entries, 1);
    assert_eq!(trace.input, [(3, 1)]);
    let mut trace = Trace {
        reject_hook: Some(RejectedHook::InputComplete),
        ..Trace::default()
    };
    assert_eq!(entries.finish(&mut trace).err(), Some(HOOK_DENIED));
    assert_eq!(trace.input, [(3, 1)]);
}
