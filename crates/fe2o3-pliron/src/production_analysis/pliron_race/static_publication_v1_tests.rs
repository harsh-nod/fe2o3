use super::*;
use crate::{
    ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
    ProductionRankedOperationV1 as O, ProductionRankedTerminatorV1 as T,
    ProductionRankedValueIdV1 as Id, ProductionRankedValueV1 as V, ProductionSessionLimitsV1,
    compile_ranked_kernel_for_gfx942_lowering_v1,
};
use dialect_kernel::IndexBinaryKindAttr;
include!("static_publication_split_v1_tests.rs");

fn local(index: u32) -> V {
    V::Local(Id::new(index))
}

fn blocks() -> Vec<ProductionRankedBlockV1> {
    let view = |result, origin, noalias| O::ViewInSpace {
        result: Id::new(result),
        element_width: 32,
        writable: true,
        shape: vec![DYNAMIC_EXTENT],
        dynamic_extents: vec![local(1)],
        memory_space: MemorySpaceAttr::Global,
        allocation_origin: origin,
        noalias_class: noalias,
    };
    vec![
        ProductionRankedBlockV1::new(
            vec![
                O::ExecutionLayout {
                    grid_identity: 0,
                    global_extents: [256, 1, 1],
                    workgroup_extents: [128, 1, 1],
                    subgroup_size: 64,
                    full_physical_workgroups: true,
                },
                O::InvocationIndex {
                    result: Id::new(0),
                    dimension: 0,
                    launch_extent: 256,
                },
                O::IndexConstant {
                    result: Id::new(1),
                    value: 128,
                },
                O::IndexBinary {
                    result: Id::new(2),
                    kind: IndexBinaryKindAttr::Remainder,
                    lhs: local(0),
                    rhs: local(1),
                },
                view(3, 1, 1),
                view(4, 2, 2),
                view(5, 3, 0),
            ],
            T::IndexLessThan {
                lhs: local(0),
                rhs: local(1),
                true_block: 1,
                false_block: 2,
            },
        ),
        ProductionRankedBlockV1::new(
            vec![
                O::Access {
                    kind: AccessKindAttr::Write,
                    view: local(3),
                    indices: vec![local(2)],
                },
                O::PublicationAtomicStoreU32 {
                    view: local(4),
                    index: local(2),
                    value: 2,
                },
            ],
            T::Return,
        ),
        ProductionRankedBlockV1::new(
            vec![
                O::PublicationAtomicStoreU32 {
                    view: local(4),
                    index: local(2),
                    value: 1,
                },
                O::PublicationAtomicLoadU32 {
                    result: Id::new(6),
                    view: local(4),
                    index: local(2),
                },
                O::PublicationReadGuard {
                    result: Id::new(7),
                    success: Id::new(8),
                    index: local(2),
                    physical_extent: local(1),
                    acquired: local(6),
                },
                O::PredicatedAccess {
                    kind: AccessKindAttr::Read,
                    view: local(3),
                    index: local(7),
                    success: local(8),
                },
            ],
            T::Return,
        ),
    ]
}

fn compile(
    blocks: Vec<ProductionRankedBlockV1>,
) -> Result<crate::ProductionRankedKernelLoweringInputV1, Box<crate::ProductionRankedCompileErrorV1>>
{
    let kernel = ProductionRankedKernelV1::new("publication_race", 0, blocks).unwrap();
    compile_ranked_kernel_for_gfx942_lowering_v1(
        ProductionConstructionV1::ranked_kernel("publication_race", kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
        [2, 3],
    )
    .map_err(Box::new)
}

#[test]
fn static_publication_live_full_domain_proves_exactly_128_cell_pairs() {
    let owner = compile(blocks()).unwrap();
    let report = owner.race_report();
    assert!(report.is_clean());
    let proof = report.static_publication().unwrap();
    assert_eq!(proof.payload_origin(), 1);
    assert_eq!(proof.flags_origin(), 2);
    assert_eq!(proof.maximum_invocations(), 256);
    assert_eq!(proof.potentially_conflicting_cell_pairs(), 128);
    assert_eq!(proof.discharged_cell_pairs(), 128);
    assert_eq!(proof.unresolved_cell_pairs(), 0);
    assert_eq!(
        proof.sites().map(|site| (site.block(), site.operation())),
        [(1, 0), (1, 1), (2, 0), (2, 1), (2, 2), (2, 3)]
    );
    assert!(!report.grants_artifact_or_launch_authority());
}

#[test]
fn static_publication_live_rejects_protocol_role_domain_and_alias_mutants() {
    for mutant in 0..12 {
        let original = blocks();
        let mut operations = original
            .iter()
            .map(|block| block.operations().to_vec())
            .collect::<Vec<_>>();
        let mut terminators = original
            .iter()
            .map(|block| block.terminator().clone())
            .collect::<Vec<_>>();
        match mutant {
            0 => operations[1].push(O::AtomicAccess {
                kind: AccessKindAttr::AtomicWrite,
                view: local(5),
                indices: vec![local(2)],
                ordering: AtomicOrderingAttr::Release,
                scope: AtomicScopeAttr::System,
            }),
            1 => {
                terminators[0] = T::IndexLessThan {
                    lhs: local(0),
                    rhs: local(1),
                    true_block: 2,
                    false_block: 1,
                }
            }
            2 => {
                operations[0][3] = O::IndexConstant {
                    result: Id::new(2),
                    value: 0,
                }
            }
            3 => terminators[2] = T::Branch { target: 2 },
            4 => {
                operations[2].remove(0);
            }
            5 => {
                operations[1].pop();
            }
            6 => operations[1].swap(0, 1),
            7 => operations[2].swap(0, 1),
            8 => {
                if let O::ExecutionLayout { global_extents, .. } = &mut operations[0][0] {
                    *global_extents = [128, 1, 1];
                }
            }
            9 => operations[1].push(O::Access {
                kind: AccessKindAttr::Read,
                view: local(3),
                indices: vec![local(2)],
            }),
            10 => terminators[2] = T::Branch { target: 1 },
            11 => operations[1].push(O::Access {
                kind: AccessKindAttr::Read,
                view: local(5),
                indices: vec![local(2)],
            }),
            _ => unreachable!(),
        }
        let blocks = operations
            .into_iter()
            .zip(terminators)
            .map(|(ops, term)| ProductionRankedBlockV1::new(ops, term))
            .collect();
        assert!(compile(blocks).is_err(), "mutant {mutant}");
    }
}
