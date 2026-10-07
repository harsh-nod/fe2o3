use super::{Plan, Result};
use fe2o3_runtime::KfdRuntimeXgmiSegmentsObservationV1;
use std::fmt::Write;

pub(super) fn rows(
    records: Vec<KfdRuntimeXgmiSegmentsObservationV1>,
    ids: [u64; 2],
    plan: &Plan,
    times: &[u128],
) -> Result<Vec<String>> {
    if records.len() != plan.bands * 2 || times.len() != records.len() {
        return Err("incomplete ordered attribution roster".into());
    }
    records
        .into_iter()
        .enumerate()
        .map(|(ordinal, record)| {
            let host = record.host;
            let sum = [
                host.admission_validation_ns,
                host.preparation_ns,
                host.opening_currentness_ns,
                host.submission_ns,
                host.wait_ns,
                host.closing_currentness_ns,
                host.settlement_ns,
            ]
            .into_iter()
            .try_fold(0u64, u64::checked_add);
            // Two allocation/allocation/stream preparations consume handles 1..=6.
            if record.backend_submission != ordinal as u64 + 7
                || record.source_device != ids[ordinal % 2]
                || record.destination_device != ids[1 - ordinal % 2]
                || record.descriptor_count as usize != plan.segments.len()
                || record.useful_bytes != plan.useful as u64
                || host.submission_calls != record.descriptor_count
                || host.wait_calls != record.descriptor_count
                || host.total_ns == 0
                || u128::from(host.total_ns) > times[ordinal]
                || sum.is_none_or(|sum| sum > host.total_ns)
                || !host.opening.is_complete()
                || !host.closing.is_complete()
                || host
                    .opening
                    .total_ns
                    .is_none_or(|n| n > host.opening_currentness_ns)
                || host
                    .closing
                    .total_ns
                    .is_none_or(|n| n > host.closing_currentness_ns)
            {
                return Err("invalid joined ordered attribution".into());
            }
            Ok(row(ordinal, record))
        })
        .collect()
}

fn row(ordinal: usize, record: KfdRuntimeXgmiSegmentsObservationV1) -> String {
    let host = record.host;
    let mut row = format!(
        "schema=fe2o3.xgmi-ordered-segments-host-attribution.v1 backend=kfd ordinal={ordinal} backend_submission={} source_uid={:016x} destination_uid={:016x} descriptor_count={} useful_bytes={} submission_calls={} wait_calls={} admission_validation_ns={} preparation_ns={} opening_currentness_ns={} submission_ns={} wait_ns={} closing_currentness_ns={} settlement_ns={} total_ns={}",
        record.backend_submission,
        record.source_device,
        record.destination_device,
        record.descriptor_count,
        record.useful_bytes,
        host.submission_calls,
        host.wait_calls,
        host.admission_validation_ns,
        host.preparation_ns,
        host.opening_currentness_ns,
        host.submission_ns,
        host.wait_ns,
        host.closing_currentness_ns,
        host.settlement_ns,
        host.total_ns,
    );
    for (prefix, pair) in [("opening", host.opening), ("closing", host.closing)] {
        for (name, value) in [
            ("source_before_ns", pair.source_before_ns),
            ("peer_before_ns", pair.peer_before_ns),
            ("topology_discovery_ns", pair.topology_discovery_ns),
            ("route_and_equality_ns", pair.route_and_equality_ns),
            ("source_after_ns", pair.source_after_ns),
            ("peer_after_ns", pair.peer_after_ns),
            ("pair_total_ns", pair.total_ns),
            ("topology_tree_ns", pair.topology.topology_tree_ns),
            (
                "topology_initial_identity_ns",
                pair.topology.initial_identity_ns,
            ),
            (
                "topology_render_correlation_ns",
                pair.topology.render_correlation_ns,
            ),
            (
                "topology_closing_identity_ns",
                pair.topology.closing_identity_ns,
            ),
            ("topology_total_ns", pair.topology.total_ns),
        ] {
            write!(
                &mut row,
                " {prefix}_{name}={}",
                value.expect("validated complete currentness")
            )
            .expect("format into string");
        }
    }
    row.push_str(
        " authority=none teardown=explicit timing=backend-ordered-segments-currentness-host-only",
    );
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kfd::{Gfx942TopologyDiscoveryDiagnosticsV1, Gfx942XgmiPairCurrentnessDiagnosticsV1};
    use fe2o3_runtime::KfdRuntimeXgmiSegmentsTimingV1;

    fn records(plan: &Plan) -> Vec<KfdRuntimeXgmiSegmentsObservationV1> {
        let pair = Gfx942XgmiPairCurrentnessDiagnosticsV1 {
            source_before_ns: Some(1),
            peer_before_ns: Some(1),
            topology_discovery_ns: Some(4),
            route_and_equality_ns: Some(1),
            source_after_ns: Some(1),
            peer_after_ns: Some(1),
            total_ns: Some(9),
            topology: Gfx942TopologyDiscoveryDiagnosticsV1 {
                topology_tree_ns: Some(1),
                initial_identity_ns: Some(1),
                render_correlation_ns: Some(1),
                closing_identity_ns: Some(1),
                total_ns: Some(4),
            },
        };
        (0..plan.bands * 2)
            .map(|ordinal| KfdRuntimeXgmiSegmentsObservationV1 {
                backend_submission: ordinal as u64 + 7,
                source_device: [71, 93][ordinal % 2],
                destination_device: [93, 71][ordinal % 2],
                descriptor_count: plan.segments.len() as u32,
                useful_bytes: plan.useful as u64,
                host: KfdRuntimeXgmiSegmentsTimingV1 {
                    admission_validation_ns: 1,
                    preparation_ns: 1,
                    opening_currentness_ns: 10,
                    submission_ns: 1,
                    wait_ns: 1,
                    closing_currentness_ns: 10,
                    settlement_ns: 1,
                    submission_calls: plan.segments.len() as u32,
                    wait_calls: plan.segments.len() as u32,
                    total_ns: 25,
                    opening: pair,
                    closing: pair,
                },
            })
            .collect()
    }

    #[test]
    fn exact_roster_serializes_only_complete_bounded_nested_intervals() {
        let plan = Plan::new(65536, 65, 2, 10).unwrap();
        let records = records(&plan);
        let output = rows(records.clone(), [71, 93], &plan, &[30; 26]).unwrap();
        assert_eq!(output.len(), 26);
        for (ordinal, row) in output.iter().enumerate() {
            let fields: std::collections::BTreeMap<_, _> = row
                .split_whitespace()
                .map(|field| field.split_once('=').unwrap())
                .collect();
            assert_eq!(fields.len(), 45);
            assert_eq!(fields["ordinal"], ordinal.to_string());
            assert_eq!(fields["backend_submission"], (ordinal + 7).to_string());
            assert_eq!(fields["descriptor_count"], "65");
            assert_eq!(fields["useful_bytes"], "65536");
            assert_eq!(fields["authority"], "none");
            assert_eq!(fields["teardown"], "explicit");
        }
        assert!(rows(records.clone(), [93, 71], &plan, &[30; 26]).is_err());
        assert!(rows(records.clone(), [71, 93], &plan, &[24; 26]).is_err());
        assert!(rows(records.clone(), [71, 93], &plan, &[30; 25]).is_err());
        assert!(rows(records[..25].to_vec(), [71, 93], &plan, &[30; 26]).is_err());
        for mutation in 0..12 {
            let mut changed = records.clone();
            let row = &mut changed[25];
            match mutation {
                0 => row.backend_submission -= 1,
                1 => row.source_device = 0,
                2 => row.destination_device = 0,
                3 => row.descriptor_count = 64,
                4 => row.useful_bytes = 65535,
                5 => row.host.submission_calls = 64,
                6 => row.host.wait_calls = 64,
                7 => row.host.total_ns = 0,
                8 => row.host.wait_ns = u64::MAX,
                9 => row.host.opening.total_ns = Some(11),
                10 => row.host.closing.peer_before_ns = None,
                11 => row.host.opening.topology.total_ns = Some(5),
                _ => unreachable!(),
            }
            assert!(
                rows(changed, [71, 93], &plan, &[30; 26]).is_err(),
                "mutation {mutation}"
            );
        }
    }
}
