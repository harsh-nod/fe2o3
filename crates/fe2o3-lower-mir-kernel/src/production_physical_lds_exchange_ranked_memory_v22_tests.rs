//! Projection-only controls using one inert shared graph; no live source owner.
use super::*;
use dialect_gpu::{AddressSpaceAttr, HierarchyAttr, MemoryOrderAttr, MemoryScopeAttr};
fn compile(r: Recipe) -> bool {
    let construction = ProductionConstructionV1::ranked_kernel("lds_inert_conditional", r).unwrap();
    compile_ranked_kernel_for_gfx942_lowering_v1(
        construction,
        ProductionSessionLimitsV1::default(),
        std::iter::empty(),
    )
    .is_ok()
}
fn change_entry(r: &Recipe, ops: Vec<OpR>) -> Recipe {
    let mut blocks = r.blocks().to_vec();
    blocks[0] = Block::new(ops, r.blocks()[0].terminator().clone());
    Recipe::new("lds_projection_control", 2, blocks).unwrap()
}
#[test]
fn lds_exchange_ranked_exact_frame_and_publication_are_actual_operations() {
    let owner = owner(&fixture::module());
    let formal = formal(&owner, 128);
    assert!(memory::required_conditions(&formal));
    memory::join_actual_rows(&owner, &formal).unwrap();
    let r = projected(&owner);
    let ops = r.blocks()[0].operations();
    assert!(matches!(
        ops[0],
        OpR::ExecutionLayout {
            workgroup_extents: [128, 1, 1],
            subgroup_size: 64,
            full_physical_workgroups: true,
            ..
        }
    ));
    assert!(ops.iter().any(|op| matches!(op,OpR::ViewInSpace {
        result, element_width:32,writable:true,shape,dynamic_extents,
        memory_space:dialect_kernel::MemorySpaceAttr::Workgroup,allocation_origin:4,noalias_class:4,
    } if *result==LDS&&shape==&[128]&&dynamic_extents.is_empty())));
    let write = ops
        .iter()
        .position(
            |op| matches!(op,OpR::Access{kind:Access::Write,view,..}if *view==ValueR::Local(LDS)),
        )
        .unwrap();
    let barrier = ops
        .iter()
        .position(|op| {
            matches!(
                op,
                OpR::Barrier {
                    execution_scope: HierarchyAttr::Workgroup,
                    memory_scope: MemoryScopeAttr::Workgroup,
                    address_space: AddressSpaceAttr::Workgroup,
                    order: MemoryOrderAttr::AcquireRelease,
                }
            )
        })
        .unwrap();
    let read = ops
        .iter()
        .position(
            |op| matches!(op,OpR::Access{kind:Access::Read,view,..}if *view==ValueR::Local(LDS)),
        )
        .unwrap();
    assert!(write < barrier && barrier < read);
    assert!(matches!(ops[read + 1], OpR::IndexUnknown { .. }));
    assert_eq!(
        ops.iter()
            .filter(|op| matches!(op, OpR::Barrier { .. }))
            .count(),
        1
    );
    assert_eq!(formal.lds_write().value(), formal.input_read().result());
    assert_eq!(formal.lds_read().value(), formal.output_store().value());
    assert_ne!(formal.lds_read().value(), formal.input_read().result());
}
#[test]
fn lds_exchange_ranked_barrier_absence_wrong_scope_or_global_only_is_not_clean() {
    let r = projected(&owner(&fixture::module()));
    for mutation in 0..4 {
        let mut ops = r.blocks()[0].operations().to_vec();
        let index = ops
            .iter()
            .position(|op| matches!(op, OpR::Barrier { .. }))
            .unwrap();
        match mutation {
            0 => {
                ops.remove(index);
            }
            1 => {
                if let OpR::Barrier {
                    execution_scope, ..
                } = &mut ops[index]
                {
                    *execution_scope = HierarchyAttr::Subgroup;
                }
            }
            2 => {
                if let OpR::Barrier { address_space, .. } = &mut ops[index] {
                    *address_space = AddressSpaceAttr::Global;
                }
            }
            3 => {
                if let OpR::Barrier { order, .. } = &mut ops[index] {
                    *order = MemoryOrderAttr::Acquire;
                }
            }
            _ => unreachable!(),
        }
        assert!(!compile(change_entry(&r, ops)), "mutation {mutation}");
    }
}
#[test]
fn lds_exchange_ranked_missing_write_or_short_frame_is_not_clean() {
    let r = projected(&owner(&fixture::module()));
    for mutation in 0..2 {
        let mut ops = r.blocks()[0].operations().to_vec();
        if mutation == 0 {
            let at=ops.iter().position(|op|matches!(op,OpR::Access{kind:Access::Write,view,..}if *view==ValueR::Local(LDS))).unwrap();
            ops.remove(at);
        } else {
            let frame = ops
                .iter_mut()
                .find(|op| matches!(op,OpR::ViewInSpace{result,..}if *result==LDS))
                .unwrap();
            let OpR::ViewInSpace { shape, .. } = frame else {
                unreachable!()
            };
            *shape = vec![127];
        }
        assert!(!compile(change_entry(&r, ops)), "mutation {mutation}");
    }
}
fn numeric(value: ValueR, values: &std::collections::BTreeMap<IdR, Option<u64>>) -> Option<u64> {
    match value {
        ValueR::Local(id) => *values.get(&id).unwrap(),
        ValueR::Argument(_) => Some(128),
        _ => None,
    }
}
#[test]
fn lds_exchange_ranked_address_projection_is_exact_for_all_128_invocations() {
    let r = projected(&owner(&fixture::module()));
    for invocation in 0u64..128 {
        let mut values = std::collections::BTreeMap::new();
        let mut accesses = Vec::new();
        let mut byte_offsets = Vec::new();
        for op in r.blocks()[0].operations() {
            let row = match op {
                OpR::IndexConstant { result, value } => Some((*result, Some(*value))),
                OpR::IndexUnknown { result } | OpR::DeterministicJoin { result, .. } => {
                    Some((*result, None))
                }
                OpR::InvocationIndex {
                    result,
                    dimension: 0,
                    launch_extent: 128,
                } => Some((*result, Some(invocation))),
                OpR::IndexBinary {
                    result,
                    kind,
                    lhs,
                    rhs,
                } => {
                    if *kind == Binary::Multiply && numeric(*rhs, &values) == Some(4) {
                        byte_offsets
                            .push(numeric(*lhs, &values).expect("actual LDS source index") * 4);
                    }
                    let value = numeric(*lhs, &values)
                        .zip(numeric(*rhs, &values))
                        .map(|(a, b)| match kind {
                            Binary::Add => a.checked_add(b).unwrap(),
                            Binary::Multiply => a.checked_mul(b).unwrap(),
                            Binary::Divide => a / b,
                            Binary::Remainder => a % b,
                        });
                    Some((*result, value))
                }
                OpR::Access {
                    kind,
                    view,
                    indices,
                } if *view == ValueR::Local(LDS) => {
                    assert_eq!(indices.len(), 1);
                    accesses.push((
                        *kind,
                        numeric(indices[0], &values).expect("address not opaque"),
                    ));
                    None
                }
                _ => None,
            };
            if let Some((id, value)) = row {
                assert!(values.insert(id, value).is_none());
            }
        }
        assert_eq!(
            accesses,
            vec![(Access::Write, invocation), (Access::Read, invocation ^ 64)]
        );
        assert_eq!(byte_offsets, vec![invocation * 4, (invocation ^ 64) * 4]);
    }
}
#[test]
fn lds_exchange_formal_short_launch_is_refused_not_silently_widened() {
    let owner = owner(&fixture::module());
    for count in [0, 1, 64, 127, 129, 256] {
        let mut work = Work::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 4_000_000);
        budget.reserve_storage(83).unwrap();
        assert!(
            derive_physical_lds_exchange_memory_obligations_v22(
                &owner,
                &owner.module().kernels[0].id,
                ExplicitLaunchExtent::Exact {
                    rank: 1,
                    extents: [count, 1, 1]
                },
                FormalIndexWidth::Bits64,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), 83);
    }
}

#[test]
fn lds_exchange_ranked_byte_address_relations_refuse_missing_stale_duplicate_and_excess() {
    let mut map = memory::Census::default();
    let a = ValueId(101);
    let b = ValueId(102);
    let bytes_a = ValueR::Local(IdR::new(51));
    let bytes_b = ValueR::Local(IdR::new(52));
    let index_a = ValueR::Local(IdR::new(31));
    let index_b = ValueR::Local(IdR::new(32));
    assert!(map.address_index(a, bytes_a).is_err());
    map.record_address(a, bytes_a, index_a).unwrap();
    assert_eq!(map.address_index(a, bytes_a).unwrap(), index_a);
    assert!(map.address_index(a, bytes_b).is_err());
    assert!(map.address_index(b, bytes_a).is_err());
    assert!(map.record_address(a, bytes_b, index_b).is_err());
    assert_eq!(map.address_index(a, bytes_a).unwrap(), index_a);
    map.record_address(b, bytes_b, index_b).unwrap();
    assert_eq!(map.address_index(b, bytes_b).unwrap(), index_b);
    assert!(map.record_address(ValueId(103), bytes_b, index_b).is_err());
    assert!(!map.complete()); // mapping does not invent memory/barrier execution
}
