//! Epoch-bound preservation analysis for Kernel IR V13 capability provenance.
//!
//! Compiler-issued contexts have no runtime memory effect, but they are not
//! dead values: their position, brand, source identity, and complete use chain
//! are protected provenance. Portable execution requirements are protected in
//! the same replay because deleting one can make a transformed graph appear
//! legal on a target that cannot execute it.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use fe2o3_kernel_ir::{
    BlockId, ExecutionCapabilityOpV1, ExecutionCapabilityRequirementV1, ExecutionCapabilityTypeV1,
    FunctionId, KernelContextSourceIdentityV1, KernelContextTypeV1, KernelId, Module,
    OperationKind, TargetCapability, Type, ValueId, VerifiedCanonicalKernelIrErrorV13,
    VerifiedCanonicalKernelIrIdentityV13, VerifiedCanonicalKernelIrV13,
};

/// Stable location of one direct logical context type in canonical KIR.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum KernelContextTypeLocationV1 {
    FunctionParameter {
        index: usize,
        value: ValueId,
    },
    FunctionResult {
        index: usize,
    },
    BlockParameter {
        block: BlockId,
        index: usize,
    },
    OperationResult {
        block: BlockId,
        operation: usize,
        index: usize,
    },
}

/// One context-typed definition or ABI position and its exact nominal brand.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct KernelContextTypeFactV1 {
    function: FunctionId,
    location: KernelContextTypeLocationV1,
    context: KernelContextTypeV1,
}

impl KernelContextTypeFactV1 {
    pub const fn function(&self) -> &FunctionId {
        &self.function
    }

    pub const fn location(&self) -> &KernelContextTypeLocationV1 {
        &self.location
    }

    pub const fn context(&self) -> &KernelContextTypeV1 {
        &self.context
    }
}

/// Exact coordinate at which a context SSA value is consumed.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum KernelContextUseLocationV1 {
    OperationOperand {
        block: BlockId,
        operation: usize,
        operand: usize,
    },
    TerminatorOperand {
        block: BlockId,
        operand: usize,
    },
}

/// One use of a context value. The value identity is retained deliberately:
/// substituting a context from another root or join is not preservation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct KernelContextUseFactV1 {
    function: FunctionId,
    location: KernelContextUseLocationV1,
    value: ValueId,
    context: KernelContextTypeV1,
}

impl KernelContextUseFactV1 {
    pub const fn function(&self) -> &FunctionId {
        &self.function
    }

    pub const fn location(&self) -> &KernelContextUseLocationV1 {
        &self.location
    }

    pub const fn value(&self) -> ValueId {
        self.value
    }

    pub const fn context(&self) -> &KernelContextTypeV1 {
        &self.context
    }
}

/// One unique, provenance-bearing context issuance.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct KernelContextIssuanceFactV1 {
    function: FunctionId,
    block: BlockId,
    operation: usize,
    result: ValueId,
    context: KernelContextTypeV1,
    source: KernelContextSourceIdentityV1,
}

/// Stable coordinate of one execution-capability type in canonical KIR V13.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExecutionCapabilityTypeLocationV1 {
    FunctionParameter {
        index: usize,
        value: ValueId,
    },
    FunctionResult {
        index: usize,
    },
    BlockParameter {
        block: BlockId,
        index: usize,
    },
    OperationResult {
        block: BlockId,
        operation: usize,
        index: usize,
        value: ValueId,
    },
}

/// One exact branded execution authority and its graph coordinate.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ExecutionCapabilityTypeFactV1 {
    function: FunctionId,
    location: ExecutionCapabilityTypeLocationV1,
    capability: ExecutionCapabilityTypeV1,
}

impl ExecutionCapabilityTypeFactV1 {
    pub const fn function(&self) -> &FunctionId {
        &self.function
    }

    pub const fn location(&self) -> &ExecutionCapabilityTypeLocationV1 {
        &self.location
    }

    pub const fn capability(&self) -> &ExecutionCapabilityTypeV1 {
        &self.capability
    }
}

/// One exact V13 execution operation, including operands, source identity,
/// provenance, role transitions, epochs, and safety obligations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionCapabilityOperationFactV1 {
    function: FunctionId,
    block: BlockId,
    operation: usize,
    contract: ExecutionCapabilityOpV1,
}

impl ExecutionCapabilityOperationFactV1 {
    pub const fn function(&self) -> &FunctionId {
        &self.function
    }

    pub const fn block(&self) -> BlockId {
        self.block
    }

    pub const fn operation(&self) -> usize {
        self.operation
    }

    pub const fn contract(&self) -> &ExecutionCapabilityOpV1 {
        &self.contract
    }
}

impl KernelContextIssuanceFactV1 {
    pub const fn function(&self) -> &FunctionId {
        &self.function
    }

    pub const fn block(&self) -> BlockId {
        self.block
    }

    pub const fn operation(&self) -> usize {
        self.operation
    }

    pub const fn result(&self) -> ValueId {
        self.result
    }

    pub const fn context(&self) -> &KernelContextTypeV1 {
        &self.context
    }

    pub const fn source(&self) -> KernelContextSourceIdentityV1 {
        self.source
    }

    /// Issuance is provenance only. This says nothing about uniformity,
    /// memory safety, synchronization, target support, or launch authority.
    pub const fn has_zero_runtime_memory_effect(&self) -> bool {
        true
    }
}

/// Declaration scope retained for one portable execution requirement.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExecutionRequirementScopeV1 {
    Module,
    Function(FunctionId),
    Kernel(KernelId),
}

/// One exact portable requirement at its declaration scope.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ScopedExecutionRequirementV1 {
    scope: ExecutionRequirementScopeV1,
    requirement: ExecutionCapabilityRequirementV1,
}

impl ScopedExecutionRequirementV1 {
    pub const fn scope(&self) -> &ExecutionRequirementScopeV1 {
        &self.scope
    }

    pub const fn requirement(&self) -> &ExecutionCapabilityRequirementV1 {
        &self.requirement
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProtectedCapabilityFactsV1 {
    context_types: Vec<KernelContextTypeFactV1>,
    context_uses: Vec<KernelContextUseFactV1>,
    issuances: Vec<KernelContextIssuanceFactV1>,
    execution_types: Vec<ExecutionCapabilityTypeFactV1>,
    execution_operations: Vec<ExecutionCapabilityOperationFactV1>,
    scoped_requirements: Vec<ScopedExecutionRequirementV1>,
    effective_scoped_requirements: Vec<ScopedExecutionRequirementV1>,
    effective_requirements: Vec<ExecutionCapabilityRequirementV1>,
}

/// Immutable analysis of the V13 facts that no ordinary optimization may
/// synthesize, delete, duplicate, move, or weaken.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelCapabilityPreservationAnalysisV1 {
    graph_epoch: u64,
    canonical_identity: VerifiedCanonicalKernelIrIdentityV13,
    facts: ProtectedCapabilityFactsV1,
}

impl KernelCapabilityPreservationAnalysisV1 {
    pub const fn graph_epoch(&self) -> u64 {
        self.graph_epoch
    }

    pub const fn canonical_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.canonical_identity
    }

    pub fn context_types(&self) -> &[KernelContextTypeFactV1] {
        &self.facts.context_types
    }

    pub fn context_uses(&self) -> &[KernelContextUseFactV1] {
        &self.facts.context_uses
    }

    pub fn issuances(&self) -> &[KernelContextIssuanceFactV1] {
        &self.facts.issuances
    }

    pub fn execution_types(&self) -> &[ExecutionCapabilityTypeFactV1] {
        &self.facts.execution_types
    }

    pub fn execution_operations(&self) -> &[ExecutionCapabilityOperationFactV1] {
        &self.facts.execution_operations
    }

    pub fn scoped_requirements(&self) -> &[ScopedExecutionRequirementV1] {
        &self.facts.scoped_requirements
    }

    pub fn effective_requirements(&self) -> &[ExecutionCapabilityRequirementV1] {
        &self.facts.effective_requirements
    }

    /// Effective requirements retain module, function, and kernel scope. In
    /// particular, moving a requirement between unrelated helpers or kernels
    /// cannot be hidden by an unchanged module-wide union.
    pub fn effective_scoped_requirements(&self) -> &[ScopedExecutionRequirementV1] {
        &self.facts.effective_scoped_requirements
    }

    pub const fn grants_uniformity_authority(&self) -> bool {
        false
    }

    pub const fn grants_memory_or_synchronization_authority(&self) -> bool {
        false
    }

    pub const fn grants_target_support_authority(&self) -> bool {
        false
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    /// Re-analyzes `candidate` and admits only an exact protected-fact replay
    /// at the epoch dictated by whether canonical V13 bytes changed.
    pub fn replay_candidate(
        &self,
        observed_input_epoch: u64,
        candidate_output_epoch: u64,
        committed_mutations: u64,
        candidate: &Module,
    ) -> Result<KernelCapabilityPreservationReplayV1, KernelCapabilityPreservationErrorV1> {
        if observed_input_epoch != self.graph_epoch {
            return Err(KernelCapabilityPreservationErrorV1::StaleAnalysisEpoch {
                captured: self.graph_epoch,
                observed: observed_input_epoch,
            });
        }

        let (canonical, facts) = capture(candidate)
            .map_err(|source| KernelCapabilityPreservationErrorV1::CandidateRejected { source })?;
        let changed = canonical.identity() != &self.canonical_identity;
        if changed != (committed_mutations != 0) {
            return Err(
                KernelCapabilityPreservationErrorV1::MutationPresenceMismatch {
                    canonical_changed: changed,
                    committed_mutations,
                },
            );
        }
        let expected_output_epoch = observed_input_epoch
            .checked_add(committed_mutations)
            .ok_or(KernelCapabilityPreservationErrorV1::MutationEpochOverflow {
                input: observed_input_epoch,
                committed_mutations,
            })?;
        if candidate_output_epoch != expected_output_epoch {
            return Err(KernelCapabilityPreservationErrorV1::InvalidOutputEpoch {
                input: observed_input_epoch,
                expected: expected_output_epoch,
                observed: candidate_output_epoch,
                changed,
            });
        }
        if facts.context_types != self.facts.context_types {
            return Err(KernelCapabilityPreservationErrorV1::ContextTypesChanged);
        }
        if facts.context_uses != self.facts.context_uses {
            return Err(KernelCapabilityPreservationErrorV1::ContextUsesChanged);
        }
        if facts.issuances != self.facts.issuances {
            return Err(KernelCapabilityPreservationErrorV1::IssuancesChanged);
        }
        if facts.execution_types != self.facts.execution_types {
            return Err(KernelCapabilityPreservationErrorV1::ExecutionCapabilityTypesChanged);
        }
        if facts.execution_operations != self.facts.execution_operations {
            return Err(KernelCapabilityPreservationErrorV1::ExecutionCapabilityOperationsChanged);
        }
        if facts.scoped_requirements != self.facts.scoped_requirements {
            return Err(KernelCapabilityPreservationErrorV1::ScopedRequirementsChanged);
        }
        if facts.effective_scoped_requirements != self.facts.effective_scoped_requirements {
            return Err(KernelCapabilityPreservationErrorV1::ScopedRequirementClosureChanged);
        }
        if facts.effective_requirements != self.facts.effective_requirements {
            return Err(KernelCapabilityPreservationErrorV1::RequirementClosureChanged);
        }

        Ok(KernelCapabilityPreservationReplayV1 {
            input_identity: self.canonical_identity,
            output_identity: *canonical.identity(),
            input_epoch: observed_input_epoch,
            output_epoch: candidate_output_epoch,
            changed,
        })
    }
}

/// Analyze one verified exact V13 graph at its immutable mutation epoch.
pub fn analyze_kernel_capability_preservation_v1(
    module: &Module,
    graph_epoch: u64,
) -> Result<KernelCapabilityPreservationAnalysisV1, VerifiedCanonicalKernelIrErrorV13> {
    let (canonical, facts) = capture(module)?;
    Ok(KernelCapabilityPreservationAnalysisV1 {
        graph_epoch,
        canonical_identity: *canonical.identity(),
        facts,
    })
}

/// Successful affected-analysis replay for one exact candidate graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use = "capability preservation replay is not semantic or target-support authority"]
pub struct KernelCapabilityPreservationReplayV1 {
    input_identity: VerifiedCanonicalKernelIrIdentityV13,
    output_identity: VerifiedCanonicalKernelIrIdentityV13,
    input_epoch: u64,
    output_epoch: u64,
    changed: bool,
}

impl KernelCapabilityPreservationReplayV1 {
    pub const fn input_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.input_identity
    }

    pub const fn output_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.output_identity
    }

    pub const fn input_epoch(&self) -> u64 {
        self.input_epoch
    }

    pub const fn output_epoch(&self) -> u64 {
        self.output_epoch
    }

    pub const fn changed(&self) -> bool {
        self.changed
    }

    pub const fn grants_semantic_preservation_authority(&self) -> bool {
        false
    }

    pub const fn grants_target_support_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum KernelCapabilityPreservationErrorV1 {
    StaleAnalysisEpoch {
        captured: u64,
        observed: u64,
    },
    MutationEpochOverflow {
        input: u64,
        committed_mutations: u64,
    },
    MutationPresenceMismatch {
        canonical_changed: bool,
        committed_mutations: u64,
    },
    InvalidOutputEpoch {
        input: u64,
        expected: u64,
        observed: u64,
        changed: bool,
    },
    CandidateRejected {
        source: VerifiedCanonicalKernelIrErrorV13,
    },
    ContextTypesChanged,
    ContextUsesChanged,
    IssuancesChanged,
    ExecutionCapabilityTypesChanged,
    ExecutionCapabilityOperationsChanged,
    ScopedRequirementsChanged,
    ScopedRequirementClosureChanged,
    RequirementClosureChanged,
}

impl fmt::Display for KernelCapabilityPreservationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleAnalysisEpoch { captured, observed } => write!(
                formatter,
                "kernel-capability analysis captured epoch {captured}, observed stale epoch {observed}"
            ),
            Self::MutationEpochOverflow {
                input,
                committed_mutations,
            } => write!(
                formatter,
                "kernel-capability replay cannot add {committed_mutations} mutations to epoch {input}"
            ),
            Self::MutationPresenceMismatch {
                canonical_changed,
                committed_mutations,
            } => write!(
                formatter,
                "kernel-capability replay observed canonical change {canonical_changed} with {committed_mutations} committed mutations"
            ),
            Self::InvalidOutputEpoch {
                input,
                expected,
                observed,
                changed,
            } => write!(
                formatter,
                "kernel-capability replay from epoch {input} expected output epoch {expected}, observed {observed} (canonical graph changed: {changed})"
            ),
            Self::CandidateRejected { source } => {
                write!(formatter, "candidate Kernel IR V13 was rejected: {source}")
            }
            Self::ContextTypesChanged => formatter.write_str(
                "candidate changed a logical kernel-context type, brand, or type coordinate",
            ),
            Self::ContextUsesChanged => formatter.write_str(
                "candidate changed a logical kernel-context use, value, or use coordinate",
            ),
            Self::IssuancesChanged => formatter.write_str(
                "candidate deleted, duplicated, moved, or substituted a kernel-context issuance",
            ),
            Self::ExecutionCapabilityTypesChanged => formatter.write_str(
                "candidate changed a branded execution-capability type or its exact coordinate",
            ),
            Self::ExecutionCapabilityOperationsChanged => formatter.write_str(
                "candidate changed, moved, duplicated, or substituted an execution-capability operation contract",
            ),
            Self::ScopedRequirementsChanged => formatter.write_str(
                "candidate changed a portable execution requirement or its declaration scope",
            ),
            Self::ScopedRequirementClosureChanged => formatter.write_str(
                "candidate changed the effective execution-requirement closure of a module, function, or kernel",
            ),
            Self::RequirementClosureChanged => formatter.write_str(
                "candidate changed the effective portable execution-requirement closure",
            ),
        }
    }
}

impl Error for KernelCapabilityPreservationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CandidateRejected { source } => Some(source),
            Self::StaleAnalysisEpoch { .. }
            | Self::MutationEpochOverflow { .. }
            | Self::MutationPresenceMismatch { .. }
            | Self::InvalidOutputEpoch { .. }
            | Self::ContextTypesChanged
            | Self::ContextUsesChanged
            | Self::IssuancesChanged
            | Self::ExecutionCapabilityTypesChanged
            | Self::ExecutionCapabilityOperationsChanged
            | Self::ScopedRequirementsChanged
            | Self::ScopedRequirementClosureChanged
            | Self::RequirementClosureChanged => None,
        }
    }
}

fn capture(
    module: &Module,
) -> Result<
    (VerifiedCanonicalKernelIrV13, ProtectedCapabilityFactsV1),
    VerifiedCanonicalKernelIrErrorV13,
> {
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone())?;
    Ok((canonical, protected_facts(module)))
}

fn protected_facts(module: &Module) -> ProtectedCapabilityFactsV1 {
    let mut context_types = Vec::new();
    let mut context_uses = Vec::new();
    let mut issuances = Vec::new();
    let mut execution_types = Vec::new();
    let mut execution_operations = Vec::new();

    for function in &module.functions {
        for (index, ty) in function.signature.parameters.iter().enumerate() {
            if let Type::KernelContext(context) = ty {
                let value = function
                    .body
                    .as_ref()
                    .and_then(|body| body.parameters.get(index))
                    .copied()
                    .expect("verified internal helper context parameter has a body value");
                context_types.push(KernelContextTypeFactV1 {
                    function: function.id.clone(),
                    location: KernelContextTypeLocationV1::FunctionParameter { index, value },
                    context: context.clone(),
                });
            }
            if let Type::ExecutionCapability(capability) = ty {
                let value = function
                    .body
                    .as_ref()
                    .and_then(|body| body.parameters.get(index))
                    .copied()
                    .expect("verified execution-capability parameter has a body value");
                execution_types.push(ExecutionCapabilityTypeFactV1 {
                    function: function.id.clone(),
                    location: ExecutionCapabilityTypeLocationV1::FunctionParameter { index, value },
                    capability: capability.clone(),
                });
            }
        }
        for (index, ty) in function.signature.results.iter().enumerate() {
            if let Type::KernelContext(context) = ty {
                context_types.push(KernelContextTypeFactV1 {
                    function: function.id.clone(),
                    location: KernelContextTypeLocationV1::FunctionResult { index },
                    context: context.clone(),
                });
            }
            if let Type::ExecutionCapability(capability) = ty {
                execution_types.push(ExecutionCapabilityTypeFactV1 {
                    function: function.id.clone(),
                    location: ExecutionCapabilityTypeLocationV1::FunctionResult { index },
                    capability: capability.clone(),
                });
            }
        }

        let Some(body) = &function.body else {
            continue;
        };
        let mut context_values = function
            .signature
            .parameters
            .iter()
            .zip(&body.parameters)
            .filter_map(|(ty, value)| match ty {
                Type::KernelContext(context) => Some((*value, context.clone())),
                Type::Unit
                | Type::Scalar(_)
                | Type::Pointer(_)
                | Type::Slice(_)
                | Type::GlobalCapability(_)
                | Type::ExecutionCapability(_) => None,
            })
            .collect::<BTreeMap<_, _>>();

        for block in &body.blocks {
            for (index, parameter) in block.parameters.iter().enumerate() {
                if let Type::KernelContext(context) = &parameter.ty {
                    context_values.insert(parameter.id, context.clone());
                    context_types.push(KernelContextTypeFactV1 {
                        function: function.id.clone(),
                        location: KernelContextTypeLocationV1::BlockParameter {
                            block: block.id,
                            index,
                        },
                        context: context.clone(),
                    });
                }
                if let Type::ExecutionCapability(capability) = &parameter.ty {
                    execution_types.push(ExecutionCapabilityTypeFactV1 {
                        function: function.id.clone(),
                        location: ExecutionCapabilityTypeLocationV1::BlockParameter {
                            block: block.id,
                            index,
                        },
                        capability: capability.clone(),
                    });
                }
            }
            for (operation_index, operation) in block.operations.iter().enumerate() {
                for (index, result) in operation.results.iter().enumerate() {
                    if let Type::KernelContext(context) = &result.ty {
                        context_values.insert(result.id, context.clone());
                        context_types.push(KernelContextTypeFactV1 {
                            function: function.id.clone(),
                            location: KernelContextTypeLocationV1::OperationResult {
                                block: block.id,
                                operation: operation_index,
                                index,
                            },
                            context: context.clone(),
                        });
                    }
                    if let Type::ExecutionCapability(capability) = &result.ty {
                        execution_types.push(ExecutionCapabilityTypeFactV1 {
                            function: function.id.clone(),
                            location: ExecutionCapabilityTypeLocationV1::OperationResult {
                                block: block.id,
                                operation: operation_index,
                                index,
                                value: result.id,
                            },
                            capability: capability.clone(),
                        });
                    }
                }
                if let OperationKind::ExecutionCapability(contract) = &operation.kind {
                    execution_operations.push(ExecutionCapabilityOperationFactV1 {
                        function: function.id.clone(),
                        block: block.id,
                        operation: operation_index,
                        contract: contract.clone(),
                    });
                }
                if let OperationKind::KernelContextIssue(issue) = &operation.kind
                    && let [result] = operation.results.as_slice()
                    && let Type::KernelContext(context) = &result.ty
                {
                    issuances.push(KernelContextIssuanceFactV1 {
                        function: function.id.clone(),
                        block: block.id,
                        operation: operation_index,
                        result: result.id,
                        context: context.clone(),
                        source: issue.source(),
                    });
                }
            }
        }

        for block in &body.blocks {
            for (operation_index, operation) in block.operations.iter().enumerate() {
                for (operand, value) in operation.operands().into_iter().enumerate() {
                    if let Some(context) = context_values.get(&value) {
                        context_uses.push(KernelContextUseFactV1 {
                            function: function.id.clone(),
                            location: KernelContextUseLocationV1::OperationOperand {
                                block: block.id,
                                operation: operation_index,
                                operand,
                            },
                            value,
                            context: context.clone(),
                        });
                    }
                }
            }
            if let Some(terminator) = &block.terminator {
                for (operand, value) in terminator.operands().into_iter().enumerate() {
                    if let Some(context) = context_values.get(&value) {
                        context_uses.push(KernelContextUseFactV1 {
                            function: function.id.clone(),
                            location: KernelContextUseLocationV1::TerminatorOperand {
                                block: block.id,
                                operand,
                            },
                            value,
                            context: context.clone(),
                        });
                    }
                }
            }
        }
    }

    let mut scoped_requirements = Vec::new();
    extend_requirements(
        &mut scoped_requirements,
        ExecutionRequirementScopeV1::Module,
        &module.required_capabilities,
    );
    for function in &module.functions {
        extend_requirements(
            &mut scoped_requirements,
            ExecutionRequirementScopeV1::Function(function.id.clone()),
            &function.required_capabilities,
        );
    }
    for kernel in &module.kernels {
        extend_requirements(
            &mut scoped_requirements,
            ExecutionRequirementScopeV1::Kernel(kernel.id.clone()),
            &kernel.required_capabilities,
        );
    }
    let (effective_scoped_requirements, effective_requirements) =
        effective_execution_requirements(module);

    ProtectedCapabilityFactsV1 {
        context_types,
        context_uses,
        issuances,
        execution_types,
        execution_operations,
        scoped_requirements,
        effective_scoped_requirements,
        effective_requirements,
    }
}

fn effective_execution_requirements(
    module: &Module,
) -> (
    Vec<ScopedExecutionRequirementV1>,
    Vec<ExecutionCapabilityRequirementV1>,
) {
    let mut closures = module
        .functions
        .iter()
        .map(|function| {
            (
                function.id.clone(),
                execution_requirements(function.effective_capabilities()),
            )
        })
        .collect::<BTreeMap<_, _>>();

    // Calls are part of the exact verified graph. Fixed-point propagation
    // records the requirement closure of recursive and mutually recursive
    // helper graphs without relying on traversal order.
    loop {
        let previous = closures.clone();
        let mut changed = false;
        for function in &module.functions {
            let Some(body) = &function.body else {
                continue;
            };
            let closure = closures
                .get_mut(&function.id)
                .expect("every verified function has an initialized closure");
            for operation in body.blocks.iter().flat_map(|block| &block.operations) {
                let OperationKind::Call { callee, .. } = &operation.kind else {
                    continue;
                };
                if let Some(callee_requirements) = previous.get(callee) {
                    let before = closure.len();
                    closure.extend(callee_requirements.iter().cloned());
                    changed |= closure.len() != before;
                }
            }
        }
        if !changed {
            break;
        }
    }

    let declared_module = execution_requirements(module.required_capabilities.iter().cloned());
    let mut module_closure = declared_module.clone();
    for requirements in closures.values() {
        module_closure.extend(requirements.iter().cloned());
    }

    let mut scoped = Vec::new();
    extend_requirement_set(
        &mut scoped,
        ExecutionRequirementScopeV1::Module,
        &module_closure,
    );
    for function in &module.functions {
        extend_requirement_set(
            &mut scoped,
            ExecutionRequirementScopeV1::Function(function.id.clone()),
            closures
                .get(&function.id)
                .expect("every verified function retains its closure"),
        );
    }
    for kernel in &module.kernels {
        let mut requirements = execution_requirements(kernel.required_capabilities.iter().cloned());
        if let Some(entry_requirements) = closures.get(&kernel.entry) {
            requirements.extend(entry_requirements.iter().cloned());
        }
        extend_requirement_set(
            &mut scoped,
            ExecutionRequirementScopeV1::Kernel(kernel.id.clone()),
            &requirements,
        );
    }

    (scoped, module_closure.into_iter().collect())
}

fn execution_requirements(
    capabilities: impl IntoIterator<Item = TargetCapability>,
) -> BTreeSet<ExecutionCapabilityRequirementV1> {
    capabilities
        .into_iter()
        .filter_map(|capability| match capability {
            TargetCapability::Execution(requirement) => Some(requirement),
            TargetCapability::Float16
            | TargetCapability::BFloat16
            | TargetCapability::Float64
            | TargetCapability::Int64
            | TargetCapability::Subgroups
            | TargetCapability::SubgroupSize(_)
            | TargetCapability::WorkgroupMemory
            | TargetCapability::WorkgroupBarrier
            | TargetCapability::Atomic { .. }
            | TargetCapability::DynamicWorkgroupMemory
            | TargetCapability::Extension { .. }
            | TargetCapability::WaveWidth(_) => None,
        })
        .collect()
}

fn extend_requirement_set(
    output: &mut Vec<ScopedExecutionRequirementV1>,
    scope: ExecutionRequirementScopeV1,
    requirements: &BTreeSet<ExecutionCapabilityRequirementV1>,
) {
    output.extend(
        requirements
            .iter()
            .cloned()
            .map(|requirement| ScopedExecutionRequirementV1 {
                scope: scope.clone(),
                requirement,
            }),
    );
}

fn extend_requirements(
    output: &mut Vec<ScopedExecutionRequirementV1>,
    scope: ExecutionRequirementScopeV1,
    capabilities: &BTreeSet<TargetCapability>,
) {
    output.extend(
        capabilities
            .iter()
            .filter_map(|capability| match capability {
                TargetCapability::Execution(requirement) => Some(ScopedExecutionRequirementV1 {
                    scope: scope.clone(),
                    requirement: requirement.clone(),
                }),
                TargetCapability::Float16
                | TargetCapability::BFloat16
                | TargetCapability::Float64
                | TargetCapability::Int64
                | TargetCapability::Subgroups
                | TargetCapability::SubgroupSize(_)
                | TargetCapability::WorkgroupMemory
                | TargetCapability::WorkgroupBarrier
                | TargetCapability::Atomic { .. }
                | TargetCapability::DynamicWorkgroupMemory
                | TargetCapability::Extension { .. }
                | TargetCapability::WaveWidth(_) => None,
            }),
    );
}
