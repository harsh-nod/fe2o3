//! Active-mask and collective-sequencing facts derived from exact PLIRON traces.
//!
//! The analysis recognizes only executable operations retained in the current
//! IR. Proof contracts do not create participation or ordering facts.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use dialect_gpu::HierarchyAttr;

use crate::production_analysis::pliron_invocation_trace::{
    PlironInvocationTraceV1, PlironTraceEventV1, PlironTraceLocationV1,
    ProductionInvocationTraceResourceAdmissionV1,
};
use crate::production_analysis::{
    ProductionAnalysisResourceLimitV1, ProductionAnalysisResourceLimitsV1,
    ProductionAnalysisResourcePhaseV1, ProductionAnalysisResourceUpperBoundV1,
};

pub const MAX_PLIRON_SIMT_PROTOCOL_ISSUES_V1: usize = 4_096;

fn simt_resource_overflow_v1() -> ProductionAnalysisResourceLimitV1 {
    ProductionAnalysisResourceLimitV1 {
        phase: ProductionAnalysisResourcePhaseV1::SimtProtocol,
        resource: "SIMT protocol resource upper bound",
    }
}

fn checked_simt_mul_v1(lhs: usize, rhs: usize) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    lhs.checked_mul(rhs).ok_or_else(simt_resource_overflow_v1)
}

fn checked_simt_sum_v1(items: &[usize]) -> Result<usize, ProductionAnalysisResourceLimitV1> {
    items.iter().try_fold(0_usize, |total, item| {
        total
            .checked_add(*item)
            .ok_or_else(simt_resource_overflow_v1)
    })
}

/// Bounds protocol grouping, sequence comparison, participant sets, and the
/// largest retained hostile report before those collections are constructed.
pub(crate) fn preflight_simt_protocol_resource_upper_bound_v1(
    trace: Option<ProductionInvocationTraceResourceAdmissionV1>,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let Some(trace) = trace else {
        // The trace failure is fixed-size and already cached. SIMT preparation
        // clones it once into its own attempted-cache result.
        let attempt = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            ProductionAnalysisResourcePhaseV1::SimtProtocol,
            1,
            1,
            0,
        )?;
        return limits.require(ProductionAnalysisResourcePhaseV1::SimtProtocol, attempt);
    };
    let events = trace.event_upper_bound();
    let invocations = trace.invocation_count();
    let launch_rank = trace.launch_rank();
    // Each invocation can contribute at most one phase mismatch and each
    // tensor event can independently contribute participation and claimed-mask
    // issues. `push_issue` guards the cap before constructing an attempted
    // issue, so no additional issue payload can coexist at the limit.
    let issues = checked_simt_sum_v1(&[invocations, checked_simt_mul_v1(events, 2)?])?
        .min(MAX_PLIRON_SIMT_PROTOCOL_ISSUES_V1);
    let work = checked_simt_sum_v1(&[
        checked_simt_mul_v1(events, 12)?,
        checked_simt_mul_v1(invocations, 4)?,
    ])?;
    // Groups partition traces: all phase-sequence clones together retain at
    // most two event rosters, all partial-participation findings at most one
    // lane roster per tensor event, and phase invocation coordinates at most
    // two launch-rank vectors per invocation. Fixed diagnostic fields scale
    // with the independently bounded issue count.
    let retained = checked_simt_sum_v1(&[
        checked_simt_mul_v1(events, 3)?,
        checked_simt_mul_v1(checked_simt_mul_v1(launch_rank, invocations)?, 2)?,
        checked_simt_mul_v1(issues, 12)?,
    ])?;
    let temporary = checked_simt_sum_v1(&[
        checked_simt_mul_v1(invocations, 2)?,
        checked_simt_mul_v1(events, 6)?,
    ])?;
    let bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::SimtProtocol,
        work,
        retained,
        temporary,
    )?;
    limits.require(ProductionAnalysisResourcePhaseV1::SimtProtocol, bound)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlironSimtProtocolCoverageV1;

impl PlironSimtProtocolCoverageV1 {
    pub const fn tensor_instruction(self) -> bool {
        true
    }

    pub const fn barrier(self) -> bool {
        true
    }

    pub const fn shuffle(self) -> bool {
        false
    }

    pub const fn reduction(self) -> bool {
        false
    }

    pub const fn async_copy_and_wait(self) -> bool {
        false
    }
}

pub const PLIRON_SIMT_PROTOCOL_COVERAGE_V1: PlironSimtProtocolCoverageV1 =
    PlironSimtProtocolCoverageV1;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlironProtocolLocationV1 {
    block: usize,
    operation: usize,
}

impl From<PlironTraceLocationV1> for PlironProtocolLocationV1 {
    fn from(location: PlironTraceLocationV1) -> Self {
        Self {
            block: location.block,
            operation: location.operation,
        }
    }
}

impl PlironProtocolLocationV1 {
    pub const fn block(self) -> usize {
        self.block
    }

    pub const fn operation(self) -> usize {
        self.operation
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PlironProtocolEventKindV1 {
    TensorInstruction,
    SubgroupBarrier,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PlironProtocolEventV1 {
    kind: PlironProtocolEventKindV1,
    location: PlironProtocolLocationV1,
}

impl PlironProtocolEventV1 {
    pub const fn kind(self) -> PlironProtocolEventKindV1 {
        self.kind
    }

    pub const fn location(self) -> PlironProtocolLocationV1 {
        self.location
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironSimtProtocolIssueV1 {
    PhaseMismatch {
        grid: u64,
        workgroup: u64,
        subgroup: u64,
        first_invocation: Vec<u64>,
        first: Vec<PlironProtocolEventV1>,
        second_invocation: Vec<u64>,
        second: Vec<PlironProtocolEventV1>,
    },
    PartialTensorParticipation {
        grid: u64,
        workgroup: u64,
        subgroup: u64,
        location: PlironProtocolLocationV1,
        expected_lanes: u16,
        actual_lanes: Vec<u64>,
    },
    ClaimedActiveMaskMismatch {
        location: PlironProtocolLocationV1,
        claimed_active_lanes: u32,
        actual_active_lanes: usize,
    },
    ResourceLimitExceeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironSimtProtocolAnalysisV1 {
    issues: Vec<PlironSimtProtocolIssueV1>,
}

impl PlironSimtProtocolAnalysisV1 {
    pub fn issues(&self) -> &[PlironSimtProtocolIssueV1] {
        &self.issues
    }
}

#[derive(Clone, Copy, Debug)]
struct TensorSiteV1 {
    subgroup_width: u16,
    claimed_active_lanes: u32,
}

pub(crate) fn analyze_pliron_simt_protocol_v1(
    traces: &[PlironInvocationTraceV1],
) -> PlironSimtProtocolAnalysisV1 {
    let mut issues = Vec::new();
    let mut groups = BTreeMap::<(u64, u64, u64), Vec<&PlironInvocationTraceV1>>::new();
    for trace in traces {
        groups
            .entry((trace.grid, trace.workgroup, trace.subgroup))
            .or_default()
            .push(trace);
    }

    for ((grid, workgroup, subgroup), group) in groups {
        let mut first_trace = None::<(&PlironInvocationTraceV1, Vec<PlironProtocolEventV1>)>;
        let mut phase_reported = false;
        let mut participants = HashMap::<PlironProtocolLocationV1, BTreeSet<u64>>::new();
        let mut sites = HashMap::<PlironProtocolLocationV1, TensorSiteV1>::new();
        for trace in group {
            let sequence = protocol_sequence(trace);
            let mismatch = first_trace
                .as_ref()
                .is_some_and(|(_, first)| first != &sequence);
            if mismatch && !phase_reported {
                let (first_invocation, first) = first_trace
                    .as_ref()
                    .expect("a mismatched protocol has a baseline trace");
                phase_reported = true;
                if !push_issue(&mut issues, || PlironSimtProtocolIssueV1::PhaseMismatch {
                    grid,
                    workgroup,
                    subgroup,
                    first_invocation: first_invocation.invocation.clone(),
                    first: first.clone(),
                    second_invocation: trace.invocation.clone(),
                    second: sequence.clone(),
                }) {
                    return PlironSimtProtocolAnalysisV1 { issues };
                }
            }
            if first_trace.is_none() {
                first_trace = Some((trace, sequence));
            }
            for event in &trace.events {
                let PlironTraceEventV1::TensorInstruction {
                    location,
                    subgroup_width,
                    claimed_active_lanes,
                } = event
                else {
                    continue;
                };
                let location = (*location).into();
                participants.entry(location).or_default().insert(trace.lane);
                sites.entry(location).or_insert(TensorSiteV1 {
                    subgroup_width: *subgroup_width,
                    claimed_active_lanes: *claimed_active_lanes,
                });
            }
        }

        for (location, site) in sites {
            let actual_lanes = participants.remove(&location).unwrap_or_default();
            if actual_lanes.len() != usize::from(site.subgroup_width)
                && !push_issue(&mut issues, || {
                    PlironSimtProtocolIssueV1::PartialTensorParticipation {
                        grid,
                        workgroup,
                        subgroup,
                        location,
                        expected_lanes: site.subgroup_width,
                        actual_lanes: actual_lanes.iter().copied().collect(),
                    }
                })
            {
                return PlironSimtProtocolAnalysisV1 { issues };
            }
            if actual_lanes.len() != site.claimed_active_lanes as usize
                && !push_issue(&mut issues, || {
                    PlironSimtProtocolIssueV1::ClaimedActiveMaskMismatch {
                        location,
                        claimed_active_lanes: site.claimed_active_lanes,
                        actual_active_lanes: actual_lanes.len(),
                    }
                })
            {
                return PlironSimtProtocolAnalysisV1 { issues };
            }
        }
    }
    PlironSimtProtocolAnalysisV1 { issues }
}

fn protocol_sequence(trace: &PlironInvocationTraceV1) -> Vec<PlironProtocolEventV1> {
    trace
        .events
        .iter()
        .filter_map(|event| match event {
            PlironTraceEventV1::TensorInstruction { location, .. } => Some(PlironProtocolEventV1 {
                kind: PlironProtocolEventKindV1::TensorInstruction,
                location: (*location).into(),
            }),
            PlironTraceEventV1::Barrier {
                location,
                execution_scope: HierarchyAttr::Subgroup,
                ..
            } => Some(PlironProtocolEventV1 {
                kind: PlironProtocolEventKindV1::SubgroupBarrier,
                location: (*location).into(),
            }),
            _ => None,
        })
        .collect()
}

fn push_issue(
    issues: &mut Vec<PlironSimtProtocolIssueV1>,
    make_issue: impl FnOnce() -> PlironSimtProtocolIssueV1,
) -> bool {
    if issues.len() == MAX_PLIRON_SIMT_PROTOCOL_ISSUES_V1 {
        issues.clear();
        issues.push(PlironSimtProtocolIssueV1::ResourceLimitExceeded);
        false
    } else {
        issues.push(make_issue());
        true
    }
}

#[cfg(test)]
mod resource_upper_bound_tests {
    use std::cell::Cell;

    use super::*;
    use crate::production_analysis::ProductionAnalysisInputCensusV1;

    fn trace_admission() -> ProductionInvocationTraceResourceAdmissionV1 {
        super::super::pliron_invocation_trace::invocation_trace_resource_upper_bound_for_shape_v1(
            ProductionAnalysisInputCensusV1 {
                blocks: 2,
                operations: 9,
                max_operation_arity: 3,
                max_successor_arity: 2,
                ..ProductionAnalysisInputCensusV1::default()
            },
            8,
            3,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap()
    }

    #[test]
    fn simt_bound_has_exact_and_one_under_admission() {
        let exact = preflight_simt_protocol_resource_upper_bound_v1(
            Some(trace_admission()),
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        assert!(exact.work_upper_bound() > 0);
        assert!(exact.peak_storage_upper_bound() > 0);
        assert_eq!(
            preflight_simt_protocol_resource_upper_bound_v1(
                Some(trace_admission()),
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound() - 1,
                    exact.peak_storage_upper_bound(),
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::SimtProtocol,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            preflight_simt_protocol_resource_upper_bound_v1(
                Some(trace_admission()),
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound(),
                    exact.peak_storage_upper_bound() - 1,
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::SimtProtocol,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn one_event_reserves_both_independent_finding_paths() {
        let admission =
            ProductionInvocationTraceResourceAdmissionV1::from_counts_for_test_v1(1, 1, 1);
        assert_eq!(admission.event_upper_bound(), 1);
        // One invocation can add one phase issue and one tensor event can add
        // both participation and active-mask issues.
        let issue_count = 1 + 2;
        let exact_work = 12 + 4;
        let exact_retained = 3 + 2 + 12 * issue_count;
        let exact_temporary = 2 + 6;
        let exact_peak = exact_retained + exact_temporary;
        let exact = preflight_simt_protocol_resource_upper_bound_v1(
            Some(admission),
            ProductionAnalysisResourceLimitsV1::new(exact_work, exact_peak),
        )
        .expect("one-event two-finding envelope");
        assert_eq!(exact.work_upper_bound(), exact_work);
        assert_eq!(exact.retained_storage_upper_bound(), exact_retained);
        assert_eq!(exact.peak_storage_upper_bound(), exact_peak);
        assert!(
            preflight_simt_protocol_resource_upper_bound_v1(
                Some(admission),
                ProductionAnalysisResourceLimitsV1::new(exact_work, exact_peak - 1),
            )
            .is_err()
        );
    }

    #[test]
    fn rejected_trace_attempt_is_admitted_and_cached() {
        let exact = preflight_simt_protocol_resource_upper_bound_v1(
            None,
            ProductionAnalysisResourceLimitsV1::new(1, 1),
        )
        .expect("fixed rejected-cache attempt");
        assert_eq!(exact.work_upper_bound(), 1);
        assert_eq!(exact.retained_storage_upper_bound(), 1);
        assert_eq!(exact.peak_storage_upper_bound(), 1);
        assert!(
            preflight_simt_protocol_resource_upper_bound_v1(
                None,
                ProductionAnalysisResourceLimitsV1::new(0, 1),
            )
            .is_err()
        );
        assert!(
            preflight_simt_protocol_resource_upper_bound_v1(
                None,
                ProductionAnalysisResourceLimitsV1::new(1, 0),
            )
            .is_err()
        );
    }

    #[test]
    fn issue_cap_is_checked_before_attempted_payload_construction() {
        let mut issues = vec![
            PlironSimtProtocolIssueV1::ClaimedActiveMaskMismatch {
                location: PlironProtocolLocationV1 {
                    block: 0,
                    operation: 0,
                },
                claimed_active_lanes: 1,
                actual_active_lanes: 0,
            };
            MAX_PLIRON_SIMT_PROTOCOL_ISSUES_V1
        ];
        let constructed = Cell::new(false);
        assert!(!push_issue(&mut issues, || {
            constructed.set(true);
            PlironSimtProtocolIssueV1::PhaseMismatch {
                grid: 0,
                workgroup: 0,
                subgroup: 0,
                first_invocation: vec![0; 64],
                first: vec![
                    PlironProtocolEventV1 {
                        kind: PlironProtocolEventKindV1::TensorInstruction,
                        location: PlironProtocolLocationV1 {
                            block: 0,
                            operation: 0,
                        },
                    };
                    64
                ],
                second_invocation: vec![1; 64],
                second: Vec::new(),
            }
        }));
        assert!(!constructed.get());
        assert_eq!(
            issues,
            vec![PlironSimtProtocolIssueV1::ResourceLimitExceeded]
        );
    }

    #[test]
    fn issue_cardinality_overflow_is_typed() {
        assert_eq!(
            checked_simt_sum_v1(&[usize::MAX, 1]),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::SimtProtocol,
                resource: "SIMT protocol resource upper bound",
            })
        );
    }
}
