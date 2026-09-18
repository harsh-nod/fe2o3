use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateLayoutV1, SemanticAggregateTypeV1, SemanticLayoutIdentityV1,
    SemanticPointerTypeV1, SemanticRustTypeKindV1, SemanticTypeLayoutV1,
};

pub(super) const CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
pub(super) const OTHER_CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
pub(super) const WORKGROUP: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
pub(super) const TILE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(10);
pub(super) const FRAGMENT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(11);
const OTHER_FRAGMENT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(12);
pub(super) const MUT_CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(14);
pub(super) const SHARED_CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(15);
pub(super) const RAW_CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(16);
pub(super) const OTHER_MUT_CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(17);

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
    fields: &[u32],
    offsets: &[u64],
    size: u64,
    align: u64,
) -> SemanticTypeDeclV1 {
    declaration(
        tag,
        SemanticTypeLayoutV1::aggregate(
            Some(size),
            align,
            SemanticAggregateLayoutV1::new(offsets.to_vec(), vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(
                fields
                    .iter()
                    .copied()
                    .map(SemanticTypeIdV1::from_index)
                    .collect(),
            )
            .unwrap(),
        ),
    )
}

// Inert declarations with the carrier field shapes. These consistency tests do
// not mint a source owner or establish producer authenticity from nominal tags.
pub(super) fn types() -> Vec<SemanticTypeDeclV1> {
    let scalar = |tag, bits, size| {
        declaration(
            tag,
            SemanticTypeLayoutV1::new(Some(size), size).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits,
            }),
        )
    };
    let marker = aggregate(3, &[], &[], 0, 1);
    let context = |tag| {
        aggregate(tag, &[3; 5], &[0; 5], 0, 1).with_rust_type_kind(
            SemanticRustTypeKindV1::Execution(SemanticExecutionRoleV29::KernelContext),
        )
    };
    let carrier = |tag, role| {
        aggregate(tag, &[8, 9, 3, 3], &[0, 8, 10, 10], 12, 4)
            .with_rust_type_kind(SemanticRustTypeKindV1::Execution(role))
    };
    let reference = |tag, pointee, kind, mutability| {
        declaration(
            tag,
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    pointee,
                    kind,
                    mutability,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
    };
    vec![
        scalar(0, 64, 8),
        declaration(
            1,
            SemanticTypeLayoutV1::new(Some(1), 1).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        ),
        scalar(2, 32, 4),
        marker,
        aggregate(4, &[3; 3], &[0; 3], 0, 1),
        context(5),
        context(6),
        aggregate(7, &[0, 0, 4, 3], &[0, 8, 16, 16], 16, 8).with_rust_type_kind(
            SemanticRustTypeKindV1::Execution(SemanticExecutionRoleV29::Workgroup),
        ),
        declaration(
            8,
            SemanticTypeLayoutV1::new(Some(8), 4).unwrap(),
            SemanticTypeShapeV1::Array {
                element: SemanticTypeIdV1::from_index(2),
                length: 2,
            },
        ),
        declaration(
            9,
            SemanticTypeLayoutV1::new(Some(2), 1).unwrap(),
            SemanticTypeShapeV1::Array {
                element: SemanticTypeIdV1::from_index(1),
                length: 2,
            },
        ),
        carrier(
            10,
            SemanticExecutionRoleV29::MaskedTileU32 {
                lanes: 4,
                elements: 2,
            },
        ),
        carrier(
            11,
            SemanticExecutionRoleV29::LaneFragmentU32 {
                lanes: 4,
                elements: 2,
            },
        ),
        carrier(
            12,
            SemanticExecutionRoleV29::LaneFragmentU32 {
                lanes: 8,
                elements: 2,
            },
        ),
        declaration(
            13,
            SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        reference(
            14,
            CONTEXT,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
        ),
        reference(
            15,
            CONTEXT,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
        ),
        reference(
            16,
            CONTEXT,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Mutable,
        ),
        reference(
            17,
            OTHER_CONTEXT,
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
        ),
    ]
}

fn occurrence(instance: usize, block: u32) -> ProductionCallOccurrenceV1 {
    ProductionCallOccurrenceV1 {
        caller: ProductionCallInstanceIdV1(instance),
        block: SemanticBlockIdV1::from_index(block),
    }
}

fn context(types: &[SemanticTypeDeclV1]) -> SemanticExecutionBindingV29 {
    SemanticExecutionBindingV29::context(types, CONTEXT, occurrence(0, 1), ValueId(10)).unwrap()
}

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn borrow(
    types: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
    kind: SemanticBorrowKindV1,
    binding: &SemanticExecutionBindingV29,
) -> Result<SemanticExecutionBorrowBindingV29, &'static str> {
    let destination = place(3, reference);
    let source = place(2, binding.semantic_type());
    SemanticExecutionBorrowBindingV29::from_source(
        types,
        SemanticExecutionBorrowSourceV29 {
            instance: ProductionCallInstanceIdV1(1),
            block: SemanticBlockIdV1::from_index(2),
            statement: 3,
            destination: &destination,
            kind,
            source: &source,
        },
        binding,
    )
}

#[test]
fn execution_chain_preserves_exact_producer_and_parent_identities() {
    let types = types();
    let context = context(&types);
    let group = SemanticExecutionBindingV29::workgroup(
        &types,
        WORKGROUP,
        occurrence(1, 0),
        ValueId(11),
        &context,
    )
    .unwrap();
    let tile =
        SemanticExecutionBindingV29::tile(&types, TILE, occurrence(1, 1), ValueId(12), &group)
            .unwrap();
    let fragment = SemanticExecutionBindingV29::fragment(
        &types,
        FRAGMENT,
        occurrence(1, 2),
        ValueId(13),
        &tile,
    )
    .unwrap();
    for (binding, ty, role, value, producer) in [
        (
            &context,
            CONTEXT,
            SemanticExecutionRoleV29::KernelContext,
            10,
            occurrence(0, 1),
        ),
        (
            &group,
            WORKGROUP,
            SemanticExecutionRoleV29::Workgroup,
            11,
            occurrence(1, 0),
        ),
        (
            &tile,
            TILE,
            SemanticExecutionRoleV29::MaskedTileU32 {
                lanes: 4,
                elements: 2,
            },
            12,
            occurrence(1, 1),
        ),
        (
            &fragment,
            FRAGMENT,
            SemanticExecutionRoleV29::LaneFragmentU32 {
                lanes: 4,
                elements: 2,
            },
            13,
            occurrence(1, 2),
        ),
    ] {
        assert_eq!(binding.semantic_type(), ty);
        assert_eq!(binding.role(), role);
        assert_eq!(binding.value(), ValueId(value));
        assert_eq!(binding.producer(), producer);
        assert_eq!(binding.context_identity(), context.identity);
        assert_eq!(
            binding.kir_type().unwrap(),
            Type::Execution(semantic_execution_kir_role_v29(role).unwrap())
        );
        binding.check_type(&types, ty).unwrap();
    }
    assert_eq!(context.workgroup_identity(), None);
    for binding in [&group, &tile, &fragment] {
        assert_eq!(binding.workgroup_identity(), Some(group.identity));
    }
    let another = SemanticExecutionBindingV29::workgroup(
        &types,
        WORKGROUP,
        occurrence(2, 0),
        ValueId(11),
        &context,
    )
    .unwrap();
    assert_ne!(another.identity, group.identity);
}

#[test]
fn execution_binding_rejects_role_geometry_and_nominal_substitution() {
    let mut types = types();
    let context = context(&types);
    let group = SemanticExecutionBindingV29::workgroup(
        &types,
        WORKGROUP,
        occurrence(1, 0),
        ValueId(11),
        &context,
    )
    .unwrap();
    let tile =
        SemanticExecutionBindingV29::tile(&types, TILE, occurrence(1, 1), ValueId(12), &group)
            .unwrap();
    assert!(context.check_type(&types, OTHER_CONTEXT).is_err());
    assert!(
        SemanticExecutionBindingV29::context(&types, WORKGROUP, occurrence(0, 0), ValueId(1))
            .is_err()
    );
    assert!(
        SemanticExecutionBindingV29::context(
            &types,
            SemanticTypeIdV1::from_index(13),
            occurrence(0, 0),
            ValueId(1)
        )
        .is_err()
    );
    assert!(
        SemanticExecutionBindingV29::context(
            &types,
            SemanticTypeIdV1::from_index(u32::MAX),
            occurrence(0, 0),
            ValueId(1)
        )
        .is_err()
    );
    assert!(
        SemanticExecutionBindingV29::workgroup(
            &types,
            WORKGROUP,
            occurrence(1, 0),
            ValueId(2),
            &group
        )
        .is_err()
    );
    assert!(
        SemanticExecutionBindingV29::tile(&types, TILE, occurrence(1, 1), ValueId(3), &context)
            .is_err()
    );
    assert!(
        SemanticExecutionBindingV29::fragment(
            &types,
            OTHER_FRAGMENT,
            occurrence(1, 2),
            ValueId(4),
            &tile
        )
        .is_err()
    );
    types[TILE.index() as usize] = types[TILE.index() as usize].clone().with_rust_type_kind(
        SemanticRustTypeKindV1::Execution(SemanticExecutionRoleV29::MaskedTileU32 {
            lanes: 0,
            elements: 2,
        }),
    );
    assert!(
        SemanticExecutionBindingV29::tile(&types, TILE, occurrence(1, 1), ValueId(3), &group)
            .is_err()
    );
    types[CONTEXT.index() as usize] = types[OTHER_CONTEXT.index() as usize].clone();
    assert!(context.check_type(&types, CONTEXT).is_err());
    assert!(
        SemanticExecutionBindingV29::workgroup(
            &types,
            WORKGROUP,
            occurrence(1, 0),
            ValueId(2),
            &context
        )
        .is_err()
    );
}

#[test]
fn execution_borrow_preserves_source_occurrence_and_exact_reference() {
    let types = types();
    let context = context(&types);
    for (reference_type, kind) in [
        (MUT_CONTEXT, SemanticBorrowKindV1::Mutable),
        (SHARED_CONTEXT, SemanticBorrowKindV1::Shared),
    ] {
        let binding = borrow(&types, reference_type, kind, &context).unwrap();
        assert_eq!(binding.reference_type(), reference_type);
        assert_eq!(binding.kind(), kind);
        assert_eq!(binding.source_local(), SemanticLocalIdV1::from_index(2));
        assert_eq!(
            binding.destination_local(),
            SemanticLocalIdV1::from_index(3)
        );
        assert_eq!(
            binding.occurrence(),
            SemanticExecutionBorrowOccurrenceV29 {
                instance: ProductionCallInstanceIdV1(1),
                block: SemanticBlockIdV1::from_index(2),
                statement: 3
            }
        );
        assert_eq!(binding.borrowed(), &context);
        let projection =
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, CONTEXT).unwrap();
        assert_eq!(
            binding
                .dereference(&types, reference_type, &projection)
                .unwrap(),
            &context
        );
        let wrong = SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, OTHER_CONTEXT)
            .unwrap();
        assert!(binding.dereference(&types, reference_type, &wrong).is_err());
        let wrong = SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), CONTEXT).unwrap();
        assert!(binding.dereference(&types, reference_type, &wrong).is_err());
        assert!(binding.check_type(&types, OTHER_MUT_CONTEXT).is_err());
    }
}

#[test]
fn execution_borrow_refuses_raw_fake_mutability_and_projected_sources() {
    let types = types();
    let context = context(&types);
    for (reference, kind) in [
        (RAW_CONTEXT, SemanticBorrowKindV1::Mutable),
        (MUT_CONTEXT, SemanticBorrowKindV1::Shared),
        (SHARED_CONTEXT, SemanticBorrowKindV1::Mutable),
        (MUT_CONTEXT, SemanticBorrowKindV1::Fake),
        (OTHER_MUT_CONTEXT, SemanticBorrowKindV1::Mutable),
        (CONTEXT, SemanticBorrowKindV1::Mutable),
    ] {
        assert!(borrow(&types, reference, kind, &context).is_err());
    }
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(2),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), CONTEXT).unwrap()],
        CONTEXT,
    )
    .unwrap();
    let destination = place(3, MUT_CONTEXT);
    assert!(
        SemanticExecutionBorrowBindingV29::from_source(
            &types,
            SemanticExecutionBorrowSourceV29 {
                instance: ProductionCallInstanceIdV1(0),
                block: SemanticBlockIdV1::from_index(0),
                statement: 0,
                destination: &destination,
                kind: SemanticBorrowKindV1::Mutable,
                source: &projected,
            },
            &context
        )
        .is_err()
    );
    let mut substituted = types.clone();
    substituted[MUT_CONTEXT.index() as usize] = types[OTHER_MUT_CONTEXT.index() as usize].clone();
    assert!(
        borrow(&types, MUT_CONTEXT, SemanticBorrowKindV1::Mutable, &context)
            .unwrap()
            .check_type(&substituted, MUT_CONTEXT)
            .is_err()
    );
}

#[test]
fn execution_bindings_cannot_flatten_or_resurrect_through_enums() {
    let types = types();
    let context = context(&types);
    let borrowed = borrow(&types, MUT_CONTEXT, SemanticBorrowKindV1::Mutable, &context).unwrap();
    for binding in [
        SemanticValueBindingV1::Execution(context),
        SemanticValueBindingV1::ExecutionBorrow(borrowed),
        SemanticValueBindingV1::Value {
            id: ValueId(40),
            ty: Type::Execution(ExecutionRoleV15::Context),
        },
    ] {
        assert!(binding.value().is_err());
        assert!(binding.values().is_err());
        assert!(!semantic_binding_can_restore_from_unique_source_v1(
            &binding
        ));
        assert!(matches!(
            semantic_binding_kind_v1(&binding),
            "nominal execution role" | "nominal execution borrow" | "ordinary value"
        ));
        let nested = SemanticValueBindingV1::Aggregate(vec![SemanticValueBindingV1::Unit, binding]);
        assert!(nested.values().is_err());
        assert!(!semantic_binding_can_restore_from_unique_source_v1(&nested));
        let payloads = BTreeMap::from([(0, vec![nested])]);
        assert!(project_enum_payload_field(0, &payloads, 0).is_err());
        let mut enum_binding = SemanticValueBindingV1::Enum {
            discriminant: ValueId(20),
            discriminant_ty: Type::Scalar(ScalarType::U32),
            semantic_type: SemanticTypeIdV1::from_index(18),
            variant: None,
            payloads,
        };
        assert!(enum_binding.values().is_err());
        assert!(
            reauthenticate_capabilities_from_enum_payload_v1(
                &mut enum_binding,
                SemanticLocalIdV1::from_index(0),
                0
            )
            .is_err()
        );
    }
    let ordinary = SemanticValueBindingV1::Aggregate(vec![
        SemanticValueBindingV1::Unit,
        SemanticValueBindingV1::Value {
            id: ValueId(30),
            ty: Type::Scalar(ScalarType::U32),
        },
    ]);
    assert_eq!(
        ordinary.values().unwrap(),
        vec![(ValueId(30), Type::Scalar(ScalarType::U32))]
    );
    assert!(semantic_binding_can_restore_from_unique_source_v1(
        &ordinary
    ));
}

#[test]
fn moved_execution_tombstone_remains_nominal_after_last_role_moves() {
    let tombstone = SemanticValueBindingV1::MovedExecution;
    assert_eq!(
        semantic_binding_kind_v1(&tombstone),
        "moved execution value"
    );
    assert!(tombstone.value().is_err());
    assert!(tombstone.values().is_err());
    let aggregate =
        SemanticValueBindingV1::Aggregate(vec![SemanticValueBindingV1::Unit, tombstone]);
    assert!(semantic_binding_contains_execution_v29(&aggregate));
    assert!(!semantic_binding_can_restore_from_unique_source_v1(
        &aggregate
    ));
    assert!(aggregate.values().is_err());
    let payloads = BTreeMap::from([
        (0, vec![aggregate]),
        (1, vec![SemanticValueBindingV1::Unit]),
    ]);
    for variant in [0, 1] {
        assert!(project_enum_payload_field(variant, &payloads, 0).is_err());
    }
    let mut binding = SemanticValueBindingV1::Enum {
        discriminant: ValueId(21),
        discriminant_ty: Type::Scalar(ScalarType::U32),
        semantic_type: SemanticTypeIdV1::from_index(18),
        variant: Some(1),
        payloads,
    };
    assert!(binding.values().is_err());
    assert!(
        reauthenticate_capabilities_from_enum_payload_v1(
            &mut binding,
            SemanticLocalIdV1::from_index(0),
            1,
        )
        .is_err()
    );
    assert!(semantic_binding_contains_execution_v29(&binding));
}

fn ordinary_types() -> Vec<SemanticTypeDeclV1> {
    types()
        .into_iter()
        .map(|ty| ty.with_rust_type_kind(SemanticRustTypeKindV1::Ordinary))
        .collect()
}

#[test]
fn ordinary_type_preflight_requires_exact_work_and_no_scratch() {
    let types = ordinary_types();
    let count = types.len();
    for floor in [0, 23] {
        for limit in [count, count - 1] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, floor);
            budget.reserve_storage(floor).unwrap();
            let result = require_execution_free_types_v29(&types, &mut budget);
            if limit == count {
                result.unwrap();
                assert_eq!(budget.work(), count);
            } else {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_),
                        )
                    )
                ));
            }
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), floor);
            assert_eq!(budget.failed_storage(), None);
        }
    }
}

#[test]
fn ordinary_type_preflight_rejects_nominal_roles_even_in_unused_rows() {
    let nominal = types();
    let ordinary = ordinary_types();
    for (index, declaration) in nominal
        .iter()
        .enumerate()
        .filter(|(_, ty)| matches!(ty.rust_type_kind(), SemanticRustTypeKindV1::Execution(_)))
    {
        for unused in [false, true] {
            let mut types = ordinary.clone();
            if unused {
                // No type or source root in the fixture names this appended row.
                types.push(declaration.clone());
            } else {
                types[index] = declaration.clone();
            }
            for floor in [0, 23] {
                let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(types.len());
                let mut budget = ArgumentBudgetV1::new(&mut work, floor);
                budget.reserve_storage(floor).unwrap();
                assert!(matches!(
                    require_execution_free_types_v29(&types, &mut budget),
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "execution roles require occurrence-bound transport, not an ordinary Rust representation",
                        ..
                    })
                ));
                assert_eq!(budget.work(), types.len());
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.peak_storage(), floor);
            }
        }
    }
}

#[test]
fn ordinary_type_preflight_charges_before_nominal_refusal_and_accepts_empty_table() {
    let types = types();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(types.len() - 1);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    assert!(matches!(
        require_execution_free_types_v29(&types, &mut budget),
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ),)
        )
    ));
    assert_eq!(budget.storage(), 0);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    require_execution_free_types_v29(&[], &mut budget).unwrap();
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.peak_storage(), 0);
}
