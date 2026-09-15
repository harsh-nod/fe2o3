//! Lifecycle checks over real captured maps and bounded direct trace controls.

use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1, Constant, Function,
    Operation as KirOperation, OperationKind, ScalarType, Signature, Terminator, Type, ValueDef,
    ValueId,
};

const WORK: usize = 1_000_000_000_000;
const STORAGE: usize = 512_000_000;

fn observed_map() -> (Owner, Owner, KirOptimizationMapV12) {
    let ty = Type::Scalar(ScalarType::U32);
    let scalar = |id| ValueDef::new(ValueId(id), ty.clone());
    let mut block = BasicBlock::new(BlockId(42));
    for (id, value) in [(2, 4), (3, 5)] {
        block.operations.push(KirOperation::effect_free(
            scalar(id),
            OperationKind::Constant(Constant::U32(value)),
        ));
    }
    block.operations.push(KirOperation::effect_free(
        scalar(4),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(2),
            rhs: ValueId(3),
        },
    ));
    block.operations.push(KirOperation::effect_free(
        scalar(5),
        OperationKind::Binary {
            op: BinaryOp::BitXor,
            lhs: ValueId(0),
            rhs: ValueId(1),
        },
    ));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(4)],
    });
    let mut module = Module::new("lifecycle");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![ty.clone(), ty.clone()], vec![ty]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));

    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (input, _) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    let (mut graph, imported) = crate::KirPlironGraphV12::import(&input, &mut budget).unwrap();
    budget.reserve_storage(imported.retained_storage()).unwrap();
    let (report, execution) = graph
        .execute_production_optimization_v12(&mut budget)
        .unwrap();
    budget
        .reserve_storage(execution.retained_storage())
        .unwrap();
    let (output, _, map, extracted) = graph
        .extract_optimized_canonical_kir_module_with_map_v12(&mut budget)
        .unwrap();
    budget
        .reserve_storage(extracted.retained_storage())
        .unwrap();
    assert!(map.matches_execution(&report));
    let graph_storage = graph.retained_storage();
    drop(graph);
    budget.release_storage(graph_storage).unwrap();
    drop(report);
    budget
        .release_storage(execution.retained_storage())
        .unwrap();
    // Transfer the returned output/map and their receipt out of this fixture ledger.
    budget
        .release_storage(extracted.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), 0);
    (input, output, map)
}

fn check_with_seeded_prefix(
    map: &KirOptimizationMapV12,
    input: &Owner,
    output: &Owner,
) -> Result<()> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    work.charge_work(11).unwrap();
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(7).unwrap();
    assert!(budget.reserve_storage(usize::MAX).is_err());
    assert!(budget.charge_work(usize::MAX).is_err());
    let result = map.check_against(input, output, &mut budget);
    assert_eq!(budget.storage(), 7);
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
    // Exact census/envelope boundaries are exercised separately. Lifecycle
    // rejection must preserve the completed precharge, not reset its prefix.
    assert!(budget.work() > 11);
    assert_eq!(work.failed_work(), Some(usize::MAX));
    result
}

#[test]
fn real_observed_creation_erasure_and_replacement_remain_admitted() {
    let (input, output, map) = observed_map();
    assert!(
        map.events
            .iter()
            .any(|event| matches!(event.change, Change::Create(_)))
    );
    assert!(
        map.events
            .iter()
            .any(|event| matches!(event.change, Change::Erase(_)))
    );
    assert!(
        map.events
            .iter()
            .any(|event| matches!(event.change, Change::Replace(_, _)))
    );
    check_with_seeded_prefix(&map, &input, &output).unwrap();
}

#[test]
fn recomputed_digest_does_not_admit_invalid_lifetime_order_or_event_kind() {
    for mutation in 0..4 {
        let (input, output, mut map) = observed_map();
        match mutation {
            0 => {
                let position = map.events.windows(2).position(|pair| {
                    matches!((pair[0].change, pair[1].change),
                        (Change::Erase(result), Change::Erase(producer))
                        if map.nodes[result as usize].kind == (Kind::Value { producer: Some(producer) }))
                }).expect("DCE observes result erasure before its producer");
                map.events.swap(position, position + 1);
            }
            1 => {
                let position = map.events.windows(2).position(|pair| {
                    matches!((pair[0].change, pair[1].change),
                        (Change::Create(producer), Change::Create(result))
                        if map.nodes[result as usize].kind == (Kind::Value { producer: Some(producer) }))
                }).expect("SCCP observes the materialized producer before its result");
                map.events.swap(position, position + 1);
            }
            2 | 3 => {
                let (position, value, producer) = map
                    .events
                    .iter()
                    .enumerate()
                    .find_map(|(i, event)| {
                        let Change::Replace(value, _) = event.change else {
                            return None;
                        };
                        let Kind::Value {
                            producer: Some(producer),
                        } = map.nodes[value as usize].kind
                        else {
                            return None;
                        };
                        Some((i, value, producer))
                    })
                    .expect("SCCP observes a live arithmetic result replacement");
                map.events[position].change = if mutation == 2 {
                    Change::Move(value)
                } else {
                    Change::Modify(producer)
                };
            }
            _ => unreachable!(),
        }
        // This must fail lifecycle validation, not merely the digest comparison.
        map.digest = map.compute_digest();
        assert_eq!(
            check_with_seeded_prefix(&map, &input, &output),
            Err(KirOptimizationMapErrorV12::Lifecycle),
            "mutation {mutation}"
        );
    }
}

fn passes_for(events: usize) -> Vec<PassSpan> {
    KIR_PLIRON_PRODUCTION_PASSES_V12
        .iter()
        .enumerate()
        .map(|(index, pass)| PassSpan {
            pass: *pass,
            input_epoch: if index == 0 { 1 } else { 2 },
            output_epoch: 2,
            start: if index == 0 { 0 } else { events },
            end: events,
        })
        .collect()
}

#[test]
fn counted_multi_result_and_zero_result_lifecycles_accept_only_live_producers() {
    let nodes = [
        Node {
            kind: Kind::Operation,
            input: None,
            result_index: None,
        },
        Node {
            kind: Kind::Value { producer: Some(0) },
            input: None,
            result_index: Some(0),
        },
        Node {
            kind: Kind::Value { producer: Some(0) },
            input: None,
            result_index: Some(1),
        },
        Node {
            kind: Kind::Operation,
            input: None,
            result_index: None,
        },
    ];
    let changes = [
        Change::Create(0),
        Change::Create(1),
        Change::Create(2),
        Change::Modify(1),
        Change::Move(0),
        Change::Erase(1),
        Change::Erase(2),
        Change::Erase(0),
        Change::Create(3),
        Change::Move(3),
        Change::Erase(3),
    ];
    let events = changes
        .into_iter()
        .map(|change| Event { pass: 0, change })
        .collect::<Vec<_>>();
    let passes = passes_for(events.len());
    validate_lifecycle(&nodes, &events, &[None; 4], &passes).unwrap();

    let mut producer_too_early = events.clone();
    producer_too_early.swap(6, 7);
    assert_eq!(
        validate_lifecycle(&nodes, &producer_too_early, &[None; 4], &passes),
        Err(KirOptimizationMapErrorV12::Lifecycle)
    );

    let mut result_too_early = events.clone();
    result_too_early.swap(0, 1);
    assert_eq!(
        validate_lifecycle(&nodes, &result_too_early, &[None; 4], &passes),
        Err(KirOptimizationMapErrorV12::Lifecycle)
    );
}

#[test]
fn lifecycle_counter_scratch_stays_within_the_existing_profile() {
    // Existing B=37 profile, unchanged by the additional per-node counter.
    let limits = CaptureLimitsV12::for_bytes(37).unwrap();
    assert_eq!(limits.work().unwrap(), 1_227_648);
    assert_eq!(limits.storage().unwrap(), 216_064);
    // On the production 64-bit host this is 1,380 logical payload bytes,
    // inside the unchanged 70,656-byte N-dependent scratch component.
    let scratch = 138 * (2 * size_of::<bool>() + size_of::<usize>());
    assert!(scratch <= 512 * 138);
}
