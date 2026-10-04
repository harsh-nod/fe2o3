//! Raw-row white-box mapping checks only, never a sealed relation or Request.
#![cfg(test)]
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirBlockCoordinateV1 as Block,
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirFunctionCoordinateV1 as Function,
    CanonicalKirOperationCoordinateV1 as Site,
};

fn site(operation: u32) -> Site {
    Site {
        block: Block {
            function: Function(2),
            block: 3,
        },
        operation,
    }
}

fn coordinate(operation: u32) -> Coordinate {
    Coordinate::Operation {
        function: 2,
        block: 3,
        operation,
    }
}

#[test]
fn conditional_relation_retained_mapping_rejects_missing_ambiguous_and_synthetic_rows() {
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 1024);
    budget.reserve_storage(31).unwrap();
    let account = budget.work_ledger_identity_v1();
    let row = Transition {
        origin: Origin::Retained(site(7)),
        output: site(5),
    };
    assert_eq!(
        retained(coordinate(7), &[row], &mut budget).unwrap(),
        coordinate(5)
    );
    assert!(matches!(
        retained(coordinate(7), &[], &mut budget),
        Err(Error::Mismatch("missing memory occurrence"))
    ));
    assert!(matches!(
        retained(coordinate(7), &[row, row], &mut budget),
        Err(Error::Mismatch("unique surviving memory occurrence"))
    ));
    let synthetic = Transition {
        origin: Origin::ConstantFrom(Definition::Result {
            operation: site(7),
            result: 0,
        }),
        output: site(5),
    };
    assert!(matches!(
        retained(coordinate(7), &[synthetic], &mut budget),
        Err(Error::Mismatch("missing memory occurrence"))
    ));
    assert!(retained(coordinate(8), &[row], &mut budget).is_err());
    assert_eq!(budget.work(), 20);
    assert_eq!(budget.storage(), 31);
    assert!(budget.work_ledger_identity_v1() == account);
}

#[test]
fn conditional_relation_retained_mapping_prepays_each_row_and_keeps_denial() {
    for limit in [3, 4] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 1024);
        let row = Transition {
            origin: Origin::Retained(site(7)),
            output: site(5),
        };
        let result = retained(coordinate(7), &[row], &mut budget);
        assert_eq!(result.is_ok(), limit == 4);
        assert_eq!(budget.failed_work(), (limit == 3).then_some(4));
        assert_eq!(budget.storage(), 0);
    }
}
