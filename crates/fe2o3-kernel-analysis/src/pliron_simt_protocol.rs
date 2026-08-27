//! Active-mask and collective-sequencing facts derived from exact PLIRON traces.
//!
//! The analysis recognizes only executable operations retained in the current
//! IR. Proof contracts do not create participation or ordering facts.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use dialect_gpu::HierarchyAttr;

use crate::pliron_invocation_trace::{
    PlironInvocationTraceV1, PlironTraceEventV1, PlironTraceLocationV1,
};

pub const MAX_PLIRON_SIMT_PROTOCOL_ISSUES_V1: usize = 4_096;

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
                if !push_issue(
                    &mut issues,
                    PlironSimtProtocolIssueV1::PhaseMismatch {
                        grid,
                        workgroup,
                        subgroup,
                        first_invocation: first_invocation.invocation.clone(),
                        first: first.clone(),
                        second_invocation: trace.invocation.clone(),
                        second: sequence.clone(),
                    },
                ) {
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
                && !push_issue(
                    &mut issues,
                    PlironSimtProtocolIssueV1::PartialTensorParticipation {
                        grid,
                        workgroup,
                        subgroup,
                        location,
                        expected_lanes: site.subgroup_width,
                        actual_lanes: actual_lanes.iter().copied().collect(),
                    },
                )
            {
                return PlironSimtProtocolAnalysisV1 { issues };
            }
            if actual_lanes.len() != site.claimed_active_lanes as usize
                && !push_issue(
                    &mut issues,
                    PlironSimtProtocolIssueV1::ClaimedActiveMaskMismatch {
                        location,
                        claimed_active_lanes: site.claimed_active_lanes,
                        actual_active_lanes: actual_lanes.len(),
                    },
                )
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
    issue: PlironSimtProtocolIssueV1,
) -> bool {
    if issues.len() == MAX_PLIRON_SIMT_PROTOCOL_ISSUES_V1 {
        issues.clear();
        issues.push(PlironSimtProtocolIssueV1::ResourceLimitExceeded);
        false
    } else {
        issues.push(issue);
        true
    }
}
