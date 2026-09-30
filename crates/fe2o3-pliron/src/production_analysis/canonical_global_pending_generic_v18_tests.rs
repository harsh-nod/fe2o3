use super::*;

#[test]
fn pending_global_generic_access_queries_retain_exact_actual_carriers_without_authority() {
    let mut module = fixture(1);
    let block = &mut module.functions[1].body.as_mut().unwrap().blocks[0];
    let generic = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Generic,
        AccessMode::ReadWrite,
    );
    block.operations.insert(
        3,
        Operation::new(
            vec![ValueDef::new(ValueId(13), generic.clone())],
            OperationKind::Cast {
                kind: fe2o3_kernel_ir::CastKind::PointerToGeneric,
                value: ValueId(6),
                to: generic,
            },
        ),
    );
    for operation in &mut block.operations[4..] {
        match &mut operation.kind {
            OperationKind::Load { pointer, access }
            | OperationKind::Store {
                pointer, access, ..
            } => {
                *pointer = ValueId(13);
                access.address_space = AddressSpace::Generic;
            }
            _ => unreachable!(),
        }
    }
    let seen = Cell::new(false);
    with_checked(&module, |checked, budget| {
        with_pending_canonical_ranked_source_roles_v18(
            checked,
            LAYOUTS,
            budget,
            |pending, budget| {
                pending.with_pending_global_accesses_v18(pending.owner, budget, |view, budget| {
                    for position in [4, 5] {
                        let actual = view
                            .operation(pending.owner, coordinate(1, position), budget)?
                            .unwrap();
                        let expected = &pending.owner.module().functions[1]
                            .body
                            .as_ref()
                            .unwrap()
                            .blocks[0]
                            .operations[position as usize];
                        assert!(std::ptr::eq(actual, expected));
                        assert!(matches!(&actual.kind,
                        OperationKind::Load { access, .. } | OperationKind::Store { access, .. }
                        if access.address_space == AddressSpace::Generic));
                    }
                    assert!(
                        view.operation(pending.owner, coordinate(1, 3), budget)?
                            .is_none()
                    );
                    assert!(!view.memory_safety_is_complete());
                    assert!(!view.native_stage_coverage_is_complete());
                    assert!(!view.grants_artifact_or_launch_authority());
                    seen.set(true);
                    Ok(())
                })
            },
        )
        .unwrap();
    });
    assert!(seen.get());
}
