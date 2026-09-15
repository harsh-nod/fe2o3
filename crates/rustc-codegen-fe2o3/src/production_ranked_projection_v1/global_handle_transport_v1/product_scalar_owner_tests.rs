use super::*;
use fe2o3_pliron::{ProductionSemanticMirOwnerV1, ProductionSemanticSsaOwnerV1};

const ENV: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const ENV_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);

fn integer(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        SCALAR,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 8).unwrap()),
    ))
}

fn owner() -> ProductionSemanticSsaOwnerV1 {
    owner_with_death(false)
}

fn owner_with_death(death: bool) -> ProductionSemanticSsaOwnerV1 {
    let pattern = super::super::super::global_enum_transport_v1::tests::build_owner();
    let mut types = pattern.source_semantic().types()[..4].to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([5; 32]),
        SemanticLayoutIdentityV1::from_sha256([5; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(24),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8, 16], vec![]).unwrap(),
        )
        .unwrap(),
        aggregate(vec![REF, REF, SCALAR]),
    ));
    let shared = SemanticAbiPointeeKindV1::SharedReference { frozen: true };
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([6; 32]),
            SemanticLayoutIdentityV1::from_sha256([6; 32]),
            types[REF.index() as usize].layout().clone(),
            pointer(ENV, false),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(SemanticAbiPointeeInfoV1::new(shared, 24, 8).unwrap()),
                None,
            ),
        ),
    );
    let base = pattern.source_semantic().functions()[0].clone();
    let source = SemanticSourceProvenanceV1::unavailable();
    let functions = (0..2u8)
        .map(|root| {
            let id = 100 + root;
            let locals = [
                UNIT, GLOBAL, GLOBAL, REF, REF, ENV, ENV_REF, SCALAR, REF, REF,
            ]
            .into_iter()
            .enumerate()
            .map(|(i, ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([i as u8 + 1; 32]),
                    ty,
                    if i == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source,
                )
            })
            .collect();
            let statement =
                |value| SemanticStatementV1::new(source, SemanticStatementKindV1::Assign(value));
            let initialized = vec![
                statement(construct(1, GLOBAL, None, vec![integer(11)])),
                statement(construct(2, GLOBAL, None, vec![integer(22)])),
                statement(assign(
                    3,
                    REF,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(1, GLOBAL),
                    },
                )),
                statement(assign(
                    4,
                    REF,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(2, GLOBAL),
                    },
                )),
                statement(construct(
                    5,
                    ENV,
                    None,
                    vec![
                        SemanticOperandV1::Move(place(3, REF)),
                        SemanticOperandV1::Move(place(4, REF)),
                        integer(7),
                    ],
                )),
                statement(assign(
                    6,
                    ENV_REF,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(5, ENV),
                    },
                )),
            ];
            let field = |index, ty| {
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(use_path(
                    6,
                    &[
                        (SemanticProjectionKindV1::Dereference, ENV),
                        (SemanticProjectionKindV1::Field(index), ty),
                    ],
                )))
            };
            let mut observed = vec![
                statement(assign(7, SCALAR, field(2, SCALAR))),
                statement(assign(8, REF, field(0, REF))),
                statement(assign(9, REF, field(1, REF))),
            ];
            if death && root == 0 {
                observed.insert(
                    0,
                    SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(5)),
                    ),
                );
            }
            let blocks = vec![
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256([1; 32]),
                    source,
                    initialized,
                    SemanticTerminatorV1::new(
                        source,
                        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::Goto,
                            SemanticBlockIdV1::from_index(1),
                        )),
                    ),
                )
                .unwrap(),
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256([2; 32]),
                    source,
                    observed,
                    SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
                )
                .unwrap(),
            ];
            SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256([id; 32]),
                SemanticFunctionRoleV1::KernelRoot,
                SemanticItemDefinitionIdentityV1::from_sha256([id; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([id; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([id; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([id; 32]),
                source,
                base.abi().clone(),
                locals,
                SemanticBlockIdV1::from_index(0),
                blocks,
            )
            .unwrap()
            .with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(format!("product_scalar_owner_{root}").into_bytes())
                    .unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256([id; 32]),
                SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
            ))
        })
        .collect();
    let roots = vec![
        SemanticFunctionIdV1::from_index(0),
        SemanticFunctionIdV1::from_index(1),
    ];
    let request = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([90; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        roots
            .iter()
            .copied()
            .map(SemanticCallableDeclV1::defined)
            .collect(),
        roots,
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(
            request,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn product_retained_storage_death_withholds_private_scalar_certificate() {
    let owner = owner_with_death(true);
    owner.verify_replay().unwrap();
    let root = SemanticFunctionIdV1::from_index(0);
    let types = owner.source_semantic().types();
    let function = owner.execution_view_for_root(root).unwrap().body();
    let reads = private_scalar_capture_v1::PrivateScalarReads::for_root(&owner, root).unwrap();
    let scalar = &function.blocks()[1].statements()[1];
    assert!(!reads.contains(function, scalar));
    let mut state = initial_product(&owner);
    assert!(!preserves_initialized_scalar_copy(
        types,
        function,
        &state,
        scalar,
        Some(&reads)
    ));
    let dominance = SemanticEnumPayloadDominanceV1::analyze(function, types).unwrap();
    transfer_capability_statements_with_transport_v1(
        types,
        function,
        1,
        &mut state,
        &dominance,
        None,
        None,
        Some(&reads),
    )
    .unwrap();
    assert!(!state.contains_key(&5));
    assert!(!state.contains_key(&8) && !state.contains_key(&9));
}

fn at(function: &SemanticFunctionDeclV1, block: usize, statement: usize) -> &SemanticAssignmentV1 {
    let SemanticStatementKindV1::Assign(value) =
        function.blocks()[block].statements()[statement].kind()
    else {
        panic!()
    };
    value
}

fn initial_product(owner: &ProductionSemanticSsaOwnerV1) -> ProjectedCapabilityStateV1 {
    let types = owner.source_semantic().types();
    let function = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .body();
    // Inert consumer custody inputs, separate from the real owner-issued private
    // read certificate below. This test does not claim collected Global issuance.
    let (_, _, inputs) = fixture();
    let mut state = HashMap::from([(3, inputs[&1].clone()), (4, inputs[&2].clone())]);
    let product = assignment(types, function, &state, at(function, 0, 4)).unwrap();
    assert!(matches!(
        product,
        ProjectedCapabilityValueV1::CapturedGlobalProduct(_)
    ));
    consume_capability_rvalue_operands_v1(
        types,
        function,
        at(function, 0, 4).value().kind(),
        &mut state,
    );
    state.insert(5, product);
    let borrowed = assignment(types, function, &state, at(function, 0, 5)).unwrap();
    state.insert(6, borrowed);
    state
}

#[test]
fn product_initialized_scalar_copy_requires_its_actual_owner_and_root_certificate() {
    let owner = owner();
    owner.verify_replay().unwrap();
    let types = owner.source_semantic().types();
    let root = SemanticFunctionIdV1::from_index(0);
    let function = owner.execution_view_for_root(root).unwrap().body();
    let reads = private_scalar_capture_v1::PrivateScalarReads::for_root(&owner, root).unwrap();
    let scalar = &function.blocks()[1].statements()[0];
    assert!(reads.contains(function, scalar));
    let state = initial_product(&owner);
    assert!(preserves_initialized_scalar_copy(
        types,
        function,
        &state,
        scalar,
        Some(&reads)
    ));
    assert!(!preserves_initialized_scalar_copy(
        types, function, &state, scalar, None
    ));
    let foreign = self::owner();
    assert_eq!(
        owner.source_semantic().canonical_encoding(),
        foreign.source_semantic().canonical_encoding()
    );
    let foreign_reads =
        private_scalar_capture_v1::PrivateScalarReads::for_root(&foreign, root).unwrap();
    let foreign_function = foreign.execution_view_for_root(root).unwrap().body();
    assert!(foreign_reads.contains(
        foreign_function,
        &foreign_function.blocks()[1].statements()[0]
    ));
    assert!(!preserves_initialized_scalar_copy(
        types,
        function,
        &state,
        scalar,
        Some(&foreign_reads)
    ));
    let other_root = private_scalar_capture_v1::PrivateScalarReads::for_root(
        &owner,
        SemanticFunctionIdV1::from_index(1),
    )
    .unwrap();
    let other_function = owner
        .execution_view_for_root(SemanticFunctionIdV1::from_index(1))
        .unwrap()
        .body();
    assert!(other_root.contains(other_function, &other_function.blocks()[1].statements()[0]));
    assert!(!preserves_initialized_scalar_copy(
        types,
        function,
        &state,
        scalar,
        Some(&other_root)
    ));
    let dominance = SemanticEnumPayloadDominanceV1::analyze(function, types).unwrap();
    for certificate in [Some(&reads), None, Some(&foreign_reads), Some(&other_root)] {
        let mut observed = state.clone();
        transfer_capability_statements_with_transport_v1(
            types,
            function,
            1,
            &mut observed,
            &dominance,
            None,
            None,
            certificate,
        )
        .unwrap();
        if certificate.is_some_and(|actual| std::ptr::eq(actual, &reads)) {
            let (_, _, expected) = fixture();
            assert_eq!(observed.get(&8), expected.get(&1));
            assert_eq!(observed.get(&9), expected.get(&2));
            assert_eq!(observed.get(&6), state.get(&6));
        } else {
            assert!(!observed.contains_key(&8) && !observed.contains_key(&9));
            assert_eq!(observed.get(&6), Some(&ProjectedCapabilityValueV1::Invalid));
        }
    }
}

#[test]
fn product_private_read_certificate_cannot_resurrect_dead_or_rebound_source() {
    let owner = owner();
    let root = SemanticFunctionIdV1::from_index(0);
    let types = owner.source_semantic().types();
    let function = owner.execution_view_for_root(root).unwrap().body();
    let reads = private_scalar_capture_v1::PrivateScalarReads::for_root(&owner, root).unwrap();
    let scalar = &function.blocks()[1].statements()[0];
    assert!(reads.contains(function, scalar));
    let initial = initial_product(&owner);
    for mutation in 0..4 {
        let mut state = initial.clone();
        match mutation {
            0 => {
                state.remove(&5);
            }
            1 => invalidate_capability_local_v1(&mut state, 5),
            2 => {
                state.remove(&6);
            }
            _ => {
                let (_, _, inputs) = fixture();
                state.insert(3, inputs[&15].clone());
                state.insert(4, inputs[&2].clone());
                let changed = assignment(types, function, &state, at(function, 0, 4)).unwrap();
                state.insert(5, changed);
            }
        }
        assert!(
            !preserves_initialized_scalar_copy(types, function, &state, scalar, Some(&reads)),
            "mutation={mutation}"
        );
        let dominance = SemanticEnumPayloadDominanceV1::analyze(function, types).unwrap();
        transfer_capability_statements_with_transport_v1(
            types,
            function,
            1,
            &mut state,
            &dominance,
            None,
            None,
            Some(&reads),
        )
        .unwrap();
        assert!(
            !state.contains_key(&8) && !state.contains_key(&9),
            "mutation={mutation}"
        );
    }
}
