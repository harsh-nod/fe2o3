use super::*;

fn id(index: u32) -> ProductionRankedValueIdV1 {
    ProductionRankedValueIdV1::new(index)
}

fn local(index: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(id(index))
}

fn publication_operations() -> Vec<ProductionRankedOperationV1> {
    use ProductionRankedOperationV1 as O;
    vec![
        O::IndexConstant {
            result: id(0),
            value: 128,
        },
        O::IndexConstant {
            result: id(1),
            value: 7,
        },
        O::ViewInSpace {
            result: id(2),
            element_width: 32,
            writable: true,
            shape: vec![DYNAMIC_EXTENT],
            dynamic_extents: vec![local(0)],
            memory_space: MemorySpaceAttr::Global,
            allocation_origin: 1,
            noalias_class: 1,
        },
        O::ViewInSpace {
            result: id(3),
            element_width: 32,
            writable: true,
            shape: vec![DYNAMIC_EXTENT],
            dynamic_extents: vec![local(0)],
            memory_space: MemorySpaceAttr::Global,
            allocation_origin: 2,
            noalias_class: 2,
        },
        O::PublicationAtomicStoreU32 {
            view: local(2),
            index: local(1),
            value: 1,
        },
        O::PublicationAtomicLoadU32 {
            result: id(4),
            view: local(2),
            index: local(1),
        },
        O::PublicationReadGuard {
            result: id(5),
            success: id(6),
            index: local(1),
            physical_extent: local(0),
            acquired: local(4),
        },
        O::PredicatedAccess {
            kind: AccessKindAttr::Read,
            view: local(3),
            index: local(5),
            success: local(6),
        },
    ]
}

fn recipe(
    operations: Vec<ProductionRankedOperationV1>,
) -> Result<ProductionRankedKernelV1, ProductionRankedKernelErrorV1> {
    ProductionRankedKernelV1::new(
        "publication_recipe",
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
}

#[test]
fn publication_recipe_accepts_exact_markers_and_keeps_real_acquire_identity() {
    for marker in [1, 2] {
        let mut operations = publication_operations();
        let ProductionRankedOperationV1::PublicationAtomicStoreU32 { value, .. } =
            &mut operations[4]
        else {
            unreachable!()
        };
        *value = marker;
        recipe(operations).unwrap();
    }
    let locals = [
        RecipeValueKindV1::Index,
        RecipeValueKindV1::Index,
        RecipeValueKindV1::View {
            rank: 1,
            element_width: 32,
            writable: true,
            dynamic_extent: Some(local(0)),
            memory_space: MemorySpaceAttr::Global,
            leading_extent: DYNAMIC_EXTENT,
        },
    ];
    let operation = ProductionRankedOperationV1::PublicationAtomicLoadU32 {
        result: id(3),
        view: local(2),
        index: local(1),
    };
    assert_eq!(
        validate_operation(&operation, 0, &locals).unwrap(),
        Some((
            id(3),
            RecipeValueKindV1::PublicationAcquiredU32 {
                view: local(2),
                index: local(1)
            }
        ))
    );
}

#[test]
fn publication_recipe_rejects_fabricated_acquires_wrong_cells_and_capability_misuse() {
    for mutant in 0..9 {
        let mut operations = publication_operations();
        match mutant {
            0 => {
                operations[5] = ProductionRankedOperationV1::IndexConstant {
                    result: id(4),
                    value: 2,
                }
            }
            1 => {
                if let ProductionRankedOperationV1::PublicationReadGuard { index, .. } =
                    &mut operations[6]
                {
                    *index = local(0);
                }
            }
            2 => {
                if let ProductionRankedOperationV1::PublicationReadGuard {
                    physical_extent, ..
                } = &mut operations[6]
                {
                    *physical_extent = local(1);
                }
            }
            3 => {
                if let ProductionRankedOperationV1::PublicationReadGuard { success, .. } =
                    &mut operations[6]
                {
                    *success = id(7);
                }
            }
            4 => {
                if let ProductionRankedOperationV1::PredicatedAccess { kind, .. } =
                    &mut operations[7]
                {
                    *kind = AccessKindAttr::Write;
                }
            }
            5 => {
                if let ProductionRankedOperationV1::PredicatedAccess { index, .. } =
                    &mut operations[7]
                {
                    *index = local(1);
                }
            }
            6 => {
                operations[7] = ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Read,
                    view: local(3),
                    indices: vec![local(5)],
                }
            }
            7 => operations.push(ProductionRankedOperationV1::PublicationAtomicStoreU32 {
                view: local(2),
                index: local(5),
                value: 2,
            }),
            8 => {
                operations.pop();
            }
            _ => unreachable!(),
        }
        assert!(recipe(operations).is_err(), "mutant {mutant}");
    }
}

#[test]
fn publication_recipe_rejects_wrong_view_shapes_spaces_and_markers() {
    for mutant in 0..7 {
        let mut operations = publication_operations();
        if let ProductionRankedOperationV1::ViewInSpace {
            element_width,
            writable,
            shape,
            dynamic_extents,
            memory_space,
            ..
        } = &mut operations[2]
        {
            match mutant {
                0 => *element_width = 16,
                1 => *writable = false,
                2 => {
                    *shape = vec![128];
                    dynamic_extents.clear();
                }
                3 => {
                    *shape = vec![DYNAMIC_EXTENT, 1];
                }
                4 => *memory_space = MemorySpaceAttr::Workgroup,
                _ => (),
            }
        }
        if mutant >= 5
            && let ProductionRankedOperationV1::PublicationAtomicStoreU32 { value, .. } =
                &mut operations[4]
        {
            *value = if mutant == 5 { 0 } else { 3 };
        }
        assert!(recipe(operations).is_err(), "mutant {mutant}");
    }
}

#[test]
fn publication_recipe_materialization_keeps_scope_value_results_and_guard() {
    let mut context = pliron::context::Context::new();
    dialect_kernel::register_dialect(
        &mut context,
        &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    let extent = IndexConstantOp::new(&mut context, 128).result(&context);
    let index = IndexConstantOp::new(&mut context, 7).result(&context);
    let ty = RankedViewType::new(&context, 32, true, vec![DYNAMIC_EXTENT]).unwrap();
    let view = RankedViewOp::new_in_space(&mut context, ty, vec![extent], MemorySpaceAttr::Global)
        .unwrap()
        .result(&context);
    let mut locals = vec![extent, index, view];
    let operation = ProductionRankedOperationV1::PublicationAtomicStoreU32 {
        view: local(2),
        index: local(1),
        value: 2,
    };
    let (pointer, result) = materialize_publication_recipe_v1(
        &mut context,
        &operation,
        &[],
        &mut locals,
        &HashMap::new(),
    )
    .unwrap();
    assert!(result.is_none());
    let store = Operation::get_op::<RankedAccessOp>(pointer, &context).unwrap();
    pliron::op::verify_op(&store, &context).unwrap();
    assert_eq!(
        store.atomic_ordering(&context),
        Some(AtomicOrderingAttr::Release)
    );
    assert_eq!(store.atomic_scope(&context), Some(AtomicScopeAttr::System));
    assert_eq!(
        store
            .publication_atomic_kind(&context)
            .unwrap()
            .stored_value(),
        Some(2)
    );
    let operation = ProductionRankedOperationV1::PublicationAtomicLoadU32 {
        result: id(3),
        view: local(2),
        index: local(1),
    };
    let (pointer, result) = materialize_publication_recipe_v1(
        &mut context,
        &operation,
        &[],
        &mut locals,
        &HashMap::new(),
    )
    .unwrap();
    let (result_id, acquired) = result.unwrap();
    assert_eq!(result_id, id(3));
    locals.push(acquired);
    let load = Operation::get_op::<RankedAccessOp>(pointer, &context).unwrap();
    pliron::op::verify_op(&load, &context).unwrap();
    assert_eq!(
        load.atomic_ordering(&context),
        Some(AtomicOrderingAttr::Acquire)
    );
    assert_eq!(load.atomic_scope(&context), Some(AtomicScopeAttr::System));
    assert_eq!(load.publication_read_result(&context), Some(acquired));
    let operation = ProductionRankedOperationV1::PublicationReadGuard {
        result: id(4),
        success: id(5),
        index: local(1),
        physical_extent: local(0),
        acquired: local(3),
    };
    let (pointer, result) = materialize_publication_recipe_v1(
        &mut context,
        &operation,
        &[],
        &mut locals,
        &HashMap::new(),
    )
    .unwrap();
    assert!(result.is_none());
    let guard = Operation::get_op::<PublicationReadGuardOp>(pointer, &context).unwrap();
    pliron::op::verify_op(&guard, &context).unwrap();
    assert_eq!(locals[4], guard.result(&context));
    assert_eq!(locals[5], guard.success(&context));
    assert_eq!(guard.acquired(&context), acquired);
}
