use super::*;
use crate::OperationHandleError as Error;
use crate::graph_analysis_v1::snapshot_policy_v1::SnapshotPolicyV1 as Policy;
use sha2::Sha256;
use std::{cell::Cell, fmt};

fn hard_work() -> usize {
    crate::production_analysis::ProductionAnalysisResourceLimitsV1::production_hard_ceiling()
        .max_work()
}
fn policy(bytes: usize) -> Policy {
    Policy::new(bytes, hard_work()).unwrap()
}
fn old_digest(text: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(crate::graph_analysis_v1::OPERATION_GRAPH_DIGEST_DOMAIN_V1);
    hash.update((text.len() as u64).to_le_bytes());
    hash.update(text.as_bytes());
    hash.finalize().into()
}
fn hash(policy: &mut Policy, text: &str) -> Result<[u8; 32], Error> {
    policy.digest(|sink| sink.write_str(text))
}

#[test]
fn snapshot_differential_old_bytes_utf8_and_exact_cap() {
    for text in ["", "module @sample {}", "λ中🦀\n"] {
        let mut selected = policy(text.len());
        assert_eq!(hash(&mut selected, text).unwrap(), old_digest(text));
        assert_eq!(selected.observation().1, 1);
        assert_eq!(selected.observation().2, None);
    }
}
#[test]
fn snapshot_one_short_utf8_byte_cap_is_sticky_and_skips_second_pass() {
    let calls = Cell::new(0);
    let text = "λ中";
    let mut selected = policy(text.len() - 1);
    let error = selected
        .digest(|sink| {
            calls.set(calls.get() + 1);
            sink.write_str(text)
        })
        .unwrap_err();
    assert_eq!(
        error,
        Error::OperationGraphSnapshotResourceLimit {
            resource: "presentation UTF8 bytes"
        }
    );
    assert_eq!(calls.get(), 1);
    let work = selected.observation().0;
    assert_eq!(
        selected.digest(|_| panic!("sticky failure must not render")),
        Err(error)
    );
    assert_eq!(selected.observation(), (work, 0, Some(error)));
}
#[test]
fn snapshot_exact_and_one_short_work_are_admitted_before_hash() {
    let text = "abc";
    let exact = crate::graph_analysis_v1::OPERATION_GRAPH_DIGEST_DOMAIN_V1.len()
        + 14
        + 3 * (text.len() + 1);
    let mut selected = Policy::new(text.len(), exact).unwrap();
    assert_eq!(hash(&mut selected, text), Ok(old_digest(text)));
    assert_eq!(selected.observation(), (exact, 1, None));
    let mut short = Policy::new(text.len(), exact - 1).unwrap();
    assert!(matches!(
        hash(&mut short, text),
        Err(Error::OperationGraphSnapshotResourceLimit {
            resource: "presentation hash work"
        })
    ));
    assert!(short.observation().0 <= exact - 1);
    assert_eq!(short.observation().1, 0);
}
#[test]
fn snapshot_existing_ceiling_rejects_larger_or_overflowing_configuration() {
    assert!(Policy::new(crate::HARD_MAX_OPERATION_IMPORT_BYTES, hard_work()).is_some());
    assert!(Policy::new(crate::HARD_MAX_OPERATION_IMPORT_BYTES + 1, hard_work()).is_none());
    assert!(Policy::new(0, hard_work().checked_add(1).unwrap()).is_none());
    assert!(Policy::new(usize::MAX, usize::MAX).is_none());
}
#[test]
fn snapshot_zero_work_refuses_before_any_rendering() {
    let mut selected = Policy::new(0, 0).unwrap();
    assert!(matches!(
        selected.digest(|_| panic!("unpaid render")),
        Err(Error::OperationGraphSnapshotResourceLimit {
            resource: "presentation hash work"
        })
    ));
    assert_eq!(selected.observation().0, 0);
}
#[test]
fn snapshot_changed_same_length_between_passes_refuses() {
    let calls = Cell::new(0);
    let mut selected = policy(3);
    assert_eq!(
        selected.digest(|sink| {
            let call = calls.get();
            calls.set(call + 1);
            sink.write_str(if call == 0 { "abc" } else { "abd" })
        }),
        Err(Error::OperationGraphPresentationChanged)
    );
    assert_eq!(calls.get(), 2);
    assert_eq!(selected.observation().1, 0);
    assert_eq!(
        hash(&mut selected, "abc"),
        Err(Error::OperationGraphPresentationChanged)
    );
}
#[test]
fn snapshot_changed_length_and_second_pass_cap_are_distinct_refusals() {
    for (cap, expected) in [
        (4, Error::OperationGraphPresentationChanged),
        (
            3,
            Error::OperationGraphSnapshotResourceLimit {
                resource: "presentation UTF8 bytes",
            },
        ),
    ] {
        let calls = Cell::new(0);
        let mut selected = policy(cap);
        assert_eq!(
            selected.digest(|sink| {
                let call = calls.get();
                calls.set(call + 1);
                sink.write_str(if call == 0 { "abc" } else { "abcd" })
            }),
            Err(expected)
        );
        assert_eq!(selected.observation().1, 0);
    }
}
#[test]
fn snapshot_formatter_error_cannot_publish_and_is_sticky() {
    let mut selected = policy(3);
    assert_eq!(
        selected.digest(|_| Err(fmt::Error)),
        Err(Error::OperationGraphPresentationRejected)
    );
    assert_eq!(
        hash(&mut selected, "abc"),
        Err(Error::OperationGraphPresentationRejected)
    );
}
#[test]
fn snapshot_formatter_cannot_swallow_a_sink_limit_error() {
    let mut selected = policy(2);
    assert_eq!(
        selected.digest(|sink| {
            let _ = sink.write_str("abc");
            Ok(())
        }),
        Err(Error::OperationGraphSnapshotResourceLimit {
            resource: "presentation UTF8 bytes"
        })
    );
    assert_eq!(selected.observation().1, 0);
}
#[test]
fn snapshot_panic_is_caught_and_first_failure_survives() {
    let mut selected = policy(3);
    assert_eq!(
        selected.digest(|_| panic!("controlled printer panic")),
        Err(Error::UpstreamPanicked)
    );
    assert_eq!(hash(&mut selected, "abc"), Err(Error::UpstreamPanicked));
}
#[test]
fn snapshot_fragmentation_does_not_change_digest_but_costs_each_write() {
    let text = "λ中";
    let mut whole = policy(text.len());
    let mut split = policy(text.len());
    assert_eq!(
        hash(&mut whole, text).unwrap(),
        split
            .digest(|sink| {
                sink.write_str("λ")?;
                sink.write_str("中")
            })
            .unwrap()
    );
    assert_eq!(split.observation().0, whole.observation().0 + 3);
}
#[test]
fn snapshot_empty_writes_are_not_free_and_ignored_refusal_stays_failed() {
    let mut selected = Policy::new(0, 8).unwrap();
    assert_eq!(
        selected.digest(|sink| {
            for _ in 0..16 {
                let _ = sink.write_str("");
            }
            Ok(())
        }),
        Err(Error::OperationGraphSnapshotResourceLimit {
            resource: "presentation hash work"
        })
    );
    assert!(selected.observation().0 <= 8);
}
#[test]
fn snapshot_work_is_cumulative_across_successes_not_reset_per_graph() {
    let text = "x";
    let one = crate::graph_analysis_v1::OPERATION_GRAPH_DIGEST_DOMAIN_V1.len() + 14 + 3 * 2;
    let mut selected = Policy::new(1, one).unwrap();
    assert!(hash(&mut selected, text).is_ok());
    assert!(hash(&mut selected, text).is_err());
    assert_eq!(selected.observation().1, 1);
}

// Original typed read constructor, not authenticated Rust-source or GPU evidence.
fn construction(index_value: u64) -> ProductionConstructionV1 {
    use ProductionRankedOperationV1 as Op;
    use ProductionRankedValueV1 as Value;
    let view = ProductionRankedValueIdV1::new(0);
    let index = ProductionRankedValueIdV1::new(1);
    let kernel = ProductionRankedKernelV1::new(
        "checked",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                Op::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [1; 3],
                    workgroup_extents: [1; 3],
                    subgroup_size: 1,
                    full_physical_workgroups: true,
                },
                Op::View {
                    result: view,
                    element_width: 32,
                    writable: false,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                Op::IndexConstant {
                    result: index,
                    value: index_value,
                },
                Op::Access {
                    kind: AccessKindAttr::Read,
                    view: Value::Local(view),
                    indices: vec![Value::Local(index)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    ProductionConstructionV1::ranked_kernel("snapshot_root", kernel).unwrap()
}
fn compile_snapshot(
    index: u64,
    bytes: usize,
    work: usize,
) -> Result<ProductionRankedKernelLoweringInputV1, ProductionRankedCompileErrorV1> {
    compile_ranked_kernel_for_lowering_with_resource_policy_v1(
        construction(index),
        ProductionSessionLimitsV1::default(),
        None,
        crate::production_analysis::ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        Some(Policy::new(bytes, work).unwrap()),
    )
}
#[test]
fn snapshot_actual_same_graph_digest_equals_original_string_route() {
    let mut session = crate::PlironSession::new(crate::ShellLimits::default(), []).unwrap();
    let root = session.create_module("same_graph").unwrap();
    let old = session.operation_graph_snapshot_v1(&root).unwrap();
    assert!(session.snapshot_policy.is_none());
    // Test-only selection on one existing graph gives an exact old/new digest oracle.
    // Production has no setter; its optional policy is installed by construction.
    session.snapshot_policy = Some(policy(crate::HARD_MAX_OPERATION_IMPORT_BYTES));
    assert_eq!(session.operation_graph_snapshot_v1(&root).unwrap(), old);
    assert_eq!(session.snapshot_policy.as_ref().unwrap().observation().1, 1);
}
#[test]
fn snapshot_actual_ranked_route_retains_selected_policy_and_same_kernel() {
    let original = compile_ranked_kernel_for_lowering_v1(
        construction(0),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    let bounded = compile_snapshot(0, crate::HARD_MAX_OPERATION_IMPORT_BYTES, hard_work()).unwrap();
    assert!(original._session.inner.snapshot_policy.is_none());
    let (work, count, failed) = bounded
        ._session
        .inner
        .snapshot_policy
        .as_ref()
        .unwrap()
        .observation();
    assert!(work > 0 && count > 0);
    assert_eq!(failed, None);
    assert_eq!(original.kernel(), bounded.kernel());
    assert_eq!(
        original.exact_graph_identity(),
        bounded.exact_graph_identity()
    );
    assert!(bounded.all_mandatory_reports_are_clean());
    assert!(!bounded.grants_artifact_or_launch_authority());
}
#[test]
fn snapshot_policy_is_selected_before_first_create_module_snapshot() {
    let mut session = ProductionPlironSessionV1::new_ranked_with_resource_policy_v1(
        ProductionSessionLimitsV1::default(),
        crate::production_analysis::ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        Some(policy(0)),
    )
    .unwrap();
    assert!(matches!(
        session.inner.create_module("refuse"),
        Err(Error::OperationGraphSnapshotResourceLimit {
            resource: "presentation UTF8 bytes"
        })
    ));
    assert!(session.inner.is_poisoned());
    assert_eq!(
        session
            .inner
            .snapshot_policy
            .as_ref()
            .unwrap()
            .observation()
            .1,
        0
    );
    assert!(matches!(
        session.inner.create_module("retry"),
        Err(Error::SessionPoisoned)
    ));
}
#[test]
fn snapshot_policy_does_not_bypass_actual_ranked_bounds_refusal() {
    assert!(matches!(
        compile_snapshot(1, crate::HARD_MAX_OPERATION_IMPORT_BYTES, hard_work()),
        Err(ProductionRankedCompileErrorV1::Session(
            ProductionSessionErrorV1::RankedBounds(_)
        ))
    ));
}
#[test]
fn snapshot_policy_does_not_replace_analysis_allowance() {
    let result = compile_ranked_kernel_for_lowering_with_resource_policy_v1(
        construction(0),
        ProductionSessionLimitsV1::default(),
        None,
        crate::production_analysis::ProductionAnalysisResourceLimitsV1::new(0, 0),
        Some(policy(crate::HARD_MAX_OPERATION_IMPORT_BYTES)),
    );
    assert!(matches!(
        result,
        Err(ProductionRankedCompileErrorV1::Session(
            ProductionSessionErrorV1::AnalysisResourceLimit { .. }
        ))
    ));
}
