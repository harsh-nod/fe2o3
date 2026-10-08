use super::*;

// These are original admitted semantic/SSA owner fixtures, not fabricated
// receipts and not a claim of an actual rustc or native transaction.
const SCALAR41: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const RAW41: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const TOKEN41: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const RECEIVER41: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const STORAGE41: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const CELL41: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const ATOMIC41: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const RAW_ATOMIC41: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
const REF_ATOMIC41: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);
const REF_CELL41: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(10);
const RAW_CELL41: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(11);
const RAW_STORAGE41: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(12);
const ADDRESS_INT41: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(13);

#[derive(Clone, Copy, Debug)]
enum Fault41 {
    None,
    ByValue,
    DeadRoot,
    OrdinaryLoad,
    OrdinaryStore,
    MutableBorrow,
    DescendantCast,
    ForeignCast,
    ReverseCast,
    Offset,
    Expose,
    BaseAliasRead,
    PreViewCopy,
    PreViewMove,
}

fn declaration41(
    tag: u8,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        layout,
        shape,
    )
}
fn pointer_backend41(reference: bool) -> SemanticBackendReprV1 {
    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(u128::from(reference), u64::MAX.into()),
    ))
}
fn pointer41(tag: u8, pointee: SemanticTypeIdV1, shared: bool) -> SemanticTypeDeclV1 {
    pointer41_mutability(
        tag,
        pointee,
        shared,
        if shared {
            SemanticMutabilityV1::Immutable
        } else {
            SemanticMutabilityV1::Mutable
        },
    )
}
fn pointer41_mutability(
    tag: u8,
    pointee: SemanticTypeIdV1,
    shared: bool,
    mutability: SemanticMutabilityV1,
) -> SemanticTypeDeclV1 {
    let row = declaration41(
        tag,
        SemanticTypeLayoutV1::new_with_backend_repr(Some(8), 8, pointer_backend41(shared), false)
            .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                if shared {
                    SemanticPointerKindV1::Reference
                } else {
                    SemanticPointerKindV1::Raw
                },
                mutability,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    );
    if shared {
        row
    } else {
        row.with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
                None,
            ),
        )
    }
}
fn aggregate41(tag: u8, child: SemanticTypeIdV1) -> SemanticTypeDeclV1 {
    declaration41(
        tag,
        SemanticTypeLayoutV1::aggregate(
            Some(4),
            4,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![child]).unwrap()),
    )
}
fn types41() -> Vec<SemanticTypeDeclV1> {
    let unit = owner(Case::Shared).source_semantic().types()[0].clone();
    let scalar = declaration41(
        2,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    );
    let token = declaration41(
        4,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            8,
            8,
            SemanticFieldsShapeV1::arbitrary(vec![0], vec![0]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            pointer_backend41(false),
            None,
            false,
            None,
            8,
            8,
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            ),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![RAW41]).unwrap()),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
            None,
        ),
    );
    vec![
        unit,
        scalar,
        pointer41(3, SCALAR41, false),
        token,
        pointer41(5, TOKEN41, true),
        aggregate41(6, SCALAR41),
        aggregate41(7, STORAGE41),
        aggregate41(8, CELL41).with_rust_type_kind(SemanticRustTypeKindV1::AtomicU32),
        pointer41(9, ATOMIC41, false),
        pointer41(10, ATOMIC41, true),
        pointer41(11, CELL41, true),
        pointer41(12, CELL41, false),
        pointer41(13, STORAGE41, false),
        declaration41(
            14,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 64, 8),
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            }),
        ),
    ]
}
fn abi41(
    tag: u8,
    kernel: bool,
    input: SemanticTypeIdV1,
    output: SemanticTypeIdV1,
    exclusive: bool,
) -> SemanticFunctionAbiV1 {
    // Initialized raw words require no_undef, without alias/non-null/capture facts.
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let value = SemanticAbiValueV1::new(input, SemanticAbiPassModeV1::Direct(attributes.clone()));
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        if kernel {
            SemanticCanonAbiV1::GpuKernel
        } else {
            SemanticCanonAbiV1::Rust
        },
        if kernel {
            SemanticExternAbiV1::GpuKernel
        } else {
            SemanticExternAbiV1::Rust
        },
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(value)],
        SemanticAbiValueV1::new(
            output,
            if output == UNIT {
                SemanticAbiPassModeV1::Ignore
            } else {
                SemanticAbiPassModeV1::Direct(attributes)
            },
        ),
    )
    .unwrap()
    .with_source_argument_ownership(vec![if exclusive {
        SemanticSourceArgumentOwnershipV1::ExclusiveOwner
    } else {
        SemanticSourceArgumentOwnershipV1::ByValue
    }])
    .unwrap()
}
fn function41(
    tag: u8,
    kernel: bool,
    abi: SemanticFunctionAbiV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let row = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        if kernel {
            SemanticFunctionRoleV1::KernelRoot
        } else {
            SemanticFunctionRoleV1::InternalHelper
        },
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        source(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    if kernel {
        // This fixture has its own exact source launch contract before admission.
        // The older generic reference fixture intentionally has no such contract.
        row.with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"source_reference_component".to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([10; 32]),
            SemanticKernelSourceContractV1::new(
                Some(
                    SemanticKernelLaunchBoundsV1::new(
                        Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                        Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                        None,
                    )
                    .unwrap(),
                ),
                None,
                None,
            )
            .unwrap(),
        ))
    } else {
        row
    }
}
fn cast41(
    destination: u32,
    output: SemanticTypeIdV1,
    input: u32,
    ty: SemanticTypeIdV1,
) -> SemanticStatementV1 {
    assign(
        place(destination, output),
        SemanticRvalueKindV1::Cast {
            kind: SemanticCastKindV1::Pointer,
            operand: SemanticOperandV1::Copy(place(input, ty)),
        },
    )
}
fn projected41(local: u32, types: &[SemanticTypeIdV1]) -> SemanticPlaceV1 {
    projected(
        local,
        &types
            .iter()
            .enumerate()
            .map(|(index, ty)| {
                (
                    if index == 0 {
                        SemanticProjectionKindV1::Dereference
                    } else {
                        SemanticProjectionKindV1::Field(0)
                    },
                    *ty,
                )
            })
            .collect::<Vec<_>>(),
    )
}
fn owner41(fault: Fault41) -> ProductionSemanticSsaOwnerV1 {
    owner41_atomic(fault, None)
}
fn owner41_atomic(
    fault: Fault41,
    atomic: Option<(SemanticAtomicRmwOpV1, SemanticAtomicOrderingV1)>,
) -> ProductionSemanticSsaOwnerV1 {
    let mut owner = owner41_atomic_uncaptured(fault, atomic);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    owner
        .try_capture_occurrences_with_budget_v1(&mut ArgumentBudgetV1::new(&mut work, usize::MAX))
        .unwrap();
    owner
}

// The consuming source entrance owns occurrence capture and its live reservation.
// Component tests keep the historical captured wrapper above.
fn owner41_atomic_uncaptured(
    fault: Fault41,
    atomic: Option<(SemanticAtomicRmwOpV1, SemanticAtomicOrderingV1)>,
) -> ProductionSemanticSsaOwnerV1 {
    owner41_atomic_formation_uncaptured(fault, atomic, None)
}
fn owner41_atomic_formation_uncaptured(
    fault: Fault41,
    atomic: Option<(SemanticAtomicRmwOpV1, SemanticAtomicOrderingV1)>,
    address_of: Option<SemanticMutabilityV1>,
) -> ProductionSemanticSsaOwnerV1 {
    owner41_atomic_path_uncaptured(
        fault,
        atomic,
        address_of,
        matches!(fault, Fault41::DescendantCast),
    )
}
fn owner41_atomic_path_uncaptured(
    fault: Fault41,
    atomic: Option<(SemanticAtomicRmwOpV1, SemanticAtomicOrderingV1)>,
    address_of: Option<SemanticMutabilityV1>,
    descendant: bool,
) -> ProductionSemanticSsaOwnerV1 {
    let mut locals: Vec<_> = [
        UNIT,
        TOKEN41,
        RECEIVER41,
        RAW41,
        RAW_ATOMIC41,
        REF_ATOMIC41,
        REF_CELL41,
        RAW_CELL41,
        RAW_STORAGE41,
        RAW41,
        SCALAR41,
        ADDRESS_INT41,
    ]
    .into_iter()
    .enumerate()
    .map(|(i, ty)| {
        local(
            10 + i as u8,
            ty,
            if i == 0 {
                SemanticLocalRoleV1::Return
            } else if i == 1 {
                SemanticLocalRoleV1::Argument(0)
            } else {
                SemanticLocalRoleV1::Temporary
            },
        )
    })
    .collect();
    if matches!(fault, Fault41::ForeignCast) {
        locals.push(local(
            22,
            SemanticTypeIdV1::from_index(14),
            SemanticLocalRoleV1::Temporary,
        ));
    }
    let mut initial = vec![
        assign(
            place(2, RECEIVER41),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(1, TOKEN41),
            },
        ),
        assign(
            place(3, RAW41),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected41(2, &[TOKEN41, RAW41]))),
        ),
    ];
    if matches!(fault, Fault41::PreViewCopy | Fault41::PreViewMove) {
        initial.push(assign(
            place(9, RAW41),
            SemanticRvalueKindV1::Use(if matches!(fault, Fault41::PreViewMove) {
                SemanticOperandV1::Move(place(3, RAW41))
            } else {
                SemanticOperandV1::Copy(place(3, RAW41))
            }),
        ));
        initial.push(assign(
            place(10, SCALAR41),
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                projected41(9, &[SCALAR41]),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        ));
    }
    if matches!(fault, Fault41::DeadRoot) {
        initial.push(dead(1));
    }
    let helper_call = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(1),
            vec![SemanticOperandV1::Copy(place(
                if matches!(fault, Fault41::PreViewMove) {
                    9
                } else {
                    3
                },
                RAW41,
            ))],
            Some(SemanticCallDestinationV1::new(
                place(4, RAW_ATOMIC41),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(1),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let mut body = vec![
        assign(
            place(5, REF_ATOMIC41),
            SemanticRvalueKindV1::Borrow {
                kind: if matches!(fault, Fault41::MutableBorrow) {
                    SemanticBorrowKindV1::Mutable
                } else {
                    SemanticBorrowKindV1::Shared
                },
                place: projected41(4, &[ATOMIC41]),
            },
        ),
        assign(
            place(6, REF_CELL41),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: projected41(5, &[ATOMIC41, CELL41]),
            },
        ),
        cast41(7, RAW_CELL41, 6, REF_CELL41),
        cast41(8, RAW_STORAGE41, 7, RAW_CELL41),
        cast41(
            9,
            RAW41,
            if descendant { 7 } else { 8 },
            if descendant {
                RAW_CELL41
            } else {
                RAW_STORAGE41
            },
        ),
    ];
    if descendant {
        // A genuine direct typed Cast traverses both committed transparent
        // edges. Do not emit an unused intermediate cast to make the test pass.
        body.remove(3);
    }
    if let Some(mutability) = address_of {
        body[1] = assign(
            place(7, RAW_CELL41),
            SemanticRvalueKindV1::AddressOf {
                place: projected41(5, &[ATOMIC41, CELL41]),
                mutability,
            },
        );
        body.remove(2);
    }
    match fault {
        Fault41::BaseAliasRead => body.push(assign(
            place(10, SCALAR41),
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                projected41(3, &[SCALAR41]),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        )),
        Fault41::OrdinaryLoad => body.push(assign(
            place(10, SCALAR41),
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                projected41(9, &[SCALAR41]),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        )),
        Fault41::OrdinaryStore => body.push(assign(
            projected41(9, &[SCALAR41]),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                SCALAR41,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 4).unwrap()),
            ))),
        )),
        Fault41::ReverseCast => body.push(cast41(4, RAW_ATOMIC41, 9, RAW41)),
        Fault41::ForeignCast => body.push(cast41(12, SemanticTypeIdV1::from_index(14), 9, RAW41)),
        Fault41::Offset => body.push(assign(
            place(9, RAW41),
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Offset,
                left: SemanticOperandV1::Copy(place(9, RAW41)),
                right: SemanticOperandV1::Constant(SemanticConstantV1::new(
                    SCALAR41,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 4).unwrap()),
                )),
            },
        )),
        Fault41::Expose => body.push(assign(
            place(11, ADDRESS_INT41),
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::PointerExposeProvenance,
                operand: SemanticOperandV1::Copy(place(9, RAW41)),
            },
        )),
        _ => {}
    }
    if let Some((operation, ordering)) = atomic {
        body.push(statement(SemanticStatementKindV1::AtomicRmw(
            SemanticAtomicRmwV1::new(
                place(10, SCALAR41),
                projected41(9, &[SCALAR41]),
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    SCALAR41,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 4).unwrap()),
                )),
                operation,
                SemanticAtomicAccessV1::new(ordering, SemanticAtomicScopeV1::System),
            ),
        )));
    }
    body.push(unit());
    let root = function41(
        10,
        true,
        abi41(10, true, TOKEN41, UNIT, !matches!(fault, Fault41::ByValue)),
        locals,
        vec![
            block(10, initial, helper_call),
            block(11, body, SemanticTerminatorKindV1::Return),
        ],
    );
    let helper = function41(
        20,
        false,
        abi41(20, false, RAW41, RAW_ATOMIC41, false),
        vec![
            local(30, RAW_ATOMIC41, SemanticLocalRoleV1::Return),
            local(31, RAW41, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![block(
            20,
            vec![cast41(0, RAW_ATOMIC41, 1, RAW41)],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let mut types = types41();
    if matches!(fault, Fault41::ForeignCast) {
        types.push(pointer41(15, UNIT, false));
    }
    if let Some(mutability) = address_of {
        types[RAW_CELL41.index() as usize] = pointer41_mutability(12, CELL41, false, mutability);
    }
    if atomic.is_some_and(|(kind, _)| {
        matches!(
            kind,
            SemanticAtomicRmwOpV1::SignedMinimum | SemanticAtomicRmwOpV1::SignedMaximum
        )
    }) {
        types[SCALAR41.index() as usize] = declaration41(
            2,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(true, 32, 4),
                    SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: true,
                bits: 32,
            }),
        );
        types[ATOMIC41.index() as usize] =
            aggregate41(8, CELL41).with_rust_type_kind(SemanticRustTypeKindV1::AtomicI32);
    }
    if matches!(fault, Fault41::MutableBorrow) {
        types[REF_ATOMIC41.index() as usize] = declaration41(
            10,
            SemanticTypeLayoutV1::new_with_backend_repr(Some(8), 8, pointer_backend41(true), false)
                .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ATOMIC41,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        );
    }
    let request = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![
            SemanticCallableDeclV1::defined(ROOT),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        ],
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v41(SemanticMirLimitsV1::default())
    .unwrap();
    let mir =
        ProductionSemanticMirOwnerV1::try_new(request, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(mir, ProductionSemanticSsaLimitsV1::default()).unwrap()
}
#[test]
fn atomic_view_v41_original_root_helper_return_and_three_field_chain_are_owned() {
    cells_tests::run_cells(owner41(Fault41::None), |plan, budget| {
        assert_eq!(plan.instances.instances().len(), 2);
        assert_eq!(plan.loans.len(), 1);
        assert_eq!(plan.atomic_formations.len(), 6);
        assert_eq!(plan.atomic_custody.len(), 5);
        let first = plan.atomic_custody[0];
        assert_eq!(first.anchor.argument, 0);
        assert_eq!(first.instance, plan.root);
        assert_eq!(first.local, SemanticLocalIdV1::from_index(1));
        assert_eq!(first.generation, 0);
        assert!(first.view.is_none());
        assert!(
            plan.atomic_custody
                .iter()
                .all(|fact| fact.parent == first.parent
                    && fact.anchor == first.anchor
                    && fact.instance == first.instance
                    && fact.local == first.local)
        );
        assert!(
            plan.atomic_custody
                .iter()
                .any(|fact| fact.view.is_some_and(|view| view.is_scalar_leaf()))
        );
        for formation in &plan.atomic_formations {
            let function = plan
                .instances
                .instance(formation.site.instance)
                .unwrap()
                .declaration();
            let site = ExecutionSiteV29::Statement {
                block: SsaBlockIdV1::new(formation.site.block.index()),
                statement: formation.site.statement.unwrap() as u32,
            };
            let SemanticStatementKindV1::Assign(assignment) =
                scoped_source_statement_v29(function, site).unwrap()
            else {
                panic!("actual assignment")
            };
            assert_eq!(
                plan.atomic_view_formation_v41(
                    formation.site.instance,
                    site,
                    assignment.value(),
                    budget
                )?,
                Some(*formation)
            );
        }
        Ok(())
    })
    .unwrap();
}
#[test]
fn atomic_view_v41_original_by_value_pointer_never_gains_owner_custody() {
    assert!(cells_tests::run_cells(owner41(Fault41::ByValue), |_, _| Ok(())).is_err());
}
#[test]
fn atomic_view_v41_original_lender_death_is_not_hidden_by_helper_transport() {
    assert!(cells_tests::run_cells(owner41(Fault41::DeadRoot), |_, _| Ok(())).is_err());
}
#[test]
fn atomic_view_v41_original_ordinary_load_store_and_mutable_borrow_refuse() {
    for fault in [
        Fault41::OrdinaryLoad,
        Fault41::OrdinaryStore,
        Fault41::MutableBorrow,
    ] {
        assert!(
            cells_tests::run_cells(owner41(fault), |_, _| Ok(())).is_err(),
            "{fault:?}"
        );
    }
}
#[test]
fn atomic_view_v41_original_foreign_and_reverse_casts_refuse() {
    for fault in [Fault41::ForeignCast, Fault41::ReverseCast] {
        assert!(
            cells_tests::run_cells(owner41(fault), |_, _| Ok(())).is_err(),
            "{fault:?}"
        );
    }
}
#[test]
fn atomic_view_v41_original_typed_descendant_cast_is_not_a_projected_skip() {
    cells_tests::run_cells(owner41(Fault41::DescendantCast), |plan, _| {
        assert!(plan.atomic_formations.iter().any(|row| {
            row.operation == SourceAtomicViewOperationV41::Cast
                && plan.atomic_custody[row.input]
                    .view
                    .is_some_and(|path| path.depth == 1)
                && plan.atomic_custody[row.output]
                    .view
                    .is_some_and(|path| path.depth == 3)
        }));
        Ok(())
    })
    .unwrap();
}
#[test]
fn atomic_view_v41_same_shaped_foreign_formation_cannot_be_consumed() {
    let reached = std::cell::Cell::new(false);
    assert!(
        cells_tests::run_cells(owner41(Fault41::None), |plan, budget| {
            reached.set(true);
            let row = plan.atomic_formations[0];
            let site = ExecutionSiteV29::Statement {
                block: SsaBlockIdV1::new(row.site.block.index()),
                statement: row.site.statement.unwrap() as u32,
            };
            let function = plan
                .instances
                .instance(row.site.instance)
                .unwrap()
                .declaration();
            let SemanticStatementKindV1::Assign(original) =
                scoped_source_statement_v29(function, site).unwrap()
            else {
                panic!("actual assignment")
            };
            let copied = original.value().clone();
            let denied = plan.atomic_view_formation_v41(row.site.instance, site, &copied, budget);
            assert!(denied.is_err());
            denied.map(|_| ())
        })
        .is_err()
    );
    assert!(
        reached.get(),
        "original-plan consumer must reach the intended refusal"
    );
}

#[test]
fn atomic_view_v41_original_offset_and_exposure_never_erase_custody() {
    for fault in [Fault41::Offset, Fault41::Expose] {
        assert!(
            cells_tests::run_cells(owner41(fault), |_, _| Ok(())).is_err(),
            "{fault:?}"
        );
    }
}

#[test]
fn atomic_view_v41_earlier_raw_alias_cannot_bypass_atomic_only_access() {
    assert!(cells_tests::run_cells(owner41(Fault41::BaseAliasRead), |_, _| Ok(())).is_err());
}
#[test]
fn atomic_view_v41_denied_formation_storage_retains_failure_without_receipt() {
    let reached = std::cell::Cell::new(false);
    assert!(
        cells_tests::run_cells(owner41(Fault41::None), |plan, budget| {
            reached.set(true);
            let row = plan.atomic_formations[0];
            let site = ExecutionSiteV29::Statement {
                block: SsaBlockIdV1::new(row.site.block.index()),
                statement: row.site.statement.unwrap() as u32,
            };
            let function = plan
                .instances
                .instance(row.site.instance)
                .unwrap()
                .declaration();
            let SemanticStatementKindV1::Assign(original) =
                scoped_source_statement_v29(function, site).unwrap()
            else {
                panic!("actual assignment")
            };
            // The source/owner checks precede receipt materialization; no remaining
            // storage is available for the first required observation envelope.
            let padding = usize::MAX - budget.storage();
            budget.reserve_storage(padding)?;
            let denied =
                plan.atomic_view_formation_v41(row.site.instance, site, original.value(), budget);
            assert!(matches!(
                denied,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                )
            ));
            budget.release_storage(padding)?;
            Ok(())
        })
        .is_err()
    );
    assert!(
        reached.get(),
        "original-plan consumer must reach the intended refusal"
    );
}

#[test]
fn atomic_view_v41_fixture_pointer_layouts_distinguish_raw_and_reference_validity() {
    let types = types41();
    for row in &types {
        if let SemanticTypeShapeV1::Pointer(pointer) = row.shape() {
            let reference = pointer.kind() == SemanticPointerKindV1::Reference;
            assert_eq!(row.layout().backend_repr(), &pointer_backend41(reference));
            assert_eq!(row.layout().size_bytes(), Some(8));
            assert_eq!(row.layout().alignment_bytes(), 8);
        }
    }
    assert_ne!(pointer_backend41(false), pointer_backend41(true));
}

#[path = "production_source_atomic_emission_v41_tests.rs"]
mod physical_tests;

#[test]
fn atomic_view_v41_fixture_abi_is_initialized_raw_without_drop_glue_override() {
    let owner = owner41(Fault41::None);
    let semantic = owner.source_semantic();
    for function in semantic.functions() {
        for value in function
            .abi()
            .arguments()
            .iter()
            .map(|a| a.value())
            .chain(std::iter::once(function.abi().return_value()))
        {
            assert!(value.pointee_override().is_none());
            if let SemanticAbiPassModeV1::Direct(attributes) = value.mode() {
                assert!(attributes.regular().no_undef());
                assert!(!attributes.regular().non_null());
                assert!(!attributes.regular().no_alias());
                assert!(!attributes.regular().read_only());
                assert!(attributes.regular().pointer_capture().is_none());
                assert_eq!(
                    semantic.types()[value.ty().index() as usize]
                        .abi_properties()
                        .first_pointee()
                        .unwrap()
                        .kind(),
                    SemanticAbiPointeeKindV1::Raw
                );
            }
        }
    }
    for fault in 0..3 {
        let mut types = semantic.types().to_vec();
        let mut functions = semantic.functions().to_vec();
        if fault == 2 {
            types[RAW41.index() as usize] = types[RAW41.index() as usize]
                .clone()
                .with_rustc_abi_properties(SemanticTypeAbiPropertiesV1::new(false, false));
        } else {
            let selected = usize::from(fault == 1);
            let old = &semantic.functions()[selected];
            let original = old.abi().arguments()[0].value();
            let value = if fault == 0 {
                original.clone().with_pointee_override(
                    SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap(),
                )
            } else {
                SemanticAbiValueV1::new(
                    original.ty(),
                    SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
                )
            };
            let abi = SemanticFunctionAbiV1::from_rustc(
                old.abi().identity(),
                old.abi().layout_identity(),
                old.abi().canon_abi(),
                old.abi().extern_abi(),
                old.abi().can_unwind(),
                old.abi().c_variadic(),
                old.abi().fixed_count(),
                vec![SemanticAbiArgumentV1::source(value)],
                old.abi().return_value().clone(),
            )
            .unwrap()
            .with_source_argument_ownership(old.abi().source_argument_ownership().to_vec())
            .unwrap();
            let mut replacement = SemanticFunctionDeclV1::new(
                old.identity(),
                old.role(),
                old.item_definition_identity(),
                old.monomorphization_identity(),
                old.generic_type_arguments_identity(),
                old.const_generic_arguments_identity(),
                old.source(),
                abi,
                old.locals().to_vec(),
                old.entry(),
                old.blocks().to_vec(),
            )
            .unwrap();
            if let Some(entry) = old.kernel_entry() {
                replacement = replacement.with_kernel_entry(entry.clone());
            }
            functions[selected] = replacement;
        }
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            semantic.target(),
            types,
            semantic.allocations().to_vec(),
            semantic.statics().to_vec(),
            semantic.vtables().to_vec(),
            functions,
            semantic.callables().to_vec(),
            semantic.roots().to_vec(),
        )
        .unwrap()
        .admit_exact_v41(SemanticMirLimitsV1::default());
        assert!(
            matches!(
                admitted,
                Err(fe2o3_mir_model::semantic_mir_v1::SemanticMirErrorV1::InvalidFunctionAbi)
            ),
            "independent ABI fault {fault}"
        );
    }
}

#[test]
fn atomic_view_v41_promoted_only_still_refuses_helper_return_lifetime() {
    let consumed = std::cell::Cell::new(false);
    let result = run_owner(owner41(Fault41::None), |_, _| {
        consumed.set(true);
        Ok(())
    });
    assert!(!consumed.get());
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "source reference return escape requires lifetime transport",
            ..
        })
    ));
}

#[test]
fn atomic_view_v41_fixture_source_launch_is_exact_and_mismatch_refuses() {
    let owner = owner41(Fault41::None);
    let semantic = owner.source_semantic();
    let root = &semantic.functions()[semantic.roots()[0].index() as usize];
    let entry = root.kernel_entry().unwrap();
    let source_launch = entry.source_contract().launch().unwrap();
    assert_eq!(source_launch.required().unwrap().as_array(), [64, 1, 1]);
    assert_eq!(source_launch.maximum().unwrap().as_array(), [64, 1, 1]);
    for dimensions in [Some([64, 1, 1]), Some([32, 1, 1]), None] {
        let result = crate::ProductionSourceLaunchRosterV1::try_new(
            semantic,
            &[crate::ProductionSourceLaunchRootInputV1::new(
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                *entry.kernel_binding_identity().as_bytes(),
                crate::ProductionSourceLaunchInputV1::new(1, dimensions, [1, 1, 1]),
            )],
        );
        match dimensions {
            Some([64, 1, 1]) => {
                drop(result.unwrap());
            }
            Some(_) => assert!(matches!(
                result,
                Err(crate::ProductionSourceLaunchErrorV1::Unsupported(
                    "authenticated LaunchContract workgroup disagrees with semantic source workgroup"
                ))
            )),
            None => assert!(matches!(
                result,
                Err(crate::ProductionSourceLaunchErrorV1::Incomplete(
                    "concurrency verification requires an exact authenticated LaunchContract workgroup"
                ))
            )),
        }
    }
}

#[test]
fn atomic_view_v41_consuming_fixture_defers_capture_without_changing_source() {
    let atomic = Some((
        SemanticAtomicRmwOpV1::Exchange,
        SemanticAtomicOrderingV1::Relaxed,
    ));
    let uncaptured = owner41_atomic_uncaptured(Fault41::None, atomic);
    let captured = owner41_atomic(Fault41::None, atomic);
    assert!(uncaptured.occurrence_storage().is_none());
    assert!(captured.occurrence_storage().unwrap().retained_storage() > 0);
    assert_eq!(uncaptured.identity(), captured.identity());
    assert_eq!(
        uncaptured.source_semantic_sha256(),
        captured.source_semantic_sha256()
    );
}
