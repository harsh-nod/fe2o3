//! Final-graph semantics for target-neutral KIR V13 execution capabilities.
//!
//! This analysis is bounded and authority-free. It checks the exact canonical
//! graph supplied by its owner and retains counterexamples or unsupported
//! cases for the production W4 witness.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    BlockId, ExecutionAtomicKindV1, ExecutionCapabilityOpV1, ExecutionCapabilityOperationV1,
    ExecutionCapabilitySourceV1, ExecutionLdsStateV1, ExecutionMemoryAccessV1,
    ExecutionMemoryAddressSpaceV1, ExecutionMemoryInitializationV1, ExecutionMemoryOrderingV1,
    ExecutionMemoryScopeV1, ExecutionMemorySpacesV1, Function, FunctionId, Kernel, KernelId,
    Module, Operation, OperationKind, SynchronizationScope, Terminator, Type, ValueId,
    VerifiedCanonicalKernelIrErrorV13, VerifiedCanonicalKernelIrIdentityV13,
    VerifiedCanonicalKernelIrV13, decode_module_v13,
};

use crate::uniformity::analyze_function_with_root_inputs;
use crate::{KernelCheckStatusV1, ProductionCapabilityAnalysisKindV1, Variation};

pub const EXECUTION_CAPABILITY_FINAL_GRAPH_SEMANTICS_VERSION_V1: u16 = 1;
pub const MAX_EXECUTION_SEMANTIC_ROOTS_V1: usize = 1_024;
pub const MAX_EXECUTION_SEMANTIC_FUNCTIONS_V1: usize = 1_024;
pub const MAX_EXECUTION_SEMANTIC_BLOCKS_V1: usize = 65_536;
pub const MAX_EXECUTION_SEMANTIC_OPERATIONS_V1: usize = 1_048_576;
pub const MAX_EXECUTION_SEMANTIC_CALL_EDGES_V1: usize = 65_536;
pub const MAX_EXECUTION_SEMANTIC_FLOW_STATES_V1: usize = 262_144;
pub const MAX_EXECUTION_SEMANTIC_SEQUENCE_STATES_V1: usize = 262_144;
pub const MAX_EXECUTION_SEMANTIC_FINDINGS_V1: usize = 4_096;

pub const EXECUTION_CAPABILITY_REJECTED_DIAGNOSTIC_V1: &str = "FE2O3-CAP-ANALYSIS001";
pub const EXECUTION_CAPABILITY_INCOMPLETE_DIAGNOSTIC_V1: &str = "FE2O3-CAP-ANALYSIS002";

/// Exact canonical and source coordinates for one bounded finding.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ExecutionCapabilitySemanticLocationV1 {
    root: KernelId,
    function: FunctionId,
    block: BlockId,
    operation: usize,
    source: Option<ExecutionCapabilitySourceV1>,
}

impl ExecutionCapabilitySemanticLocationV1 {
    pub const fn root(&self) -> &KernelId {
        &self.root
    }

    pub const fn function(&self) -> &FunctionId {
        &self.function
    }

    pub const fn block(&self) -> BlockId {
        self.block
    }

    pub const fn operation(&self) -> usize {
        self.operation
    }

    pub const fn source(&self) -> Option<ExecutionCapabilitySourceV1> {
        self.source
    }
}

/// Stable reason retained by a final-graph semantic finding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionCapabilitySemanticReasonV1 {
    ResourceLimit {
        resource: &'static str,
        limit: usize,
    },
    UnsupportedExternalCall {
        callee: FunctionId,
    },
    RecursiveCapabilityCall {
        callee: FunctionId,
    },
    UniformityIncomplete,
    NonUniformArrival {
        scope: SynchronizationScope,
        control: Variation,
    },
    BarrierSequenceMismatch {
        scope: SynchronizationScope,
    },
    BarrierCountMismatch {
        scope: SynchronizationScope,
    },
    DynamicBarrierSequenceUnsupported {
        scope: SynchronizationScope,
    },
    IncompleteCollectiveParticipation {
        expected: u64,
        observed: u64,
    },
    MissingEpochProvenance,
    StaleEpoch {
        expected: [u8; 32],
        observed: [u8; 32],
    },
    LdsStateMismatch {
        expected: &'static str,
        observed: &'static str,
    },
    LdsReuseBeforeEpochTransition,
    InvalidAtomicOperation,
    InsufficientAtomicScope {
        scope: ExecutionMemoryScopeV1,
    },
    InsufficientAtomicOrdering {
        ordering: ExecutionMemoryOrderingV1,
    },
    ConflictingEffects,
    ConflictingAccessCorrespondenceUnavailable,
    UnsupportedMemoryEffect,
}

/// One bounded rejection or incomplete obligation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionCapabilitySemanticFindingV1 {
    stage: ProductionCapabilityAnalysisKindV1,
    status: KernelCheckStatusV1,
    location: ExecutionCapabilitySemanticLocationV1,
    related: Option<ExecutionCapabilitySemanticLocationV1>,
    reason: ExecutionCapabilitySemanticReasonV1,
}

impl ExecutionCapabilitySemanticFindingV1 {
    pub const fn stage(&self) -> ProductionCapabilityAnalysisKindV1 {
        self.stage
    }

    pub const fn status(&self) -> KernelCheckStatusV1 {
        self.status
    }

    pub const fn location(&self) -> &ExecutionCapabilitySemanticLocationV1 {
        &self.location
    }

    pub const fn related(&self) -> Option<&ExecutionCapabilitySemanticLocationV1> {
        self.related.as_ref()
    }

    pub const fn reason(&self) -> &ExecutionCapabilitySemanticReasonV1 {
        &self.reason
    }

    pub const fn diagnostic_code(&self) -> &'static str {
        match self.status {
            KernelCheckStatusV1::Rejected => EXECUTION_CAPABILITY_REJECTED_DIAGNOSTIC_V1,
            KernelCheckStatusV1::Clean | KernelCheckStatusV1::Incomplete => {
                EXECUTION_CAPABILITY_INCOMPLETE_DIAGNOSTIC_V1
            }
        }
    }
}

impl fmt::Display for ExecutionCapabilitySemanticFindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "error[{}]: {:?} for root {} in helper {} at KIR block {} op {}",
            self.diagnostic_code(),
            self.reason,
            self.location.root,
            self.location.function,
            self.location.block.0,
            self.location.operation,
        )?;
        if let Some(source) = self.location.source {
            write!(
                formatter,
                "; source function {:02x?} block {} operation {:02x?}",
                source.function, source.block, source.operation,
            )?;
        }
        if let Some(related) = &self.related {
            write!(
                formatter,
                "; related helper {} KIR block {} op {}",
                related.function, related.block.0, related.operation,
            )?;
            if let Some(source) = related.source {
                write!(
                    formatter,
                    " source function {:02x?} block {} operation {:02x?}",
                    source.function, source.block, source.operation,
                )?;
            }
        }
        Ok(())
    }
}

/// Exact authority-free report over one canonical final graph and epoch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionCapabilityFinalGraphReportV1 {
    canonical_identity: VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    checked_roots: usize,
    checked_functions: usize,
    checked_operations: usize,
    checked_barrier_sites: usize,
    checked_collective_sites: usize,
    checked_epoch_transitions: usize,
    checked_atomic_operations: usize,
    checked_memory_effects: usize,
    checked_sequence_states: usize,
    conflicting_effects_composed_with_retained_pliron: bool,
    findings: Vec<ExecutionCapabilitySemanticFindingV1>,
}

impl ExecutionCapabilityFinalGraphReportV1 {
    pub const fn canonical_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.canonical_identity
    }

    pub const fn final_epoch(&self) -> u64 {
        self.final_epoch
    }

    pub const fn checked_roots(&self) -> usize {
        self.checked_roots
    }

    pub const fn checked_functions(&self) -> usize {
        self.checked_functions
    }

    pub const fn checked_operations(&self) -> usize {
        self.checked_operations
    }

    pub const fn checked_barrier_sites(&self) -> usize {
        self.checked_barrier_sites
    }

    pub const fn checked_collective_sites(&self) -> usize {
        self.checked_collective_sites
    }

    pub const fn checked_epoch_transitions(&self) -> usize {
        self.checked_epoch_transitions
    }

    pub const fn checked_atomic_operations(&self) -> usize {
        self.checked_atomic_operations
    }

    pub const fn checked_memory_effects(&self) -> usize {
        self.checked_memory_effects
    }

    pub const fn checked_sequence_states(&self) -> usize {
        self.checked_sequence_states
    }

    pub const fn conflicting_effects_composed_with_retained_pliron(&self) -> bool {
        self.conflicting_effects_composed_with_retained_pliron
    }

    pub fn findings(&self) -> &[ExecutionCapabilitySemanticFindingV1] {
        &self.findings
    }

    pub fn status(&self) -> KernelCheckStatusV1 {
        self.findings
            .iter()
            .fold(KernelCheckStatusV1::Clean, |status, finding| {
                status.join(finding.status)
            })
    }

    pub fn stage_status(&self, stage: ProductionCapabilityAnalysisKindV1) -> KernelCheckStatusV1 {
        self.findings
            .iter()
            .filter(|finding| finding.stage == stage)
            .fold(KernelCheckStatusV1::Clean, |status, finding| {
                status.join(finding.status)
            })
    }

    pub fn first_non_clean(&self) -> Option<&ExecutionCapabilitySemanticFindingV1> {
        self.findings.first()
    }

    pub const fn grants_proof_machine_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum ExecutionCapabilityFinalGraphErrorV1 {
    Canonical(VerifiedCanonicalKernelIrErrorV13),
    Decode,
    ModuleSubstituted,
}

impl fmt::Display for ExecutionCapabilityFinalGraphErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Canonical(error) => write!(formatter, "canonical KIR V13 is invalid: {error}"),
            Self::Decode => formatter.write_str("canonical KIR V13 could not be decoded"),
            Self::ModuleSubstituted => {
                formatter.write_str("canonical KIR V13 module was substituted")
            }
        }
    }
}

impl Error for ExecutionCapabilityFinalGraphErrorV1 {}

/// Checks the exact canonical V13 owner and its final optimization epoch.
pub fn analyze_execution_capability_final_graph_v1(
    canonical: &VerifiedCanonicalKernelIrV13,
    module: &Module,
    final_epoch: u64,
) -> Result<ExecutionCapabilityFinalGraphReportV1, ExecutionCapabilityFinalGraphErrorV1> {
    canonical
        .revalidate()
        .map_err(ExecutionCapabilityFinalGraphErrorV1::Canonical)?;
    let decoded = decode_module_v13(canonical.canonical_bytes())
        .map_err(|_| ExecutionCapabilityFinalGraphErrorV1::Decode)?;
    if &decoded != module {
        return Err(ExecutionCapabilityFinalGraphErrorV1::ModuleSubstituted);
    }
    Ok(analyze_module(
        module,
        *canonical.identity(),
        final_epoch,
        false,
    ))
}

/// W4-only entry after every retained live function has clean exact race and
/// hierarchical-ownership reports. The returned report remains authority-free.
#[cfg(feature = "pliron-analysis")]
pub(crate) fn analyze_execution_capability_final_graph_with_composed_conflicts_v1(
    canonical: &VerifiedCanonicalKernelIrV13,
    module: &Module,
    final_epoch: u64,
) -> Result<ExecutionCapabilityFinalGraphReportV1, ExecutionCapabilityFinalGraphErrorV1> {
    canonical
        .revalidate()
        .map_err(ExecutionCapabilityFinalGraphErrorV1::Canonical)?;
    let decoded = decode_module_v13(canonical.canonical_bytes())
        .map_err(|_| ExecutionCapabilityFinalGraphErrorV1::Decode)?;
    if &decoded != module {
        return Err(ExecutionCapabilityFinalGraphErrorV1::ModuleSubstituted);
    }
    Ok(analyze_module(
        module,
        *canonical.identity(),
        final_epoch,
        true,
    ))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum LdsPhase {
    Uninitialized,
    InvocationInitialized,
    Published,
    Consumed,
}

impl LdsPhase {
    const fn name(self) -> &'static str {
        match self {
            Self::Uninitialized => "uninitialized",
            Self::InvocationInitialized => "invocation-initialized",
            Self::Published => "published",
            Self::Consumed => "consumed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ViewPhase {
    Uninitialized,
    InvocationInitialized,
    Published,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct FlowState {
    epochs: BTreeMap<[u8; 32], BTreeSet<[u8; 32]>>,
    lds: BTreeMap<ValueId, BTreeSet<LdsPhase>>,
    views: BTreeMap<ValueId, BTreeSet<ViewPhase>>,
    pending_global_writes: Vec<ExecutionCapabilitySemanticLocationV1>,
    publication_ready: Vec<ExecutionCapabilitySemanticLocationV1>,
    relaxed_acquire: Option<ExecutionCapabilitySemanticLocationV1>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SequenceReturnSite {
    function: FunctionId,
    block: BlockId,
    operation: usize,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SequenceProgramCounter {
    function: FunctionId,
    block: BlockId,
    operation: usize,
    returns: Vec<SequenceReturnSite>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SequenceEvent {
    identity: [u8; 32],
    location: ExecutionCapabilitySemanticLocationV1,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SequenceCursor {
    Ready(SequenceProgramCounter),
    Waiting {
        event: SequenceEvent,
        resume: SequenceProgramCounter,
    },
    Exited(ExecutionCapabilitySemanticLocationV1),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SequencePair {
    left: SequenceCursor,
    right: SequenceCursor,
}

enum SequenceStep {
    Advance(Box<SequenceCursor>),
    Choices {
        cursors: Vec<SequenceCursor>,
        uniform: bool,
    },
}

type SequenceFailure = Box<(
    ExecutionCapabilitySemanticLocationV1,
    ExecutionCapabilitySemanticReasonV1,
)>;

struct Analyzer<'a> {
    module: &'a Module,
    identity: VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    checked_functions: BTreeSet<FunctionId>,
    checked_operations: usize,
    checked_barrier_sites: usize,
    checked_collective_sites: usize,
    checked_epoch_transitions: usize,
    checked_atomic_operations: usize,
    checked_memory_effects: usize,
    call_edges: usize,
    flow_states: usize,
    sequence_states: usize,
    conflicting_effects_composed_with_retained_pliron: bool,
    findings: Vec<ExecutionCapabilitySemanticFindingV1>,
}

fn analyze_module(
    module: &Module,
    identity: VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    conflicting_effects_composed_with_retained_pliron: bool,
) -> ExecutionCapabilityFinalGraphReportV1 {
    let mut analyzer = Analyzer {
        module,
        identity,
        final_epoch,
        checked_functions: BTreeSet::new(),
        checked_operations: 0,
        checked_barrier_sites: 0,
        checked_collective_sites: 0,
        checked_epoch_transitions: 0,
        checked_atomic_operations: 0,
        checked_memory_effects: 0,
        call_edges: 0,
        flow_states: 0,
        sequence_states: 0,
        conflicting_effects_composed_with_retained_pliron,
        findings: Vec::new(),
    };
    if module.kernels.len() > MAX_EXECUTION_SEMANTIC_ROOTS_V1 {
        if let Some(kernel) = module.kernels.first() {
            analyzer.resource_finding(kernel, "kernel roots", MAX_EXECUTION_SEMANTIC_ROOTS_V1);
        }
    } else {
        for kernel in &module.kernels {
            analyzer.analyze_root(kernel);
            if analyzer.findings.len() == MAX_EXECUTION_SEMANTIC_FINDINGS_V1 {
                break;
            }
        }
    }
    ExecutionCapabilityFinalGraphReportV1 {
        canonical_identity: analyzer.identity,
        final_epoch: analyzer.final_epoch,
        checked_roots: module.kernels.len().min(MAX_EXECUTION_SEMANTIC_ROOTS_V1),
        checked_functions: analyzer.checked_functions.len(),
        checked_operations: analyzer.checked_operations,
        checked_barrier_sites: analyzer.checked_barrier_sites,
        checked_collective_sites: analyzer.checked_collective_sites,
        checked_epoch_transitions: analyzer.checked_epoch_transitions,
        checked_atomic_operations: analyzer.checked_atomic_operations,
        checked_memory_effects: analyzer.checked_memory_effects,
        checked_sequence_states: analyzer.sequence_states,
        conflicting_effects_composed_with_retained_pliron: analyzer
            .conflicting_effects_composed_with_retained_pliron,
        findings: analyzer.findings,
    }
}

impl Analyzer<'_> {
    fn analyze_root(&mut self, kernel: &Kernel) {
        let Some(entry) = self.module.function(&kernel.entry) else {
            self.push(
                ProductionCapabilityAnalysisKindV1::CanonicalTyping,
                KernelCheckStatusV1::Incomplete,
                location(kernel, &kernel.entry, BlockId(0), 0, None),
                None,
                ExecutionCapabilitySemanticReasonV1::UnsupportedExternalCall {
                    callee: kernel.entry.clone(),
                },
            );
            return;
        };
        let reachable = self.reachable_functions(kernel, entry);
        if reachable.is_empty() {
            return;
        }
        let reports = self.root_uniformity_reports(kernel, &reachable);
        let incoming = self.function_control(kernel, &reachable, &reports);
        let workgroup_size = kernel.workgroup_size.and_then(|size| {
            u64::from(size.x)
                .checked_mul(u64::from(size.y))?
                .checked_mul(u64::from(size.z))
        });
        for function_id in reachable {
            let Some(function) = self.module.function(&function_id) else {
                continue;
            };
            self.checked_functions.insert(function_id.clone());
            let Some(body) = &function.body else {
                continue;
            };
            if body.blocks.len() > MAX_EXECUTION_SEMANTIC_BLOCKS_V1 {
                self.resource_finding(kernel, "CFG blocks", MAX_EXECUTION_SEMANTIC_BLOCKS_V1);
                continue;
            }
            let Some(report) = reports.get(&function_id) else {
                continue;
            };
            self.check_convergence(
                kernel,
                function,
                report,
                incoming
                    .get(&function_id)
                    .copied()
                    .unwrap_or(Variation::Varying),
                workgroup_size,
            );
            self.check_flow(kernel, function, workgroup_size);
        }
        self.check_dynamic_barrier_sequences(kernel, &reports);
    }

    fn root_uniformity_reports(
        &mut self,
        kernel: &Kernel,
        reachable: &[FunctionId],
    ) -> BTreeMap<FunctionId, crate::AnalysisReport> {
        let mut parameters = reachable
            .iter()
            .filter_map(|function_id| {
                let function = self.module.function(function_id)?;
                Some((
                    function_id.clone(),
                    vec![Variation::GridUniform; function.signature.parameters.len()],
                ))
            })
            .collect::<BTreeMap<_, _>>();
        let rounds = reachable.len().saturating_mul(5).saturating_add(1);
        for _ in 0..rounds {
            let reports = self.build_root_reports(kernel, reachable, &parameters);
            let mut changed = false;
            for function_id in reachable {
                let (Some(function), Some(report)) =
                    (self.module.function(function_id), reports.get(function_id))
                else {
                    continue;
                };
                let Some(body) = &function.body else {
                    continue;
                };
                for operation in body.blocks.iter().flat_map(|block| &block.operations) {
                    let OperationKind::Call { callee, arguments } = &operation.kind else {
                        continue;
                    };
                    let Some(callee_parameters) = parameters.get_mut(callee) else {
                        continue;
                    };
                    for (parameter, argument) in callee_parameters.iter_mut().zip(arguments) {
                        let next = parameter.join(report.value(*argument));
                        if next != *parameter {
                            *parameter = next;
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                return reports;
            }
        }
        self.push(
            ProductionCapabilityAnalysisKindV1::Uniformity,
            KernelCheckStatusV1::Incomplete,
            location(kernel, &kernel.entry, BlockId(0), 0, None),
            None,
            ExecutionCapabilitySemanticReasonV1::ResourceLimit {
                resource: "interprocedural uniformity iterations",
                limit: rounds,
            },
        );
        self.build_root_reports(kernel, reachable, &parameters)
    }

    fn build_root_reports(
        &self,
        kernel: &Kernel,
        reachable: &[FunctionId],
        parameters: &BTreeMap<FunctionId, Vec<Variation>>,
    ) -> BTreeMap<FunctionId, crate::AnalysisReport> {
        reachable
            .iter()
            .filter_map(|function_id| {
                let function = self.module.function(function_id)?;
                let inputs = parameters.get(function_id)?;
                Some((
                    function_id.clone(),
                    analyze_function_with_root_inputs(
                        self.module,
                        function,
                        inputs,
                        kernel.workgroup_size,
                    ),
                ))
            })
            .collect()
    }

    fn check_dynamic_barrier_sequences(
        &mut self,
        kernel: &Kernel,
        reports: &BTreeMap<FunctionId, crate::AnalysisReport>,
    ) {
        let scopes = reports
            .keys()
            .filter_map(|function| self.module.function(function))
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match &operation.kind {
                OperationKind::ExecutionCapability(contract) => {
                    convergence_scope(&contract.operation)
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        for scope in scopes {
            if !self.check_dynamic_barrier_sequence_scope(kernel, reports, scope) {
                return;
            }
        }
    }

    fn check_dynamic_barrier_sequence_scope(
        &mut self,
        kernel: &Kernel,
        reports: &BTreeMap<FunctionId, crate::AnalysisReport>,
        scope: SynchronizationScope,
    ) -> bool {
        let Some(entry) = self.module.function(&kernel.entry) else {
            return false;
        };
        let Some(entry_block) = entry.body.as_ref().and_then(|body| body.blocks.first()) else {
            return false;
        };
        let start = SequenceProgramCounter {
            function: kernel.entry.clone(),
            block: entry_block.id,
            operation: 0,
            returns: Vec::new(),
        };
        let start = SequencePair {
            left: SequenceCursor::Ready(start.clone()),
            right: SequenceCursor::Ready(start),
        };
        let mut pending = VecDeque::from([start.clone()]);
        let mut discovered = BTreeSet::from([start]);
        let mut parents = BTreeMap::<SequencePair, SequencePair>::new();

        while let Some(pair) = pending.pop_front() {
            self.sequence_states = self.sequence_states.saturating_add(1);
            if self.sequence_states > MAX_EXECUTION_SEMANTIC_SEQUENCE_STATES_V1 {
                self.push(
                    ProductionCapabilityAnalysisKindV1::BarrierConvergence,
                    KernelCheckStatusV1::Incomplete,
                    sequence_pair_location(kernel, &pair),
                    None,
                    ExecutionCapabilitySemanticReasonV1::ResourceLimit {
                        resource: "dynamic barrier sequence states",
                        limit: MAX_EXECUTION_SEMANTIC_SEQUENCE_STATES_V1,
                    },
                );
                return false;
            }
            let Some(successors) = self.settle_sequence_pair(scope, pair.clone()) else {
                return false;
            };
            for successor in successors {
                let expanded = match self.expand_sequence_pair(kernel, reports, scope, &successor) {
                    Ok(expanded) => expanded,
                    Err(failure) => {
                        let (location, reason) = *failure;
                        self.push(
                            ProductionCapabilityAnalysisKindV1::BarrierConvergence,
                            KernelCheckStatusV1::Incomplete,
                            location,
                            None,
                            reason,
                        );
                        return false;
                    }
                };
                for next in expanded {
                    if discovered.insert(next.clone()) {
                        parents.insert(next.clone(), pair.clone());
                        pending.push_back(next);
                    } else if sequence_pair_is_asynchronous(&next)
                        && sequence_pair_is_ancestor(&next, &pair, &parents)
                    {
                        self.push(
                            ProductionCapabilityAnalysisKindV1::BarrierConvergence,
                            KernelCheckStatusV1::Incomplete,
                            sequence_pair_location(kernel, &next),
                            None,
                            ExecutionCapabilitySemanticReasonV1::DynamicBarrierSequenceUnsupported {
                                scope,
                            },
                        );
                        return false;
                    }
                }
            }
        }
        true
    }

    fn settle_sequence_pair(
        &mut self,
        scope: SynchronizationScope,
        pair: SequencePair,
    ) -> Option<Vec<SequencePair>> {
        match (&pair.left, &pair.right) {
            (
                SequenceCursor::Waiting {
                    event: left,
                    resume: left_resume,
                },
                SequenceCursor::Waiting {
                    event: right,
                    resume: right_resume,
                },
            ) => {
                if left.identity != right.identity {
                    self.push(
                        ProductionCapabilityAnalysisKindV1::BarrierConvergence,
                        KernelCheckStatusV1::Rejected,
                        left.location.clone(),
                        Some(right.location.clone()),
                        ExecutionCapabilitySemanticReasonV1::BarrierSequenceMismatch { scope },
                    );
                    None
                } else {
                    Some(vec![SequencePair {
                        left: SequenceCursor::Ready(left_resume.clone()),
                        right: SequenceCursor::Ready(right_resume.clone()),
                    }])
                }
            }
            (SequenceCursor::Waiting { event, .. }, SequenceCursor::Exited(exit))
            | (SequenceCursor::Exited(exit), SequenceCursor::Waiting { event, .. }) => {
                self.push(
                    ProductionCapabilityAnalysisKindV1::BarrierConvergence,
                    KernelCheckStatusV1::Rejected,
                    event.location.clone(),
                    Some(exit.clone()),
                    ExecutionCapabilitySemanticReasonV1::BarrierCountMismatch { scope },
                );
                None
            }
            (SequenceCursor::Exited(_), SequenceCursor::Exited(_)) => Some(Vec::new()),
            _ => Some(vec![pair]),
        }
    }

    fn expand_sequence_pair(
        &self,
        kernel: &Kernel,
        reports: &BTreeMap<FunctionId, crate::AnalysisReport>,
        scope: SynchronizationScope,
        pair: &SequencePair,
    ) -> Result<Vec<SequencePair>, SequenceFailure> {
        let left = self.sequence_step(kernel, reports, scope, &pair.left)?;
        let right = self.sequence_step(kernel, reports, scope, &pair.right)?;
        if let (
            SequenceCursor::Ready(left_pc),
            SequenceCursor::Ready(right_pc),
            SequenceStep::Choices {
                cursors: left,
                uniform: true,
            },
            SequenceStep::Choices {
                cursors: right,
                uniform: true,
            },
        ) = (&pair.left, &pair.right, &left, &right)
            && left_pc == right_pc
            && left.len() == right.len()
        {
            return Ok(left
                .iter()
                .cloned()
                .zip(right.iter().cloned())
                .map(|(left, right)| SequencePair { left, right })
                .collect());
        }
        let left = sequence_step_cursors(left);
        let right = sequence_step_cursors(right);
        let mut pairs = Vec::new();
        for left in &left {
            for right in &right {
                if pairs.len() == MAX_EXECUTION_SEMANTIC_SEQUENCE_STATES_V1 {
                    return Err(Box::new((
                        sequence_pair_location(kernel, pair),
                        ExecutionCapabilitySemanticReasonV1::ResourceLimit {
                            resource: "dynamic barrier sequence successors",
                            limit: MAX_EXECUTION_SEMANTIC_SEQUENCE_STATES_V1,
                        },
                    )));
                }
                pairs.push(SequencePair {
                    left: left.clone(),
                    right: right.clone(),
                });
            }
        }
        Ok(pairs)
    }

    fn sequence_step(
        &self,
        kernel: &Kernel,
        reports: &BTreeMap<FunctionId, crate::AnalysisReport>,
        scope: SynchronizationScope,
        cursor: &SequenceCursor,
    ) -> Result<SequenceStep, SequenceFailure> {
        let SequenceCursor::Ready(counter) = cursor else {
            return Ok(SequenceStep::Advance(Box::new(cursor.clone())));
        };
        let at = |source| {
            location(
                kernel,
                &counter.function,
                counter.block,
                counter.operation,
                source,
            )
        };
        let Some(function) = self.module.function(&counter.function) else {
            return Err(Box::new((
                at(None),
                ExecutionCapabilitySemanticReasonV1::UnsupportedExternalCall {
                    callee: counter.function.clone(),
                },
            )));
        };
        let Some(body) = &function.body else {
            return Err(Box::new((
                at(None),
                ExecutionCapabilitySemanticReasonV1::UnsupportedExternalCall {
                    callee: counter.function.clone(),
                },
            )));
        };
        let Some(block) = body.blocks.iter().find(|block| block.id == counter.block) else {
            return Err(Box::new((
                at(None),
                ExecutionCapabilitySemanticReasonV1::UniformityIncomplete,
            )));
        };
        if let Some(operation) = block.operations.get(counter.operation) {
            let mut resume = counter.clone();
            resume.operation = resume.operation.saturating_add(1);
            match &operation.kind {
                OperationKind::ExecutionCapability(contract)
                    if convergence_scope(&contract.operation) == Some(scope) =>
                {
                    let location = at(Some(contract.source));
                    return Ok(SequenceStep::Advance(Box::new(SequenceCursor::Waiting {
                        event: SequenceEvent {
                            identity: contract.source.operation,
                            location,
                        },
                        resume,
                    })));
                }
                OperationKind::Call { callee, .. } => {
                    let Some(callee_function) = self.module.function(callee) else {
                        return Err(Box::new((
                            at(None),
                            ExecutionCapabilitySemanticReasonV1::UnsupportedExternalCall {
                                callee: callee.clone(),
                            },
                        )));
                    };
                    let Some(entry) = callee_function
                        .body
                        .as_ref()
                        .and_then(|body| body.blocks.first())
                    else {
                        return Err(Box::new((
                            at(None),
                            ExecutionCapabilitySemanticReasonV1::UnsupportedExternalCall {
                                callee: callee.clone(),
                            },
                        )));
                    };
                    if counter.function == *callee
                        || counter
                            .returns
                            .iter()
                            .any(|return_site| return_site.function == *callee)
                    {
                        return Err(Box::new((
                            at(None),
                            ExecutionCapabilitySemanticReasonV1::RecursiveCapabilityCall {
                                callee: callee.clone(),
                            },
                        )));
                    }
                    if counter.returns.len() == MAX_EXECUTION_SEMANTIC_FUNCTIONS_V1 {
                        return Err(Box::new((
                            at(None),
                            ExecutionCapabilitySemanticReasonV1::ResourceLimit {
                                resource: "dynamic barrier call depth",
                                limit: MAX_EXECUTION_SEMANTIC_FUNCTIONS_V1,
                            },
                        )));
                    }
                    let mut returns = counter.returns.clone();
                    returns.push(SequenceReturnSite {
                        function: counter.function.clone(),
                        block: counter.block,
                        operation: resume.operation,
                    });
                    return Ok(SequenceStep::Advance(Box::new(SequenceCursor::Ready(
                        SequenceProgramCounter {
                            function: callee.clone(),
                            block: entry.id,
                            operation: 0,
                            returns,
                        },
                    ))));
                }
                OperationKind::InlineAssembly(_) => {
                    return Err(Box::new((
                        at(None),
                        ExecutionCapabilitySemanticReasonV1::UnsupportedMemoryEffect,
                    )));
                }
                _ => {
                    return Ok(SequenceStep::Advance(Box::new(SequenceCursor::Ready(
                        resume,
                    ))));
                }
            }
        }
        if counter.operation != block.operations.len() {
            return Err(Box::new((
                at(None),
                ExecutionCapabilitySemanticReasonV1::UniformityIncomplete,
            )));
        }
        let Some(terminator) = &block.terminator else {
            return Err(Box::new((
                at(None),
                ExecutionCapabilitySemanticReasonV1::UniformityIncomplete,
            )));
        };
        let target = |block| {
            SequenceCursor::Ready(SequenceProgramCounter {
                function: counter.function.clone(),
                block,
                operation: 0,
                returns: counter.returns.clone(),
            })
        };
        match terminator {
            Terminator::Branch { target: block, .. } => {
                Ok(SequenceStep::Advance(Box::new(target(*block))))
            }
            Terminator::ConditionalBranch {
                condition,
                then_target,
                else_target,
                ..
            } => Ok(SequenceStep::Choices {
                cursors: vec![target(*then_target), target(*else_target)],
                uniform: reports
                    .get(&counter.function)
                    .is_some_and(|report| report.value(*condition).is_uniform_for(scope)),
            }),
            Terminator::Switch {
                selector,
                cases,
                default_target,
                ..
            } => Ok(SequenceStep::Choices {
                cursors: cases
                    .iter()
                    .map(|case| target(case.target))
                    .chain([target(*default_target)])
                    .collect(),
                uniform: reports
                    .get(&counter.function)
                    .is_some_and(|report| report.value(*selector).is_uniform_for(scope)),
            }),
            Terminator::IntegerSwitch {
                selector,
                cases,
                default_target,
                ..
            } => Ok(SequenceStep::Choices {
                cursors: cases
                    .iter()
                    .map(|case| target(case.target))
                    .chain([target(*default_target)])
                    .collect(),
                uniform: reports
                    .get(&counter.function)
                    .is_some_and(|report| report.value(*selector).is_uniform_for(scope)),
            }),
            Terminator::Return { .. } | Terminator::Unreachable => {
                let mut returns = counter.returns.clone();
                match returns.pop() {
                    Some(return_site) => {
                        let cursor = SequenceCursor::Ready(SequenceProgramCounter {
                            function: return_site.function,
                            block: return_site.block,
                            operation: return_site.operation,
                            returns,
                        });
                        Ok(SequenceStep::Advance(Box::new(cursor)))
                    }
                    None => Ok(SequenceStep::Advance(Box::new(SequenceCursor::Exited(at(
                        None,
                    ))))),
                }
            }
        }
    }

    fn reachable_functions(&mut self, kernel: &Kernel, entry: &Function) -> Vec<FunctionId> {
        let mut pending = VecDeque::from([entry.id.clone()]);
        let mut reachable = BTreeSet::new();
        while let Some(function_id) = pending.pop_front() {
            if !reachable.insert(function_id.clone()) {
                continue;
            }
            if reachable.len() > MAX_EXECUTION_SEMANTIC_FUNCTIONS_V1 {
                self.resource_finding(
                    kernel,
                    "reachable functions",
                    MAX_EXECUTION_SEMANTIC_FUNCTIONS_V1,
                );
                return Vec::new();
            }
            let Some(function) = self.module.function(&function_id) else {
                continue;
            };
            let Some(body) = &function.body else {
                continue;
            };
            for block in &body.blocks {
                for (operation_index, operation) in block.operations.iter().enumerate() {
                    let OperationKind::Call { callee, .. } = &operation.kind else {
                        continue;
                    };
                    self.call_edges = self.call_edges.saturating_add(1);
                    if self.call_edges > MAX_EXECUTION_SEMANTIC_CALL_EDGES_V1 {
                        self.resource_finding(
                            kernel,
                            "call edges",
                            MAX_EXECUTION_SEMANTIC_CALL_EDGES_V1,
                        );
                        return Vec::new();
                    }
                    match self.module.function(callee) {
                        Some(target) if target.body.is_some() => pending.push_back(callee.clone()),
                        _ => self.push(
                            ProductionCapabilityAnalysisKindV1::MemoryVisibility,
                            KernelCheckStatusV1::Incomplete,
                            location(kernel, &function.id, block.id, operation_index, None),
                            None,
                            ExecutionCapabilitySemanticReasonV1::UnsupportedExternalCall {
                                callee: callee.clone(),
                            },
                        ),
                    }
                }
            }
        }
        reachable.into_iter().collect()
    }

    fn function_control(
        &mut self,
        kernel: &Kernel,
        reachable: &[FunctionId],
        reports: &BTreeMap<FunctionId, crate::AnalysisReport>,
    ) -> BTreeMap<FunctionId, Variation> {
        let mut incoming = BTreeMap::from([(kernel.entry.clone(), Variation::GridUniform)]);
        let mut changed = true;
        let mut rounds = 0usize;
        while changed {
            changed = false;
            rounds = rounds.saturating_add(1);
            if rounds > reachable.len().saturating_mul(4).saturating_add(1) {
                self.push(
                    ProductionCapabilityAnalysisKindV1::Uniformity,
                    KernelCheckStatusV1::Incomplete,
                    location(kernel, &kernel.entry, BlockId(0), 0, None),
                    None,
                    ExecutionCapabilitySemanticReasonV1::RecursiveCapabilityCall {
                        callee: kernel.entry.clone(),
                    },
                );
                break;
            }
            for function_id in reachable {
                let Some(function) = self.module.function(function_id) else {
                    continue;
                };
                let Some(body) = &function.body else {
                    continue;
                };
                let caller_control = incoming
                    .get(function_id)
                    .copied()
                    .unwrap_or(Variation::Varying);
                let Some(report) = reports.get(function_id) else {
                    continue;
                };
                for block in &body.blocks {
                    for operation in &block.operations {
                        let OperationKind::Call { callee, .. } = &operation.kind else {
                            continue;
                        };
                        if !reachable.contains(callee) {
                            continue;
                        }
                        let control = caller_control.join(report.block_control(block.id));
                        let prior = incoming
                            .get(callee)
                            .copied()
                            .unwrap_or(Variation::GridUniform);
                        let next = prior.join(control);
                        if incoming.insert(callee.clone(), next) != Some(next) {
                            changed = true;
                        }
                    }
                }
            }
        }
        incoming
    }

    fn check_convergence(
        &mut self,
        kernel: &Kernel,
        function: &Function,
        report: &crate::AnalysisReport,
        incoming: Variation,
        workgroup_size: Option<u64>,
    ) {
        let Some(body) = &function.body else {
            return;
        };
        for block in &body.blocks {
            let control = incoming.join(report.block_control(block.id));
            for (operation_index, operation) in block.operations.iter().enumerate() {
                self.checked_operations = self.checked_operations.saturating_add(1);
                if self.checked_operations > MAX_EXECUTION_SEMANTIC_OPERATIONS_V1 {
                    self.resource_finding(
                        kernel,
                        "operations",
                        MAX_EXECUTION_SEMANTIC_OPERATIONS_V1,
                    );
                    return;
                }
                let OperationKind::ExecutionCapability(contract) = &operation.kind else {
                    continue;
                };
                let Some(scope) = convergence_scope(&contract.operation) else {
                    continue;
                };
                let is_collective = matches!(
                    contract.operation,
                    ExecutionCapabilityOperationV1::WorkgroupCollective { .. }
                        | ExecutionCapabilityOperationV1::SubgroupCollective { .. }
                );
                if is_collective {
                    self.checked_collective_sites = self.checked_collective_sites.saturating_add(1);
                } else {
                    self.checked_barrier_sites = self.checked_barrier_sites.saturating_add(1);
                }
                let location = location(
                    kernel,
                    &function.id,
                    block.id,
                    operation_index,
                    Some(contract.source),
                );
                if !control.is_uniform_for(scope) {
                    let status = if function.id != kernel.entry || function_has_cfg_cycle(function)
                    {
                        KernelCheckStatusV1::Incomplete
                    } else {
                        KernelCheckStatusV1::Rejected
                    };
                    self.push(
                        ProductionCapabilityAnalysisKindV1::BarrierConvergence,
                        status,
                        location.clone(),
                        None,
                        ExecutionCapabilitySemanticReasonV1::NonUniformArrival { scope, control },
                    );
                }
                match &contract.operation {
                    ExecutionCapabilityOperationV1::LdsInitializeByInvocation {
                        elements, ..
                    }
                    | ExecutionCapabilityOperationV1::WorkgroupCollective {
                        elements, ..
                    } => match workgroup_size {
                        Some(expected) if expected != *elements => self.push(
                            ProductionCapabilityAnalysisKindV1::BarrierConvergence,
                            KernelCheckStatusV1::Rejected,
                            location.clone(),
                            None,
                            ExecutionCapabilitySemanticReasonV1::IncompleteCollectiveParticipation {
                                expected,
                                observed: *elements,
                            },
                        ),
                        None => self.push(
                            ProductionCapabilityAnalysisKindV1::BarrierConvergence,
                            KernelCheckStatusV1::Incomplete,
                            location.clone(),
                            None,
                            ExecutionCapabilitySemanticReasonV1::UniformityIncomplete,
                        ),
                        _ => {}
                    },
                    ExecutionCapabilityOperationV1::SubgroupCollective { width, .. }
                    | ExecutionCapabilityOperationV1::SubgroupBarrier { width, .. } => {
                        match workgroup_size {
                            Some(participants)
                                if participants < u64::from(*width)
                                    || !participants.is_multiple_of(u64::from(*width)) =>
                            {
                                self.push(
                                    ProductionCapabilityAnalysisKindV1::BarrierConvergence,
                                    KernelCheckStatusV1::Rejected,
                                    location,
                                    None,
                                    ExecutionCapabilitySemanticReasonV1::IncompleteCollectiveParticipation {
                                        expected: u64::from(*width),
                                        observed: participants,
                                    },
                                );
                            }
                            None => self.push(
                                ProductionCapabilityAnalysisKindV1::BarrierConvergence,
                                KernelCheckStatusV1::Incomplete,
                                location,
                                None,
                                ExecutionCapabilitySemanticReasonV1::UniformityIncomplete,
                            ),
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn check_flow(&mut self, kernel: &Kernel, function: &Function, workgroup_size: Option<u64>) {
        let Some(body) = &function.body else {
            return;
        };
        let Some(entry) = body.blocks.first() else {
            return;
        };
        let mut input = FlowState::default();
        seed_types(
            &mut input,
            body.parameters
                .iter()
                .copied()
                .zip(&function.signature.parameters),
        );
        seed_types(
            &mut input,
            entry
                .parameters
                .iter()
                .map(|parameter| (parameter.id, &parameter.ty)),
        );
        let mut states = BTreeMap::from([(entry.id, input)]);
        let mut pending = VecDeque::from([entry.id]);
        while let Some(block_id) = pending.pop_front() {
            self.flow_states = self.flow_states.saturating_add(1);
            if self.flow_states > MAX_EXECUTION_SEMANTIC_FLOW_STATES_V1 {
                self.resource_finding(kernel, "flow states", MAX_EXECUTION_SEMANTIC_FLOW_STATES_V1);
                return;
            }
            let Some(block) = body.blocks.iter().find(|block| block.id == block_id) else {
                continue;
            };
            let mut state = states.get(&block_id).cloned().unwrap_or_default();
            seed_types(
                &mut state,
                block
                    .parameters
                    .iter()
                    .map(|parameter| (parameter.id, &parameter.ty)),
            );
            for (operation_index, operation) in block.operations.iter().enumerate() {
                let OperationKind::ExecutionCapability(contract) = &operation.kind else {
                    if matches!(operation.kind, OperationKind::InlineAssembly(_)) {
                        self.push(
                            ProductionCapabilityAnalysisKindV1::MemoryVisibility,
                            KernelCheckStatusV1::Incomplete,
                            location(kernel, &function.id, block.id, operation_index, None),
                            None,
                            ExecutionCapabilitySemanticReasonV1::UnsupportedMemoryEffect,
                        );
                    }
                    continue;
                };
                let site = location(
                    kernel,
                    &function.id,
                    block.id,
                    operation_index,
                    Some(contract.source),
                );
                self.apply_epoch(contract, &site, &mut state);
                self.apply_lifecycle(contract, operation, &site, &mut state);
                self.apply_memory(kernel, contract, &site, &mut state, workgroup_size);
            }
            let Some(terminator) = &block.terminator else {
                continue;
            };
            for successor in successors(terminator) {
                let Some(successor_block) = body.blocks.iter().find(|block| block.id == successor)
                else {
                    continue;
                };
                let mut candidate = state.clone();
                seed_types(
                    &mut candidate,
                    successor_block
                        .parameters
                        .iter()
                        .map(|parameter| (parameter.id, &parameter.ty)),
                );
                let changed = match states.get_mut(&successor) {
                    Some(existing) => merge_state(existing, &candidate),
                    None => {
                        states.insert(successor, candidate);
                        true
                    }
                };
                if changed {
                    pending.push_back(successor);
                }
            }
        }
    }

    fn apply_epoch(
        &mut self,
        contract: &ExecutionCapabilityOpV1,
        location: &ExecutionCapabilitySemanticLocationV1,
        state: &mut FlowState,
    ) {
        if let ExecutionCapabilityOperationV1::WorkgroupDerive { .. } = contract.operation {
            if let (Some(brand), Some(epoch)) = (contract.workgroup_brand, contract.epoch_before) {
                state.epochs.insert(brand, BTreeSet::from([epoch]));
            }
            return;
        }
        let (Some(brand), Some(expected)) = (contract.workgroup_brand, contract.epoch_before)
        else {
            return;
        };
        match state.epochs.get(&brand) {
            Some(observed) if observed.iter().any(|epoch| *epoch != expected) => {
                let actual = observed
                    .iter()
                    .find(|epoch| **epoch != expected)
                    .copied()
                    .unwrap_or(expected);
                self.push(
                    ProductionCapabilityAnalysisKindV1::WorkgroupMemoryEpochs,
                    KernelCheckStatusV1::Rejected,
                    location.clone(),
                    None,
                    ExecutionCapabilitySemanticReasonV1::StaleEpoch {
                        expected,
                        observed: actual,
                    },
                );
            }
            None => self.push(
                ProductionCapabilityAnalysisKindV1::WorkgroupMemoryEpochs,
                KernelCheckStatusV1::Incomplete,
                location.clone(),
                None,
                ExecutionCapabilitySemanticReasonV1::MissingEpochProvenance,
            ),
            _ => {}
        }
        if let Some(after) = contract.epoch_after {
            self.checked_epoch_transitions = self.checked_epoch_transitions.saturating_add(1);
            state.epochs.insert(brand, BTreeSet::from([after]));
        }
    }

    fn apply_lifecycle(
        &mut self,
        contract: &ExecutionCapabilityOpV1,
        operation: &Operation,
        location: &ExecutionCapabilitySemanticLocationV1,
        state: &mut FlowState,
    ) {
        use ExecutionCapabilityOperationV1 as Op;
        match &contract.operation {
            Op::LdsAllocate { .. } => {
                if let Some(value) = lds_result(operation, ExecutionLdsStateV1::Uninitialized) {
                    state
                        .lds
                        .insert(value, BTreeSet::from([LdsPhase::Uninitialized]));
                }
            }
            Op::LdsInitializeByInvocation { .. } => {
                self.transition_lds(
                    contract.operands.first().copied(),
                    lds_result(operation, ExecutionLdsStateV1::InvocationInitialized),
                    LdsPhase::Uninitialized,
                    LdsPhase::InvocationInitialized,
                    location,
                    state,
                );
            }
            Op::LdsPublish { .. } => {
                self.transition_lds(
                    contract.operands.get(1).copied(),
                    lds_result(operation, ExecutionLdsStateV1::Published),
                    LdsPhase::InvocationInitialized,
                    LdsPhase::Published,
                    location,
                    state,
                );
            }
            Op::LdsReadPublished { .. } => {
                self.require_lds(
                    contract.operands.first().copied(),
                    LdsPhase::Published,
                    location,
                    state,
                );
            }
            Op::WorkgroupCollective { .. } => {
                self.transition_lds(
                    contract.operands.get(1).copied(),
                    lds_result(operation, ExecutionLdsStateV1::Uninitialized),
                    LdsPhase::Uninitialized,
                    LdsPhase::Uninitialized,
                    location,
                    state,
                );
            }
            Op::WorkgroupMemoryAllocate { .. } => {
                if let Some(value) =
                    view_result(operation, ExecutionMemoryInitializationV1::Uninitialized)
                {
                    state
                        .views
                        .insert(value, BTreeSet::from([ViewPhase::Uninitialized]));
                }
            }
            Op::MemoryStore {
                space: ExecutionMemoryAddressSpaceV1::Workgroup,
                access: ExecutionMemoryAccessV1::DisjointWrite,
                ..
            } => {
                if let Some(view) = contract.operands.first() {
                    state
                        .views
                        .insert(*view, BTreeSet::from([ViewPhase::InvocationInitialized]));
                }
            }
            Op::WorkgroupMemoryPublish { .. } => {
                let input = contract.operands.get(1).copied();
                let output = view_result(operation, ExecutionMemoryInitializationV1::Published);
                let observed = input.and_then(|value| state.views.get(&value));
                if !observed.is_some_and(|phases| {
                    phases.len() == 1 && phases.contains(&ViewPhase::InvocationInitialized)
                }) {
                    self.push(
                        ProductionCapabilityAnalysisKindV1::Initialization,
                        KernelCheckStatusV1::Rejected,
                        location.clone(),
                        None,
                        ExecutionCapabilitySemanticReasonV1::LdsStateMismatch {
                            expected: "invocation-initialized workgroup memory",
                            observed: "uninitialized or path-dependent workgroup memory",
                        },
                    );
                }
                if let Some(output) = output {
                    state
                        .views
                        .insert(output, BTreeSet::from([ViewPhase::Published]));
                }
            }
            Op::MemoryLoad {
                space: ExecutionMemoryAddressSpaceV1::Workgroup,
                ..
            } => {
                let observed = contract
                    .operands
                    .first()
                    .and_then(|value| state.views.get(value));
                if !observed.is_some_and(|phases| phases.contains(&ViewPhase::Published)) {
                    self.push(
                        ProductionCapabilityAnalysisKindV1::Initialization,
                        KernelCheckStatusV1::Rejected,
                        location.clone(),
                        None,
                        ExecutionCapabilitySemanticReasonV1::LdsStateMismatch {
                            expected: "published workgroup memory",
                            observed: "unpublished workgroup memory",
                        },
                    );
                }
            }
            _ => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn transition_lds(
        &mut self,
        input: Option<ValueId>,
        output: Option<ValueId>,
        expected: LdsPhase,
        next: LdsPhase,
        location: &ExecutionCapabilitySemanticLocationV1,
        state: &mut FlowState,
    ) {
        self.require_lds(input, expected, location, state);
        if let Some(input) = input {
            state
                .lds
                .insert(input, BTreeSet::from([LdsPhase::Consumed]));
        }
        if let Some(output) = output {
            state.lds.insert(output, BTreeSet::from([next]));
        }
    }

    fn require_lds(
        &mut self,
        input: Option<ValueId>,
        expected: LdsPhase,
        location: &ExecutionCapabilitySemanticLocationV1,
        state: &FlowState,
    ) {
        let observed = input.and_then(|value| state.lds.get(&value));
        if observed.is_some_and(|phases| phases.len() == 1 && phases.contains(&expected)) {
            return;
        }
        let phase = observed
            .and_then(|phases| phases.iter().next().copied())
            .unwrap_or(LdsPhase::Consumed);
        let reason = if phase == LdsPhase::Consumed && expected == LdsPhase::Uninitialized {
            ExecutionCapabilitySemanticReasonV1::LdsReuseBeforeEpochTransition
        } else {
            ExecutionCapabilitySemanticReasonV1::LdsStateMismatch {
                expected: expected.name(),
                observed: phase.name(),
            }
        };
        let stage = if matches!(
            &reason,
            ExecutionCapabilitySemanticReasonV1::LdsReuseBeforeEpochTransition
        ) {
            ProductionCapabilityAnalysisKindV1::WorkgroupMemoryEpochs
        } else {
            ProductionCapabilityAnalysisKindV1::Initialization
        };
        self.push(
            stage,
            KernelCheckStatusV1::Rejected,
            location.clone(),
            None,
            reason,
        );
    }

    fn apply_memory(
        &mut self,
        kernel: &Kernel,
        contract: &ExecutionCapabilityOpV1,
        location: &ExecutionCapabilitySemanticLocationV1,
        state: &mut FlowState,
        workgroup_size: Option<u64>,
    ) {
        use ExecutionCapabilityOperationV1 as Op;
        match &contract.operation {
            Op::MemoryStore { space, access, .. } => {
                self.checked_memory_effects = self.checked_memory_effects.saturating_add(1);
                if *space == ExecutionMemoryAddressSpaceV1::Global {
                    push_bounded_location(&mut state.pending_global_writes, location.clone());
                }
                if matches!(
                    (space, access),
                    (
                        ExecutionMemoryAddressSpaceV1::Workgroup
                            | ExecutionMemoryAddressSpaceV1::Global,
                        ExecutionMemoryAccessV1::ExclusiveReadWrite
                    )
                ) && workgroup_size.is_none_or(|participants| participants > 1)
                    && !self.conflicting_effects_composed_with_retained_pliron
                {
                    self.push(
                        ProductionCapabilityAnalysisKindV1::RaceFreedom,
                        KernelCheckStatusV1::Incomplete,
                        location.clone(),
                        None,
                        ExecutionCapabilitySemanticReasonV1::ConflictingAccessCorrespondenceUnavailable,
                    );
                }
            }
            Op::MemoryLoad {
                space: ExecutionMemoryAddressSpaceV1::Global,
                ..
            } => {
                self.checked_memory_effects = self.checked_memory_effects.saturating_add(1);
                if let Some(atomic) = &state.relaxed_acquire {
                    self.push(
                        ProductionCapabilityAnalysisKindV1::MemoryVisibility,
                        KernelCheckStatusV1::Rejected,
                        location.clone(),
                        Some(atomic.clone()),
                        ExecutionCapabilitySemanticReasonV1::InsufficientAtomicOrdering {
                            ordering: ExecutionMemoryOrderingV1::Relaxed,
                        },
                    );
                }
            }
            Op::WorkgroupBarrier { semantics, .. }
                if matches!(
                    semantics.spaces,
                    ExecutionMemorySpacesV1::Global | ExecutionMemorySpacesV1::GlobalAndWorkgroup
                ) && semantics.ordering == ExecutionMemoryOrderingV1::AcquireRelease =>
            {
                state.publication_ready = std::mem::take(&mut state.pending_global_writes);
            }
            Op::Atomic {
                kind,
                address_space,
                scope,
                success,
                ..
            } => {
                self.checked_atomic_operations = self.checked_atomic_operations.saturating_add(1);
                self.checked_memory_effects = self.checked_memory_effects.saturating_add(1);
                if !contract.operation.is_well_formed() {
                    self.push(
                        ProductionCapabilityAnalysisKindV1::AtomicLegality,
                        KernelCheckStatusV1::Rejected,
                        location.clone(),
                        None,
                        ExecutionCapabilitySemanticReasonV1::InvalidAtomicOperation,
                    );
                    return;
                }
                let spans_workgroups = kernel_spans_workgroups(kernel);
                if *address_space == ExecutionMemoryAddressSpaceV1::Global
                    && spans_workgroups
                    && !matches!(
                        kind,
                        ExecutionAtomicKindV1::BindGlobalLocation
                            | ExecutionAtomicKindV1::BindGlobalView
                    )
                    && !matches!(
                        scope,
                        ExecutionMemoryScopeV1::Device | ExecutionMemoryScopeV1::System
                    )
                {
                    self.push(
                        ProductionCapabilityAnalysisKindV1::AtomicLegality,
                        KernelCheckStatusV1::Rejected,
                        location.clone(),
                        None,
                        ExecutionCapabilitySemanticReasonV1::InsufficientAtomicScope {
                            scope: *scope,
                        },
                    );
                }
                let ordering = success.unwrap_or(ExecutionMemoryOrderingV1::Relaxed);
                let writes = matches!(
                    kind,
                    ExecutionAtomicKindV1::Store
                        | ExecutionAtomicKindV1::FetchAdd
                        | ExecutionAtomicKindV1::CompareExchange
                );
                let reads = matches!(
                    kind,
                    ExecutionAtomicKindV1::Load
                        | ExecutionAtomicKindV1::FetchAdd
                        | ExecutionAtomicKindV1::CompareExchange
                );
                if writes && !state.publication_ready.is_empty() && !has_release(ordering) {
                    self.push(
                        ProductionCapabilityAnalysisKindV1::MemoryVisibility,
                        KernelCheckStatusV1::Rejected,
                        location.clone(),
                        state.publication_ready.first().cloned(),
                        ExecutionCapabilitySemanticReasonV1::InsufficientAtomicOrdering {
                            ordering,
                        },
                    );
                }
                if writes && has_release(ordering) {
                    state.publication_ready.clear();
                }
                if reads {
                    state.relaxed_acquire = (!has_acquire(ordering)).then(|| location.clone());
                }
            }
            _ => {}
        }
    }

    fn resource_finding(&mut self, kernel: &Kernel, resource: &'static str, limit: usize) {
        self.push(
            ProductionCapabilityAnalysisKindV1::ResourceLegality,
            KernelCheckStatusV1::Incomplete,
            location(kernel, &kernel.entry, BlockId(0), 0, None),
            None,
            ExecutionCapabilitySemanticReasonV1::ResourceLimit { resource, limit },
        );
    }

    fn push(
        &mut self,
        stage: ProductionCapabilityAnalysisKindV1,
        status: KernelCheckStatusV1,
        location: ExecutionCapabilitySemanticLocationV1,
        related: Option<ExecutionCapabilitySemanticLocationV1>,
        reason: ExecutionCapabilitySemanticReasonV1,
    ) {
        if self.findings.len() == MAX_EXECUTION_SEMANTIC_FINDINGS_V1 {
            return;
        }
        let finding = ExecutionCapabilitySemanticFindingV1 {
            stage,
            status,
            location,
            related,
            reason,
        };
        if !self.findings.contains(&finding) {
            self.findings.push(finding);
        }
    }
}

fn sequence_step_cursors(step: SequenceStep) -> Vec<SequenceCursor> {
    match step {
        SequenceStep::Advance(cursor) => vec![*cursor],
        SequenceStep::Choices { cursors, .. } => cursors,
    }
}

fn sequence_pair_is_asynchronous(pair: &SequencePair) -> bool {
    match (&pair.left, &pair.right) {
        (SequenceCursor::Ready(left), SequenceCursor::Ready(right)) => left != right,
        (SequenceCursor::Waiting { .. }, SequenceCursor::Ready(_))
        | (SequenceCursor::Ready(_), SequenceCursor::Waiting { .. })
        | (SequenceCursor::Exited(_), SequenceCursor::Ready(_))
        | (SequenceCursor::Ready(_), SequenceCursor::Exited(_)) => true,
        _ => false,
    }
}

fn sequence_pair_is_ancestor(
    candidate: &SequencePair,
    current: &SequencePair,
    parents: &BTreeMap<SequencePair, SequencePair>,
) -> bool {
    let mut cursor = current;
    for _ in 0..=parents.len() {
        if cursor == candidate {
            return true;
        }
        let Some(parent) = parents.get(cursor) else {
            return false;
        };
        cursor = parent;
    }
    false
}

fn sequence_pair_location(
    kernel: &Kernel,
    pair: &SequencePair,
) -> ExecutionCapabilitySemanticLocationV1 {
    sequence_cursor_location(kernel, &pair.left)
}

fn sequence_cursor_location(
    kernel: &Kernel,
    cursor: &SequenceCursor,
) -> ExecutionCapabilitySemanticLocationV1 {
    match cursor {
        SequenceCursor::Ready(counter) => location(
            kernel,
            &counter.function,
            counter.block,
            counter.operation,
            None,
        ),
        SequenceCursor::Waiting { event, .. } => event.location.clone(),
        SequenceCursor::Exited(exit) => exit.clone(),
    }
}

fn convergence_scope(operation: &ExecutionCapabilityOperationV1) -> Option<SynchronizationScope> {
    match operation {
        ExecutionCapabilityOperationV1::LdsInitializeByInvocation { .. }
        | ExecutionCapabilityOperationV1::LdsPublish { .. }
        | ExecutionCapabilityOperationV1::WorkgroupBarrier { .. }
        | ExecutionCapabilityOperationV1::WorkgroupCollective { .. }
        | ExecutionCapabilityOperationV1::AsyncWait { .. }
        | ExecutionCapabilityOperationV1::WorkgroupMemoryPublish { .. } => {
            Some(SynchronizationScope::Workgroup)
        }
        ExecutionCapabilityOperationV1::SubgroupBarrier { .. }
        | ExecutionCapabilityOperationV1::SubgroupCollective { .. }
        | ExecutionCapabilityOperationV1::MatrixAccess { .. } => {
            Some(SynchronizationScope::Subgroup)
        }
        _ => None,
    }
}

fn location(
    kernel: &Kernel,
    function: &FunctionId,
    block: BlockId,
    operation: usize,
    source: Option<ExecutionCapabilitySourceV1>,
) -> ExecutionCapabilitySemanticLocationV1 {
    ExecutionCapabilitySemanticLocationV1 {
        root: kernel.id.clone(),
        function: function.clone(),
        block,
        operation,
        source,
    }
}

fn successors(terminator: &Terminator) -> Vec<BlockId> {
    match terminator {
        Terminator::Branch { target, .. } => vec![*target],
        Terminator::ConditionalBranch {
            then_target,
            else_target,
            ..
        } => vec![*then_target, *else_target],
        Terminator::Switch {
            cases,
            default_target,
            ..
        } => cases
            .iter()
            .map(|case| case.target)
            .chain([*default_target])
            .collect(),
        Terminator::IntegerSwitch {
            cases,
            default_target,
            ..
        } => cases
            .iter()
            .map(|case| case.target)
            .chain([*default_target])
            .collect(),
        Terminator::Return { .. } | Terminator::Unreachable => Vec::new(),
    }
}

fn function_has_cfg_cycle(function: &Function) -> bool {
    let Some(body) = &function.body else {
        return false;
    };
    let mut indegree = body
        .blocks
        .iter()
        .map(|block| (block.id, 0usize))
        .collect::<BTreeMap<_, _>>();
    for block in &body.blocks {
        let Some(terminator) = &block.terminator else {
            continue;
        };
        for successor in successors(terminator) {
            if let Some(count) = indegree.get_mut(&successor) {
                *count = count.saturating_add(1);
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(block, count)| (*count == 0).then_some(*block))
        .collect::<VecDeque<_>>();
    let mut removed = 0usize;
    while let Some(block_id) = ready.pop_front() {
        removed = removed.saturating_add(1);
        let Some(block) = body.blocks.iter().find(|block| block.id == block_id) else {
            continue;
        };
        let Some(terminator) = &block.terminator else {
            continue;
        };
        for successor in successors(terminator) {
            let Some(count) = indegree.get_mut(&successor) else {
                continue;
            };
            *count = count.saturating_sub(1);
            if *count == 0 {
                ready.push_back(successor);
            }
        }
    }
    removed != body.blocks.len()
}

fn seed_types<'a>(state: &mut FlowState, values: impl IntoIterator<Item = (ValueId, &'a Type)>) {
    for (value, ty) in values {
        let Type::ExecutionCapability(capability) = ty else {
            continue;
        };
        if let (Some(brand), Some(epoch)) = (capability.workgroup_brand, capability.epoch) {
            state.epochs.entry(brand).or_default().insert(epoch);
        }
        match &capability.role {
            fe2o3_kernel_ir::ExecutionCapabilityRoleV1::Lds { state: phase, .. } => {
                let phase = match phase {
                    ExecutionLdsStateV1::Uninitialized => LdsPhase::Uninitialized,
                    ExecutionLdsStateV1::InvocationInitialized => LdsPhase::InvocationInitialized,
                    ExecutionLdsStateV1::Published => LdsPhase::Published,
                    ExecutionLdsStateV1::PendingAsyncCopy => continue,
                };
                state.lds.entry(value).or_default().insert(phase);
            }
            fe2o3_kernel_ir::ExecutionCapabilityRoleV1::MemoryView {
                space: ExecutionMemoryAddressSpaceV1::Workgroup,
                initialization,
                ..
            } => {
                let phase = match initialization {
                    ExecutionMemoryInitializationV1::Uninitialized => ViewPhase::Uninitialized,
                    ExecutionMemoryInitializationV1::InvocationInitialized
                    | ExecutionMemoryInitializationV1::SelectedWriteInitializes => {
                        ViewPhase::InvocationInitialized
                    }
                    ExecutionMemoryInitializationV1::Published
                    | ExecutionMemoryInitializationV1::FullyInitialized => ViewPhase::Published,
                };
                state.views.entry(value).or_default().insert(phase);
            }
            _ => {}
        }
    }
}

fn lds_result(operation: &Operation, expected: ExecutionLdsStateV1) -> Option<ValueId> {
    operation.results.iter().find_map(|result| match &result.ty {
        Type::ExecutionCapability(capability)
            if matches!(
                capability.role,
                fe2o3_kernel_ir::ExecutionCapabilityRoleV1::Lds { state, .. } if state == expected
            ) => Some(result.id),
        _ => None,
    })
}

fn view_result(
    operation: &Operation,
    expected: ExecutionMemoryInitializationV1,
) -> Option<ValueId> {
    operation
        .results
        .iter()
        .find_map(|result| match &result.ty {
            Type::ExecutionCapability(capability)
                if matches!(
                    capability.role,
                    fe2o3_kernel_ir::ExecutionCapabilityRoleV1::MemoryView {
                        initialization,
                        ..
                    } if initialization == expected
                ) =>
            {
                Some(result.id)
            }
            _ => None,
        })
}

fn merge_state(existing: &mut FlowState, candidate: &FlowState) -> bool {
    let before = existing.clone();
    merge_sets(&mut existing.epochs, &candidate.epochs);
    merge_sets(&mut existing.lds, &candidate.lds);
    merge_sets(&mut existing.views, &candidate.views);
    for location in &candidate.pending_global_writes {
        push_bounded_location(&mut existing.pending_global_writes, location.clone());
    }
    for location in &candidate.publication_ready {
        push_bounded_location(&mut existing.publication_ready, location.clone());
    }
    if existing.relaxed_acquire.is_none() {
        existing.relaxed_acquire = candidate.relaxed_acquire.clone();
    }
    *existing != before
}

fn merge_sets<K: Ord + Clone, V: Ord + Clone>(
    existing: &mut BTreeMap<K, BTreeSet<V>>,
    candidate: &BTreeMap<K, BTreeSet<V>>,
) {
    for (key, values) in candidate {
        existing
            .entry(key.clone())
            .or_default()
            .extend(values.iter().cloned());
    }
}

fn push_bounded_location(
    locations: &mut Vec<ExecutionCapabilitySemanticLocationV1>,
    location: ExecutionCapabilitySemanticLocationV1,
) {
    if locations.len() < MAX_EXECUTION_SEMANTIC_FINDINGS_V1 && !locations.contains(&location) {
        locations.push(location);
    }
}

fn kernel_spans_workgroups(kernel: &Kernel) -> bool {
    let Some(workgroup) = kernel.workgroup_size else {
        return true;
    };
    kernel
        .domain
        .extents()
        .zip([workgroup.x, workgroup.y, workgroup.z])
        .any(|(extent, workgroup_extent)| match extent {
            fe2o3_kernel_ir::LaunchExtent::Dynamic => true,
            fe2o3_kernel_ir::LaunchExtent::Static(global_extent) => {
                global_extent > workgroup_extent
            }
        })
}

const fn has_release(ordering: ExecutionMemoryOrderingV1) -> bool {
    matches!(
        ordering,
        ExecutionMemoryOrderingV1::Release
            | ExecutionMemoryOrderingV1::AcquireRelease
            | ExecutionMemoryOrderingV1::SequentiallyConsistent
    )
}

const fn has_acquire(ordering: ExecutionMemoryOrderingV1) -> bool {
    matches!(
        ordering,
        ExecutionMemoryOrderingV1::Acquire
            | ExecutionMemoryOrderingV1::AcquireRelease
            | ExecutionMemoryOrderingV1::SequentiallyConsistent
    )
}

#[cfg(test)]
mod tests {
    use fe2o3_kernel_ir::{
        Axis, BasicBlock, ComparePredicate, Constant, ExecutionCapabilityProvenanceV1,
        ExecutionCapabilityRoleV1, ExecutionCapabilitySignatureV1, ExecutionCapabilityTypeV1,
        ExecutionCollectiveKindV1, ExecutionElementLayoutV1, ExecutionMemorySemanticsV1,
        ExecutionSafetyObligationsV1, ExecutionTypeIdentityV1, IntrinsicKind, IntrinsicOperation,
        LaunchDomain, LaunchExtent, ScalarType, Signature, ValueDef, WorkgroupSize,
        required_execution_obligations_v1,
    };

    use super::*;

    const BRAND: [u8; 32] = [0x41; 32];
    const EPOCH_1: [u8; 32] = [0x51; 32];
    const EPOCH_2: [u8; 32] = [0x52; 32];
    const EPOCH_3: [u8; 32] = [0x53; 32];

    fn id(byte: u8) -> ExecutionTypeIdentityV1 {
        ExecutionTypeIdentityV1::new([byte; 32])
    }

    fn provenance() -> ExecutionCapabilityProvenanceV1 {
        ExecutionCapabilityProvenanceV1 {
            root: FunctionId::new("entry"),
            kernel_binding: [1; 32],
            frontend_unit: [2; 32],
            kernel_marker: [3; 32],
            target_brand: [4; 32],
            launch_brand: [5; 32],
            issuance: [6; 32],
        }
    }

    fn capability(
        operation: ExecutionCapabilityOperationV1,
        operands: Vec<ValueId>,
        results: Vec<ValueDef>,
        source_operation: u8,
        epoch_before: [u8; 32],
        epoch_after: Option<[u8; 32]>,
    ) -> Operation {
        let obligations = required_execution_obligations_v1(&operation);
        Operation::new(
            results,
            OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
                operands,
                signature: ExecutionCapabilitySignatureV1::new(&[], id(0xfe)).unwrap(),
                provenance: provenance(),
                workgroup_brand: Some(BRAND),
                epoch_before: Some(epoch_before),
                epoch_after,
                obligations: ExecutionSafetyObligationsV1::from_bits(obligations),
                source: ExecutionCapabilitySourceV1 {
                    function: [0x71; 32],
                    operation: [source_operation; 32],
                    block: u32::from(source_operation),
                },
                operation,
            }),
        )
    }

    fn derive(source_operation: u8) -> Operation {
        capability(
            ExecutionCapabilityOperationV1::WorkgroupDerive {
                context: id(1),
                workgroup: id(2),
            },
            vec![],
            vec![],
            source_operation,
            EPOCH_1,
            None,
        )
    }

    fn barrier(
        source_operation: u8,
        epoch_before: [u8; 32],
        epoch_after: Option<[u8; 32]>,
    ) -> Operation {
        capability(
            ExecutionCapabilityOperationV1::WorkgroupBarrier {
                input_workgroup: id(2),
                output_workgroup: id(3),
                semantics: ExecutionMemorySemanticsV1 {
                    scope: ExecutionMemoryScopeV1::Workgroup,
                    ordering: ExecutionMemoryOrderingV1::AcquireRelease,
                    spaces: ExecutionMemorySpacesV1::Workgroup,
                },
            },
            vec![],
            vec![],
            source_operation,
            epoch_before,
            epoch_after,
        )
    }

    fn varying_condition() -> Vec<Operation> {
        vec![
            Operation::effect_free(
                ValueDef::new(ValueId(0), Type::INDEX),
                OperationKind::Intrinsic(IntrinsicOperation::new(
                    IntrinsicKind::InvocationIndex {
                        kind: fe2o3_kernel_ir::IndexKind::Local,
                        axis: Axis::X,
                    },
                    Type::INDEX,
                )),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(1), Type::INDEX),
                OperationKind::Constant(Constant::Index(0)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(2), Type::BOOL),
                OperationKind::Compare {
                    predicate: ComparePredicate::NotEqual,
                    lhs: ValueId(0),
                    rhs: ValueId(1),
                },
            ),
        ]
    }

    fn returning(id: u32) -> BasicBlock {
        let mut block = BasicBlock::new(BlockId(id));
        block.terminator = Some(Terminator::Return { values: vec![] });
        block
    }

    fn module(blocks: Vec<BasicBlock>, global_extent: u32) -> Module {
        let mut module = Module::new("execution-capability-semantics");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            blocks,
        ));
        let mut kernel = Kernel::new(
            "kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(global_extent),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
        module
    }

    fn identity() -> VerifiedCanonicalKernelIrIdentityV13 {
        let canonical =
            VerifiedCanonicalKernelIrV13::from_module(module(vec![returning(0)], 64)).unwrap();
        *canonical.identity()
    }

    fn report(module: &Module, conflicts_composed: bool) -> ExecutionCapabilityFinalGraphReportV1 {
        analyze_module(module, identity(), 7, conflicts_composed)
    }

    fn has_reason(
        report: &ExecutionCapabilityFinalGraphReportV1,
        predicate: impl Fn(&ExecutionCapabilitySemanticReasonV1) -> bool,
    ) -> bool {
        report
            .findings()
            .iter()
            .any(|finding| predicate(finding.reason()))
    }

    #[test]
    fn exact_canonical_entry_is_clean_and_authority_free() {
        let module = module(vec![returning(0)], 64);
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
        let report = analyze_execution_capability_final_graph_v1(&canonical, &module, 9).unwrap();
        assert_eq!(report.status(), KernelCheckStatusV1::Clean);
        assert_eq!(report.final_epoch(), 9);
        assert!(!report.grants_proof_machine_artifact_or_launch_authority());
    }

    #[test]
    fn conditional_missing_barrier_has_a_bounded_count_witness() {
        let mut entry = BasicBlock::new(BlockId(0));
        entry.operations.push(derive(1));
        entry.operations.extend(varying_condition());
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target: BlockId(1),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        });
        let mut arrived = returning(1);
        arrived.operations.push(barrier(10, EPOCH_1, Some(EPOCH_2)));
        let report = report(&module(vec![entry, arrived, returning(2)], 64), true);
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        assert!(has_reason(&report, |reason| matches!(
            reason,
            ExecutionCapabilitySemanticReasonV1::BarrierCountMismatch { .. }
        )));
        let diagnostic = report
            .findings()
            .iter()
            .find(|finding| {
                matches!(
                    finding.reason(),
                    ExecutionCapabilitySemanticReasonV1::BarrierCountMismatch { .. }
                )
            })
            .unwrap()
            .to_string();
        assert!(diagnostic.contains("root kernel in helper entry"));
        assert!(diagnostic.contains("source function"));
    }

    #[test]
    fn reordered_barriers_have_an_exact_sequence_witness() {
        let mut entry = BasicBlock::new(BlockId(0));
        entry.operations.push(derive(1));
        entry.operations.extend(varying_condition());
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target: BlockId(1),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        });
        let mut left = returning(1);
        left.operations.extend([
            barrier(10, EPOCH_1, Some(EPOCH_2)),
            barrier(11, EPOCH_2, Some(EPOCH_3)),
        ]);
        let mut right = returning(2);
        right.operations.extend([
            barrier(11, EPOCH_1, Some(EPOCH_2)),
            barrier(10, EPOCH_2, Some(EPOCH_3)),
        ]);
        let report = report(&module(vec![entry, left, right], 64), true);
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        assert!(has_reason(&report, |reason| matches!(
            reason,
            ExecutionCapabilitySemanticReasonV1::BarrierSequenceMismatch { .. }
        )));
    }

    #[test]
    fn helper_barrier_and_varying_early_return_are_compared_at_the_root() {
        let mut entry = BasicBlock::new(BlockId(0));
        entry.operations.push(derive(1));
        entry.operations.extend(varying_condition());
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target: BlockId(1),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        });
        let mut called = returning(1);
        called.operations.push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: FunctionId::new("helper"),
                arguments: vec![],
            },
        ));
        let mut module = module(vec![entry, called, returning(2)], 64);
        let mut helper = returning(3);
        helper.operations.push(barrier(12, EPOCH_1, Some(EPOCH_2)));
        module.functions.push(Function::internal_helper(
            "helper",
            Signature::new(vec![], vec![]),
            vec![],
            vec![helper],
        ));
        let report = report(&module, true);
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        assert!(has_reason(&report, |reason| matches!(
            reason,
            ExecutionCapabilitySemanticReasonV1::BarrierCountMismatch { .. }
        )));
    }

    #[test]
    fn uniform_loop_and_uniform_early_return_have_compatible_sequences() {
        let mut entry = BasicBlock::new(BlockId(0));
        entry.operations.push(derive(1));
        entry.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(0), Type::BOOL),
            OperationKind::Constant(Constant::Bool(true)),
        ));
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(0),
            then_target: BlockId(1),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        });
        let mut body = BasicBlock::new(BlockId(1));
        body.operations.push(derive(2));
        body.operations.push(barrier(20, EPOCH_1, None));
        body.terminator = Some(Terminator::Branch {
            target: BlockId(0),
            arguments: vec![],
        });
        let report = report(&module(vec![entry, body, returning(2)], 64), true);
        assert_eq!(report.status(), KernelCheckStatusV1::Clean);
    }

    #[test]
    fn unresolved_varying_loop_trip_count_is_incomplete() {
        let mut header = BasicBlock::new(BlockId(0));
        header.operations.push(derive(1));
        header.operations.extend(varying_condition());
        header.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target: BlockId(1),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        });
        let mut body = BasicBlock::new(BlockId(1));
        body.terminator = Some(Terminator::Branch {
            target: BlockId(0),
            arguments: vec![],
        });
        let mut exit = returning(2);
        exit.operations.push(barrier(21, EPOCH_1, None));
        let report = report(&module(vec![header, body, exit], 64), true);
        assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
        assert!(has_reason(&report, |reason| matches!(
            reason,
            ExecutionCapabilitySemanticReasonV1::DynamicBarrierSequenceUnsupported { .. }
        )));
    }

    #[test]
    fn varying_loop_with_a_barrier_has_a_count_counterexample() {
        let mut header = BasicBlock::new(BlockId(0));
        header.operations.push(derive(1));
        header.operations.extend(varying_condition());
        header.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(2),
            then_target: BlockId(1),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        });
        let mut body = BasicBlock::new(BlockId(1));
        body.operations.push(barrier(22, EPOCH_1, None));
        body.terminator = Some(Terminator::Branch {
            target: BlockId(0),
            arguments: vec![],
        });
        let report = report(&module(vec![header, body, returning(2)], 64), true);
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        assert!(has_reason(&report, |reason| matches!(
            reason,
            ExecutionCapabilitySemanticReasonV1::BarrierCountMismatch { .. }
        )));
    }

    fn lds_type(state: ExecutionLdsStateV1, epoch: [u8; 32]) -> Type {
        Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
            source_type: id(0x80),
            provenance: provenance(),
            workgroup_brand: Some(BRAND),
            epoch: Some(epoch),
            role: ExecutionCapabilityRoleV1::Lds {
                element: id(0x81),
                layout: ExecutionElementLayoutV1 {
                    byte_size: 4,
                    byte_alignment: 4,
                },
                elements: 64,
                state,
            },
        })
    }

    fn lds_allocate(result: u32, source: u8, epoch: [u8; 32]) -> Operation {
        capability(
            ExecutionCapabilityOperationV1::LdsAllocate {
                workgroup: id(2),
                lds: id(0x80),
                element: id(0x81),
                layout: ExecutionElementLayoutV1 {
                    byte_size: 4,
                    byte_alignment: 4,
                },
                elements: 64,
            },
            vec![],
            vec![ValueDef::new(
                ValueId(result),
                lds_type(ExecutionLdsStateV1::Uninitialized, epoch),
            )],
            source,
            epoch,
            None,
        )
    }

    fn collective(
        input: u32,
        result: u32,
        source: u8,
        elements: u64,
        before: [u8; 32],
        after: [u8; 32],
    ) -> Operation {
        capability(
            ExecutionCapabilityOperationV1::WorkgroupCollective {
                kind: ExecutionCollectiveKindV1::ReduceSum,
                input_workgroup: id(2),
                scratch: id(0x80),
                element: id(0x81),
                transition: id(0x82),
                value_type: ScalarType::U32,
                layout: ExecutionElementLayoutV1 {
                    byte_size: 4,
                    byte_alignment: 4,
                },
                elements,
            },
            vec![ValueId(99), ValueId(input)],
            vec![ValueDef::new(
                ValueId(result),
                lds_type(ExecutionLdsStateV1::Uninitialized, after),
            )],
            source,
            before,
            Some(after),
        )
    }

    #[test]
    fn collective_participation_and_lds_reuse_are_checked_separately() {
        let mut participation = returning(0);
        participation.operations.extend([
            derive(1),
            lds_allocate(10, 2, EPOCH_1),
            collective(10, 11, 3, 32, EPOCH_1, EPOCH_2),
        ]);
        let participation = report(&module(vec![participation], 64), true);
        assert_eq!(participation.status(), KernelCheckStatusV1::Rejected);
        assert!(has_reason(&participation, |reason| matches!(
            reason,
            ExecutionCapabilitySemanticReasonV1::IncompleteCollectiveParticipation {
                expected: 64,
                observed: 32
            }
        )));

        let mut reuse = returning(0);
        reuse.operations.extend([
            derive(1),
            lds_allocate(10, 2, EPOCH_1),
            collective(10, 11, 3, 64, EPOCH_1, EPOCH_2),
            collective(10, 12, 4, 64, EPOCH_2, EPOCH_3),
        ]);
        let reuse = report(&module(vec![reuse], 64), true);
        assert!(has_reason(&reuse, |reason| matches!(
            reason,
            ExecutionCapabilitySemanticReasonV1::LdsReuseBeforeEpochTransition
        )));
        assert_eq!(
            reuse
                .findings()
                .iter()
                .find(|finding| matches!(
                    finding.reason(),
                    ExecutionCapabilitySemanticReasonV1::LdsReuseBeforeEpochTransition
                ))
                .unwrap()
                .stage(),
            ProductionCapabilityAnalysisKindV1::WorkgroupMemoryEpochs
        );
    }

    #[test]
    fn read_before_initialization_and_stale_epoch_are_rejected() {
        let read = capability(
            ExecutionCapabilityOperationV1::LdsReadPublished {
                lds_reference: id(0x80),
                lds: id(0x80),
                workgroup: id(2),
                index: id(3),
                option: id(4),
                element: id(0x81),
                layout: ExecutionElementLayoutV1 {
                    byte_size: 4,
                    byte_alignment: 4,
                },
                elements: 64,
            },
            vec![ValueId(77)],
            vec![],
            3,
            EPOCH_1,
            None,
        );
        let mut block = returning(0);
        block.operations.extend([derive(1), read]);
        let read = report(&module(vec![block], 64), true);
        assert!(has_reason(&read, |reason| matches!(
            reason,
            ExecutionCapabilitySemanticReasonV1::LdsStateMismatch { .. }
        )));

        let mut block = returning(0);
        block.operations.extend([
            derive(1),
            barrier(4, EPOCH_1, Some(EPOCH_2)),
            lds_allocate(20, 5, EPOCH_1),
        ]);
        let stale = report(&module(vec![block], 64), true);
        assert!(has_reason(&stale, |reason| matches!(
            reason,
            ExecutionCapabilitySemanticReasonV1::StaleEpoch { .. }
        )));
    }

    fn atomic(
        source: u8,
        scope: ExecutionMemoryScopeV1,
        ordering: ExecutionMemoryOrderingV1,
        epoch: [u8; 32],
    ) -> Operation {
        capability(
            ExecutionCapabilityOperationV1::Atomic {
                kind: ExecutionAtomicKindV1::Store,
                authority: id(0x90),
                location_input: id(0x91),
                location: id(0x92),
                element: id(0x93),
                operand: Some(id(0x93)),
                replacement: None,
                result: id(0x94),
                value_type: ScalarType::U32,
                address_space: ExecutionMemoryAddressSpaceV1::Global,
                scope,
                success: Some(ordering),
                failure: None,
            },
            vec![],
            vec![],
            source,
            epoch,
            None,
        )
    }

    fn global_store(source: u8, access: ExecutionMemoryAccessV1) -> Operation {
        capability(
            ExecutionCapabilityOperationV1::MemoryStore {
                view: id(0xa0),
                workgroup: None,
                index: id(0xa1),
                element: id(0xa2),
                layout: ExecutionElementLayoutV1 {
                    byte_size: 4,
                    byte_alignment: 4,
                },
                result: id(0xa3),
                space: ExecutionMemoryAddressSpaceV1::Global,
                access,
            },
            vec![],
            vec![],
            source,
            EPOCH_1,
            None,
        )
    }

    #[test]
    fn atomic_scope_and_publication_ordering_are_enforced() {
        let mut scope = returning(0);
        scope.operations.extend([
            derive(1),
            atomic(
                2,
                ExecutionMemoryScopeV1::Workgroup,
                ExecutionMemoryOrderingV1::Relaxed,
                EPOCH_1,
            ),
        ]);
        let scope = report(&module(vec![scope], 128), true);
        assert!(has_reason(&scope, |reason| matches!(
            reason,
            ExecutionCapabilitySemanticReasonV1::InsufficientAtomicScope { .. }
        )));

        let mut ordering = returning(0);
        let mut publish = barrier(3, EPOCH_1, Some(EPOCH_2));
        let OperationKind::ExecutionCapability(contract) = &mut publish.kind else {
            unreachable!()
        };
        let ExecutionCapabilityOperationV1::WorkgroupBarrier { semantics, .. } =
            &mut contract.operation
        else {
            unreachable!()
        };
        semantics.spaces = ExecutionMemorySpacesV1::GlobalAndWorkgroup;
        ordering.operations.extend([
            derive(1),
            global_store(2, ExecutionMemoryAccessV1::DisjointWrite),
            publish,
            atomic(
                4,
                ExecutionMemoryScopeV1::Device,
                ExecutionMemoryOrderingV1::Relaxed,
                EPOCH_2,
            ),
        ]);
        let ordering = report(&module(vec![ordering], 128), true);
        assert!(has_reason(&ordering, |reason| matches!(
            reason,
            ExecutionCapabilitySemanticReasonV1::InsufficientAtomicOrdering { .. }
        )));
    }

    #[test]
    fn exclusive_access_needs_exact_retained_graph_collision_evidence() {
        let mut block = returning(0);
        block.operations.extend([
            derive(1),
            global_store(2, ExecutionMemoryAccessV1::ExclusiveReadWrite),
        ]);
        let module = module(vec![block], 64);
        let canonical_only = report(&module, false);
        assert_eq!(canonical_only.status(), KernelCheckStatusV1::Incomplete);
        assert!(has_reason(&canonical_only, |reason| matches!(
            reason,
            ExecutionCapabilitySemanticReasonV1::ConflictingAccessCorrespondenceUnavailable
        )));

        let composed = report(&module, true);
        assert_eq!(
            composed.stage_status(ProductionCapabilityAnalysisKindV1::RaceFreedom),
            KernelCheckStatusV1::Clean
        );
        assert!(composed.conflicting_effects_composed_with_retained_pliron());
    }
}
