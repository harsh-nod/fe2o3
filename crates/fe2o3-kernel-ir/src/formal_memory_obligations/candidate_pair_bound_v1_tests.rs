use super::*;
use crate::{
    Atomic, AtomicKind, BasicBlock, MemoryOrdering, Signature, SynchronizationScope, Terminator,
    ValueDef,
};

fn fixture(count: usize, space: AddressSpace) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = (0..count)
        .map(|_| {
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(0),
                    value: ValueId(1),
                    access: MemoryAccess::new(space, 4),
                },
            )
        })
        .collect();
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("preflight");
    module.functions.push(Function::definition(
        "helper",
        Signature::new(
            vec![
                Type::pointer(scalar.clone(), space, AccessMode::ReadWrite),
                scalar,
                Type::BOOL,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    module
}
fn bound(
    module: &Module,
    limit: usize,
) -> Result<FormalMemoryCandidatePairBoundV1, FormalMemoryCandidatePairErrorV1> {
    bound_formal_memory_candidate_pairs_v1(verify_module_ref(module).unwrap(), limit)
}

#[test]
fn every_ordinary_access_appending_arm_is_counted_once_including_guarded_fallback() {
    let mut module = fixture(0, AddressSpace::Global);
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    module.functions[0].body.as_mut().unwrap().blocks[0].operations = vec![
        Operation::new(
            vec![ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32))],
            OperationKind::Load {
                pointer: ValueId(0),
                access,
            },
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32))],
            OperationKind::GuardedLoad {
                pointer: ValueId(0),
                predicate: ValueId(2),
                fallback: ValueId(1),
                access,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(1),
                access,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::GuardedStore {
                pointer: ValueId(0),
                predicate: ValueId(2),
                value: ValueId(1),
                access,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Atomic(Atomic {
                kind: AtomicKind::Store,
                pointer: ValueId(0),
                value: Some(ValueId(1)),
                compare: None,
                access,
                scope: SynchronizationScope::Device,
                ordering: MemoryOrdering::Relaxed,
                failure_ordering: None,
            }),
        ),
    ];
    let report = bound(&module, 15).unwrap();
    assert_eq!((report.accesses(), report.pairs()), (5, 15));
    assert_eq!(
        bound(&module, 14),
        Err(FormalMemoryCandidatePairErrorV1::Limit {
            actual: 15,
            limit: 14
        })
    );
}

#[test]
fn pair_census_includes_private_unreachable_and_uncalled_non_root_bodies() {
    let mut module = fixture(2, AddressSpace::Private);
    let mut dead = module.functions[0].body.as_ref().unwrap().blocks[0].clone();
    dead.id = BlockId(9);
    module.functions[0].body.as_mut().unwrap().blocks.push(dead);
    let mut helper = module.functions[0].clone();
    helper.id = "uncalled".into();
    module.functions.push(helper);
    module.functions.push(Function::external_import(
        "declaration",
        Signature::new(vec![], vec![]),
    ));
    let result = bound(&module, 36).unwrap();
    assert_eq!((result.accesses(), result.pairs()), (8, 36));
    assert_eq!(
        bound(&module, 35),
        Err(FormalMemoryCandidatePairErrorV1::Limit {
            actual: 36,
            limit: 35
        })
    );
}

#[test]
fn empty_exact_one_short_and_fixed_maximum_are_checked_before_any_report() {
    assert_eq!(
        bound(&fixture(0, AddressSpace::Global), 0).unwrap().pairs(),
        0
    );
    assert_eq!(
        bound(&fixture(1, AddressSpace::Global), 0),
        Err(FormalMemoryCandidatePairErrorV1::Limit {
            actual: 1,
            limit: 0
        })
    );
    let accepted = bound(&fixture(44, AddressSpace::Global), 990).unwrap();
    assert_eq!((accepted.accesses(), accepted.pairs()), (44, 990));
    assert_eq!(
        bound(&fixture(44, AddressSpace::Global), 989),
        Err(FormalMemoryCandidatePairErrorV1::Limit {
            actual: 990,
            limit: 989
        })
    );
    for count in [45, 256, 4096] {
        assert_eq!(
            bound(&fixture(count, AddressSpace::Global), usize::MAX),
            Err(FormalMemoryCandidatePairErrorV1::Limit {
                actual: 1035,
                limit: MAX_FORMAL_MEMORY_CANDIDATE_PAIRS_V1
            })
        );
    }
}
