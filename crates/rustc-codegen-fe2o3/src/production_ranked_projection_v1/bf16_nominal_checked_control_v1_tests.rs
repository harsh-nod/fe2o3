//! Synthetic fixed-container controls only. They cannot construct a real
//! prepared-flow loan or substitute for genuine source/canonical qualification.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
const LIMIT: usize = 1_000_000;
fn fixture() -> (
    Controls,
    Vec<ProductionRankedBlockV1>,
    [Option<usize>; MAX_BLOCKS],
) {
    let mut controls = Controls::empty();
    controls.prepared = true;
    controls.length_count = 1;
    controls.lengths[0] = Some(Length {
        parameter: 7,
        source_local: 11,
        source_argument: 1,
        slot: 1,
    });
    controls.sites[0] = Some(Site {
        fact: NominalCheckedViewV1::synthetic(0, 7, 11, 1, 1, 2, 256),
        slot: 1,
        constant: ProductionRankedValueIdV1::new(99),
        ranked_block: Some(1),
    });
    let mut bases = [None; MAX_BLOCKS];
    bases[0] = Some(1);
    bases[1] = Some(2);
    bases[2] = Some(3);
    let blocks = vec![
        ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::IndexConstant {
                result: ProductionRankedValueIdV1::new(99),
                value: 256,
            }],
            ProductionRankedTerminatorV1::Branch { target: 1 },
        ),
        ProductionRankedBlockV1::new(
            vec![],
            ProductionRankedTerminatorV1::IndexLessThan {
                lhs: ProductionRankedValueV1::Argument(1),
                rhs: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(99)),
                true_block: 3,
                false_block: 2,
            },
        ),
        ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return),
        ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Trap),
    ];
    (controls, blocks, bases)
}
fn validate(
    controls: &Controls,
    blocks: &[ProductionRankedBlockV1],
    bases: &[Option<usize>; MAX_BLOCKS],
    work_limit: usize,
) -> (bool, usize, bool) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let result = controls.validate(
        blocks,
        bases,
        &mut PreparationResourcesV1::new(&mut budget, &mut owned),
    );
    assert_eq!(budget.storage(), 0);
    assert_eq!(owned, 0);
    (
        result.is_ok(),
        budget.work(),
        budget.failed_work().is_some(),
    )
}
#[test]
fn exact_dynamic_length_comparison_keeps_failure_and_success_paths() {
    let (controls, blocks, bases) = fixture();
    assert_eq!(controls.argument_count(), 2);
    assert!(validate(&controls, &blocks, &bases, LIMIT).0);
    let site = controls.site(0).unwrap();
    assert_eq!(
        expected_terminator(site, &bases).unwrap(),
        *blocks[1].terminator()
    );
    assert_eq!(controls.site_at_ranked(1), Some(site));
    assert_eq!(controls.site_at_ranked(2), None);
}
#[test]
fn zero_extent_slot_wrong_identity_missing_constants_and_edge_swaps_refuse() {
    for mutation in 0..26 {
        let (mut controls, mut blocks, mut bases) = fixture();
        match mutation {
            0 => controls.length_count = 0,
            1 => controls.length_count = MAX_BLOCKS + 1,
            2 => controls.lengths[0] = None,
            3 => controls.lengths[0].as_mut().unwrap().slot = 0,
            4 => controls.lengths[0].as_mut().unwrap().parameter = 8,
            5 => controls.sites[0].as_mut().unwrap().slot = 0,
            6 => controls.sites[0].as_mut().unwrap().slot = 2,
            7 => {
                controls.sites[0].as_mut().unwrap().fact =
                    NominalCheckedViewV1::synthetic(0, 7, 11, 1, 1, 2, 257)
            }
            8 => {
                controls.sites[0].as_mut().unwrap().fact =
                    NominalCheckedViewV1::synthetic(1, 7, 11, 1, 1, 2, 256)
            }
            9 => {
                controls.sites[0].as_mut().unwrap().fact =
                    NominalCheckedViewV1::synthetic(0, 7, 11, 1, 1, 1, 256)
            }
            10 => {
                controls.length_count = 2;
                controls.lengths[1] = Some(Length {
                    slot: 2,
                    ..controls.lengths[0].unwrap()
                });
            }
            11 => controls.prepared = false,
            12 => controls.sites[0].as_mut().unwrap().ranked_block = None,
            13 => controls.sites[0].as_mut().unwrap().ranked_block = Some(0),
            14 | 15 | 16 => {
                blocks[1] = ProductionRankedBlockV1::new(
                    vec![],
                    ProductionRankedTerminatorV1::IndexLessThan {
                        lhs: ProductionRankedValueV1::Argument(if mutation == 15 { 0 } else { 1 }),
                        rhs: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(
                            if mutation == 16 { 100 } else { 99 },
                        )),
                        true_block: if mutation == 14 { 2 } else { 3 },
                        false_block: if mutation == 14 { 3 } else { 2 },
                    },
                )
            }
            17 | 18 | 19 => {
                let mut operations = vec![];
                if mutation != 19 {
                    operations.push(ProductionRankedOperationV1::IndexConstant {
                        result: ProductionRankedValueIdV1::new(99),
                        value: if mutation == 17 { 255 } else { 256 },
                    });
                }
                if mutation == 18 {
                    operations.push(ProductionRankedOperationV1::IndexConstant {
                        result: ProductionRankedValueIdV1::new(99),
                        value: 256,
                    });
                }
                blocks[0] = ProductionRankedBlockV1::new(
                    operations,
                    ProductionRankedTerminatorV1::Branch { target: 1 },
                );
            }
            20 => bases[2] = None,
            21 => {
                controls.sites[2] = Some(Site {
                    fact: NominalCheckedViewV1::synthetic(2, 7, 11, 1, 1, 2, 256),
                    ..controls.sites[0].unwrap()
                });
            }
            22 => controls.lengths[MAX_BLOCKS - 1] = controls.lengths[0],
            23 => controls.lengths[0].as_mut().unwrap().slot = 2,
            24 => {
                controls.sites[0].as_mut().unwrap().fact =
                    NominalCheckedViewV1::synthetic(0, 7, 11, 2, 1, 2, 256)
            }
            25 => {
                controls.sites[0].as_mut().unwrap().fact =
                    NominalCheckedViewV1::synthetic(0, 7, 12, 1, 1, 2, 256)
            }
            _ => unreachable!(),
        }
        assert!(
            !validate(&controls, &blocks, &bases, LIMIT).0,
            "mutation {mutation}"
        );
    }
}
#[test]
fn only_exact_parameter_and_source_identity_deduplicate() {
    let mut controls = Controls::empty();
    controls.prepared = true;
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut meter = PreparationResourcesV1::new(&mut budget, &mut owned);
    assert_eq!(controls.length_slot(91, 11, 0, &mut meter).unwrap(), 1);
    assert_eq!(controls.length_slot(7, 22, 1, &mut meter).unwrap(), 2);
    assert_eq!(controls.length_slot(91, 11, 0, &mut meter).unwrap(), 1);
    assert_eq!(controls.argument_count(), 3);
    for (parameter, local, argument) in [(91, 22, 0), (91, 11, 1), (8, 11, 2), (8, 33, 1)] {
        assert!(
            controls
                .length_slot(parameter, local, argument, &mut meter)
                .is_err()
        );
    }
    assert_eq!(controls.argument_count(), 3);
}
#[test]
fn dedicated_namespace_cap_and_exact_work_one_short_are_bounded() {
    let mut controls = Controls::empty();
    controls.prepared = true;
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut meter = PreparationResourcesV1::new(&mut budget, &mut owned);
    for i in 0..MAX_BLOCKS as u32 {
        assert_eq!(
            controls
                .length_slot(100 + i, 200 + i, 300 + i, &mut meter)
                .unwrap(),
            i + 1
        );
    }
    assert!(controls.length_slot(999, 998, 997, &mut meter).is_err());
    assert_eq!(controls.argument_count(), MAX_BLOCKS + 1);
    let (controls, blocks, bases) = fixture();
    let (_, measured, _) = validate(&controls, &blocks, &bases, LIMIT);
    assert!(validate(&controls, &blocks, &bases, measured).0);
    let one_short = validate(&controls, &blocks, &bases, measured - 1);
    assert!(!one_short.0 && one_short.2);
}
#[test]
fn accounting_denials_and_unmetered_requests_cannot_add_length_slots() {
    let mut controls = Controls::empty();
    controls.prepared = true;
    assert!(
        controls
            .length_slot(1, 2, 3, &mut PreparationResourcesV1::unmetered())
            .is_err()
    );
    assert_eq!(controls.argument_count(), 1);
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    assert!(
        controls
            .length_slot(
                1,
                2,
                3,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    assert!(budget.failed_work().is_some());
    assert_eq!(controls.argument_count(), 1);
    assert!(
        controls
            .length_slot(
                1,
                2,
                3,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            )
            .is_err()
    );
}
#[test]
fn emitted_site_is_single_use_and_original_edges_are_an_unordered_pair() {
    let (mut controls, _, _) = fixture();
    assert!(controls.record(0, 4).is_err());
    assert!(controls.record(MAX_BLOCKS, 4).is_err());
    assert!(same_edges(2, 7, 2, 7));
    assert!(same_edges(7, 2, 2, 7));
    for (first, second, success, failure) in
        [(2, 7, 2, 2), (2, 7, 2, 8), (2, 2, 2, 7), (7, 7, 2, 7)]
    {
        assert!(!same_edges(first, second, success, failure));
    }
}
#[test]
fn unchanged_empty_legacy_operand_namespace_stays_one_argument() {
    let controls = Controls::empty();
    let blocks = vec![ProductionRankedBlockV1::new(
        vec![],
        ProductionRankedTerminatorV1::Return,
    )];
    assert_eq!(controls.argument_count(), 1);
    assert!(validate(&controls, &blocks, &[None; MAX_BLOCKS], LIMIT).0);
}
