use super::*;
use fe2o3_mir_model::{SsaBlockIdV1, SsaResolvedEventV1};

#[path = "ordered_context_measured_tests.rs"]
mod measured;
#[path = "context_call_transfer_tests.rs"]
mod call_transfers;
#[path = "grid_direct_use_tests.rs"]
mod grid_direct_uses;

// Synthetic component only. bb7/stmt64 matches the measured failure coordinate,
// not its source identity. These facts do not authenticate a production entry.
struct Fixture {
    types: Vec<SemanticTypeDeclV1>,
    callables: Vec<SemanticCallableDeclV1>,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
}

fn shared_type(kind: SemanticPointerKindV1, pointee: u32) -> SemanticTypeDeclV1 {
    let layout = context_types(
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Mutable,
    )[2]
    .layout()
    .clone();
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([24; 32]),
        SemanticLayoutIdentityV1::from_sha256([24; 32]),
        layout,
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                ty(pointee),
                kind,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
}

fn policy_callable(tag: u8) -> SemanticCallableDeclV1 {
    let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([172; 32]),
        SemanticLayoutIdentityV1::from_sha256([172; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![ty(4)],
        ty(5),
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            ty(4),
            SemanticAbiPassModeV1::Direct(attributes(true, false)),
        ))],
        SemanticAbiValueV1::new(ty(5), SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
    .unwrap();
    let contract = SemanticExecutionCapabilityContractV1::new_kernel_scoped(
        E::NumericalPolicyIssue {
            context: ty(4),
            capability: ty(5),
            policy: SemanticTypeIdentityV1::from_sha256([173; 32]),
        },
        SemanticExecutionCapabilitySignatureV1::new(&[ty(4)], ty(5)).unwrap(),
        provenance(),
        SemanticFunctionIdentityV1::from_sha256([172; 32]),
    )
    .unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([172; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([172; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([172; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([172; 32]),
            source(),
            abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([172; 32]),
    }
}

fn deref(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(1)).unwrap()],
        ty(1),
    )
    .unwrap()
}

fn jump(to: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
        SemanticEdgeRoleV1::Goto,
        SemanticBlockIdV1::from_index(to),
    ))
}

fn fork(real: u32, imaginary: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::FalseEdge {
        real_target: SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::FalseEdgeReal,
            SemanticBlockIdV1::from_index(real),
        ),
        imaginary_target: SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::FalseEdgeImaginary,
            SemanticBlockIdV1::from_index(imaginary),
        ),
    }
}

impl Fixture {
    fn new() -> Self {
        let mut types = context_types(
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
        );
        types.push(shared_type(SemanticPointerKindV1::Reference, 1));
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([25; 32]),
            SemanticLayoutIdentityV1::from_sha256([25; 32]),
            types[0].layout().clone(),
            types[0].shape().clone(),
        ));
        let locals = [0, 1, 1, 2, 2, 4, 4, 4, 5, 4, 5, 2, 3, 2, 3, 4, 2, 1]
            .into_iter()
            .enumerate()
            .map(|(i, t)| {
                local(
                    i as u8,
                    t,
                    match i {
                        0 => SemanticLocalRoleV1::Return,
                        1 => SemanticLocalRoleV1::Argument(0),
                        _ => SemanticLocalRoleV1::Temporary,
                    },
                )
            })
            .collect();
        let mut blocks = (0..7)
            .map(|b| block(b, vec![], jump(u32::from(b) + 1)))
            .collect::<Vec<_>>();
        let mut statements = vec![statement(SemanticStatementKindV1::Nop); 64];
        statements.extend([
            assign(
                2,
                1,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, 1))),
            ),
            borrow(3, 2, place(2, 1), SemanticBorrowKindV1::Mutable),
            assign(
                4,
                2,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(3, 2))),
            ),
            borrow(5, 4, deref(4), SemanticBorrowKindV1::Shared),
            alias(6, 5, 4),
            borrow(7, 4, deref(6), SemanticBorrowKindV1::Shared),
        ]);
        blocks.extend([
            block(
                7,
                statements,
                call(
                    0,
                    vec![SemanticOperandV1::Copy(place(7, 4))],
                    place(8, 5),
                    8,
                ),
            ),
            block(
                8,
                vec![borrow(9, 4, deref(4), SemanticBorrowKindV1::Shared)],
                call(
                    0,
                    vec![SemanticOperandV1::Copy(place(9, 4))],
                    place(10, 5),
                    9,
                ),
            ),
            block(
                9,
                vec![borrow(11, 2, deref(4), SemanticBorrowKindV1::Mutable)],
                call(
                    1,
                    vec![SemanticOperandV1::Move(place(11, 2))],
                    place(12, 3),
                    10,
                ),
            ),
            block(
                10,
                vec![statement(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(2),
                ))],
                SemanticTerminatorKindV1::Return,
            ),
        ]);
        Self {
            types,
            callables: vec![
                policy_callable(172),
                context_callable(170, SemanticSourceArgumentOwnershipV1::UniqueBorrow, 2),
            ],
            locals,
            blocks,
        }
    }

    fn body(&self) -> SemanticFunctionDeclV1 {
        function(
            175,
            context_abi(1, 0, SemanticSourceArgumentOwnershipV1::ByValue),
            self.locals.clone(),
            self.blocks.clone(),
        )
    }

    fn sites(&self) -> BTreeSet<SemanticTransparentBorrowSiteV1> {
        typed_direct_sites(&self.body(), &self.types, &self.callables)
    }

    fn insert(&mut self, b: usize, at: usize, item: SemanticStatementV1) {
        let mut statements = self.blocks[b].statements().to_vec();
        statements.insert(at, item);
        self.blocks[b] = block(
            b as u8,
            statements,
            self.blocks[b].terminator().kind().clone(),
        );
    }
}

fn site(block: u32, statement: u32) -> SemanticTransparentBorrowSiteV1 {
    SemanticTransparentBorrowSiteV1 { block, statement }
}

#[test]
fn ordered_context_bb7_stmt64_keeps_parameter_definition_and_original_events() {
    let f = Fixture::new();
    let body = f.body();
    let original = body.clone();
    let sites = f.sites();
    assert_eq!(
        sites,
        BTreeSet::from([
            site(7, 65),
            site(7, 67),
            site(7, 69),
            site(8, 0),
            site(9, 0)
        ])
    );
    let mut parameter_events = None;
    let (input, _, _) = semantic_function_ssa_input_with_event_origins_v1(
        &body,
        Some(&f.types),
        &f.callables,
        &sites,
        |b, s, range| {
            if b == 7 && s == Some(64) {
                assert!(parameter_events.replace(range).is_none());
            }
        },
    );
    let (unclassified, _, _) =
        semantic_function_ssa_input_v1(&body, Some(&f.types), &f.callables, &BTreeSet::new());
    assert!(input.promotable()[2]);
    assert!(!unclassified.promotable()[2]);
    assert_eq!(
        input.blocks(),
        unclassified.blocks(),
        "no fabricated event or erased Move/Kill"
    );
    let range = parameter_events.unwrap();
    assert!(
        input.blocks()[7].events()[range.clone()]
            .contains(&SsaEventV1::Define(SsaVariableIdV1::new(2)))
    );
    let plan = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
    let definitions = plan.resolved_events(SsaBlockIdV1::new(7)).unwrap().iter()
        .filter(|(event, item)| range.contains(&(*event as usize))
            && matches!(item, SsaResolvedEventV1::Define { variable, .. } if variable.get() == 2))
        .count();
    assert_eq!(definitions, 1);
    let rejected = plan_ssa_with_limits_v1(&unclassified, SsaPlannerLimitsV1::default()).unwrap();
    assert!(!rejected.resolved_events(SsaBlockIdV1::new(7)).unwrap().iter()
        .any(|(_, item)| matches!(item, SsaResolvedEventV1::Define { variable, .. } if variable.get() == 2)));
    assert_eq!(body, original);
}

#[test]
fn ordered_context_retains_existing_erased_parameter_marker_without_issuing_authority() {
    let mut f = Fixture::new();
    let marker = assign(
        2,
        1,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty(1),
            SemanticConstantValueV1::ZeroSized,
        ))),
    );
    let mut statements = f.blocks[7].statements().to_vec();
    statements[64] = marker.clone();
    f.blocks[7] = block(7, statements, f.blocks[7].terminator().kind().clone());
    let sites = f.sites();
    assert_eq!(sites, Fixture::new().sites());
    let body = f.body();
    let (input, _, _) = semantic_function_ssa_input_v1(&body, Some(&f.types), &f.callables, &sites);
    let plan = plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
    assert!(input.promotable()[2]);
    assert_eq!(body.blocks()[7].statements()[64], marker);
    assert_eq!(
        plan.resolved_events(SsaBlockIdV1::new(7))
            .unwrap()
            .iter()
            .filter(
                |(_, event)| matches!(event, SsaResolvedEventV1::Define { variable, .. }
            if variable.get() == 2)
            )
            .count(),
        1
    );
    // A classifier result is not KernelContextEntryPlan or a replayed source receipt.
    assert!(typed_direct_sites(&body, &f.types, &[]).is_empty());
}

#[test]
fn ordered_context_rejects_unknown_consumer_and_well_typed_address_escape() {
    let mut f = Fixture::new();
    f.callables.push(policy_callable(174));
    f.blocks[8] = block(
        8,
        f.blocks[8].statements().to_vec(),
        call(
            2,
            vec![SemanticOperandV1::Copy(place(9, 4))],
            place(10, 5),
            9,
        ),
    );
    assert!(f.sites().is_empty());

    let mut f = Fixture::new();
    f.types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([26; 32]),
        SemanticLayoutIdentityV1::from_sha256([26; 32]),
        f.types[4].layout().clone(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                ty(4),
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    f.locals.push(local(18, 6, SemanticLocalRoleV1::Temporary));
    f.insert(
        8,
        0,
        assign(
            18,
            6,
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: place(7, 4),
            },
        ),
    );
    assert!(f.sites().is_empty());
}

#[test]
fn ordered_context_shared_move_retains_its_kill_and_cannot_revive_dead_reference() {
    let mut f = Fixture::new();
    let mut statements = f.blocks[7].statements().to_vec();
    statements[68] = assign(
        6,
        4,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(5, 4))),
    );
    f.blocks[7] = block(7, statements, f.blocks[7].terminator().kind().clone());
    let sites = f.sites();
    assert_eq!(sites, Fixture::new().sites());
    let (input, _, _) =
        semantic_function_ssa_input_v1(&f.body(), Some(&f.types), &f.callables, &sites);
    assert!(
        input.blocks()[7]
            .events()
            .contains(&SsaEventV1::Kill(SsaVariableIdV1::new(5)))
    );
    plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();

    f.insert(
        8,
        1,
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(9),
        )),
    );
    let sites = f.sites();
    assert_eq!(
        sites,
        Fixture::new().sites(),
        "transparency is not reference liveness"
    );
    let (input, _, _) =
        semantic_function_ssa_input_v1(&f.body(), Some(&f.types), &f.callables, &sites);
    assert!(
        matches!(plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()),
        Err(SsaPlannerErrorV1::UndefinedAtUse { block, variable, .. })
            if block.get() == 8 && variable.get() == 9)
    );
}

#[test]
fn ordered_context_shared_descendants_on_both_cfg_edges_end_before_resumption() {
    let mut f = Fixture::new();
    f.blocks[7] = block(7, f.blocks[7].statements().to_vec(), fork(8, 9));
    f.blocks[8] = block(
        8,
        vec![],
        call(
            0,
            vec![SemanticOperandV1::Copy(place(7, 4))],
            place(8, 5),
            10,
        ),
    );
    f.blocks[9] = block(
        9,
        vec![borrow(9, 4, deref(4), SemanticBorrowKindV1::Shared)],
        call(
            0,
            vec![SemanticOperandV1::Copy(place(9, 4))],
            place(10, 5),
            10,
        ),
    );
    f.blocks[10] = block(
        10,
        vec![borrow(11, 2, deref(4), SemanticBorrowKindV1::Mutable)],
        call(
            1,
            vec![SemanticOperandV1::Move(place(11, 2))],
            place(12, 3),
            11,
        ),
    );
    f.blocks
        .push(block(11, vec![], SemanticTerminatorKindV1::Return));
    let sites = f.sites();
    assert_eq!(
        sites,
        BTreeSet::from([
            site(7, 65),
            site(7, 67),
            site(7, 69),
            site(9, 0),
            site(10, 0)
        ])
    );
    let (input, _, _) =
        semantic_function_ssa_input_v1(&f.body(), Some(&f.types), &f.callables, &sites);
    assert!(input.promotable()[2]);
    plan_ssa_with_limits_v1(&input, SsaPlannerLimitsV1::default()).unwrap();
}

#[test]
fn ordered_context_later_unrelated_loop_does_not_revive_finished_loans() {
    let mut f = Fixture::new();
    f.blocks[10] = block(10, f.blocks[10].statements().to_vec(), jump(11));
    f.blocks.push(block(11, vec![], jump(11)));
    assert_eq!(f.sites(), Fixture::new().sites());
}

#[test]
fn ordered_context_rejects_late_shared_use_including_descendant_alias() {
    for local in [5, 6, 7, 9] {
        let mut f = Fixture::new();
        f.blocks[10] = block(
            10,
            vec![],
            call(
                0,
                vec![SemanticOperandV1::Copy(place(local, 4))],
                place(10, 5),
                11,
            ),
        );
        f.blocks
            .push(block(11, vec![], SemanticTerminatorKindV1::Return));
        assert!(f.sites().is_empty(), "late shared descendant {local}");
    }
}

#[test]
fn ordered_context_rejects_backedges_including_imaginary_edges() {
    for target in [7, 8, 9] {
        let mut f = Fixture::new();
        f.blocks[10] = block(10, vec![], fork(11, target));
        f.blocks
            .push(block(11, vec![], SemanticTerminatorKindV1::Return));
        assert!(f.sites().is_empty(), "backedge to {target}");
    }
}

#[test]
fn ordered_context_rejects_overlapping_mutable_descendants_and_copy_forks() {
    let mut f = Fixture::new();
    f.insert(8, 0, borrow(13, 2, deref(4), SemanticBorrowKindV1::Mutable));
    f.blocks[10] = block(
        10,
        vec![],
        call(
            1,
            vec![SemanticOperandV1::Move(place(13, 2))],
            place(14, 3),
            11,
        ),
    );
    f.blocks
        .push(block(11, vec![], SemanticTerminatorKindV1::Return));
    assert!(f.sites().is_empty());
    let mut f = Fixture::new();
    let mut statements = f.blocks[7].statements().to_vec();
    statements[66] = alias(4, 3, 2);
    f.blocks[7] = block(7, statements, f.blocks[7].terminator().kind().clone());
    assert!(
        f.sites().is_empty(),
        "mutable Copy does not gain an ordered exception"
    );
}

#[test]
fn ordered_context_rejects_owner_death_redefinition_and_move_during_descendant_use() {
    for change in [
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(2),
        )),
        statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(2),
        )),
        assign(
            2,
            1,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                ty(1),
                SemanticConstantValueV1::ZeroSized,
            ))),
        ),
        assign(
            17,
            1,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(2, 1))),
        ),
    ] {
        let mut f = Fixture::new();
        f.insert(8, 0, change);
        assert!(f.sites().is_empty());
    }
}

#[test]
fn ordered_context_rejects_foreign_bindings_raw_reference_and_nominal_pointee_change() {
    let mut f = Fixture::new();
    f.callables[0] = policy_callable(174);
    assert!(f.sites().is_empty());
    let mut f = Fixture::new();
    f.callables[1] = context_callable(174, SemanticSourceArgumentOwnershipV1::UniqueBorrow, 2);
    assert!(f.sites().is_empty());
    for (kind, pointee) in [
        (SemanticPointerKindV1::Raw, 1),
        (SemanticPointerKindV1::Reference, 3),
    ] {
        let mut f = Fixture::new();
        f.types[4] = shared_type(kind, pointee);
        assert!(f.sites().is_empty());
    }
}

#[test]
fn ordered_context_does_not_promote_a_dead_path_or_duplicate_reference_definition() {
    let mut f = Fixture::new();
    f.blocks[0] = block(0, vec![], jump(10));
    assert!(f.sites().is_empty());
    let mut f = Fixture::new();
    f.insert(8, 0, alias(6, 5, 4));
    assert!(f.sites().is_empty());
}

#[test]
fn ordered_context_requires_all_roots_of_same_owner_to_end_before_exclusive_resumption() {
    let mut f = Fixture::new();
    f.insert(
        8,
        0,
        borrow(15, 4, place(2, 1), SemanticBorrowKindV1::Shared),
    );
    f.blocks[10] = block(
        10,
        vec![],
        call(
            0,
            vec![SemanticOperandV1::Copy(place(15, 4))],
            place(10, 5),
            11,
        ),
    );
    f.blocks
        .push(block(11, vec![], SemanticTerminatorKindV1::Return));
    let sites = f.sites();
    assert!(
        !sites.contains(&site(7, 65)),
        "conflicting root cannot preserve mutable owner transparency"
    );
    let (input, _, _) =
        semantic_function_ssa_input_v1(&f.body(), Some(&f.types), &f.callables, &sites);
    assert!(!input.promotable()[2]);
}

#[test]
fn ordered_context_proof_work_is_bounded_without_raising_classifier_limit() {
    let f = Fixture::new();
    let body = f.body();
    let expected = f.sites();
    assert!(!expected.is_empty());
    let run = |limit| sites(&body, &f.callables, &[], limit, Some(&f.types))
        .map_err(flow_work_profile_v1::original_error_for_test);
    let mut low = 0;
    let mut high = MAX_FLOW_WORK;
    assert_eq!(run(high).unwrap(), expected);
    while low < high {
        let middle = low + (high - low) / 2;
        match run(middle) {
            Ok(found) => {
                assert_eq!(found, expected);
                high = middle;
            }
            Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                resource: SsaPlannerResourceV1::WorkUnits,
                required,
                limit,
            }) => {
                assert_eq!((required, limit), (middle + 1, middle));
                low = middle + 1;
            }
            Err(error) => panic!("unexpected proof failure: {error:?}"),
        }
    }
    assert_eq!(run(high).unwrap(), expected);
    assert!(
        matches!(run(high - 1), Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
        resource: SsaPlannerResourceV1::WorkUnits, required, limit,
    }) if required == high && limit == high - 1)
    );
}
