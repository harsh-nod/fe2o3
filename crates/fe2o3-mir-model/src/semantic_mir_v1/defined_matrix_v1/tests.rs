use super::*;
use crate::semantic_mir_v1::defined_math_v1::tests::permutations::{
    BIND_ORDERS, canonical_permute,
};

pub(in crate::semantic_mir_v1) struct Fixture {
    functions: Vec<SemanticFunctionDeclV1>,
    callables: Vec<SemanticCallableDeclV1>,
    declarations: Vec<SemanticTypeDeclV1>,
    types: SemanticPolicyGfx950NarrowTypesV1,
    identity: SemanticDefinedMatrixIdentityV1,
}

impl Fixture {
    pub(in crate::semantic_mir_v1) fn bind(
        &self,
    ) -> Result<SemanticPolicyMatrixBindV1, SemanticMirErrorV1> {
        SemanticPolicyMatrixBindV1::for_defined_function(
            SemanticFunctionIdV1(2),
            &self.functions,
            &self.callables,
            &self.declarations,
            self.types.bind,
            self.identity,
        )
    }

    pub(in crate::semantic_mir_v1) fn narrow(
        &self,
    ) -> Result<SemanticPolicyGfx950NarrowV1, SemanticMirErrorV1> {
        SemanticPolicyGfx950NarrowV1::for_defined_function(
            SemanticFunctionIdV1(0),
            &self.functions,
            &self.callables,
            &self.declarations,
            self.types,
            self.identity,
        )
    }
}

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1(local), vec![], ty).unwrap()
}

fn local(index: u8, ty: SemanticTypeIdV1, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
    SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1([index; 32]),
        ty,
        role,
        SemanticSourceProvenanceV1::unavailable(),
    )
}

fn assignment(destination: SemanticPlaceV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            destination.clone(),
            SemanticRvalueV1::new(destination.ty(), value),
        )),
    )
}

fn aggregate_operands(
    body: &mut SemanticFunctionDeclV1,
    block: usize,
) -> &mut Box<[SemanticOperandV1]> {
    let SemanticStatementKindV1::Assign(assignment) = &mut body.blocks[block].statements[0].kind
    else {
        panic!()
    };
    let SemanticRvalueKindV1::Aggregate(aggregate) = &mut assignment.value.kind else {
        panic!()
    };
    &mut aggregate.operands
}

fn entry_call(body: &mut SemanticFunctionDeclV1) -> &mut SemanticDirectCallV1 {
    let SemanticTerminatorKindV1::Call(call) =
        &mut body.blocks[body.entry.index() as usize].terminator.kind
    else {
        panic!()
    };
    call
}

pub(in crate::semantic_mir_v1) fn fixture() -> Fixture {
    let mut base = crate::semantic_mir_v1::defined_math_v1::tests::fixture();
    base.callables.truncate(3);
    let types =
        SemanticPolicyGfx950NarrowTypesV1::new([4, 2, 6, 5, 7, 9, 10].map(SemanticTypeIdV1));
    let marker = SemanticTypeIdV1(8);
    let zero = SemanticOperandV1::Constant(SemanticConstantV1::new(
        marker,
        SemanticConstantValueV1::ZeroSized,
    ));
    let operands = aggregate_operands(&mut base.functions[2], 0);
    *operands = operands.iter().cloned().chain([zero.clone()]).collect();
    let bound = &mut base.declarations[7];
    bound.shape = SemanticTypeShapeV1::Aggregate(
        SemanticAggregateTypeV1::new(vec![
            types.bind.matrix_reference,
            types.bind.policy_reference,
            marker,
            marker,
        ])
        .unwrap(),
    );
    bound.layout = SemanticTypeLayoutV1::aggregate_with_backend_repr(
        Some(16),
        8,
        *bound.layout.backend_repr(),
        false,
        SemanticAggregateLayoutV1::new(vec![0, 8, 16, 16], vec![]).unwrap(),
    )
    .unwrap();
    let mut reference = base.declarations[4].clone();
    reference.identity = SemanticTypeIdentityV1([19; 32]);
    reference.layout_identity = SemanticLayoutIdentityV1([19; 32]);
    let SemanticTypeShapeV1::Pointer(pointer) = &mut reference.shape else {
        panic!()
    };
    pointer.pointee = types.bind.bound;
    base.declarations.push(reference.clone());
    base.declarations.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1([20; 32]),
        SemanticLayoutIdentityV1([20; 32]),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(8),
            8,
            *reference.layout.backend_repr(),
            false,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![types.bound_reference, marker]).unwrap(),
        ),
    ));
    let attributes = match base.functions[2].abi.return_value.mode {
        SemanticAbiPassModeV1::Pair { first, .. } => first,
        _ => panic!(),
    };
    let mut abi = base.functions[0].abi.clone();
    abi.source_signature.inputs = vec![types.bound_reference].into();
    abi.arguments = vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
        types.bound_reference,
        SemanticAbiPassModeV1::Direct(attributes),
    ))]
    .into();
    abi.source_signature.output = types.narrowed;
    abi.return_value =
        SemanticAbiValueV1::new(types.narrowed, SemanticAbiPassModeV1::Direct(attributes));
    let narrowed = &mut base.functions[0];
    narrowed.abi = abi.clone();
    narrowed.locals = vec![
        local(1, types.narrowed, SemanticLocalRoleV1::Return),
        local(2, types.bound_reference, SemanticLocalRoleV1::Argument(0)),
        local(
            3,
            types.bind.matrix_reference,
            SemanticLocalRoleV1::Temporary,
        ),
    ]
    .into();
    let call = entry_call(narrowed);
    call.arguments = vec![SemanticOperandV1::Copy(place(1, types.bound_reference))].into();
    call.destination.as_mut().unwrap().place = place(2, types.bind.matrix_reference);
    narrowed.blocks[1].statements = vec![assignment(
        place(0, types.narrowed),
        SemanticRvalueKindV1::aggregate(
            SemanticAggregateKindV1::Aggregate,
            vec![
                SemanticOperandV1::Copy(place(1, types.bound_reference)),
                zero,
            ],
        )
        .unwrap(),
    )]
    .into();
    let getter = &mut base.functions[1];
    abi.identity = getter.abi.identity;
    abi.source_signature.output = types.bind.matrix_reference;
    abi.return_value = SemanticAbiValueV1::new(
        types.bind.matrix_reference,
        SemanticAbiPassModeV1::Direct(attributes),
    );
    getter.abi = abi;
    getter.locals = vec![
        local(1, types.bind.matrix_reference, SemanticLocalRoleV1::Return),
        local(2, types.bound_reference, SemanticLocalRoleV1::Argument(0)),
    ]
    .into();
    getter.blocks = vec![getter.blocks[0].clone()].into();
    getter.blocks[0].terminator.kind = SemanticTerminatorKindV1::Return;
    getter.blocks[0].statements = vec![assignment(
        place(0, types.bind.matrix_reference),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1(1),
                vec![
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Dereference,
                        types.bind.bound,
                    )
                    .unwrap(),
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Field(0),
                        types.bind.matrix_reference,
                    )
                    .unwrap(),
                ],
                types.bind.matrix_reference,
            )
            .unwrap(),
        )),
    )]
    .into();
    Fixture {
        functions: base.functions,
        callables: base.callables,
        declarations: base.declarations,
        types,
        identity: SemanticDefinedMatrixIdentityV1::new(
            base.provenance,
            base.policy,
            base.brand,
            SemanticTypeIdentityV1([108; 32]),
            SemanticTypeIdentityV1([109; 32]),
        )
        .unwrap(),
    }
}

pub(in crate::semantic_mir_v1) fn phase_fixture() -> Fixture {
    let mut f = fixture();
    f.identity = f
        .identity
        .with_execution_brand(SemanticTypeIdentityV1([110; 32]))
        .unwrap();
    f
}

#[test]
fn defined_matrix_phase_pair_retains_original_bodies_and_policy_root() {
    let direct = fixture();
    let phase = phase_fixture();
    let bind = phase.bind().unwrap();
    let narrow = phase.narrow().unwrap();
    assert_eq!(bind.origin, direct.bind().unwrap().origin);
    assert_eq!(narrow.origin, direct.narrow().unwrap().origin);
    assert_eq!(narrow.projection(), direct.narrow().unwrap().projection());
    assert_eq!(bind.identity(), narrow.identity());
    assert_eq!(bind.types(), direct.bind().unwrap().types());
    assert_eq!(bind.reference_arguments(), [0, 1]);
    assert_eq!(bind.reference_fields(), [0, 1]);
    assert_eq!(
        bind.identity().kernel_brand(),
        direct.identity.kernel_brand()
    );
    assert_ne!(
        bind.identity().execution_brand(),
        bind.identity().kernel_brand()
    );
    validate_bind_attachment(&phase.functions[2], bind).unwrap();
    validate_narrow_attachment(&phase.functions[0], narrow).unwrap();
    let mut claims = IntrinsicCapabilityClaimsV1::default();
    assert!(record_claim(&mut claims, bind.types(), bind.identity()));
    assert!(record_claim(
        &mut claims,
        narrow.types().bind,
        narrow.identity()
    ));
    assert_eq!(
        claims.numerical_math_brands.get(&bind.types().capability),
        Some(&phase.identity.kernel_brand())
    );
    assert_eq!(
        claims
            .execution_workgroups
            .get(&phase.identity.execution_brand()),
        Some(&phase.identity.provenance())
    );
    assert!(
        !claims
            .execution_workgroups
            .contains_key(&phase.identity.kernel_brand())
    );
}

#[test]
fn defined_matrix_phase_pair_rejects_zero_aliases_and_replacement() {
    let direct = fixture().identity;
    assert_eq!(direct.execution_brand(), direct.kernel_brand());
    assert!(!direct.has_distinct_execution_brand());
    for execution in [
        SemanticTypeIdentityV1([0; 32]),
        direct.policy(),
        direct.kernel_brand(),
        direct.matrix_brand(),
        direct.epoch(),
    ] {
        assert_eq!(
            direct.with_execution_brand(execution),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        );
    }
    let phase = phase_fixture().identity;
    assert_eq!(
        phase.with_execution_brand(phase.execution_brand()).unwrap(),
        phase
    );
    assert_eq!(
        phase.with_execution_brand(SemanticTypeIdentityV1([111; 32])),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    );
}

#[test]
fn defined_matrix_phase_pair_rejects_runtime_type_as_execution_marker() {
    let mut f = phase_fixture();
    f.declarations[f.types.bind.matrix.index() as usize].identity = f.identity.execution_brand();
    assert_eq!(f.bind(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
    assert_eq!(f.narrow(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
}

#[test]
fn defined_matrix_bind_retains_both_references_and_full_identity() {
    let f = fixture();
    let record = f.bind().unwrap();
    assert_eq!(record.reference_arguments(), [0, 1]);
    assert_eq!(record.reference_fields(), [0, 1]);
    assert_eq!(record.types(), f.types.bind);
    assert_eq!(record.identity(), f.identity);
    assert_eq!(record.identity().width(), 64);
    assert_eq!(
        record.identity().mode(),
        SemanticNumericalModeV1::StrictIeee
    );
    validate_bind_attachment(&f.functions[2], record).unwrap();
    let mut claims = IntrinsicCapabilityClaimsV1::default();
    assert!(record_claim(&mut claims, record.types(), record.identity()));
    assert!(record_claim(
        &mut claims,
        record.types(),
        f.narrow().unwrap().identity()
    ));
    let changed = SemanticDefinedMatrixIdentityV1::new(
        f.identity.provenance(),
        SemanticTypeIdentityV1([150; 32]),
        f.identity.kernel_brand(),
        f.identity.matrix_brand(),
        f.identity.epoch(),
    )
    .unwrap();
    assert!(!record_claim(&mut claims, record.types(), changed));
}

#[test]
fn defined_matrix_narrow_requires_original_getter_and_same_receiver() {
    let f = fixture();
    let record = f.narrow().unwrap();
    assert_eq!(record.projection().function(), SemanticFunctionIdV1(1));
    assert_eq!(
        record.projection().source_identity(),
        f.functions[1].identity()
    );
    assert_eq!(
        record.projection().abi_identity(),
        f.functions[1].abi().identity()
    );
    assert_eq!(record.receiver_argument(), 0);
    assert_eq!(record.reference_field(), 0);
    assert_eq!(record.identity(), f.bind().unwrap().identity());
    validate_narrow_attachment(&f.functions[0], record).unwrap();
}

#[test]
fn defined_matrix_canonical_local_and_block_permutations() {
    for order in BIND_ORDERS {
        for blocks in [[0, 1], [1, 0]] {
            let mut f = fixture();
            canonical_permute(&mut f.functions[2], &order, &[0]);
            canonical_permute(&mut f.functions[0], &order, &blocks);
            let before = f.functions.clone();
            validate_bind_attachment(&f.functions[2], f.bind().unwrap()).unwrap();
            validate_narrow_attachment(&f.functions[0], f.narrow().unwrap()).unwrap();
            assert_eq!(before, f.functions);
        }
    }
}

#[test]
fn defined_matrix_rejects_swapped_moved_or_missing_bind_references() {
    for mutation in 0..3 {
        let mut f = fixture();
        let operands = aggregate_operands(&mut f.functions[2], 0);
        match mutation {
            0 => operands.swap(0, 1),
            1 => {
                let SemanticOperandV1::Copy(place) = operands[0].clone() else {
                    panic!()
                };
                operands[0] = SemanticOperandV1::Move(place);
            }
            2 => {
                let mut changed = operands.to_vec();
                changed.remove(1);
                *operands = changed.into();
            }
            _ => unreachable!(),
        }
        assert_eq!(f.bind(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
    }
}

#[test]
fn defined_matrix_rejects_erased_or_substituted_projection_call() {
    for mutation in 0..5 {
        let mut f = fixture();
        let function = &mut f.functions[0];
        match mutation {
            0 => function.blocks[0].terminator.kind = SemanticTerminatorKindV1::Return,
            1 => entry_call(function).callee = SemanticCallableIdV1(2),
            2 => entry_call(function).arguments = Box::new([]),
            3 => {
                entry_call(function).arguments[0] =
                    SemanticOperandV1::Move(place(1, f.types.bound_reference))
            }
            4 => {
                entry_call(function)
                    .destination
                    .as_mut()
                    .unwrap()
                    .edge
                    .target = SemanticBlockIdV1(0)
            }
            _ => unreachable!(),
        }
        assert_eq!(f.narrow(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
    }
}

#[test]
fn defined_matrix_rejects_changed_projection_field_and_narrow_capture() {
    let mut f = fixture();
    let SemanticStatementKindV1::Assign(assignment) =
        &mut f.functions[1].blocks[0].statements[0].kind
    else {
        panic!()
    };
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected)) = &mut assignment.value.kind
    else {
        panic!()
    };
    projected.projections[1].kind = SemanticProjectionKindV1::Field(1);
    assert_eq!(f.narrow(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
    let mut f = fixture();
    aggregate_operands(&mut f.functions[0], 1)[0] =
        SemanticOperandV1::Move(place(1, f.types.bound_reference));
    assert_eq!(f.narrow(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
}

#[test]
fn defined_matrix_attachment_rejects_body_and_abi_identity_substitution() {
    for bind in [false, true] {
        let mut f = fixture();
        if bind {
            let record = f.bind().unwrap();
            f.functions[2].abi.identity = SemanticAbiIdentityV1([150; 32]);
            assert_eq!(
                validate_bind_attachment(&f.functions[2], record),
                Err(SemanticMirErrorV1::InvalidFunctionAbi)
            );
        } else {
            let record = f.narrow().unwrap();
            f.functions[0].identity = SemanticFunctionIdentityV1([151; 32]);
            assert_eq!(
                validate_narrow_attachment(&f.functions[0], record),
                Err(SemanticMirErrorV1::InvalidFunctionAbi)
            );
        }
    }
}

#[test]
fn defined_matrix_rejects_mutable_reference_and_collapsed_identity_axes() {
    let mut f = fixture();
    let SemanticTypeShapeV1::Pointer(pointer) = &mut f.declarations[4].shape else {
        panic!()
    };
    pointer.mutability = SemanticMutabilityV1::Mutable;
    assert_eq!(f.bind(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
    assert_eq!(f.narrow(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
    assert_eq!(
        SemanticDefinedMatrixIdentityV1::new(
            f.identity.provenance(),
            f.identity.policy(),
            f.identity.kernel_brand(),
            f.identity.matrix_brand(),
            f.identity.matrix_brand()
        ),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    );
}
