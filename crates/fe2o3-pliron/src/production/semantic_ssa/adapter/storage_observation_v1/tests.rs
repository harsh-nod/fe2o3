use super::super::{
    classify_storage_observable_locals_v1, semantic_function_ssa_input_with_event_origins_v1,
};
use super::*;
use fe2o3_mir_model::SsaResolvedEventV1;
use fe2o3_mir_model::semantic_mir_v1::*;

fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}
fn local(index: u32) -> SemanticLocalIdV1 {
    SemanticLocalIdV1::from_index(index)
}
fn site(block: u32, statement: u32) -> SemanticTransparentBorrowSiteV1 {
    SemanticTransparentBorrowSiteV1 { block, statement }
}
fn place(index: u32, kind: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(local(index), vec![], ty(kind)).unwrap()
}
fn projected(index: u32, projection: SemanticProjectionKindV1, result: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        local(index),
        vec![SemanticProjectionV1::new(projection, ty(result)).unwrap()],
        ty(result),
    )
    .unwrap()
}
fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}
fn borrow_to(destination: SemanticPlaceV1, source: SemanticPlaceV1) -> SemanticStatementV1 {
    let result = destination.ty();
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        destination,
        SemanticRvalueV1::new(
            result,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: source,
            },
        ),
    )))
}
fn borrow(source: u32) -> SemanticStatementV1 {
    borrow_to(place(2, 1), place(source, 0))
}
fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    end: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        source,
        statements,
        SemanticTerminatorV1::new(source, end),
    )
    .unwrap()
}
fn fixture(blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([50; 32]),
        SemanticLayoutIdentityV1::from_sha256([51; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(ty(0), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let locals = [
        (0, SemanticLocalRoleV1::Return),
        (0, SemanticLocalRoleV1::Argument(0)),
        (1, SemanticLocalRoleV1::Temporary),
        (1, SemanticLocalRoleV1::Temporary),
        (1, SemanticLocalRoleV1::Argument(1)),
        (0, SemanticLocalRoleV1::Temporary),
        (0, SemanticLocalRoleV1::Argument(2)),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (kind, role))| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([60 + index as u8; 32]),
            ty(kind),
            role,
            source,
        )
    })
    .collect();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([0xab; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([53; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([54; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([55; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([56; 32]),
        source,
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}
fn one(statements: Vec<SemanticStatementV1>) -> SemanticFunctionDeclV1 {
    fixture(vec![block(1, statements, SemanticTerminatorKindV1::Return)])
}
fn selector(
    function: &SemanticFunctionDeclV1,
    selected: u32,
    query: SemanticTransparentBorrowSiteV1,
) -> String {
    let body = function
        .identity()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("{body}/{selected}/{}/{}", query.block, query.statement)
}
fn rendered(observation: &Observation) -> Vec<u8> {
    let mut output = Vec::new();
    observation.write_to(&mut output).unwrap();
    output
}

// Frozen pre-observer classifier (ct adapter.rs:1131), including its private
// marking helpers. The oracle never calls the instrumented classifier/helpers.
fn old_classify(
    function: &SemanticFunctionDeclV1,
    transparent_borrows: &BTreeSet<SemanticTransparentBorrowSiteV1>,
    promotable: &mut [bool],
) {
    for (block_index, block) in function.blocks().iter().enumerate() {
        for (statement_index, statement) in block.statements().iter().enumerate() {
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    if !assignment.destination().projections().is_empty() {
                        old_mark(assignment.destination(), promotable);
                    }
                    old_rvalue(
                        assignment.value().kind(),
                        transparent_borrows.contains(&SemanticTransparentBorrowSiteV1 {
                            block: block_index as u32,
                            statement: statement_index as u32,
                        }),
                        promotable,
                    );
                }
                SemanticStatementKindV1::Store(store) => old_mark(store.destination(), promotable),
                SemanticStatementKindV1::AtomicRmw(operation) => {
                    if !operation.destination().projections().is_empty() {
                        old_mark(operation.destination(), promotable);
                    }
                    old_mark(operation.address(), promotable);
                }
                SemanticStatementKindV1::AtomicCompareExchange(operation) => {
                    if !operation.destination().projections().is_empty() {
                        old_mark(operation.destination(), promotable);
                    }
                    old_mark(operation.address(), promotable);
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => old_mark(place, promotable),
                SemanticStatementKindV1::Assume(_) => {}
                SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Nop => {}
            }
        }
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => {
                if let Some(destination) = call.destination()
                    && !destination.place().projections().is_empty()
                {
                    old_mark(destination.place(), promotable);
                }
            }
            SemanticTerminatorKindV1::TailCall(_) | SemanticTerminatorKindV1::SwitchInt { .. } => {}
            SemanticTerminatorKindV1::Drop { place, .. } => old_mark(place, promotable),
            SemanticTerminatorKindV1::Assert { .. } => {}
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => {}
        }
    }
}
fn old_rvalue(value: &SemanticRvalueKindV1, transparent_borrow: bool, promotable: &mut [bool]) {
    match value {
        SemanticRvalueKindV1::Borrow { .. } if transparent_borrow => {}
        SemanticRvalueKindV1::Borrow { place, .. }
        | SemanticRvalueKindV1::AddressOf { place, .. } => old_mark(place, promotable),
        SemanticRvalueKindV1::Load(load) => old_mark(load.source(), promotable),
        SemanticRvalueKindV1::Use(_)
        | SemanticRvalueKindV1::Unary { .. }
        | SemanticRvalueKindV1::Binary { .. }
        | SemanticRvalueKindV1::CheckedBinary(_)
        | SemanticRvalueKindV1::UncheckedBinary(_)
        | SemanticRvalueKindV1::Cast { .. }
        | SemanticRvalueKindV1::Aggregate(_)
        | SemanticRvalueKindV1::Length(_)
        | SemanticRvalueKindV1::Discriminant(_) => {}
    }
}
fn old_mark(place: &SemanticPlaceV1, promotable: &mut [bool]) {
    let rooted_behind_pointer = matches!(
        place
            .projections()
            .first()
            .map(|projection| projection.kind()),
        Some(SemanticProjectionKindV1::Dereference)
    );
    if !rooted_behind_pointer
        && let Some(value) = promotable.get_mut(place.local().index() as usize)
    {
        *value = false;
    }
}

fn compare(
    function: &SemanticFunctionDeclV1,
    transparent: &[SemanticTransparentBorrowSiteV1],
    selected: u32,
    query: SemanticTransparentBorrowSiteV1,
) -> (
    Observation,
    SsaConstructionInputV1,
    Result<SsaConstructionPlanV1, SsaPlannerErrorV1>,
) {
    let original = function.clone();
    let transparent = transparent.iter().copied().collect();
    let mut old = vec![true; function.locals().len()];
    let mut off = old.clone();
    let mut on = old.clone();
    let mut observation =
        Observation::selected(function, &selector(function, selected, query)).unwrap();
    old_classify(function, &transparent, &mut old);
    classify_storage_observable_locals_v1(function, &transparent, &mut off, None);
    classify_storage_observable_locals_v1(function, &transparent, &mut on, Some(&mut observation));
    assert_eq!(old, off);
    assert_eq!(old, on);
    let mut ranges = Vec::new();
    let (input, implicit, _) = semantic_function_ssa_input_with_event_origins_v1(
        function,
        None,
        &[],
        &transparent,
        |block, statement, range| ranges.push((block, statement, range)),
    );
    assert!(implicit.is_empty());
    assert_eq!(input.variable_count() as usize, old.len());
    assert_eq!(
        ranges.len(),
        function
            .blocks()
            .iter()
            .map(|block| block.statements().len() + 1)
            .sum::<usize>()
    );
    let expected = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default());
    for promotable in [old, off, on] {
        // Copy the COMPLETE actual adapter input; only the observed vector is
        // substituted. No hand-authored SSA events, edges or entry definitions.
        let reconstructed = SsaConstructionInputV1::new(
            input.entry(),
            input.variable_count(),
            promotable,
            input.entry_definitions().to_vec(),
            input.blocks().to_vec(),
        );
        assert_eq!(reconstructed, input);
        assert_eq!(
            plan_ssa_with_limits_v1(&reconstructed, SsaPlannerLimitsV1::default()),
            expected
        );
    }
    assert_eq!(*function, original);
    (observation, input, expected)
}

#[test]
fn storage_observation95_nontransparent_borrow_keeps_original_read_unpromoted() {
    let function = one(vec![borrow(1)]);
    let (observation, input, outcome) = compare(&function, &[], 1, site(0, 0));
    assert_eq!(
        input.promotable(),
        &[true, false, true, true, true, true, true]
    );
    assert!(
        matches!(input.blocks()[0].events().first(), Some(SsaEventV1::Use(variable)) if variable.get() == 1)
    );
    let plan = outcome.unwrap();
    assert!(!plan.promoted_variables().contains(&SsaVariableIdV1::new(1)));
    assert!(
        plan.resolved_events(SsaBlockIdV1::new(0))
            .unwrap()
            .iter()
            .all(|(_, event)| {
                !matches!(event, SsaResolvedEventV1::Use { variable, .. } if variable.get() == 1)
            })
    );
    assert_eq!(observation.queried_borrow, Some((1, Some(false))));
    let first = observation.first.as_ref().unwrap();
    assert_eq!(
        (
            first.block,
            first.statement,
            first.kind,
            first.transparent_borrow
        ),
        (0, Some(0), "assign-borrow", Some(false))
    );
    assert_eq!(
        String::from_utf8(rendered(&observation)).unwrap(),
        format!(
            "capability-ssa-storage-observation body={} local=1 query=0:0 queried-local=1 queried-transparent=Some(false) first-block=0 first-statement=Some(0) first-kind=assign-borrow first-transparent=Some(false) diagnostic-only=true\n",
            "ab".repeat(32),
        ),
    );
}

#[test]
fn storage_observation95_transparent_query_and_first_other_demoter() {
    for demoter_first in [false, true] {
        let mut statements = vec![
            borrow(1),
            statement(SemanticStatementKindV1::Deinitialize(place(1, 0))),
        ];
        if demoter_first {
            statements.swap(0, 1);
        }
        statements.push(statement(SemanticStatementKindV1::Deinitialize(place(
            1, 0,
        ))));
        let query = site(0, u32::from(demoter_first));
        let function = one(statements);
        let (observation, input, outcome) = compare(&function, &[query], 1, query);
        assert!(outcome.is_ok());
        assert!(!input.promotable()[1]);
        assert_eq!(observation.queried_borrow, Some((1, Some(true))));
        let first = observation.first.as_ref().unwrap();
        assert_eq!(
            (
                first.block,
                first.statement,
                first.kind,
                first.transparent_borrow
            ),
            (0, Some(u32::from(!demoter_first)), "deinitialize", None)
        );
    }
}

#[test]
fn storage_observation95_whole_assignment_does_not_attribute_demotion_to_borrow() {
    let function = one(vec![borrow_to(
        projected(1, SemanticProjectionKindV1::Field(0), 1),
        place(6, 0),
    )]);
    let (observation, input, outcome) = compare(&function, &[site(0, 0)], 1, site(0, 0));
    assert!(outcome.is_ok());
    assert!(!input.promotable()[1]);
    assert!(input.promotable()[6]);
    assert_eq!(observation.queried_borrow, Some((6, Some(true))));
    let first = observation.first.as_ref().unwrap();
    // The transparent Borrow did not demote local 1; its projected destination
    // did. This label describes the whole MIR statement, not a subplace cause.
    assert_eq!(
        (first.kind, first.transparent_borrow),
        ("assign-borrow", Some(true))
    );
}

#[test]
fn storage_observation95_dereference_and_missing_query_do_not_invent_demotion() {
    let function = one(vec![
        borrow_to(
            place(2, 1),
            projected(4, SemanticProjectionKindV1::Dereference, 0),
        ),
        statement(SemanticStatementKindV1::Nop),
    ]);
    for query in [site(0, 0), site(0, 1), site(0, 2), site(u32::MAX, u32::MAX)] {
        let (observation, input, outcome) = compare(&function, &[], 4, query);
        assert!(outcome.is_ok());
        assert!(input.promotable().iter().all(|value| *value));
        assert!(observation.first.is_none());
        assert_eq!(
            observation.queried_borrow,
            (query == site(0, 0)).then_some((4, Some(false)))
        );
        let text = String::from_utf8(rendered(&observation)).unwrap();
        assert!(text.contains("first-demotion=unavailable"));
        if query != site(0, 0) {
            assert!(text.contains("queried-borrow=unavailable"));
        }
    }
}

#[test]
fn storage_observation95_wrong_selected_local_reports_actual_queried_root() {
    let function = one(vec![borrow(1)]);
    let (observation, input, outcome) = compare(&function, &[], 6, site(0, 0));
    assert!(outcome.is_ok());
    assert!(!input.promotable()[1] && input.promotable()[6]);
    assert_eq!(observation.queried_borrow, Some((1, Some(false))));
    assert!(observation.first.is_none());
    let text = String::from_utf8(rendered(&observation)).unwrap();
    assert!(text.contains(" local=6 query=0:0 queried-local=1 queried-transparent=Some(false)"));
}

#[test]
fn storage_observation95_drop_terminator_is_first_before_later_statement() {
    let function = fixture(vec![
        block(
            1,
            vec![statement(SemanticStatementKindV1::Nop)],
            SemanticTerminatorKindV1::Drop {
                place: place(1, 0),
                drop_glue: SemanticFunctionIdV1::from_index(0),
                target: SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::DropReturn,
                    SemanticBlockIdV1::from_index(1),
                ),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        block(2, vec![borrow(1)], SemanticTerminatorKindV1::Return),
    ]);
    let (observation, _, outcome) = compare(&function, &[], 1, site(1, 0));
    assert!(outcome.is_ok());
    assert_eq!(observation.queried_borrow, Some((1, Some(false))));
    let first = observation.first.as_ref().unwrap();
    assert_eq!(
        (
            first.block,
            first.statement,
            first.kind,
            first.transparent_borrow
        ),
        (0, None, "drop", None)
    );
}

#[test]
fn storage_observation95_undefined_and_killed_original_reads_keep_exact_planner_error() {
    for killed in [false, true] {
        let owner = if killed { 1 } else { 5 };
        let mut statements = Vec::new();
        if killed {
            statements.push(statement(SemanticStatementKindV1::StorageDead(local(
                owner,
            ))));
        }
        statements.push(borrow(owner));
        let query = site(0, u32::from(killed));
        let function = one(statements);
        let (observation, input, outcome) = compare(&function, &[query], owner, query);
        assert!(input.promotable()[owner as usize]);
        assert!(
            matches!(outcome, Err(SsaPlannerErrorV1::UndefinedAtUse { variable, .. }) if variable.get() == owner)
        );
        assert_eq!(observation.queried_borrow, Some((owner, Some(true))));
        assert!(observation.first.is_none());
    }
}

#[test]
fn storage_observation95_initial_false_and_unavailable_bit_are_not_transitions() {
    let function = one(vec![borrow(1)]);
    let mut observation =
        Observation::selected(&function, &selector(&function, 1, site(0, 0))).unwrap();
    let mut old = vec![true; function.locals().len()];
    old[1] = false;
    let mut off = old.clone();
    let mut on = old.clone();
    old_classify(&function, &BTreeSet::new(), &mut old);
    classify_storage_observable_locals_v1(&function, &BTreeSet::new(), &mut off, None);
    classify_storage_observable_locals_v1(
        &function,
        &BTreeSet::new(),
        &mut on,
        Some(&mut observation),
    );
    assert_eq!(old, off);
    assert_eq!(old, on);
    assert!(observation.first.is_none());
    assert_eq!(observation.value(&[]), None);
    observation.statement(
        site(0, 0),
        function.blocks()[0].statements()[0].kind(),
        None,
        None,
        &[],
    );
    assert!(observation.first.is_none());
    assert_eq!(observation.queried_borrow, Some((1, None)));
    assert!(
        String::from_utf8(rendered(&observation))
            .unwrap()
            .contains("queried-transparent=None")
    );
}

#[test]
fn storage_observation95_selector_and_coordinate_grammar_fail_closed() {
    let function = one(vec![borrow(1)]);
    let valid = selector(&function, 1, site(0, 0));
    assert!(Observation::selected(&function, &valid).is_some());
    let body = &valid[..64];
    for raw in [
        String::new(),
        valid.to_uppercase(),
        format!("{valid}/0"),
        format!("{body}/1/0"),
        format!("{body}/01/0/0"),
        format!("{body}/1/00/0"),
        format!("{body}/1/0/00"),
        format!("{body}/7/0/0"),
        format!("{body}/1/-1/0"),
        format!("{body}/1/+1/0"),
        format!("{body}/1/0/4294967296"),
        format!("{body}/1/0/0\n"),
        format!("{body}/1//0"),
        format!("{}/1/0/0", "00".repeat(32)),
        format!("g{}/1/0/0", &body[1..]),
        format!("{}/1/0/0", &body[1..]),
        "a".repeat(98),
    ] {
        assert!(
            Observation::selected(&function, &raw).is_none(),
            "accepted {raw:?}"
        );
    }
    assert_eq!(coordinate("0"), Some(0));
    assert_eq!(coordinate("4294967295"), Some(u32::MAX));
    for raw in [
        "",
        "00",
        "01",
        "-1",
        "+1",
        " 1",
        "1 ",
        "1.0",
        "4294967296",
        "\n",
    ] {
        assert_eq!(coordinate(raw), None, "accepted {raw:?}");
    }
    for byte in 0..=u8::MAX {
        let expected = b"0123456789abcdef"
            .iter()
            .position(|digit| *digit == byte)
            .map(|index| index as u8);
        assert_eq!(hex(byte), expected);
    }
}

struct FailAfter {
    bytes: Vec<u8>,
    remaining: usize,
}
impl Write for FailAfter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let count = self.remaining.min(bytes.len());
        self.bytes.extend_from_slice(&bytes[..count]);
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn storage_observation95_format_bound_and_every_short_write_preserve_state() {
    let function = one(vec![borrow(1)]);
    let (mut observation, input, outcome) = compare(&function, &[], 1, site(0, 0));
    for queried in [
        None,
        Some((u32::MAX, None)),
        Some((u32::MAX, Some(false))),
        Some((u32::MAX, Some(true))),
    ] {
        for first in [
            None,
            Some(Demotion {
                block: u32::MAX,
                statement: Some(u32::MAX),
                kind: "atomic-compare-exchange",
                transparent_borrow: Some(false),
            }),
        ] {
            observation.local = u32::MAX;
            observation.query = site(u32::MAX, u32::MAX);
            observation.queried_borrow = queried;
            observation.first = first;
            let expected = rendered(&observation);
            assert!(expected.len() <= 512, "{} bytes", expected.len());
            assert_eq!(expected.iter().filter(|byte| **byte == b'\n').count(), 1);
            assert!(expected.ends_with(b" diagnostic-only=true\n"));
            for remaining in 0..expected.len() {
                let mut writer = FailAfter {
                    bytes: Vec::new(),
                    remaining,
                };
                assert_eq!(
                    observation.write_to(&mut writer).unwrap_err().kind(),
                    io::ErrorKind::BrokenPipe
                );
                assert_eq!(writer.bytes, expected[..remaining]);
                assert_eq!(rendered(&observation), expected);
            }
            let mut writer = FailAfter {
                bytes: Vec::new(),
                remaining: expected.len(),
            };
            observation.write_to(&mut writer).unwrap();
            assert_eq!(writer.bytes, expected);
        }
    }
    assert_eq!(
        plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()),
        outcome
    );
}
