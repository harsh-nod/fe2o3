use super::*;
fn t(n: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(n)
}
fn l(n: u32) -> SemanticLocalIdV1 {
    SemanticLocalIdV1::from_index(n)
}
fn provenance() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
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
fn ty(index: u8, shape: SemanticTypeShapeV1, bytes: u64, alignment: u64) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([index + 1; 32]),
        SemanticLayoutIdentityV1::from_sha256([index + 10; 32]),
        SemanticTypeLayoutV1::new(Some(bytes), alignment).unwrap(),
        shape,
    )
}
fn aggregate(fields: Vec<SemanticTypeIdV1>) -> SemanticTypeShapeV1 {
    SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap())
}
fn pointer(
    kind: SemanticPointerKindV1,
    mutable: SemanticMutabilityV1,
    width: u16,
) -> SemanticTypeShapeV1 {
    SemanticTypeShapeV1::Pointer(
        SemanticPointerTypeV1::new_with_kind(
            t(0),
            kind,
            mutable,
            0,
            width,
            SemanticPointerMetadataV1::None,
        )
        .unwrap(),
    )
}
fn declarations() -> Vec<SemanticTypeDeclV1> {
    vec![
        ty(0, aggregate(vec![t(2), t(3), t(4)]), 16, 8),
        ty(
            1,
            pointer(
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                64,
            ),
            8,
            8,
        ),
        ty(
            2,
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            }),
            8,
            8,
        ),
        ty(3, aggregate(vec![t(5), t(5)]), 8, 4),
        ty(4, aggregate(vec![]), 0, 1),
        ty(
            5,
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
            4,
            4,
        ),
    ]
}
fn function(mutation: u8) -> SemanticFunctionDeclV1 {
    let output = if mutation == 1 { t(2) } else { t(3) };
    let ownership = if mutation == 2 {
        SemanticSourceArgumentOwnershipV1::ByValue
    } else if mutation == 3 {
        SemanticSourceArgumentOwnershipV1::UniqueBorrow
    } else {
        SemanticSourceArgumentOwnershipV1::SharedBorrow
    };
    let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([20; 32]),
        SemanticLayoutIdentityV1::from_sha256([21; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        mutation == 4,
        false,
        1,
        vec![t(1)],
        output,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            t(1),
            SemanticAbiPassModeV1::Direct(attributes()),
        ))],
        SemanticAbiValueV1::new(output, SemanticAbiPassModeV1::Direct(attributes())),
    )
    .unwrap()
    .with_source_argument_ownership(vec![ownership])
    .unwrap();
    let mut projections = vec![
        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, t(0)).unwrap(),
        SemanticProjectionV1::new(
            SemanticProjectionKindV1::Field(if mutation == 5 { 8 } else { 1 }),
            output,
        )
        .unwrap(),
    ];
    if mutation == 6 {
        projections.remove(0);
    }
    let place = SemanticPlaceV1::new(l(1), projections, output).unwrap();
    let operand = if mutation == 7 {
        SemanticOperandV1::Move(place)
    } else {
        SemanticOperandV1::Copy(place)
    };
    let mut statements = vec![SemanticStatementV1::new(
        provenance(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(l(0), vec![], output).unwrap(),
            SemanticRvalueV1::new(output, SemanticRvalueKindV1::Use(operand)),
        )),
    )];
    if mutation == 8 {
        statements.push(SemanticStatementV1::new(
            provenance(),
            SemanticStatementKindV1::Nop,
        ));
    }
    let terminator = if mutation == 9 {
        SemanticTerminatorKindV1::Unreachable
    } else {
        SemanticTerminatorKindV1::Return
    };
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([1; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([2; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([3; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([4; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([5; 32]),
        provenance(),
        abi,
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([1; 32]),
                output,
                SemanticLocalRoleV1::Return,
                provenance(),
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([2; 32]),
                t(1),
                SemanticLocalRoleV1::Argument(0),
                provenance(),
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([3; 32]),
                provenance(),
                statements,
                SemanticTerminatorV1::new(provenance(), terminator),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}
fn observe_with(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    work: &mut usize,
) -> Result<Option<Reader>, &'static str> {
    observe(function, types, &mut |n| {
        *work = work.checked_sub(n).ok_or("shared-work-exhausted")?;
        Ok(())
    })
}

#[test]
fn grid_field_reader_observes_exact_nested_unsigned_copy() {
    let reader = observe_with(&function(0), &declarations(), &mut 1000)
        .unwrap()
        .unwrap();
    assert_eq!(
        reader,
        Reader {
            reference: t(1),
            owned: t(0),
            result: t(3),
            receiver: l(1),
            field: 1
        }
    );
}
#[test]
fn grid_field_reader_rejects_body_abi_and_move_mutations() {
    for mutation in 1..=9 {
        assert_eq!(
            observe_with(&function(mutation), &declarations(), &mut 1000),
            Ok(None),
            "mutation {mutation}"
        );
    }
}
#[test]
fn grid_field_reader_rejects_raw_mutable_wrongwidth_and_layout() {
    for mutation in 0..5 {
        let mut types = declarations();
        types[1] = match mutation {
            0 => ty(
                1,
                pointer(
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Immutable,
                    64,
                ),
                8,
                8,
            ),
            1 => ty(
                1,
                pointer(
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Mutable,
                    64,
                ),
                8,
                8,
            ),
            2 => ty(
                1,
                pointer(
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    32,
                ),
                4,
                4,
            ),
            3 => ty(
                1,
                pointer(
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    64,
                ),
                4,
                4,
            ),
            _ => ty(
                1,
                pointer(
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    64,
                ),
                8,
                4,
            ),
        };
        assert_eq!(
            observe_with(&function(0), &types, &mut 1000),
            Ok(None),
            "mutation {mutation}"
        );
    }
}
#[test]
fn grid_field_reader_rejects_non_snapshot_siblings() {
    for mutation in 0..5 {
        let mut types = declarations();
        types[2] = match mutation {
            0 => ty(
                2,
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: true,
                    bits: 64,
                }),
                8,
                8,
            ),
            1 => ty(
                2,
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
                1,
                1,
            ),
            2 => ty(
                2,
                pointer(
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    64,
                ),
                8,
                8,
            ),
            3 => ty(
                2,
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 64,
                }),
                4,
                4,
            ),
            _ => ty(2, aggregate(vec![t(0)]), 16, 8),
        };
        assert_eq!(
            observe_with(&function(0), &types, &mut 1000),
            Ok(None),
            "mutation {mutation}"
        );
    }
}
#[test]
fn grid_field_reader_consumes_existing_budget_without_reset() {
    let mut work = 1000;
    assert!(
        observe_with(&function(0), &declarations(), &mut work)
            .unwrap()
            .is_some()
    );
    let spent = 1000 - work;
    assert!(spent > 64);
    let mut exact = spent;
    assert!(
        observe_with(&function(0), &declarations(), &mut exact)
            .unwrap()
            .is_some()
    );
    assert_eq!(exact, 0);
    let mut short = spent - 1;
    assert_eq!(
        observe_with(&function(0), &declarations(), &mut short),
        Err("shared-work-exhausted")
    );
    assert_eq!(
        observe_with(&function(0), &declarations(), &mut exact),
        Err("shared-work-exhausted")
    );
    assert_eq!(exact, 0);
}
#[test]
fn grid_field_reader_charges_variable_field_roster_before_rejection() {
    let mut types = declarations();
    types[3] = ty(3, aggregate(vec![t(5); 65]), 260, 4);
    let mut largest = 0;
    let result = observe(&function(0), &types, &mut |n| -> Result<(), &'static str> {
        largest = largest.max(n);
        if n == 65 {
            Err("roster-work-exhausted")
        } else {
            Ok(())
        }
    });
    assert_eq!(result, Err("roster-work-exhausted"));
    assert_eq!(largest, 65);
    assert_eq!(observe_with(&function(0), &types, &mut 1000), Ok(None));
}

use super::super::{Facts, instance_id, primitive_read};
use crate::production::semantic_ssa::ProductionSemanticSsaErrorV1;
use fe2o3_mir_model::{SemanticCallExpansionLimitsV1, SemanticCallExpansionV1};

fn source_fixture(mutation: u8) -> AdmittedInertSemanticMirV1 {
    let mut types = declarations();
    types.push(ty(6, SemanticTypeShapeV1::Unit, 0, 1));
    types[6] = SemanticTypeDeclV1::new(
        types[6].identity(),
        types[6].layout_identity(),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    );
    let scalar = |bits: u16| {
        SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, bits, u64::from(bits / 8)),
            SemanticScalarValidityRangeV1::new(0, (1_u128 << bits) - 1),
        )
    };
    for (index, offsets) in [(0, vec![0, 8, 16]), (3, vec![0, 4]), (4, vec![])] {
        let original = &types[index];
        let backend = if index == 3 {
            SemanticBackendReprV1::scalar_pair(scalar(32), scalar(32))
        } else {
            SemanticBackendReprV1::memory(true)
        };
        types[index] = SemanticTypeDeclV1::new(
            original.identity(),
            original.layout_identity(),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                original.layout().size_bytes(),
                original.layout().alignment_bytes(),
                backend,
                false,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap(),
            original.shape().clone(),
        );
    }
    for (index, backend) in [
        (
            1,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX as u128),
            )),
        ),
        (2, SemanticBackendReprV1::scalar(scalar(64))),
        (5, SemanticBackendReprV1::scalar(scalar(32))),
    ] {
        let original = &types[index];
        types[index] = SemanticTypeDeclV1::new(
            original.identity(),
            original.layout_identity(),
            SemanticTypeLayoutV1::new_with_backend_repr(
                original.layout().size_bytes(),
                original.layout().alignment_bytes(),
                backend,
                false,
            )
            .unwrap(),
            original.shape().clone(),
        );
    }
    types[1] = types[1].clone().with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                    16,
                    8,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    let place = |local, ty| SemanticPlaceV1::new(l(local), vec![], ty).unwrap();
    let constant = |ty, value, size| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, size).unwrap()),
        ))
    };
    let assign = |local, ty, value| {
        SemanticStatementV1::new(
            provenance(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local, ty),
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let inner = assign(
        4,
        t(3),
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::Aggregate,
                vec![constant(t(5), 11, 4), constant(t(5), 13, 4)],
            )
            .unwrap(),
        ),
    );
    let initialize = assign(
        1,
        t(0),
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::Aggregate,
                vec![
                    constant(t(2), 7, 8),
                    SemanticOperandV1::Copy(place(4, t(3))),
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        t(4),
                        SemanticConstantValueV1::ZeroSized,
                    )),
                ],
            )
            .unwrap(),
        ),
    );
    let borrow = assign(
        2,
        t(1),
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: place(1, t(0)),
        },
    );
    let mut statements = vec![inner, initialize.clone(), borrow];
    match mutation {
        1 => statements.push(initialize),
        2 => statements.push(SemanticStatementV1::new(
            provenance(),
            SemanticStatementKindV1::Deinitialize(place(1, t(0))),
        )),
        3 => statements.push(assign(
            5,
            t(0),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, t(0)))),
        )),
        _ => {}
    }
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            provenance(),
            statements,
            SemanticTerminatorV1::new(provenance(), terminator),
        )
        .unwrap()
    };
    let local = |index, ty, role| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([index; 32]),
            ty,
            role,
            provenance(),
        )
    };
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1),
        vec![SemanticOperandV1::Copy(place(2, t(1)))],
        Some(SemanticCallDestinationV1::new(
            place(3, t(3)),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let root = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([31; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([32; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([33; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([34; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([35; 32]),
        provenance(),
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([36; 32]),
            SemanticLayoutIdentityV1::from_sha256([37; 32]),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(t(6), SemanticAbiPassModeV1::Ignore),
        )
        .unwrap(),
        vec![
            local(1, t(6), SemanticLocalRoleV1::Return),
            local(2, t(0), SemanticLocalRoleV1::Temporary),
            local(3, t(1), SemanticLocalRoleV1::Temporary),
            local(4, t(3), SemanticLocalRoleV1::Temporary),
            local(5, t(3), SemanticLocalRoleV1::Temporary),
            local(6, t(0), SemanticLocalRoleV1::Temporary),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            block(41, statements, SemanticTerminatorKindV1::Call(call)),
            block(42, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"grid_read_scope_test".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([43; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let original = function(0);
    let integer_attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let reference_attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        16,
        Some(8),
    )
    .unwrap();
    let helper_abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        original.abi().identity(),
        original.abi().layout_identity(),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![t(1)],
        t(3),
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            t(1),
            SemanticAbiPassModeV1::Direct(reference_attributes),
        ))],
        SemanticAbiValueV1::new(
            t(3),
            SemanticAbiPassModeV1::Pair {
                first: integer_attributes,
                second: integer_attributes,
            },
        ),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
    .unwrap();
    let read = |destination, field| {
        assign(
            destination,
            t(5),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                SemanticPlaceV1::new(
                    l(1),
                    vec![
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, t(0))
                            .unwrap(),
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), t(3))
                            .unwrap(),
                        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), t(5))
                            .unwrap(),
                    ],
                    t(5),
                )
                .unwrap(),
            )),
        )
    };
    let goto = |to| {
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(to),
        ))
    };
    let helper = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([71; 32]),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        helper_abi,
        vec![
            local(11, t(3), SemanticLocalRoleV1::Return),
            local(12, t(1), SemanticLocalRoleV1::Argument(0)),
            local(13, t(5), SemanticLocalRoleV1::Temporary),
            local(14, t(5), SemanticLocalRoleV1::Temporary),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            block(51, vec![read(2, 0)], goto(1)),
            block(52, vec![read(3, 1)], goto(2)),
            block(
                53,
                vec![assign(
                    0,
                    t(3),
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::Aggregate,
                            vec![
                                SemanticOperandV1::Copy(place(2, t(5))),
                                SemanticOperandV1::Copy(place(3, t(5))),
                            ],
                        )
                        .unwrap(),
                    ),
                )],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    )
    .unwrap();
    let request = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([60; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap();
    layout_diagnostics_v1::diagnose_type_layouts_v1(&request, SemanticMirLimitsV1::default())
        .unwrap();
    request
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap()
}
fn source_charge(
    left: &mut usize,
) -> impl FnMut(usize) -> Result<(), ProductionSemanticSsaErrorV1> + '_ {
    |n| {
        *left = left
            .checked_sub(n)
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        Ok(())
    }
}
#[test]
fn grid_read_scope_multiblock_original_sites_and_census_are_preserved() {
    let semantic = source_fixture(0);
    let expansion =
        SemanticCallExpansionV1::try_new(&semantic, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    expansion.verify_replay(&semantic).unwrap();
    let view = &expansion.roots()[0];
    let before = (
        view.body().clone(),
        view.block_origins().to_vec(),
        view.local_origins().to_vec(),
        *view.identity(),
    );
    let mut facts = Facts::default();
    facts
        .register(
            &semantic,
            view,
            instance_id(view, 0).unwrap(),
            instance_id(view, 1).unwrap(),
            None,
            &mut source_charge(&mut 10000),
        )
        .unwrap();
    assert_eq!(facts.reads.len(), 2);
    assert_eq!(facts.roots.len(), 1);
    assert_eq!(facts.pairs.get(&t(1)), Some(&t(0)));
    for (&site, &(kind, local)) in &facts.reads {
        assert_eq!(
            facts.captured(site, kind, &mut source_charge(&mut 10000)),
            Ok(Some(local))
        );
        assert_eq!(
            facts.captured(site, &kind.clone(), &mut source_charge(&mut 10000)),
            Ok(None)
        );
        assert_eq!(
            facts.captured((site.0, site.1 + 1), kind, &mut source_charge(&mut 10000)),
            Ok(None)
        );
    }
    for (&site, &(local, owned)) in &facts.roots {
        assert_eq!(
            facts.allows_root(site, local, owned, &mut source_charge(&mut 10000)),
            Ok(true)
        );
        assert_eq!(
            facts.allows_root(site, local + 1, owned, &mut source_charge(&mut 10000)),
            Ok(false)
        );
        assert_eq!(
            facts.allows_root(site, local, t(3), &mut source_charge(&mut 10000)),
            Ok(false)
        );
    }
    assert_eq!(view.body(), &before.0);
    assert_eq!(view.block_origins(), before.1.as_slice());
    assert_eq!(view.local_origins(), before.2.as_slice());
    assert_eq!(*view.identity(), before.3);
    expansion.verify_replay(&semantic).unwrap();
}
#[test]
fn grid_read_scope_absent_family_does_not_register_structurally_equal_helpers() {
    let semantic = source_fixture(0);
    let expansion =
        SemanticCallExpansionV1::try_new(&semantic, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let bindings = expansion.defined_capability_bindings(&semantic).unwrap();
    let mut left = 0;
    let facts = Facts::new(
        &semantic,
        &expansion.roots()[0],
        &bindings,
        &mut source_charge(&mut left),
    )
    .unwrap();
    assert!(facts.pairs.is_empty() && facts.roots.is_empty() && facts.reads.is_empty());
    assert_eq!(left, 0);
}
#[test]
fn grid_read_scope_rejects_source_owner_writes_lifetime_and_move() {
    for mutation in 1..=3 {
        let semantic = source_fixture(mutation);
        let expansion =
            SemanticCallExpansionV1::try_new(&semantic, SemanticCallExpansionLimitsV1::default())
                .unwrap();
        let view = &expansion.roots()[0];
        let mut facts = Facts::default();
        facts
            .register(
                &semantic,
                view,
                instance_id(view, 0).unwrap(),
                instance_id(view, 1).unwrap(),
                None,
                &mut source_charge(&mut 10000),
            )
            .unwrap();
        assert!(
            facts.pairs.is_empty() && facts.roots.is_empty() && facts.reads.is_empty(),
            "mutation {mutation}"
        );
    }
}
#[test]
fn grid_read_scope_wrong_parent_and_declared_reference_reject() {
    let semantic = source_fixture(0);
    let expansion =
        SemanticCallExpansionV1::try_new(&semantic, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let view = &expansion.roots()[0];
    let mut facts = Facts::default();
    assert_eq!(
        facts.register(
            &semantic,
            view,
            instance_id(view, 1).unwrap(),
            instance_id(view, 1).unwrap(),
            None,
            &mut source_charge(&mut 10000)
        ),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    );
    facts
        .register(
            &semantic,
            view,
            instance_id(view, 0).unwrap(),
            instance_id(view, 1).unwrap(),
            Some(t(2)),
            &mut source_charge(&mut 10000),
        )
        .unwrap();
    assert!(facts.pairs.is_empty() && facts.reads.is_empty());
}
#[test]
fn grid_read_scope_shared_budget_is_exact_and_not_refunded() {
    let semantic = source_fixture(0);
    let expansion =
        SemanticCallExpansionV1::try_new(&semantic, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    let view = &expansion.roots()[0];
    let register = |left: &mut usize| {
        Facts::default().register(
            &semantic,
            view,
            instance_id(view, 0).unwrap(),
            instance_id(view, 1).unwrap(),
            None,
            &mut source_charge(left),
        )
    };
    let mut left = 10000;
    register(&mut left).unwrap();
    let spent = 10000 - left;
    let mut exact = spent;
    register(&mut exact).unwrap();
    assert_eq!(exact, 0);
    assert_eq!(
        register(&mut exact),
        Err(ProductionSemanticSsaErrorV1::ResourceOverflow)
    );
    assert_eq!(exact, 0);
    let mut short = spent - 1;
    assert_eq!(
        register(&mut short),
        Err(ProductionSemanticSsaErrorV1::ResourceOverflow)
    );
    assert!(short < spent - 1, "failed work is never refunded");
}
#[test]
fn grid_primitive_read_rejects_move_wrong_type_and_nonfield_projection() {
    let f = source_fixture(0);
    let SemanticStatementKindV1::Assign(original) =
        f.functions()[1].blocks()[0].statements()[0].kind()
    else {
        unreachable!()
    };
    assert!(
        primitive_read::copied_field(
            original,
            l(1),
            t(0),
            f.types(),
            &mut source_charge(&mut 1000)
        )
        .unwrap()
    );
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(p)) = original.value().kind() else {
        unreachable!()
    };
    let moved = SemanticAssignmentV1::new(
        original.destination().clone(),
        SemanticRvalueV1::new(
            t(5),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p.clone())),
        ),
    );
    assert!(
        !primitive_read::copied_field(&moved, l(1), t(0), f.types(), &mut source_charge(&mut 1000))
            .unwrap()
    );
    assert!(
        !primitive_read::copied_field(
            original,
            l(0),
            t(0),
            f.types(),
            &mut source_charge(&mut 1000)
        )
        .unwrap()
    );
    assert!(
        !primitive_read::copied_field(
            original,
            l(1),
            t(3),
            f.types(),
            &mut source_charge(&mut 1000)
        )
        .unwrap()
    );
    let bad = SemanticPlaceV1::new(
        l(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, t(0)).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(0), t(0)).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), t(3)).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), t(5)).unwrap(),
        ],
        t(5),
    )
    .unwrap();
    let bad = SemanticAssignmentV1::new(
        original.destination().clone(),
        SemanticRvalueV1::new(
            t(5),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(bad)),
        ),
    );
    assert!(
        !primitive_read::copied_field(&bad, l(1), t(0), f.types(), &mut source_charge(&mut 1000))
            .unwrap()
    );
}

#[test]
fn grid_primitive_read_exact_rank_u64_and_projection_bounds() {
    let types = declarations();
    let place = SemanticPlaceV1::new(
        l(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, t(0)).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), t(2)).unwrap(),
        ],
        t(2),
    )
    .unwrap();
    let assignment = SemanticAssignmentV1::new(
        SemanticPlaceV1::new(l(0), vec![], t(2)).unwrap(),
        SemanticRvalueV1::new(
            t(2),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)),
        ),
    );
    assert!(
        primitive_read::copied_field(
            &assignment,
            l(1),
            t(0),
            &types,
            &mut source_charge(&mut 1000)
        )
        .unwrap()
    );
    assert!(
        !primitive_read::copied_field(
            &assignment,
            l(0),
            t(0),
            &types,
            &mut source_charge(&mut 1000)
        )
        .unwrap()
    );
    for (signed, bits, bytes) in [(true, 64, 8), (false, 32, 8), (false, 64, 4)] {
        let mut types = types.clone();
        types[2] = ty(
            2,
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }),
            bytes,
            bytes,
        );
        assert!(
            !primitive_read::copied_field(
                &assignment,
                l(1),
                t(0),
                &types,
                &mut source_charge(&mut 1000)
            )
            .unwrap()
        );
    }
    let long = SemanticPlaceV1::new(
        l(1),
        std::iter::once(
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, t(0)).unwrap(),
        )
        .chain(std::iter::repeat_n(
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), t(2)).unwrap(),
            8,
        ))
        .collect(),
        t(2),
    )
    .unwrap();
    let long = SemanticAssignmentV1::new(
        assignment.destination().clone(),
        SemanticRvalueV1::new(
            t(2),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(long)),
        ),
    );
    assert!(
        !primitive_read::copied_field(&long, l(1), t(0), &types, &mut source_charge(&mut 1000))
            .unwrap()
    );
}
