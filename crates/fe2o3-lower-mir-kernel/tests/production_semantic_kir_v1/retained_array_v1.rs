use super::*;

#[path = "projected_call_destinations_v1.rs"]
mod projected_call_destinations_v1;

use fe2o3_kernel_ir::{AddressSpace, Constant, ValueId};
use fe2o3_lower_mir_kernel::{
    ProductionPreRankedKirOwnerV1, ProductionSourceLaunchInputV1,
    ProductionSourceLaunchRootInputV1, ProductionSourceLaunchRosterV1,
};
use fe2o3_pliron::{ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1};

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const SCALAR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const ARRAY: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const BOOLEAN: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const RAW_ARRAY: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);

fn whole(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn indexed(local: u32, element: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                element,
            )
            .unwrap(),
        ],
        element,
    )
    .unwrap()
}

fn fixed(local: u32, offset: u64, from_end: bool, element: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset,
                    minimum_length: 8,
                    from_end,
                },
                element,
            )
            .unwrap(),
        ],
        element,
    )
    .unwrap()
}

fn st(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}

fn assign(place: SemanticPlaceV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    let ty = place.ty();
    st(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place,
        SemanticRvalueV1::new(ty, value),
    )))
}

fn number(value: u32) -> SemanticOperandV1 {
    scalar_constant(SCALAR, value, 4)
}

fn initialize(local: u32, pointer_elements: bool) -> SemanticStatementV1 {
    let operand = if pointer_elements {
        SemanticOperandV1::Copy(whole(6, POINTER))
    } else {
        number(11)
    };
    assign(
        whole(local, ARRAY),
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Array, vec![operand; 8])
                .unwrap(),
        ),
    )
}

fn element_write(local: u32, pointer_elements: bool) -> SemanticStatementV1 {
    assign(
        indexed(local, if pointer_elements { POINTER } else { SCALAR }),
        SemanticRvalueKindV1::Use(if pointer_elements {
            SemanticOperandV1::Copy(whole(6, POINTER))
        } else {
            number(99)
        }),
    )
}

fn index(value: u32) -> SemanticStatementV1 {
    assign(whole(2, SCALAR), SemanticRvalueKindV1::Use(number(value)))
}

fn read(place: SemanticPlaceV1) -> SemanticStatementV1 {
    assign(
        whole(3, place.ty()),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)),
    )
}

fn fixture(
    pointer_elements: bool,
    length: u64,
    blocks: Vec<SemanticBasicBlockV1>,
) -> ProductionSemanticMirOwnerV1 {
    let argument = pointer_elements.then(|| {
        (
            6,
            direct_abi_value(POINTER),
            SemanticSourceArgumentOwnershipV1::Unspecified,
        )
    });
    fixture_with_argument(pointer_elements, length, blocks, argument)
}

fn fixture_with_argument(
    pointer_elements: bool,
    length: u64,
    blocks: Vec<SemanticBasicBlockV1>,
    argument: Option<(u32, SemanticAbiValueV1, SemanticSourceArgumentOwnershipV1)>,
) -> ProductionSemanticMirOwnerV1 {
    let element = if pointer_elements { POINTER } else { SCALAR };
    let width = if pointer_elements { 8 } else { 4 };
    let array = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(241)),
        SemanticLayoutIdentityV1::from_sha256(bytes(241)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            length * width,
            width,
            SemanticFieldsShapeV1::array(width, length),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            width,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array { element, length },
    );
    let source = SemanticSourceProvenanceV1::unavailable();
    let types = vec![
        unit_type(),
        scalar_type(
            240,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        ),
        array,
        scalar_type(242, SemanticScalarTypeV1::Bool),
        reference_type(
            243,
            ARRAY,
            length * width,
            width,
            SemanticMutabilityV1::Mutable,
        ),
        mutable_global_u32_pointer_type(244, SCALAR),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(245)),
            SemanticLayoutIdentityV1::from_sha256(bytes(245)),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ARRAY,
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ),
    ];
    let local_types = [
        UNIT, ARRAY, SCALAR, element, ARRAY, BOOLEAN, POINTER, REFERENCE, RAW_ARRAY, SCALAR,
    ];
    let locals = local_types
        .into_iter()
        .enumerate()
        .map(|(index, ty)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(100 + index as u8)),
                ty,
                match index {
                    0 => SemanticLocalRoleV1::Return,
                    _ if argument
                        .as_ref()
                        .is_some_and(|(local, _, _)| *local as usize == index) =>
                    {
                        SemanticLocalRoleV1::Argument(0)
                    }
                    _ => SemanticLocalRoleV1::Temporary,
                },
                source,
            )
        })
        .collect();
    let Some((_, argument, ownership)) = argument else {
        return owner_from_parts(types, locals, 0, blocks, b"retained_array");
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(60)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(argument)],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![ownership])
    .unwrap();
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(60)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(60)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(60)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(60)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(60)),
        source,
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"retained_array_with_input".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(61)),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
        .unwrap()
}

fn one_block(statements: Vec<SemanticStatementV1>) -> ProductionSemanticMirOwnerV1 {
    fixture(
        false,
        8,
        vec![block(200, statements, SemanticTerminatorKindV1::Return)],
    )
}

fn lower(
    owner: ProductionSemanticMirOwnerV1,
) -> Result<ProductionSemanticKirOwnerV1, ProductionSemanticKirErrorV1> {
    ProductionSemanticKirOwnerV1::try_lower(owner, ProductionSemanticKirLimitsV1::default())
}

fn array_slot(body: &fe2o3_kernel_ir::FunctionBody, pointer_elements: bool) -> ValueId {
    let expected = if pointer_elements {
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        )
    } else {
        Type::Scalar(ScalarType::U32)
    };
    body.blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            OperationKind::Alloca {
                element,
                count: Some(_),
                address_space: AddressSpace::Private,
                ..
            } if element == &expected => Some(operation.results[0].id),
            _ => None,
        })
        .expect("one counted, typed private allocation")
}

fn assert_partial_read(error: ProductionSemanticKirErrorV1) {
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "retained array read requires whole-array initialization",
                ..
            }
        ),
        "{error}"
    );
}

#[test]
fn partial_write_materializes_counted_private_storage_without_reading_it() {
    let source = one_block(vec![index(7), element_write(1, false)]);
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[ProductionSourceLaunchRootInputV1::new(
            "retained_array",
            bytes(61),
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut work,
        512 * 1024 * 1024,
    );
    let owner = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    let storage = owner.executable_storage().retained_storage()
        + owner.assert_origin_storage().payload_storage();
    budget.reserve_storage(storage).unwrap();
    let module = owner.executable().module();
    verify_module(module).unwrap();
    let body = module.functions[0].body.as_ref().unwrap();
    let slot = array_slot(body, false);
    let operations = &body.blocks[0].operations;
    let count = operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::Alloca {
                count: Some(count), ..
            } => Some(count),
            _ => None,
        })
        .unwrap();
    assert!(operations.iter().any(|operation| {
        operation
            .results
            .first()
            .is_some_and(|value| value.id == count)
            && matches!(operation.kind, OperationKind::Constant(Constant::Index(8)))
    }));
    let (pointer, offset) = operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::GetElementPointer { base, offset } if base == slot => {
                Some((operation.results[0].id, offset))
            }
            _ => None,
        })
        .unwrap();
    assert!(operations.iter().any(|operation| {
        operation
            .results
            .first()
            .is_some_and(|value| value.id == offset)
            && matches!(operation.kind, OperationKind::Constant(Constant::Index(7)))
    }));
    assert!(operations.iter().any(|operation| {
        matches!(operation.kind, OperationKind::Store { pointer: address, .. } if address == pointer)
    }));
    assert!(
        !operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::Load { .. }))
    );
    drop(owner);
    budget.release_storage(storage).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn initialized_array_is_reloaded_after_an_element_overwrite_for_constant_and_from_end_indices() {
    let lowered = lower(one_block(vec![
        initialize(1, false),
        index(7),
        element_write(1, false),
        read(indexed(1, SCALAR)),
        read(fixed(1, 1, true, SCALAR)),
    ]))
    .unwrap();
    lowered.verify_equivalence().unwrap();
    let body = lowered.module().functions[0].body.as_ref().unwrap();
    let slot = array_slot(body, false);
    let pointers = body.blocks[0]
        .operations
        .iter()
        .filter(|operation| {
            matches!(operation.kind, OperationKind::GetElementPointer { base, .. } if base == slot)
        })
        .map(|operation| operation.results[0].id)
        .collect::<std::collections::BTreeSet<_>>();
    let accesses = body.blocks[0]
        .operations
        .iter()
        .filter_map(|operation| match operation.kind {
            OperationKind::Store { pointer, .. } if pointers.contains(&pointer) => Some("store"),
            OperationKind::Load { pointer, .. } if pointers.contains(&pointer) => Some("load"),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        accesses,
        [
            "store", "store", "store", "store", "store", "store", "store", "store", "store",
            "load", "load",
        ]
    );
}

#[test]
fn partial_writes_do_not_establish_whole_array_initialization_even_at_the_written_index() {
    for place in [indexed(1, SCALAR), fixed(1, 7, false, SCALAR)] {
        assert_partial_read(
            lower(one_block(vec![
                index(7),
                element_write(1, false),
                read(place),
            ]))
            .unwrap_err(),
        );
    }
}

#[test]
fn projected_moves_and_deinitialization_kill_initialization_locally_and_across_edges() {
    for across_edge in [false, true] {
        for moved in [false, true] {
            let invalidation = if moved {
                assign(
                    whole(3, SCALAR),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(fixed(1, 0, false, SCALAR))),
                )
            } else {
                st(SemanticStatementKindV1::Deinitialize(fixed(
                    1, 0, false, SCALAR,
                )))
            };
            let mut before = vec![
                initialize(1, false),
                index(1),
                assign(
                    fixed(1, 1, false, SCALAR),
                    SemanticRvalueKindV1::Use(number(99)),
                ),
                invalidation,
            ];
            // Constant projections satisfy the earlier partial-move checker,
            // while the projected write still requires retained array storage.
            let prefix = lower(fixture(
                false,
                8,
                vec![block(201, before.clone(), SemanticTerminatorKindV1::Return)],
            ))
            .unwrap();
            prefix.verify_equivalence().unwrap();
            array_slot(prefix.module().functions[0].body.as_ref().unwrap(), false);
            let blocks = if across_edge {
                vec![
                    block(
                        201,
                        before,
                        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::Goto,
                            SemanticBlockIdV1::from_index(1),
                        )),
                    ),
                    block(
                        202,
                        vec![read(fixed(1, 1, false, SCALAR))],
                        SemanticTerminatorKindV1::Return,
                    ),
                ]
            } else {
                before.push(read(fixed(1, 1, false, SCALAR)));
                vec![block(201, before, SemanticTerminatorKindV1::Return)]
            };
            assert_partial_read(lower(fixture(false, 8, blocks)).unwrap_err());
        }
    }
}

fn after_projected_move(
    suffix: Vec<SemanticStatementV1>,
    across_edge: bool,
) -> ProductionSemanticMirOwnerV1 {
    let mut before = vec![
        initialize(1, false),
        index(1),
        element_write(1, false),
        assign(
            whole(3, SCALAR),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(fixed(1, 0, false, SCALAR))),
        ),
    ];
    if across_edge {
        fixture(
            false,
            8,
            vec![
                block(
                    201,
                    before,
                    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::Goto,
                        SemanticBlockIdV1::from_index(1),
                    )),
                ),
                block(202, suffix, SemanticTerminatorKindV1::Return),
            ],
        )
    } else {
        before.extend(suffix);
        one_block(before)
    }
}

#[test]
fn projected_move_after_dynamic_write_loads_initialized_storage() {
    let lowered = lower(after_projected_move(vec![], false)).unwrap();
    lowered.verify_equivalence().unwrap();
    let body = lowered.module().functions[0].body.as_ref().unwrap();
    let slot = array_slot(body, false);
    let operations = &body.blocks[0].operations;
    let loads = operations
        .iter()
        .filter_map(|operation| match operation.kind {
            OperationKind::Load { pointer, .. } => Some(pointer),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [pointer] = loads.as_slice() else {
        panic!("the final projected Move must load exactly one retained element");
    };
    let offset = operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::GetElementPointer { base, offset }
                if base == slot && operation.results[0].id == *pointer =>
            {
                Some(offset)
            }
            _ => None,
        })
        .expect("the move reads the retained array allocation");
    assert!(operations.iter().any(|operation| {
        operation
            .results
            .first()
            .is_some_and(|result| result.id == offset)
            && matches!(operation.kind, OperationKind::Constant(Constant::Index(0)))
    }));
}

#[test]
fn projected_move_invalidates_subsequent_reads_and_moves_in_blocks_and_across_edges() {
    for across_edge in [false, true] {
        for offset in [0, 1] {
            for moved in [false, true] {
                let place = fixed(1, offset, false, SCALAR);
                let operand = if moved {
                    SemanticOperandV1::Move(place)
                } else {
                    SemanticOperandV1::Copy(place)
                };
                let error = lower(after_projected_move(
                    vec![assign(whole(3, SCALAR), SemanticRvalueKindV1::Use(operand))],
                    across_edge,
                ))
                .unwrap_err();
                if offset == 0 {
                    assert!(
                        matches!(
                            error,
                            ProductionSemanticKirErrorV1::SemanticSsa(
                                fe2o3_pliron::ProductionSemanticSsaErrorV1::PartialMove {
                                    block,
                                    statement,
                                    local: 1,
                                    violation: fe2o3_pliron::SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                                    ..
                                }
                            ) if block == u32::from(across_edge)
                                && statement == Some(if across_edge { 0 } else { 4 })
                        ),
                        "{error}"
                    );
                } else {
                    // SSA permits this disjoint path, but the retained array's
                    // conservative whole-initialization fact was consumed.
                    assert_partial_read(error);
                }
            }
        }
    }
}

#[test]
fn whole_reinitialization_restores_the_moved_array_on_each_edge() {
    for across_edge in [false, true] {
        let lowered = lower(after_projected_move(
            vec![
                initialize(1, false),
                read(fixed(1, 0, false, SCALAR)),
                assign(
                    whole(3, SCALAR),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(fixed(1, 1, false, SCALAR))),
                ),
            ],
            across_edge,
        ))
        .unwrap();
        lowered.verify_equivalence().unwrap();
        array_slot(lowered.module().functions[0].body.as_ref().unwrap(), false);
    }
}

#[test]
fn storage_dead_and_whole_moves_invalidate_arrays() {
    for invalidation in [
        st(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(1),
        )),
        assign(
            whole(4, ARRAY),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(whole(1, ARRAY))),
        ),
    ] {
        assert_partial_read(
            lower(one_block(vec![
                initialize(1, false),
                index(7),
                element_write(1, false),
                invalidation,
                read(fixed(1, 7, false, SCALAR)),
            ]))
            .unwrap_err(),
        );
    }
}

#[test]
fn whole_array_copy_and_self_move_load_before_destination_initialization() {
    let lowered = lower(one_block(vec![
        initialize(1, false),
        index(7),
        element_write(1, false),
        element_write(4, false),
        assign(
            whole(4, ARRAY),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(whole(1, ARRAY))),
        ),
        assign(
            whole(1, ARRAY),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(whole(1, ARRAY))),
        ),
        read(fixed(1, 7, false, SCALAR)),
        read(fixed(4, 7, false, SCALAR)),
    ]))
    .unwrap();
    lowered.verify_equivalence().unwrap();
    assert_partial_read(
        lower(one_block(vec![
            index(7),
            element_write(1, false),
            assign(
                whole(1, ARRAY),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(whole(1, ARRAY))),
            ),
        ]))
        .unwrap_err(),
    );
}

#[test]
fn whole_array_bytes_and_repeated_scalar_aggregate_share_private_storage() {
    for value in [
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Array, vec![number(11); 8])
                .unwrap(),
        ),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
            ARRAY,
            SemanticConstantValueV1::Bytes(
                SemanticConstantBytesV1::new([11_u32.to_le_bytes(); 8].concat()).unwrap(),
            ),
        ))),
    ] {
        let lowered = lower(one_block(vec![
            assign(whole(1, ARRAY), value),
            index(7),
            element_write(1, false),
            read(indexed(1, SCALAR)),
        ]))
        .unwrap();
        lowered.verify_equivalence().unwrap();
    }
}

#[test]
fn pointer_elements_retain_their_exact_pointee_address_space_and_access() {
    let lowered = lower(fixture(
        true,
        8,
        vec![block(
            203,
            vec![
                initialize(1, true),
                index(7),
                element_write(1, true),
                read(indexed(1, POINTER)),
            ],
            SemanticTerminatorKindV1::Return,
        )],
    ))
    .unwrap();
    lowered.verify_equivalence().unwrap();
    let body = lowered.module().functions[0].body.as_ref().unwrap();
    let slot = array_slot(body, true);
    assert!(body.blocks[0].operations.iter().any(|operation| {
        matches!(operation.kind, OperationKind::GetElementPointer { base, .. } if base == slot)
    }));
    assert!(body.blocks[0].operations.iter().any(|operation| {
        matches!(operation.kind, OperationKind::Load { .. })
            && operation.results[0].ty
                == Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                )
    }));
}

#[test]
fn retained_array_borrows_and_raw_addresses_fail_explicitly_even_after_full_initialization() {
    for (destination, value) in [
        (
            whole(7, REFERENCE),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: whole(1, ARRAY),
            },
        ),
        (
            whole(8, RAW_ARRAY),
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: whole(1, ARRAY),
            },
        ),
    ] {
        let error = lower(one_block(vec![
            initialize(1, false),
            assign(destination, value),
        ]))
        .unwrap_err();
        assert!(
            matches!(
                error,
                ProductionSemanticKirErrorV1::Unsupported {
                    detail: "retained array borrow or address-of requires tracked alias initialization",
                    ..
                }
            ),
            "{error}"
        );
    }
}

#[test]
fn exact_constant_index_overflow_is_rejected_without_a_wrapped_gep() {
    for value in [8, u32::MAX] {
        let error = lower(one_block(vec![index(value), element_write(1, false)])).unwrap_err();
        assert!(
            matches!(
                error,
                ProductionSemanticKirErrorV1::Unsupported {
                    detail: "retained array constant index is out of range",
                    ..
                }
            ),
            "{error}"
        );
    }
}

#[test]
fn retained_memory_array_can_exceed_the_aggregate_ssa_component_limit() {
    let lowered = lower(fixture(
        false,
        512,
        vec![block(
            204,
            vec![index(511), element_write(1, false)],
            SemanticTerminatorKindV1::Return,
        )],
    ))
    .unwrap();
    lowered.verify_equivalence().unwrap();
    array_slot(lowered.module().functions[0].body.as_ref().unwrap(), false);
}

#[test]
fn dynamic_indices_use_ssa_values_and_preserve_source_bounds_control_flow() {
    let edge =
        |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
    let blocks = vec![
        block(
            205,
            vec![initialize(1, false), index(0), element_write(1, false)],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(whole(9, SCALAR)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        block(
            206,
            vec![index(1)],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        block(
            207,
            vec![index(2)],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
        ),
        block(
            208,
            vec![assign(
                whole(5, BOOLEAN),
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: SemanticOperandV1::Copy(whole(2, SCALAR)),
                    right: number(8),
                },
            )],
            SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Copy(whole(5, BOOLEAN)),
                expected: true,
                message: SemanticAssertMessageV1::BoundsCheck {
                    length: number(8),
                    index: SemanticOperandV1::Copy(whole(2, SCALAR)),
                },
                target: edge(SemanticEdgeRoleV1::AssertSuccess, 4),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        block(
            209,
            vec![read(indexed(1, SCALAR))],
            SemanticTerminatorKindV1::Return,
        ),
    ];
    let lowered = lower(fixture_with_argument(
        false,
        8,
        blocks,
        Some((
            9,
            direct_abi_value(SCALAR),
            SemanticSourceArgumentOwnershipV1::Unspecified,
        )),
    ))
    .unwrap();
    lowered.verify_equivalence().unwrap();
    let body = lowered.module().functions[0].body.as_ref().unwrap();
    let [runtime_control] = body.parameters.as_slice() else {
        panic!("fixture must retain the exact runtime scalar argument");
    };
    let entry = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(0))
        .unwrap();
    assert!(matches!(
        entry.terminator,
        Some(Terminator::Switch { selector, .. } | Terminator::IntegerSwitch { selector, .. })
            if selector == *runtime_control
    ));
    let access = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(4))
        .unwrap();
    let index_cast = access
        .operations
        .iter()
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::Cast {
                    kind: CastKind::ZeroExtend,
                    to: Type::Scalar(ScalarType::Index),
                    ..
                }
            )
        })
        .expect("a dynamic U32 SSA value is promoted exactly to Index");
    assert!(access.operations.iter().any(|operation| {
        matches!(operation.kind, OperationKind::GetElementPointer { offset, .. }
            if offset == index_cast.results[0].id)
    }));
    let check = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(3))
        .unwrap();
    assert!(matches!(
        check.terminator,
        Some(Terminator::ConditionalBranch { .. })
    ));
    assert!(
        body.blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| {
                operation.kind == AmdGpuDiagnosticOperation::Trap.operation(None).kind
            })
    );
}

#[test]
fn runtime_indexed_write_only_array_uses_existing_retained_storage_before_whole_copy() {
    let edge = |block| {
        SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::AssertSuccess,
            SemanticBlockIdV1::from_index(block),
        )
    };
    let blocks = vec![
        block(
            210,
            vec![
                initialize(1, false),
                assign(
                    whole(5, BOOLEAN),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: SemanticOperandV1::Copy(whole(2, SCALAR)),
                        right: number(8),
                    },
                ),
            ],
            SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Copy(whole(5, BOOLEAN)),
                expected: true,
                message: SemanticAssertMessageV1::BoundsCheck {
                    length: number(8),
                    index: SemanticOperandV1::Copy(whole(2, SCALAR)),
                },
                target: edge(1),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        block(
            211,
            vec![
                element_write(1, false),
                assign(
                    whole(4, ARRAY),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(whole(1, ARRAY))),
                ),
                read(fixed(4, 0, false, SCALAR)),
            ],
            SemanticTerminatorKindV1::Return,
        ),
    ];
    let lowered = lower(fixture_with_argument(
        false,
        8,
        blocks,
        Some((
            2,
            direct_abi_value(SCALAR),
            SemanticSourceArgumentOwnershipV1::ByValue,
        )),
    ))
    .unwrap();
    lowered.verify_equivalence().unwrap();
    let body = lowered.module().functions[0].body.as_ref().unwrap();
    let slot = array_slot(body, false);
    assert_eq!(
        body.blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(operation.kind, OperationKind::Alloca { count: Some(_), .. })
            })
            .count(),
        1,
        "only the written array needs storage; the copied value remains components"
    );
    let access = body
        .blocks
        .iter()
        .find(|block| block.id == BlockId(1))
        .unwrap();
    let dynamic_pointer = access
        .operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::GetElementPointer { base, offset } if base == slot => {
                let cast = access.operations.iter().find(|candidate| {
                    candidate.results.iter().any(|result| result.id == offset)
                        && matches!(
                            candidate.kind,
                            OperationKind::Cast {
                                kind: CastKind::ZeroExtend,
                                to: Type::Scalar(ScalarType::Index),
                                ..
                            }
                        )
                })?;
                Some((operation.results[0].id, cast.results[0].id))
            }
            _ => None,
        })
        .expect("the real runtime U32 index must be widened and used by the private GEP");
    let store = access
        .operations
        .iter()
        .position(|operation| {
            matches!(operation.kind, OperationKind::Store { pointer, .. }
            if pointer == dynamic_pointer.0)
        })
        .expect("the dynamic write remains an ordinary Store");
    let gather = access
        .operations
        .iter()
        .enumerate()
        .filter_map(|(ordinal, operation)| {
            matches!(operation.kind, OperationKind::Load { .. }).then_some(ordinal)
        })
        .collect::<Vec<_>>();
    assert_eq!(gather.len(), 8);
    assert!(
        gather.iter().all(|ordinal| *ordinal > store),
        "the whole-value copy must observe the preceding indexed write"
    );
    assert!(matches!(
        body.blocks
            .iter()
            .find(|block| block.id == BlockId(0))
            .unwrap()
            .terminator,
        Some(Terminator::ConditionalBranch { .. }),
    ));
}

#[test]
fn length_is_metadata_and_does_not_read_uninitialized_elements() {
    let lowered = lower(one_block(vec![
        index(7),
        element_write(1, false),
        assign(
            whole(3, SCALAR),
            SemanticRvalueKindV1::Length(whole(1, ARRAY)),
        ),
    ]))
    .unwrap();
    let operations = &lowered.module().functions[0].body.as_ref().unwrap().blocks[0].operations;
    assert!(
        operations.iter().any(|operation| {
            matches!(operation.kind, OperationKind::Constant(Constant::U32(8)))
        })
    );
    assert!(
        !operations
            .iter()
            .any(|operation| matches!(operation.kind, OperationKind::Load { .. }))
    );
}

#[test]
fn whole_array_volatile_load_and_store_reject_without_scalarizing_the_access_contract() {
    let load = assign(
        whole(4, ARRAY),
        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
            whole(1, ARRAY),
            SemanticVolatilityV1::Volatile,
            None,
        )),
    );
    let store = st(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
        whole(4, ARRAY),
        SemanticOperandV1::Copy(whole(1, ARRAY)),
        SemanticVolatilityV1::Volatile,
        None,
    )));
    for operation in [load, store] {
        let error = lower(one_block(vec![
            initialize(1, false),
            index(7),
            element_write(1, false),
            element_write(4, false),
            operation,
        ]))
        .unwrap_err();
        assert!(
            matches!(
                error,
                ProductionSemanticKirErrorV1::Unsupported {
                    detail: "retained whole-array volatile access requires an aggregate access contract",
                    ..
                }
            ),
            "{error}"
        );
    }
}

#[test]
fn retained_array_argument_rejection_respects_source_ownership_precedence() {
    for (ownership, expected_detail) in [
        (
            SemanticSourceArgumentOwnershipV1::Unspecified,
            "by-value kernel argument requires exact component lowering",
        ),
        (
            SemanticSourceArgumentOwnershipV1::ByValue,
            "retained by-value array arguments require source-effect-bound entry scatter",
        ),
    ] {
        // The exact indirect carrier alone does not establish source ownership.
        // Only ByValue reaches component planning and the retained-slot boundary.
        let attributes = SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(
                true,
                Some(SemanticAbiPointerCaptureV1::CapturesAddress),
                true,
                false,
                false,
                true,
            ),
            SemanticAbiExtensionV1::None,
            32,
            Some(4),
        )
        .unwrap();
        let argument = SemanticAbiValueV1::new(
            ARRAY,
            SemanticAbiPassModeV1::Indirect {
                attributes,
                metadata_attributes: None,
                on_stack: false,
            },
        );
        let source = fixture_with_argument(
            false,
            8,
            vec![block(
                210,
                vec![index(7), element_write(1, false)],
                SemanticTerminatorKindV1::Return,
            )],
            Some((1, argument, ownership)),
        );
        assert_eq!(
            source.semantic().functions()[0]
                .abi()
                .source_argument_ownership(),
            [ownership]
        );
        let error = lower(source).unwrap_err();
        assert!(
            matches!(
                error,
                ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail,
                } if detail == expected_detail
            ),
            "{ownership:?}: {error}"
        );
    }
}
