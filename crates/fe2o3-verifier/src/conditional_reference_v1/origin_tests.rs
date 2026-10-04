//! Coordinate-level regressions, not fabricated source-request authority.
//! The production caller obtains both coordinates from one authenticated row.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_pliron::{
    ProductionRankedBlockV1 as Block, ProductionRankedOperationV1 as Operation,
    ProductionRankedTerminatorV1 as Term, ProductionRankedValueIdV1 as Id,
    ProductionRankedValueV1 as Operand, ProductionSemanticLoadV2 as Load,
    ProductionSemanticScalarTypeV2 as Scalar,
};

fn fixture(origin: u64, explicit_space: bool) -> (Vec<Operation>, Load) {
    let view = if explicit_space {
        Operation::ViewInSpace {
            result: Id::new(0),
            element_width: 32,
            writable: false,
            shape: vec![dialect_kernel::DYNAMIC_EXTENT],
            dynamic_extents: vec![Operand::Argument(0)],
            allocation_origin: origin,
            noalias_class: origin,
            memory_space: dialect_kernel::MemorySpaceAttr::Global,
        }
    } else {
        Operation::View {
            result: Id::new(0),
            element_width: 32,
            writable: false,
            shape: vec![dialect_kernel::DYNAMIC_EXTENT],
            dynamic_extents: vec![Operand::Argument(0)],
            allocation_origin: origin,
            noalias_class: origin,
        }
    };
    let load = Load {
        block: 0,
        operation: 1,
        scalar: Scalar::Integer {
            signed: false,
            bits: 32,
        },
        allocation_origin: 0,
        view: Operand::Local(Id::new(0)),
        indices: vec![Operand::Argument(1)].into_boxed_slice(),
    };
    (vec![view], load)
}

fn run(
    operations: Vec<Operation>,
    load: &Load,
    source: u32,
    adjusted: u32,
    limit: usize,
) -> (Result<(), Error>, usize, Option<usize>) {
    let blocks = [Block::new(operations, Term::Return)];
    let mut work = Work::new(limit);
    let mut budget = Budget::new(&mut work, 23);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(23).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let result = require_read_origins(&blocks, load, source, adjusted, &mut budget);
    assert!(budget.work_ledger_identity_v1() == identity);
    assert_eq!(budget.storage(), 23);
    assert_eq!(budget.peak_storage(), 23);
    let accepted = budget.work();
    drop(budget);
    (result, accepted, work.failed_work())
}

#[test]
fn ignored_context_zst_and_hidden_abi_coordinates_keep_distinct_origins() {
    for (name, source, adjusted) in [
        ("ignored context", 1, 0),
        ("ignored context and ZST", 3, 1),
        ("hidden ABI argument", 0, 1),
        ("hidden and shifted ABI arguments", 2, 5),
    ] {
        for explicit_space in [false, true] {
            let (operations, mut load) = fixture(u64::from(source) + 1, explicit_space);
            load.allocation_origin = u64::from(adjusted) + 1;
            assert!(
                run(operations.clone(), &load, source, adjusted, usize::MAX)
                    .0
                    .is_ok(),
                "{name}"
            );

            // A source-space expression origin must not stand in for the CPU origin.
            load.allocation_origin = u64::from(source) + 1;
            assert!(
                run(operations, &load, source, adjusted, usize::MAX)
                    .0
                    .is_err(),
                "{name}"
            );
            load.allocation_origin = u64::from(adjusted) + 1;
            let (wrong_recipe, _) = fixture(u64::from(adjusted) + 1, explicit_space);
            assert!(
                run(wrong_recipe, &load, source, adjusted, usize::MAX)
                    .0
                    .is_err(),
                "{name}"
            );

            let (operations, _) = fixture(u64::from(source) + 1, explicit_space);
            assert!(
                run(operations.clone(), &load, source + 1, adjusted, usize::MAX)
                    .0
                    .is_err(),
                "{name}"
            );
            assert!(
                run(operations, &load, source, adjusted + 1, usize::MAX)
                    .0
                    .is_err(),
                "{name}"
            );
        }
    }
}

#[test]
fn missing_duplicate_and_substituted_recipe_views_fail_closed() {
    let (mut operations, mut load) = fixture(4, false);
    load.allocation_origin = 2;
    assert!(run(vec![], &load, 3, 1, usize::MAX).0.is_err());
    operations.push(operations[0].clone());
    assert!(run(operations, &load, 3, 1, usize::MAX).0.is_err());
    let (operations, _) = fixture(4, true);
    load.view = Operand::Local(Id::new(99));
    assert!(run(operations, &load, 3, 1, usize::MAX).0.is_err());
}

#[test]
fn origin_join_preserves_exact_work_denial_and_original_storage() {
    let (operations, mut load) = fixture(4, false);
    load.allocation_origin = 2;
    let (result, exact, denied) = run(operations.clone(), &load, 3, 1, usize::MAX);
    assert!(result.is_ok());
    assert_eq!(denied, None);
    assert!(run(operations.clone(), &load, 3, 1, exact).0.is_ok());
    let (result, accepted, denied) = run(operations.clone(), &load, 3, 1, exact - 1);
    assert!(result.is_err());
    assert!(accepted < exact);
    assert_eq!(denied, Some(exact));
    let (result, accepted, denied) = run(operations, &load, 3, 1, 7);
    assert!(result.is_err());
    assert_eq!(accepted, 7);
    assert_eq!(denied, Some(10));
}
