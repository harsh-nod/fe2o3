//! Exact graph-local mutation lineage. Historical versions are not runtime authority.

use super::*;
use crate::RuntimeAllocationIdV1;
use fe2o3_runtime_model::{
    r65_version_begin_write_v1, r65_version_commit_write_v1, r65_version_input_ready_v1,
};
use std::collections::{BTreeMap, BTreeSet};

pub use fe2o3_runtime_model::R65GraphVersionStateV1 as RuntimeGraphVersionStateV1;

pub const MAX_RUNTIME_GRAPH_VERSION_REFERENCES_V1: usize = 16_384;
pub const MAX_RUNTIME_GRAPH_VERSIONS_V1: usize =
    MAX_RUNTIME_GRAPH_EFFECTS_V1 * 2 + MAX_RUNTIME_GRAPH_VERSION_REFERENCES_V1;

/// Expected producer within this graph, never a version from a previous run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeGraphVersionSourceV1 {
    InitialAtAdmission,
    ProducedBy(CompletionNodeIdV1),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_outputs() -> VersionLedger {
        VersionLedger {
            segments: Vec::new(),
            current: vec![Some(0), Some(1)],
            pending: vec![None, None],
            uses: vec![vec![
                Use {
                    segment: 0,
                    read: true,
                    write: true,
                    input: Some(0),
                    output: Some(2),
                },
                Use {
                    segment: 1,
                    read: true,
                    write: true,
                    input: Some(1),
                    output: Some(3),
                },
            ]],
            records: (0..4)
                .map(|index| Version {
                    segment: index % 2,
                    producer: None,
                    predecessor: (index >= 2).then(|| index - 2),
                    state: if index < 2 {
                        RuntimeGraphVersionStateV1::AvailableAtAdmission
                    } else {
                        RuntimeGraphVersionStateV1::Planned
                    },
                })
                .collect(),
            nodes: Vec::new(),
            started: vec![false],
            report_records: Vec::new(),
            report_inputs: Vec::new(),
        }
    }

    #[test]
    fn r65_late_begin_mismatch_leaves_all_versions_unchanged() {
        let mut ledger = two_outputs();
        ledger.current[1] = None;
        let before = ledger.current.clone();
        assert!(!ledger.begin(0));
        assert_eq!(ledger.current, before);
        assert_eq!(ledger.pending, vec![None, None]);
        assert!(!ledger.started[0]);
        assert!(
            ledger.records[2..]
                .iter()
                .all(|v| v.state == RuntimeGraphVersionStateV1::Planned)
        );
        ledger.current[1] = Some(1);
        assert!(ledger.begin(0));
        assert!(!ledger.begin(0));
    }

    #[test]
    fn r65_late_commit_owner_mismatch_leaves_all_versions_unchanged() {
        let mut ledger = two_outputs();
        assert!(ledger.begin(0));
        ledger.pending[1] = Some(2);
        assert!(!ledger.commit(0));
        assert_eq!(ledger.current, vec![None, None]);
        assert_eq!(ledger.pending, vec![Some(2), Some(2)]);
        assert!(
            ledger.records[2..]
                .iter()
                .all(|v| v.state == RuntimeGraphVersionStateV1::InFlight)
        );
        ledger.pending[1] = Some(3);
        assert!(ledger.commit(0));
        assert_eq!(ledger.current, vec![Some(2), Some(3)]);
        assert_eq!(ledger.pending, vec![None, None]);
        assert!(!ledger.commit(0));
    }
}

/// One exact segment's historical mutation lineage in one admitted execution.
/// No public constructor, cross-run consumption or conversion to native authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct RuntimeGraphDataVersionV1 {
    execution: RuntimeGraphExecutionIdentityV1,
    allocation: RuntimeAllocationIdV1,
    byte_offset: u64,
    byte_len: u64,
    producer: Option<CompletionNodeIdV1>,
}

impl RuntimeGraphDataVersionV1 {
    pub const fn execution(self) -> RuntimeGraphExecutionIdentityV1 {
        self.execution
    }
    pub const fn allocation(self) -> RuntimeAllocationIdV1 {
        self.allocation
    }
    pub const fn byte_offset(self) -> u64 {
        self.byte_offset
    }
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
    pub const fn producer(self) -> Option<CompletionNodeIdV1> {
        self.producer
    }
}

/// A post-operation version does not imply that the kernel overwrote or
/// initialized every byte. Its predecessor preserves that prior-state lineage.
/// Currentness is runtime-established availability at the final reserved
/// boundary, not at later observation time. A failed writer conservatively
/// invalidates availability even when the backend definitely rejected it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeGraphVersionRecordV1 {
    pub version: RuntimeGraphDataVersionV1,
    pub predecessor: Option<RuntimeGraphDataVersionV1>,
    pub state: RuntimeGraphVersionStateV1,
    pub current_at_terminal: bool,
}

/// A planned data input, distinct from the destination's prior storage version.
/// Availability was checked before backend entry, not proof the kernel read it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeGraphInputVersionV1 {
    pub consumer: CompletionNodeIdV1,
    pub version: RuntimeGraphDataVersionV1,
    pub available_at_issue: bool,
}

pub(super) type InputKey = (CompletionNodeIdV1, RuntimeAllocationIdV1, u64, u64);
type Region = (RuntimeAllocationIdV1, u64, u64);

impl<B: RuntimeBackendV1> RuntimeGraphRequestV1<B> {
    /// Requires every segment of an exact declared read region to come from
    /// this producer. Validation runs before context reservation/publication.
    /// Omitted expectations still receive a derived and checked local lineage.
    pub fn expect_input_version(
        &mut self,
        node: CompletionNodeIdV1,
        region: RuntimeMemoryRegionV1,
        source: RuntimeGraphVersionSourceV1,
    ) -> Result<(), RuntimeGraphValidationErrorV1> {
        if region.access == RuntimeAccessV1::Write {
            return Err(RuntimeGraphValidationErrorV1::InvalidVersionInput);
        }
        if self.version_inputs.len() >= MAX_RUNTIME_GRAPH_EFFECTS_V1 {
            return Err(RuntimeGraphValidationErrorV1::Capacity);
        }
        let key = (node, region.allocation, region.byte_offset, region.byte_len);
        if self.version_inputs.contains_key(&key) {
            return Err(RuntimeGraphValidationErrorV1::DuplicateVersionInput);
        }
        self.version_inputs.insert(key, source);
        Ok(())
    }
}

struct Effect {
    node: usize,
    region: Region,
    read: bool,
    write: bool,
}
struct Use {
    segment: usize,
    read: bool,
    write: bool,
    input: Option<usize>,
    output: Option<usize>,
}
struct Version {
    segment: usize,
    producer: Option<CompletionNodeIdV1>,
    predecessor: Option<usize>,
    state: RuntimeGraphVersionStateV1,
}

pub(super) struct VersionLedger {
    segments: Vec<Region>,
    current: Vec<Option<usize>>,
    pending: Vec<Option<usize>>,
    uses: Vec<Vec<Use>>,
    records: Vec<Version>,
    nodes: Vec<CompletionNodeIdV1>,
    started: Vec<bool>,
    report_records: Vec<RuntimeGraphVersionRecordV1>,
    report_inputs: Vec<RuntimeGraphInputVersionV1>,
}

impl VersionLedger {
    pub(super) fn prepare<B: RuntimeBackendV1>(
        request: &RuntimeGraphRequestV1<B>,
    ) -> Result<Self, RuntimeGraphValidationErrorV1> {
        let nodes = request.graph.nodes();
        let mut effects = Vec::with_capacity(request.effects);
        let mut endpoints = BTreeMap::<RuntimeAllocationIdV1, BTreeSet<u64>>::new();
        for (&id, action) in &request.actions {
            let node = nodes
                .binary_search_by_key(&id, |node| node.id())
                .expect("bound node");
            let mut add = |region: RuntimeMemoryRegionV1| {
                let points = endpoints.entry(region.allocation).or_default();
                points.insert(region.byte_offset);
                points.insert(
                    region
                        .byte_offset
                        .checked_add(region.byte_len)
                        .expect("validated effect"),
                );
                effects.push(Effect {
                    node,
                    region: (region.allocation, region.byte_offset, region.byte_len),
                    read: region.access != RuntimeAccessV1::Write,
                    write: region.access != RuntimeAccessV1::Read,
                });
            };
            match action {
                Action::Launch(launch) => {
                    for binding in launch.bindings() {
                        add(binding.region);
                    }
                }
                Action::Copy(source, destination) => {
                    add(*source);
                    add(*destination);
                }
            }
        }
        for &(node, allocation, offset, len) in request.version_inputs.keys() {
            if !effects.iter().any(|e| {
                nodes[e.node].id() == node && e.read && e.region == (allocation, offset, len)
            }) {
                return Err(RuntimeGraphValidationErrorV1::InvalidVersionInput);
            }
        }
        let mut segments = Vec::with_capacity(request.effects * 2);
        for (allocation, points) in endpoints {
            let points: Vec<_> = points.into_iter().collect();
            for pair in points.windows(2) {
                if effects.iter().any(|e| {
                    e.region.0 == allocation
                        && e.region.1 <= pair[0]
                        && e.region.1 + e.region.2 >= pair[1]
                }) {
                    segments.push((allocation, pair[0], pair[1] - pair[0]));
                }
            }
        }
        let mut accesses = BTreeMap::<(usize, usize), (bool, bool)>::new();
        for effect in &effects {
            for (segment, &(allocation, offset, len)) in segments.iter().enumerate() {
                if effect.region.0 == allocation
                    && effect.region.1 <= offset
                    && offset + len <= effect.region.1 + effect.region.2
                {
                    if !accesses.contains_key(&(effect.node, segment))
                        && accesses.len() >= MAX_RUNTIME_GRAPH_VERSION_REFERENCES_V1
                    {
                        return Err(RuntimeGraphValidationErrorV1::Capacity);
                    }
                    let access = accesses.entry((effect.node, segment)).or_default();
                    access.0 |= effect.read;
                    access.1 |= effect.write;
                }
            }
        }
        let record_capacity = segments.len() + accesses.len();
        if record_capacity > MAX_RUNTIME_GRAPH_VERSIONS_V1 {
            return Err(RuntimeGraphValidationErrorV1::Capacity);
        }
        let mut records = Vec::with_capacity(record_capacity);
        records.extend((0..segments.len()).map(|segment| Version {
            segment,
            producer: None,
            predecessor: None,
            state: RuntimeGraphVersionStateV1::AvailableAtAdmission,
        }));
        let mut ledger = Self {
            records,
            current: (0..segments.len()).map(Some).collect(),
            pending: vec![None; segments.len()],
            nodes: nodes.iter().map(|node| node.id()).collect(),
            started: vec![false; nodes.len()],
            report_records: Vec::with_capacity(record_capacity),
            report_inputs: Vec::with_capacity(accesses.len()),
            uses: (0..nodes.len()).map(|_| Vec::new()).collect(),
            segments,
        };
        for ((node, segment), (read, write)) in accesses {
            ledger.uses[node].push(Use {
                segment,
                read,
                write,
                input: None,
                output: None,
            });
        }
        let mut planned = ledger.current.clone();
        for &id in request.graph.topological_order() {
            let node = nodes.binary_search_by_key(&id, |node| node.id()).unwrap();
            for usage in &mut ledger.uses[node] {
                let prior = planned[usage.segment].expect("planned lineage is total");
                if usage.read {
                    usage.input = Some(prior);
                    let source = ledger.records[prior].producer.map_or(
                        RuntimeGraphVersionSourceV1::InitialAtAdmission,
                        RuntimeGraphVersionSourceV1::ProducedBy,
                    );
                    let (allocation, offset, len) = ledger.segments[usage.segment];
                    for (
                        &(consumer, expected_allocation, expected_offset, expected_len),
                        &expected,
                    ) in &request.version_inputs
                    {
                        if consumer == id
                            && expected_allocation == allocation
                            && expected_offset <= offset
                            && offset + len <= expected_offset + expected_len
                            && source != expected
                        {
                            return Err(RuntimeGraphValidationErrorV1::InvalidVersionInput);
                        }
                    }
                }
                if usage.write {
                    let output = ledger.records.len();
                    ledger.records.push(Version {
                        segment: usage.segment,
                        producer: Some(id),
                        predecessor: Some(prior),
                        state: RuntimeGraphVersionStateV1::Planned,
                    });
                    usage.output = Some(output);
                    planned[usage.segment] = Some(output);
                }
            }
        }
        Ok(ledger)
    }

    pub(super) fn begin(&mut self, node: usize) -> bool {
        // Check every input/output before changing any pointer, including equal aliases.
        if self.uses[node].iter().any(|usage| {
            usage.input.is_some_and(|input| {
                !r65_version_input_ready_v1(
                    self.current[usage.segment],
                    input,
                    self.records[input].state,
                )
            }) || usage.output.is_some_and(|output| {
                self.records[output].predecessor.is_none_or(|prior| {
                    !r65_version_begin_write_v1(
                        self.records[output].state,
                        self.pending[usage.segment],
                        self.current[usage.segment],
                        prior,
                    )
                })
            })
        }) {
            return false;
        }
        if self.started[node] {
            return false;
        }
        self.started[node] = true;
        for usage in &self.uses[node] {
            if let Some(output) = usage.output {
                self.current[usage.segment] = None;
                self.pending[usage.segment] = Some(output);
                self.records[output].state = RuntimeGraphVersionStateV1::InFlight;
            }
        }
        true
    }

    pub(super) fn commit(&mut self, node: usize) -> bool {
        if self.uses[node].iter().any(|usage| {
            usage.output.is_some_and(|output| {
                !r65_version_commit_write_v1(
                    self.records[output].state,
                    self.pending[usage.segment],
                    output,
                    self.current[usage.segment],
                )
            })
        }) {
            return false;
        }
        for usage in &self.uses[node] {
            if let Some(output) = usage.output {
                self.records[output].state = RuntimeGraphVersionStateV1::Committed;
                self.current[usage.segment] = Some(output);
                self.pending[usage.segment] = None;
            }
        }
        true
    }

    pub(super) fn fail(&mut self, node: usize) {
        for usage in &self.uses[node] {
            if let Some(output) = usage.output {
                assert_eq!(
                    self.pending[usage.segment],
                    Some(output),
                    "exact failed writer"
                );
                self.pending[usage.segment] = None;
                self.records[output].state = RuntimeGraphVersionStateV1::Failed;
            }
        }
    }

    pub(super) fn report(
        mut self,
        execution: RuntimeGraphExecutionIdentityV1,
    ) -> Option<(
        Vec<RuntimeGraphVersionRecordV1>,
        Vec<RuntimeGraphInputVersionV1>,
    )> {
        if self.pending.iter().any(Option::is_some)
            || self
                .records
                .iter()
                .any(|v| v.state == RuntimeGraphVersionStateV1::InFlight)
        {
            return None;
        }
        for record in &mut self.records {
            if record.state == RuntimeGraphVersionStateV1::Planned {
                record.state = RuntimeGraphVersionStateV1::NotProduced;
            }
        }
        let segments = &self.segments;
        let identity = |record: &Version| {
            let (allocation, byte_offset, byte_len) = segments[record.segment];
            RuntimeGraphDataVersionV1 {
                execution,
                allocation,
                byte_offset,
                byte_len,
                producer: record.producer,
            }
        };
        for (index, record) in self.records.iter().enumerate() {
            self.report_records.push(RuntimeGraphVersionRecordV1 {
                version: identity(record),
                predecessor: record
                    .predecessor
                    .map(|prior| identity(&self.records[prior])),
                state: record.state,
                current_at_terminal: self.current[record.segment] == Some(index),
            });
        }
        for (node, uses) in self.uses.iter().enumerate() {
            for usage in uses {
                if let Some(input) = usage.input {
                    self.report_inputs.push(RuntimeGraphInputVersionV1 {
                        consumer: self.nodes[node],
                        version: identity(&self.records[input]),
                        available_at_issue: self.started[node],
                    });
                }
            }
        }
        Some((self.report_records, self.report_inputs))
    }
}
