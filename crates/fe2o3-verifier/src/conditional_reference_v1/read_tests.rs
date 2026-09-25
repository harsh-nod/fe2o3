use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, ConditionalTotalViewAddressDomainV1 as Domain,
};

fn load_site(block: u32, operation: u32) -> Expr {
    Expr::Load(ProductionSemanticLoadV2 {
        block,
        operation,
        scalar: ProductionSemanticScalarTypeV2::Float { bits: 32 },
        allocation_origin: 7,
        view: Value::Argument(2),
        indices: vec![Value::Argument(4)].into_boxed_slice(),
    })
}

#[test]
fn cpu_read_premise_matcher_refuses_repeated_expression_occurrences() {
    let load = load_site(1, 3);
    let repeated = Expr::Compare {
        operation: ProductionSemanticComparisonV2::Equal,
        operand_scalar: ProductionSemanticScalarTypeV2::Float { bits: 32 },
        lhs: Box::new(load.clone()),
        rhs: Box::new(load.clone()),
    };
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    check_read_occurrence(&load, (1, 3), &mut budget, |_, _| Ok(())).unwrap();
    let result = check_read_occurrence(&repeated, (1, 3), &mut budget, |_, _| Ok(()));
    assert!(matches!(
        result,
        Err(Error::UnsupportedReference(
            "conditional CPU input supports only one value-expression load occurrence"
        ))
    ));
}

#[test]
fn cpu_read_premise_matcher_refuses_unused_same_input_different_sites() {
    let load = load_site(1, 3);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    for other_site in [(1, 4), (2, 3)] {
        // Identical input, view and index cannot replace exact site identity.
        let result = check_read_occurrence(&load, other_site, &mut budget, |_, _| {
            panic!("an absent site cannot reach the source binding predicate")
        });
        assert!(matches!(
            result,
            Err(Error::UnsupportedReference(
                "conditional CPU read occurrence is absent from the matched value expression"
            ))
        ));
    }
}

#[test]
fn cpu_read_premise_matching_keeps_access_and_address_domains_separate() {
    let access = Premise::ReadableInput {
        parameter: 3,
        domain: Domain::GuardedOutput,
    };
    let address = Premise::RepresentableAddress {
        parameter: 3,
        domain: Domain::GlobalLaunch,
        element_bytes: 4,
        alignment: 4,
    };
    let separate = Premise::SeparateInputOutput {
        input: 3,
        output: 5,
    };
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(31).unwrap();
    for required in [access, address, separate] {
        premise(&[access, address, separate], required, &mut budget).unwrap();
    }
    assert_eq!(budget.storage(), 31);
    for substituted in [
        Premise::ReadableInput {
            parameter: 4,
            domain: Domain::GuardedOutput,
        },
        Premise::ReadableInput {
            parameter: 3,
            domain: Domain::GlobalLaunch,
        },
        Premise::RepresentableAddress {
            parameter: 3,
            domain: Domain::GuardedOutput,
            element_bytes: 4,
            alignment: 4,
        },
        Premise::RepresentableAddress {
            parameter: 3,
            domain: Domain::GlobalLaunch,
            element_bytes: 8,
            alignment: 4,
        },
        Premise::RepresentableAddress {
            parameter: 3,
            domain: Domain::GlobalLaunch,
            element_bytes: 4,
            alignment: 8,
        },
        Premise::SeparateInputOutput {
            input: 5,
            output: 3,
        },
    ] {
        assert!(premise(&[access, address, separate], substituted, &mut budget).is_err());
    }
    // There is no output-length-dependent exemption for the separate address
    // premise: the implication includes global address formation even at N=0.
    assert!(premise(&[access, separate], address, &mut budget).is_err());
    assert_eq!(budget.storage(), 31);
}

#[test]
fn cpu_read_premise_scan_debits_the_inherited_budget_before_matching() {
    let required = Premise::ReadableInput {
        parameter: 2,
        domain: Domain::GlobalLaunch,
    };
    for (limit, succeeds) in [(8, true), (7, false)] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 31);
        budget.reserve_storage(31).unwrap();
        let account = budget.work_ledger_identity_v1();
        assert_eq!(
            premise(&[required], required, &mut budget).is_ok(),
            succeeds
        );
        assert_eq!(budget.storage(), 31);
        assert!(budget.work_ledger_identity_v1() == account);
    }
}
