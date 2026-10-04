use super::source_storage_demands_v29::{ComponentStepV29, DemandKindV29, SourceStorageDemandsV29};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::ProductionSemanticMirLimitsV1;

pub(super) const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
pub(super) const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
pub(super) const REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
pub(super) const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
pub(super) const ARRAY: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
pub(super) const MARKER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
pub(super) const CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
pub(super) const MIXED: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
pub(super) const MIXED_ARRAY: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);

fn declaration(
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

fn aggregate(
    tag: u8,
    size: u64,
    align: u64,
    fields: Vec<SemanticTypeIdV1>,
    offsets: Vec<u64>,
) -> SemanticTypeDeclV1 {
    declaration(
        tag,
        SemanticTypeLayoutV1::aggregate(
            Some(size),
            align,
            SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(fields).unwrap()),
    )
}

fn array(tag: u8, element: SemanticTypeIdV1, stride: u64, align: u64) -> SemanticTypeDeclV1 {
    declaration(
        tag,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            stride * 2,
            align,
            SemanticFieldsShapeV1::array(stride, 2),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            align,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array { element, length: 2 },
    )
}

fn types(nominal: bool) -> Vec<SemanticTypeDeclV1> {
    let pointer = declaration(
        3,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u128::from(u64::MAX)),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                WORD,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                    4,
                    4,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    let mut types = vec![
        declaration(
            1,
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
        ),
        declaration(
            2,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 32, 4),
                    SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX)),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        pointer,
        aggregate(4, 16, 8, vec![REF, REF], vec![0, 8]),
        array(5, WORD, 4, 4),
    ];
    if nominal {
        types.push(declaration(
            6,
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        ));
        let context = declaration(
            7,
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                SemanticAggregateLayoutV1::new(vec![0; 5], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![MARKER; 5]).unwrap()),
        )
        .with_rust_type_kind(SemanticRustTypeKindV1::Execution(
            SemanticExecutionRoleV29::KernelContext,
        ));
        types.push(context);
        types.push(aggregate(8, 8, 8, vec![CONTEXT, REF], vec![0, 0]));
        types.push(array(9, MIXED, 8, 8));
    }
    types
}

fn source() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn assign(destination: SemanticPlaceV1, kind: SemanticRvalueKindV1) -> SemanticStatementV1 {
    let ty = destination.ty();
    SemanticStatementV1::new(
        source(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            destination,
            SemanticRvalueV1::new(ty, kind),
        )),
    )
}
fn word() -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        WORD,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 4).unwrap()),
    ))
}
fn return_unit() -> SemanticStatementV1 {
    assign(
        place(0, UNIT),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
            UNIT,
            SemanticConstantValueV1::ZeroSized,
        ))),
    )
}
fn block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([index; 32]),
        source(),
        statements,
        SemanticTerminatorV1::new(source(), kind),
    )
    .unwrap()
}

fn function(
    tag: u8,
    root: bool,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        if root {
            SemanticCanonAbiV1::GpuKernel
        } else {
            SemanticCanonAbiV1::Rust
        },
        if root {
            SemanticExternAbiV1::GpuKernel
        } else {
            SemanticExternAbiV1::Rust
        },
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        if root {
            SemanticFunctionRoleV1::KernelRoot
        } else {
            SemanticFunctionRoleV1::InternalHelper
        },
        SemanticItemDefinitionIdentityV1::from_sha256([tag + 1; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag + 2; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag + 3; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag + 4; 32]),
        source(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    if root {
        function.with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(format!("storage_demand_{tag}").into_bytes()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([tag; 32]),
            SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
        ))
    } else {
        function
    }
}

// Genuine admitted source. Unused holders deliberately test declaration-wide
// partial-state requirements; this census is not an initialized-read proof.
pub(super) fn owner(
    roots: usize,
    helpers: bool,
    nominal: bool,
    mixed_backing: bool,
) -> ProductionSemanticSsaOwnerV1 {
    owner_with_types(roots, helpers, nominal, mixed_backing, types(nominal))
}

fn owner_with_types(
    roots: usize,
    helpers: bool,
    nominal: bool,
    mixed_backing: bool,
    types: Vec<SemanticTypeDeclV1>,
) -> ProductionSemanticSsaOwnerV1 {
    let locals: Vec<_> = types
        .iter()
        .enumerate()
        .map(|(index, _)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([30 + index as u8; 32]),
                SemanticTypeIdV1::from_index(index as u32),
                if index == 0 {
                    SemanticLocalRoleV1::Return
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                source(),
            )
        })
        .collect();
    let mut statements = vec![
        assign(place(1, WORD), SemanticRvalueKindV1::Use(word())),
        assign(
            place(2, REF),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(1, WORD),
            },
        ),
        assign(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(4),
                vec![
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::ConstantIndex {
                            offset: 0,
                            minimum_length: 2,
                            from_end: false,
                        },
                        WORD,
                    )
                    .unwrap(),
                ],
                WORD,
            )
            .unwrap(),
            SemanticRvalueKindV1::Use(word()),
        ),
    ];
    if mixed_backing {
        statements.push(assign(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(7),
                vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), REF).unwrap()],
                REF,
            )
            .unwrap(),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2, REF))),
        ));
    }
    statements.push(return_unit());
    let mut functions = Vec::new();
    let helper = SemanticFunctionIdV1::from_index(roots as u32);
    for index in 0..roots {
        let blocks = if helpers {
            (0..3)
                .map(|ordinal| {
                    block(
                        80 + ordinal as u8,
                        if ordinal == 0 {
                            statements.clone()
                        } else {
                            vec![]
                        },
                        if ordinal == 2 {
                            SemanticTerminatorKindV1::Return
                        } else {
                            SemanticTerminatorKindV1::Call(
                                SemanticDirectCallV1::new_callable(
                                    SemanticCallableIdV1::from_index(helper.index()),
                                    vec![],
                                    Some(SemanticCallDestinationV1::new(
                                        place(0, UNIT),
                                        SemanticControlFlowEdgeV1::new(
                                            SemanticEdgeRoleV1::CallReturn,
                                            SemanticBlockIdV1::from_index(ordinal + 1),
                                        ),
                                    )),
                                    SemanticUnwindActionV1::Unreachable,
                                )
                                .unwrap(),
                            )
                        },
                    )
                })
                .collect()
        } else {
            vec![block(
                80,
                statements.clone(),
                SemanticTerminatorKindV1::Return,
            )]
        };
        functions.push(function(
            100 + index as u8 * 10,
            true,
            locals.clone(),
            blocks,
        ));
    }
    if helpers {
        functions.push(function(
            180,
            false,
            locals,
            vec![block(81, statements, SemanticTerminatorKindV1::Return)],
        ));
    }
    let callables = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect();
    let request = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        (0..roots)
            .map(|index| SemanticFunctionIdV1::from_index(index as u32))
            .collect(),
    )
    .unwrap();
    let admitted = if nominal {
        request.admit_exact_v29(SemanticMirLimitsV1::default())
    } else {
        request.admit_current_production(SemanticMirLimitsV1::default())
    }
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

pub(super) fn captured(
    owner: &mut ProductionSemanticSsaOwnerV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> usize {
    let receipt = owner
        .try_capture_occurrences_with_budget_v1(budget)
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    receipt.retained_storage()
}

#[test]
fn ordinary_source_retains_backing_and_partial_holder_demands() {
    let mut owner = owner(1, false, false, false);
    let original = *owner.source_semantic_sha256();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    let credit = captured(&mut owner, &mut budget);
    let demands = SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    let types = demands.types(&owner, &mut budget).unwrap();
    assert!(types.contains(&ARRAY));
    assert!(types.contains(&REF));
    assert!(types.contains(&PAIR));
    assert!(types.windows(2).all(|pair| pair[0] < pair[1]));
    let (rows, paths) = demands.requests(&owner, &mut budget).unwrap();
    assert!(
        rows.iter()
            .any(|row| row.ty == ARRAY && row.kind == DemandKindV29::WholeBackingCandidate)
    );
    assert!(
        rows.iter()
            .any(|row| row.ty == PAIR && row.kind == DemandKindV29::ReferenceHolder)
    );
    assert!(paths.is_empty());
    assert_eq!(owner.source_semantic_sha256(), &original);
    demands.discard(&mut budget).unwrap();
    assert_eq!(budget.storage(), credit);
    drop(owner);
    budget.release_storage(credit).unwrap();
}

#[test]
fn shared_helpers_and_roots_share_types_not_object_coordinates() {
    let mut owner = owner(2, true, false, false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(2_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 2_000_000);
    let credit = captured(&mut owner, &mut budget);
    let demands = SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    let types = demands.types(&owner, &mut budget).unwrap();
    assert_eq!(types.iter().filter(|&&ty| ty == ARRAY).count(), 1);
    let (rows, _) = demands.requests(&owner, &mut budget).unwrap();
    let helper_rows: Vec<_> = rows
        .iter()
        .filter(|row| {
            row.function.index() == 2
                && row.ty == ARRAY
                && row.kind == DemandKindV29::WholeBackingCandidate
        })
        .collect();
    assert_eq!(helper_rows.len(), 4);
    let coordinates: BTreeSet<_> = helper_rows
        .iter()
        .map(|row| (row.root.index(), row.instance.index(), row.local.index()))
        .collect();
    assert_eq!(coordinates.len(), 4);
    let flat_count = rows.len();
    let mut count = 0;
    for ordinal in 0..2 {
        let (root_rows, _) = demands.root_requests(&owner, ordinal, &mut budget).unwrap();
        assert!(!root_rows.is_empty());
        assert!(
            root_rows
                .iter()
                .all(|row| row.root.index() as usize == ordinal)
        );
        assert_eq!(
            root_rows
                .iter()
                .filter(|row| row.function.index() == 2
                    && row.ty == ARRAY
                    && row.kind == DemandKindV29::WholeBackingCandidate)
                .count(),
            2
        );
        count += root_rows.len();
    }
    assert_eq!(count, flat_count);
    assert!(demands.root_requests(&owner, 2, &mut budget).is_err());
    demands.discard(&mut budget).unwrap();
    assert_eq!(budget.storage(), credit);
    drop(owner);
    budget.release_storage(credit).unwrap();
}

#[test]
fn mixed_nominal_holders_retain_exact_component_schemas_not_whole_backings() {
    let mut owner = owner(1, false, true, false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(2_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 2_000_000);
    let credit = captured(&mut owner, &mut budget);
    let demands = SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    let types = demands.types(&owner, &mut budget).unwrap();
    for excluded in [CONTEXT, MIXED, MIXED_ARRAY] {
        assert!(!types.contains(&excluded));
    }
    let (rows, paths) = demands.requests(&owner, &mut budget).unwrap();
    let pair = rows
        .iter()
        .find(|row| row.local.index() == MIXED.index())
        .unwrap();
    assert_eq!((pair.ty, pair.kind), (REF, DemandKindV29::ReferenceHolder));
    assert_eq!(&paths[pair.path.clone()], &[ComponentStepV29::Field(1)]);
    let array = rows
        .iter()
        .find(|row| row.local.index() == MIXED_ARRAY.index())
        .unwrap();
    assert_eq!(
        &paths[array.path.clone()],
        &[ComponentStepV29::Element, ComponentStepV29::Field(1)]
    );
    assert!(!rows.iter().any(|row| row.local.index() == CONTEXT.index()));
    demands.discard(&mut budget).unwrap();
    drop(owner);
    budget.release_storage(credit).unwrap();
}

#[test]
fn nominal_backing_candidates_stay_unresolved_until_the_actual_c1_join() {
    let mut owner = owner(1, false, true, true);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(2_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 2_000_000);
    let credit = captured(&mut owner, &mut budget);
    let demands = SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    assert!(!demands.types(&owner, &mut budget).unwrap().contains(&MIXED));
    let (rows, paths) = demands.requests(&owner, &mut budget).unwrap();
    assert!(rows.iter().any(|row| row.local.index() == MIXED.index()
        && row.ty == MIXED
        && row.kind == DemandKindV29::UnresolvedNominalBacking
        && row.path.is_empty()));
    assert!(rows.iter().any(|row| row.local.index() == MIXED.index()
        && row.ty == REF
        && row.kind == DemandKindV29::ReferenceHolder
        && paths[row.path.clone()] == [ComponentStepV29::Field(1)]));
    demands.discard(&mut budget).unwrap();
    assert_eq!(budget.storage(), credit);
    drop(owner);
    budget.release_storage(credit).unwrap();
}

#[test]
fn absent_original_occurrences_are_not_fabricated_by_the_census() {
    let owner = owner(1, false, false, false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(2_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 2_000_000);
    budget.reserve_storage(17).unwrap();
    assert!(SourceStorageDemandsV29::collect(&owner, &mut budget).is_err());
    assert_eq!(budget.storage(), 17);
    assert!(owner.occurrences_v1().is_none());
}

#[test]
fn recursive_pointer_layout_dependencies_terminate_without_following_reference_values() {
    let node = SemanticTypeIdV1::from_index(5);
    let pointer = SemanticTypeIdV1::from_index(6);
    let mut declarations = types(false);
    declarations.push(aggregate(20, 16, 8, vec![pointer, REF], vec![0, 8]));
    declarations.push(declaration(
        21,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                node,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let mut owner = owner_with_types(1, false, false, false, declarations);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(2_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 2_000_000);
    let credit = captured(&mut owner, &mut budget);
    let demands = SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    assert!(demands.types(&owner, &mut budget).unwrap().contains(&node));
    let (rows, _) = demands.requests(&owner, &mut budget).unwrap();
    assert!(
        rows.iter()
            .any(|row| row.ty == node && row.kind == DemandKindV29::ReferenceHolder)
    );
    assert!(
        !rows
            .iter()
            .any(|row| row.ty == pointer && row.kind == DemandKindV29::ReferenceHolder)
    );
    demands.discard(&mut budget).unwrap();
    drop(owner);
    budget.release_storage(credit).unwrap();
}

#[test]
fn ordinary_storage_free_source_has_an_authenticated_empty_census() {
    let unit = types(false).remove(0);
    let local = SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([30; 32]),
        UNIT,
        SemanticLocalRoleV1::Return,
        source(),
    );
    let function = function(
        100,
        true,
        vec![local],
        vec![block(
            80,
            vec![return_unit()],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let root = SemanticFunctionIdV1::from_index(0);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        vec![unit],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticCallableDeclV1::defined(root)],
        vec![root],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let mut owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(2_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 2_000_000);
    let credit = captured(&mut owner, &mut budget);
    let demands = SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    assert!(demands.types(&owner, &mut budget).unwrap().is_empty());
    assert!(demands.requests(&owner, &mut budget).unwrap().0.is_empty());
    assert!(
        demands
            .root_requests(&owner, 0, &mut budget)
            .unwrap()
            .0
            .is_empty()
    );
    assert!(demands.root_requests(&owner, 1, &mut budget).is_err());
    let ranges = std::mem::size_of::<source_storage_demands_v29::RootDemandRangeV29>();
    assert_eq!(budget.storage(), credit + ranges);
    assert!(budget.peak_storage() > credit + ranges);
    demands.discard(&mut budget).unwrap();
    assert_eq!(budget.storage(), credit);
    drop(owner);
    budget.release_storage(credit).unwrap();
}

#[test]
fn enum_variants_and_union_fields_keep_distinct_source_component_paths() {
    let mut declarations = types(true);
    let enumeration = SemanticTypeIdV1::from_index(declarations.len() as u32);
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX)),
    );
    let variants = (0..2)
        .map(|index| {
            SemanticEnumVariantLayoutV1::from_rustc(
                index,
                16,
                8,
                SemanticFieldsShapeV1::arbitrary(vec![8, 8], vec![0, 1]).unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                8,
                100 + u64::from(index),
                SemanticAggregateLayoutV1::new(
                    vec![8, 8],
                    vec![SemanticPaddingV1::new(4, 4).unwrap()],
                )
                .unwrap(),
            )
            .unwrap()
        })
        .collect();
    declarations.push(declaration(
        22,
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            16,
            8,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticEnumLayoutV1::new(
                variants,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            WORD,
            (0..2)
                .map(|index| {
                    SemanticEnumVariantV1::new(
                        index,
                        SemanticAggregateTypeV1::new(vec![CONTEXT, REF]).unwrap(),
                    )
                })
                .collect(),
        )
        .unwrap(),
    ));
    let union = SemanticTypeIdV1::from_index(declarations.len() as u32);
    declarations.push(declaration(
        23,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            8,
            8,
            SemanticFieldsShapeV1::Union { field_count: 2 },
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            8,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Union(SemanticAggregateTypeV1::new(vec![CONTEXT, REF]).unwrap()),
    ));
    let mut owner = owner_with_types(1, false, true, false, declarations);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(2_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 2_000_000);
    let credit = captured(&mut owner, &mut budget);
    let demands = SourceStorageDemandsV29::collect(&owner, &mut budget).unwrap();
    let (rows, paths) = demands.requests(&owner, &mut budget).unwrap();
    for variant in 0..2 {
        assert!(
            rows.iter()
                .any(|row| row.local.index() == enumeration.index()
                    && row.ty == REF
                    && row.kind == DemandKindV29::ReferenceHolder
                    && paths[row.path.clone()]
                        == [
                            ComponentStepV29::Variant(variant),
                            ComponentStepV29::Field(1)
                        ])
        );
    }
    assert!(rows.iter().any(|row| row.local.index() == union.index()
        && row.ty == REF
        && paths[row.path.clone()] == [ComponentStepV29::Field(1)]));
    assert!(
        !demands
            .types(&owner, &mut budget)
            .unwrap()
            .contains(&enumeration)
    );
    assert!(!demands.types(&owner, &mut budget).unwrap().contains(&union));
    demands.discard(&mut budget).unwrap();
    drop(owner);
    budget.release_storage(credit).unwrap();
}
