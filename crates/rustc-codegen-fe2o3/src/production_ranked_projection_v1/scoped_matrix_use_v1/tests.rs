#[test]
fn scoped_matrix_identity_keeps_all_digest_bytes_and_legacy_domain() {
    use scoped_matrix_use_v1::Issuer;
    for kind in [1, 2] {
        for root in [0, 1, u64::MAX] {
            assert_eq!(
                Issuer::Legacy(root).digest(kind),
                tensor_capability_root_v1(kind, &[root])
            );
        }
    }
    let mut other = [7; 32];
    other[31] = 8;
    let first = Issuer::Scoped(DigestV1::from_untrusted_bytes([7; 32]));
    let second = Issuer::Scoped(DigestV1::from_untrusted_bytes(other));
    assert_ne!(first, second);
    assert_ne!(first.digest(2), second.digest(2));
    assert_ne!(first, Issuer::Legacy(0x0707070707070707));
}

#[test]
fn scoped_matrix_operand_legacy_binding_bytes_stay_exact() {
    use scoped_matrix_use_v1::Issuer;
    let operand = ProjectedMfmaOperandV1 {
        contract: mfma_operand_contract(SemanticMfmaOperandRoleV1::A),
        storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
        lane_root: Issuer::Legacy(20),
        allocation: tensor_test_allocation(),
    };
    let allocation = operand.allocation;
    assert_eq!(
        tensor_operand_root_v1(operand),
        tensor_capability_root_v1(
            3,
            &[
                20,
                allocation.allocation_origin,
                allocation.noalias_class,
                u64::from(allocation.writable),
                1,
            ]
        )
    );
    let scoped = ProjectedMfmaOperandV1 {
        lane_root: Issuer::Scoped(DigestV1::from_untrusted_bytes([20; 32])),
        ..operand
    };
    assert_ne!(
        tensor_operand_root_v1(operand),
        tensor_operand_root_v1(scoped)
    );
}

#[test]
fn legacy_current_cannot_authorize_scoped_fragment_custody() {
    use scoped_matrix_use_v1::Issuer;
    let mut state = authenticated_tensor_state(
        SemanticMfmaStorageLayoutV1::RowMajor,
        SemanticMfmaStorageLayoutV1::RowMajor,
    );
    for local in [1, 2, 3] {
        match state.get_mut(&local).unwrap() {
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Operand(value)) => {
                value.lane_root = Issuer::Scoped(DigestV1::from_untrusted_bytes([3; 32]))
            }
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Accumulator(value)) => {
                value.lane_root = Issuer::Scoped(DigestV1::from_untrusted_bytes([3; 32]))
            }
            _ => panic!("exact old fixture roster"),
        }
    }
    assert_eq!(
        authenticate_tensor_instruction_with_scoped_matrix_v1(
            &tensor_test_call(),
            &state,
            mfma_operand_contract(SemanticMfmaOperandRoleV1::A),
            mfma_operand_contract(SemanticMfmaOperandRoleV1::B),
            mfma_accumulator_contract(),
            None,
        )
        .unwrap_err(),
        "an MFMA call mixed legacy and scoped custody or a different subgroup epoch"
    );
}

#[test]
fn scoped_lane_join_never_merges_distinct_source_issuers() {
    use scoped_matrix_use_v1::Issuer;
    let accumulator = |tag| {
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Accumulator(
            ProjectedMfmaAccumulatorV1 {
                contract: mfma_accumulator_contract(),
                lane_root: Issuer::Scoped(DigestV1::from_untrusted_bytes([tag; 32])),
                value_root: 30,
                flow_root: 30,
            },
        ))
    };
    assert_eq!(
        merge_capability_values_v1(accumulator(4), accumulator(5)),
        ProjectedCapabilityValueV1::Invalid
    );
    assert_eq!(
        merge_capability_values_v1(accumulator(4), accumulator(4)),
        accumulator(4)
    );
}

pub(super) fn check_scoped_matrix_actual_consumers_v1(
    owner: &ProductionSemanticSsaOwnerV1,
    root: SemanticFunctionIdV1,
    relation: &scoped_matrix_use_v1::Relation<'_>,
) {
    let view = owner.execution_view_for_root(root).unwrap();
    let function = view.body();
    let types = owner.source_semantic().types();
    let callables = owner.source_semantic().callables();
    let mut zeros = 0;
    let mut multiplies = 0;
    for (block, data) in function.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Call(call) = data.terminator().kind() else {
            continue;
        };
        let Some(scoped) = relation.use_at(function, block as u32, call).unwrap() else {
            continue;
        };
        let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } =
            &callables[call.callee().index() as usize]
        else {
            panic!("checked original source consumer")
        };
        let mut state = HashMap::new();
        match operation {
            SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero { .. } => {
                transfer_capability_terminator_with_scoped_matrix_v1(
                    types,
                    callables,
                    function,
                    block,
                    &mut state,
                    &vec![None; function.locals().len()],
                    &vec![None; function.locals().len()],
                    &[],
                    &HashMap::new(),
                    true,
                    Some(relation),
                )
                .unwrap();
                let output = call.destination().unwrap().place().local().index() as usize;
                assert!(
                    matches!(state.get(&output), Some(ProjectedCapabilityValueV1::Known(
                    ProjectedCapabilityOriginV1::Accumulator(value)))
                    if value.lane_root == scoped_matrix_use_v1::Issuer::Scoped(
                        DigestV1::from_untrusted_bytes(scoped.lane_identity())))
                );
                zeros += 1;
            }
            SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate { .. } => {
                let error = transfer_capability_terminator_with_scoped_matrix_v1(
                    types,
                    callables,
                    function,
                    block,
                    &mut state,
                    &vec![None; function.locals().len()],
                    &vec![None; function.locals().len()],
                    &[],
                    &HashMap::new(),
                    true,
                    Some(relation),
                )
                .unwrap_err();
                assert!(matches!(
                    error,
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "an MFMA lhs without one dominating checked typed-load payload"
                    )
                ));
                assert!(
                    state.is_empty(),
                    "checked Matrix custody cannot create missing load/memory proof"
                );
                multiplies += 1;
            }
            _ => {}
        }
    }
    assert_eq!((zeros, multiplies), (3, 3));
}
