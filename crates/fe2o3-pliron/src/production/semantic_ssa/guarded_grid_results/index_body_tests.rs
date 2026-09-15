use super::*;

fn t(id: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(id)
}
fn l(id: u32) -> SemanticLocalIdV1 {
    SemanticLocalIdV1::from_index(id)
}
fn b(id: u32) -> SemanticBlockIdV1 {
    SemanticBlockIdV1::from_index(id)
}
fn ty() -> IndexTypes {
    IndexTypes {
        leader: t(0),
        reference: t(1),
        raw: t(2),
        witness: t(3),
    }
}
fn p(id: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(l(id), vec![], ty).unwrap()
}
fn copy(id: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(p(id, ty))
}
fn zst(ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::ZeroSized,
    ))
}
fn provenance() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn block(
    id: u8,
    statements: Vec<SemanticStatementV1>,
    term: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([id; 32]),
        provenance(),
        statements,
        SemanticTerminatorV1::new(provenance(), term),
    )
    .unwrap()
}
fn assign(local: u32, ty: SemanticTypeIdV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        provenance(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            p(local, ty),
            SemanticRvalueV1::new(ty, value),
        )),
    )
}
fn attributes() -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap()
}
fn signature(
    inputs: &[SemanticTypeIdV1],
    ownership: Vec<SemanticSourceArgumentOwnershipV1>,
) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([20; 32]),
        SemanticLayoutIdentityV1::from_sha256([21; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        inputs.len() as u32,
        inputs.to_vec(),
        t(3),
        inputs
            .iter()
            .map(|ty| {
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    *ty,
                    SemanticAbiPassModeV1::Direct(attributes()),
                ))
            })
            .collect(),
        SemanticAbiValueV1::new(t(3), SemanticAbiPassModeV1::Direct(attributes())),
    )
    .unwrap()
    .with_source_argument_ownership(ownership)
    .unwrap()
}
fn function(
    id: u8,
    abi: SemanticFunctionAbiV1,
    locals: &[(SemanticTypeIdV1, SemanticLocalRoleV1)],
    entry: u32,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([id; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([id; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([id; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([id; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([id; 32]),
        provenance(),
        abi,
        locals
            .iter()
            .enumerate()
            .map(|(i, (ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([i as u8 + 1; 32]),
                    *ty,
                    *role,
                    provenance(),
                )
            })
            .collect(),
        b(entry),
        blocks,
    )
    .unwrap()
}

// These test the inert structural observer only, not canonical or Rust admission.
fn declarations(mutation: u8) -> Vec<SemanticTypeDeclV1> {
    let aggregate =
        |fields| SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap());
    let pointer = SemanticTypeShapeV1::Pointer(
        SemanticPointerTypeV1::new_with_kind(
            t(0),
            if mutation == 4 {
                SemanticPointerKindV1::Raw
            } else {
                SemanticPointerKindV1::Reference
            },
            if mutation == 5 {
                SemanticMutabilityV1::Mutable
            } else {
                SemanticMutabilityV1::Immutable
            },
            0,
            64,
            SemanticPointerMetadataV1::None,
        )
        .unwrap(),
    );
    let raw = if mutation == 2 {
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
    } else {
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: mutation == 1,
            bits: if mutation == 3 { 32 } else { 64 },
        })
    };
    let shapes = [
        aggregate(vec![t(4); 3]),
        pointer,
        raw,
        aggregate(if mutation == 6 {
            vec![t(4), t(2), t(4), t(4)]
        } else {
            vec![t(2), t(4), t(4), t(4)]
        }),
        aggregate(vec![]),
    ];
    shapes
        .into_iter()
        .enumerate()
        .map(|(i, shape)| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(
                    [if mutation == 7 && i == 3 {
                        2
                    } else {
                        i as u8 + 1
                    }; 32],
                ),
                SemanticLayoutIdentityV1::from_sha256([i as u8 + 10; 32]),
                SemanticTypeLayoutV1::new(
                    Some(if i == 0 || i == 4 { 0 } else { 8 }),
                    if i == 0 || i == 4 { 1 } else { 8 },
                )
                .unwrap(),
                shape,
            )
        })
        .collect()
}
fn bodies(order: [u32; 2], mutation: u8) -> Vec<SemanticFunctionDeclV1> {
    let [entry, exit] = order;
    let mut arguments = vec![if mutation == 1 {
        copy(1, t(1))
    } else {
        copy(2, t(2))
    }];
    if mutation == 2 {
        arguments.push(copy(2, t(2)));
    }
    let call = SemanticDirectCallV1::new(
        SemanticFunctionIdV1::from_index(if mutation == 3 { 0 } else { 1 }),
        arguments,
        Some(SemanticCallDestinationV1::new(
            p(if mutation == 4 { 2 } else { 0 }, t(3)),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                b(if mutation == 5 { entry } else { exit }),
            ),
        )),
        if mutation == 6 {
            SemanticUnwindActionV1::Continue
        } else {
            SemanticUnwindActionV1::Unreachable
        },
    )
    .unwrap();
    let mut forward = vec![None, None];
    forward[entry as usize] = Some(block(
        1,
        if mutation == 7 {
            vec![assign(2, t(2), SemanticRvalueKindV1::Use(copy(2, t(2))))]
        } else {
            vec![]
        },
        SemanticTerminatorKindV1::Call(call),
    ));
    forward[exit as usize] = Some(block(
        2,
        vec![],
        if mutation == 8 {
            SemanticTerminatorKindV1::Abort
        } else {
            SemanticTerminatorKindV1::Return
        },
    ));
    let body = function(
        30,
        signature(
            &[t(1), t(2)],
            vec![
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ],
        ),
        &[
            (t(3), SemanticLocalRoleV1::Return),
            (t(1), SemanticLocalRoleV1::Argument(0)),
            (t(2), SemanticLocalRoleV1::Argument(1)),
        ],
        entry,
        forward.into_iter().map(Option::unwrap).collect(),
    );
    let mut values = vec![
        if mutation == 9 {
            SemanticOperandV1::Move(p(1, t(2)))
        } else {
            copy(1, t(2))
        },
        zst(t(4)),
        zst(t(4)),
        zst(t(4)),
    ];
    if mutation == 10 {
        values.swap(0, 1);
    }
    if mutation == 11 {
        values[1] = zst(t(0));
    }
    if mutation == 12 {
        values.push(zst(t(4)));
    }
    let mut statements = vec![assign(
        0,
        t(3),
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                if mutation == 13 {
                    SemanticAggregateKindV1::Tuple
                } else {
                    SemanticAggregateKindV1::Aggregate
                },
                values,
            )
            .unwrap(),
        ),
    )];
    if mutation == 14 {
        statements.push(SemanticStatementV1::new(
            provenance(),
            SemanticStatementKindV1::Nop,
        ));
    }
    let constructor = function(
        31,
        signature(&[t(2)], vec![SemanticSourceArgumentOwnershipV1::ByValue]),
        &[
            (t(3), SemanticLocalRoleV1::Return),
            (t(2), SemanticLocalRoleV1::Argument(0)),
        ],
        0,
        vec![block(
            3,
            statements,
            if mutation == 15 {
                SemanticTerminatorKindV1::UnwindResume
            } else {
                SemanticTerminatorKindV1::Return
            },
        )],
    );
    vec![body, constructor]
}
fn observe_with(
    functions: &[SemanticFunctionDeclV1],
    types: &[SemanticTypeDeclV1],
    work: &mut usize,
) -> Result<Option<IndexBody>, ()> {
    let callables = (0..2)
        .map(|id| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(id)))
        .collect::<Vec<_>>();
    observe(
        &functions[0],
        functions,
        &callables,
        types,
        ty(),
        &mut |amount| {
            if let Some(left) = work.checked_sub(amount) {
                *work = left;
                true
            } else {
                false
            }
        },
    )
}
#[test]
fn guarded_grid_index_original_shape_and_role_permutation() {
    for order in [[0, 1], [1, 0]] {
        let functions = bodies(order, 0);
        let result = observe_with(&functions, &declarations(0), &mut 128)
            .unwrap()
            .unwrap();
        assert_eq!(
            (result.receiver, result.raw, result.returned),
            (l(1), l(2), l(0))
        );
        assert_eq!(
            (result.constructor_raw, result.constructor_return),
            (l(1), l(0))
        );
        assert_eq!(result.return_block, b(order[1]));
    }
}
#[test]
fn guarded_grid_index_wrong_operand_effect_call_and_return_reject() {
    for mutation in 1..=15 {
        assert_eq!(
            observe_with(&bodies([0, 1], mutation), &declarations(0), &mut 128),
            Ok(None),
            "mutation {mutation}"
        );
    }
}
#[test]
fn guarded_grid_index_signed_width_boolean_raw_mutable_and_identity_reject() {
    for mutation in 1..=7 {
        assert_eq!(
            observe_with(&bodies([0, 1], 0), &declarations(mutation), &mut 128),
            Ok(None),
            "type {mutation}"
        );
    }
}
#[test]
fn guarded_grid_index_exact_work_and_repeated_use_share_budget() {
    let functions = bodies([0, 1], 0);
    let types = declarations(0);
    for initial in [0, 127] {
        let mut work = initial;
        assert_eq!(observe_with(&functions, &types, &mut work), Err(()));
        assert_eq!(work, initial);
    }
    let mut work = 255;
    assert!(matches!(
        observe_with(&functions, &types, &mut work),
        Ok(Some(_))
    ));
    assert_eq!(work, 127);
    assert_eq!(observe_with(&functions, &types, &mut work), Err(()));
    assert_eq!(work, 127);
}
