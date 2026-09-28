use super::*;
use crate::neutral_optimization_v1::storage_v18::tests::{SPACE, WORK, fixture, input, observe};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn copy(map: &KirOptimizationMapPolicy3V18) -> KirOptimizationMapPolicy3V18 {
    let d = &map.data;
    KirOptimizationMapPolicy3V18 {
        data: MapData {
            input: d.input,
            output: d.output,
            nodes: d.nodes.clone(),
            events: d.events.clone(),
            terminal: d.terminal.clone(),
            passes: d.passes.clone(),
            relations: d.relations.clone(),
            targets: d.targets.clone(),
            synthesized: d.synthesized.clone(),
            digest: d.digest,
        },
    }
}

#[test]
fn nominal_map_replays_actual_endpoints_and_rejects_changed_headers_epochs_and_coverage() {
    let input = input(&fixture());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(23 + input.storage).unwrap();
    let output = observe(&input, &mut budget);
    let floor = budget.storage();
    output
        .map()
        .check_against(&input.owner, output.owner(), &mut budget)
        .unwrap();
    assert_eq!(budget.storage(), floor);
    assert_ne!(
        output.map().input_identity(),
        output.map().output_identity()
    );
    for mutation in 0..4 {
        let mut bad = copy(output.map());
        match mutation {
            0 => bad.data.input = bad.data.output,
            1 => bad.data.passes[0].output_epoch += 1,
            2 => {
                bad.data.terminal.pop();
            }
            3 => {
                bad.data.relations.pop();
            }
            _ => unreachable!(),
        }
        // An attacker can recompute diagnostic digests; structural lifecycle
        // and complete endpoint/row coverage still must independently reject.
        bad.data.digest = digest(&bad.data, FixedPolicy::Checked3);
        assert!(
            bad.check_against(&input.owner, output.owner(), &mut budget)
                .is_err(),
            "mutation {mutation}"
        );
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn v18_digest_domain_is_distinct_from_legacy_even_with_the_same_endpoint_octets() {
    let input = input(&fixture());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(input.storage).unwrap();
    let output = observe(&input, &mut budget);
    let map = &output.map().data;
    let historical = map.compute_digest_with_header(
        FixedPolicy::Checked3.map_domain(),
        [
            (map.input.digest(), map.input.canonical_length()),
            (map.output.digest(), map.output.canonical_length()),
        ],
    );
    assert_ne!(*output.map().digest(), historical);
    assert_eq!(*output.map().digest(), digest(map, FixedPolicy::Checked3));
}
