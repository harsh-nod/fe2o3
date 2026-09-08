//! Owner-held verification of the exact optimized, target-bound KIR V13 graph.

use std::{collections::BTreeSet, error::Error, fmt};

use fe2o3_kernel_analysis::{
    ExecutionCapabilityAtomicScopeErrorV1, ExecutionCapabilityAtomicScopeReportV1,
    KernelCapabilityPreservationAnalysisV1, KernelCheckStatusV1,
    PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1, PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2,
    ProductionCapabilityAnalysisExecutorV1, ProductionCapabilityAnalysisStageV1,
    ProductionPlironPreloweringErrorV2, ProductionPlironPreloweringReportV2,
    ProductionW4ExecutionErrorV1, ProductionW4FinalGraphExecutionV1,
    ProductionW4FinalGraphSubjectV1, ProductionW4HandoffErrorV1, ProductionW4LiveFunctionV1,
    ProductionW4TargetResourceInputV1, analyze_execution_capability_atomic_scope_v1,
    analyze_kernel_capability_preservation_v1,
    execute_production_w4_final_graph_capability_witness_v1,
    require_production_pliron_checks_before_lowering_v2,
};
use fe2o3_kernel_ir::{
    ExecutionCapabilityRequirementV1, FunctionId, Module, ResourceCapabilityRequirementV1,
    TargetCapability, VerifiedCanonicalKernelIrErrorV13, VerifiedCanonicalKernelIrIdentityV13,
    VerifiedCanonicalKernelIrV13, decode_module_v13,
};
use fe2o3_target_spec::{
    MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1, TargetCapabilityDecisionOutcomeV1,
    TargetCapabilityDecisionV1, TargetCapabilityModelIdentityErrorV1,
    TargetCapabilityModelIdentityV1, TargetCapabilityQueryErrorV1, TargetCapabilityRequirementV1,
    TargetResourceRequirementV1,
};

use crate::{
    KirBridgeErrorV1, KirBridgeExecutionCapabilityWitnessV1, KirBridgeRoundTripReportV1,
    KirPlironGraphV1, PlironSession, ShellLimits, kir_v13_execution_capability_witness_v1,
};

pub const PRODUCTION_FINAL_GRAPH_VERIFICATION_VERSION_V1: u16 = 1;

/// Exact W5 target decisions retained for one final graph subject.
///
/// Construction validates the decision records but grants no lowering,
/// publication, artifact, load, or launch authority.
pub struct ProductionFinalGraphTargetContractV1 {
    final_graph: VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    neutral_graph: VerifiedCanonicalKernelIrIdentityV13,
    neutral_epoch: u64,
    closure_identity: [u8; 32],
    model: TargetCapabilityModelIdentityV1,
    decisions: Box<[TargetCapabilityDecisionV1]>,
}

impl ProductionFinalGraphTargetContractV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        final_graph: &VerifiedCanonicalKernelIrV13,
        final_epoch: u64,
        neutral_graph: VerifiedCanonicalKernelIrIdentityV13,
        neutral_epoch: u64,
        closure_identity: [u8; 32],
        model: TargetCapabilityModelIdentityV1,
        decisions: impl IntoIterator<Item = TargetCapabilityDecisionV1>,
    ) -> Result<Self, ProductionFinalGraphTargetContractErrorV1> {
        final_graph
            .revalidate()
            .map_err(ProductionFinalGraphTargetContractErrorV1::FinalGraph)?;
        model
            .validate()
            .map_err(ProductionFinalGraphTargetContractErrorV1::TargetModel)?;
        let mut retained = Vec::new();
        for decision in decisions {
            if retained.len() == MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1 {
                return Err(
                    ProductionFinalGraphTargetContractErrorV1::DecisionLimitExceeded {
                        limit: MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1,
                    },
                );
            }
            decision
                .validate()
                .map_err(ProductionFinalGraphTargetContractErrorV1::Decision)?;
            if decision.model() != model {
                return Err(ProductionFinalGraphTargetContractErrorV1::WrongTargetModel);
            }
            if decision.outcome() != TargetCapabilityDecisionOutcomeV1::Supported {
                return Err(
                    ProductionFinalGraphTargetContractErrorV1::NonFinalDecision {
                        requirement: decision.requirement(),
                        outcome: decision.outcome(),
                    },
                );
            }
            if retained
                .last()
                .is_some_and(|prior: &TargetCapabilityDecisionV1| {
                    prior.requirement() >= decision.requirement()
                })
            {
                return Err(ProductionFinalGraphTargetContractErrorV1::NonCanonicalDecisions);
            }
            retained.push(decision);
        }
        if retained.is_empty() {
            return Err(ProductionFinalGraphTargetContractErrorV1::EmptyDecisions);
        }
        Ok(Self {
            final_graph: *final_graph.identity(),
            final_epoch,
            neutral_graph,
            neutral_epoch,
            closure_identity,
            model,
            decisions: retained.into_boxed_slice(),
        })
    }

    pub const fn final_graph(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.final_graph
    }

    pub const fn final_epoch(&self) -> u64 {
        self.final_epoch
    }

    pub const fn neutral_graph(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.neutral_graph
    }

    pub const fn neutral_epoch(&self) -> u64 {
        self.neutral_epoch
    }

    pub const fn closure_identity(&self) -> [u8; 32] {
        self.closure_identity
    }

    pub const fn model(&self) -> TargetCapabilityModelIdentityV1 {
        self.model
    }

    pub fn decisions(&self) -> &[TargetCapabilityDecisionV1] {
        &self.decisions
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionFinalGraphTargetContractErrorV1 {
    FinalGraph(VerifiedCanonicalKernelIrErrorV13),
    TargetModel(TargetCapabilityModelIdentityErrorV1),
    Decision(TargetCapabilityQueryErrorV1),
    DecisionLimitExceeded {
        limit: usize,
    },
    EmptyDecisions,
    WrongTargetModel,
    NonFinalDecision {
        requirement: TargetCapabilityRequirementV1,
        outcome: TargetCapabilityDecisionOutcomeV1,
    },
    NonCanonicalDecisions,
}

impl fmt::Display for ProductionFinalGraphTargetContractErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FinalGraph(error) => write!(formatter, "final KIR V13 graph is invalid: {error}"),
            Self::TargetModel(error) => write!(formatter, "target model is invalid: {error}"),
            Self::Decision(error) => write!(formatter, "target decision is invalid: {error}"),
            Self::DecisionLimitExceeded { limit } => {
                write!(
                    formatter,
                    "target decision count exceeds hard limit {limit}"
                )
            }
            Self::EmptyDecisions => formatter.write_str("target closure has no decisions"),
            Self::WrongTargetModel => {
                formatter.write_str("target decision names a different target model")
            }
            Self::NonFinalDecision {
                requirement,
                outcome,
            } => write!(
                formatter,
                "target requirement {requirement} has nonfinal outcome {outcome:?}"
            ),
            Self::NonCanonicalDecisions => formatter
                .write_str("target decisions are duplicated, reordered, or not in canonical order"),
        }
    }
}

impl Error for ProductionFinalGraphTargetContractErrorV1 {}

/// Exact resource requirements checked against the W5 target closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionFinalGraphResourceReportV1 {
    final_graph: VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    closure_identity: [u8; 32],
    requirements: Box<[TargetResourceRequirementV1]>,
}

impl ProductionFinalGraphResourceReportV1 {
    pub const fn final_graph(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.final_graph
    }

    pub const fn final_epoch(&self) -> u64 {
        self.final_epoch
    }

    pub const fn closure_identity(&self) -> [u8; 32] {
        self.closure_identity
    }

    pub fn requirements(&self) -> &[TargetResourceRequirementV1] {
        &self.requirements
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// One exact defined function and its uninterrupted nine-check report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionFinalGraphFunctionReportV1 {
    function: FunctionId,
    checks: ProductionPlironPreloweringReportV2,
}

impl ProductionFinalGraphFunctionReportV1 {
    pub const fn function(&self) -> &FunctionId {
        &self.function
    }

    pub const fn checks(&self) -> &ProductionPlironPreloweringReportV2 {
        &self.checks
    }
}

/// Non-authoritative report retained by the move-only verified graph owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionFinalGraphVerificationReportV1 {
    final_graph: VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    bridge: KirBridgeRoundTripReportV1,
    execution_graph: KirBridgeExecutionCapabilityWitnessV1,
    capability: KernelCapabilityPreservationAnalysisV1,
    atomic_scope: ExecutionCapabilityAtomicScopeReportV1,
    resources: ProductionFinalGraphResourceReportV1,
    functions: Box<[ProductionFinalGraphFunctionReportV1]>,
}

impl ProductionFinalGraphVerificationReportV1 {
    pub const fn final_graph(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.final_graph
    }

    pub const fn final_epoch(&self) -> u64 {
        self.final_epoch
    }

    pub const fn bridge(&self) -> &KirBridgeRoundTripReportV1 {
        &self.bridge
    }

    pub const fn execution_graph(&self) -> &KirBridgeExecutionCapabilityWitnessV1 {
        &self.execution_graph
    }

    pub const fn capability(&self) -> &KernelCapabilityPreservationAnalysisV1 {
        &self.capability
    }

    pub const fn atomic_scope(&self) -> &ExecutionCapabilityAtomicScopeReportV1 {
        &self.atomic_scope
    }

    pub const fn resources(&self) -> &ProductionFinalGraphResourceReportV1 {
        &self.resources
    }

    pub fn functions(&self) -> &[ProductionFinalGraphFunctionReportV1] {
        &self.functions
    }

    pub const fn schedule(&self) -> &'static [ProductionCapabilityAnalysisStageV1; 16] {
        &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Move-only custody of an imported final graph before W4 verification.
pub struct ProductionFinalGraphOwnerV1 {
    canonical: VerifiedCanonicalKernelIrV13,
    module: Module,
    final_epoch: u64,
    target: ProductionFinalGraphTargetContractV1,
    session: PlironSession,
    graph: KirPlironGraphV1,
    bridge: KirBridgeRoundTripReportV1,
}

impl ProductionFinalGraphOwnerV1 {
    pub fn try_new(
        canonical: VerifiedCanonicalKernelIrV13,
        final_epoch: u64,
        target: ProductionFinalGraphTargetContractV1,
    ) -> Result<Self, ProductionFinalGraphVerificationErrorV1> {
        canonical
            .revalidate()
            .map_err(ProductionFinalGraphVerificationErrorV1::Canonical)?;
        if target.final_graph() != canonical.identity() || target.final_epoch() != final_epoch {
            return Err(ProductionFinalGraphVerificationErrorV1::TargetSubjectMismatch);
        }
        let module = decode_module_v13(canonical.canonical_bytes())
            .map_err(|_| ProductionFinalGraphVerificationErrorV1::CanonicalDecode)?;
        let mut session = PlironSession::new(
            ShellLimits::default(),
            [
                dialect_gpu::dialect_registration()
                    .map_err(|_| ProductionFinalGraphVerificationErrorV1::DialectRegistration)?,
                dialect_kernel::dialect_registration()
                    .map_err(|_| ProductionFinalGraphVerificationErrorV1::DialectRegistration)?,
            ],
        )
        .map_err(ProductionFinalGraphVerificationErrorV1::Session)?;
        let graph = session
            .import_canonical_kir_v13_o0(&canonical)
            .map_err(ProductionFinalGraphVerificationErrorV1::Bridge)?;
        let (replayed, bridge) = session
            .extract_canonical_kir_v13_o0(&graph)
            .map_err(ProductionFinalGraphVerificationErrorV1::Bridge)?;
        if replayed.identity() != canonical.identity() || !bridge.is_exact() {
            return Err(ProductionFinalGraphVerificationErrorV1::GraphSubjectMismatch);
        }
        Ok(Self {
            canonical,
            module,
            final_epoch,
            target,
            session,
            graph,
            bridge,
        })
    }

    pub fn verify(
        self,
    ) -> Result<ProductionVerifiedFinalGraphV1, ProductionFinalGraphVerificationErrorV1> {
        self.verify_with_schedule(&PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1)
    }

    fn verify_with_schedule(
        mut self,
        schedule: &[ProductionCapabilityAnalysisStageV1],
    ) -> Result<ProductionVerifiedFinalGraphV1, ProductionFinalGraphVerificationErrorV1> {
        require_exact_schedule(schedule)?;
        self.require_exact_live_graph()?;
        let capability = analyze_kernel_capability_preservation_v1(&self.module, self.final_epoch)
            .map_err(ProductionFinalGraphVerificationErrorV1::Canonical)?;
        if capability.canonical_identity() != self.canonical.identity()
            || capability.graph_epoch() != self.final_epoch
        {
            return Err(ProductionFinalGraphVerificationErrorV1::GraphSubjectMismatch);
        }
        let execution_graph = kir_v13_execution_capability_witness_v1(&self.canonical)
            .map_err(ProductionFinalGraphVerificationErrorV1::Bridge)?;
        if execution_graph.operations().len() != capability.execution_operations().len()
            || execution_graph.final_graph_epoch() != self.bridge.input().digest()
        {
            return Err(ProductionFinalGraphVerificationErrorV1::GraphSubjectMismatch);
        }
        let atomic_scope =
            analyze_execution_capability_atomic_scope_v1(&self.module).map_err(|error| {
                ProductionFinalGraphVerificationErrorV1::AtomicScope(Box::new(error))
            })?;
        let resources = require_exact_resource_closure(
            &self.module,
            self.canonical.identity(),
            self.final_epoch,
            &self.target,
        )?;
        let function_ids = self
            .module
            .functions
            .iter()
            .filter(|function| function.body.is_some())
            .map(|function| function.id.clone())
            .collect::<Vec<_>>();
        if function_ids.is_empty() {
            return Err(ProductionFinalGraphVerificationErrorV1::NoDefinedFunctions);
        }
        let reports = self
            .session
            .with_canonical_kir_v13_functions(&self.graph, |context, functions| {
                if functions.len() != function_ids.len() {
                    return Err(ProductionFinalGraphVerificationErrorV1::GraphSubjectMismatch);
                }
                functions
                    .iter()
                    .zip(function_ids)
                    .map(|(function, function_id)| {
                        let checks =
                            require_production_pliron_checks_before_lowering_v2(context, function)
                                .map_err(|source| {
                                    ProductionFinalGraphVerificationErrorV1::FunctionCheck {
                                        function: function_id.clone(),
                                        source: Box::new(source),
                                    }
                                })?;
                        require_exact_function_report(&function_id, &checks)?;
                        Ok(ProductionFinalGraphFunctionReportV1 {
                            function: function_id,
                            checks,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(ProductionFinalGraphVerificationErrorV1::Bridge)??;
        let verification_mutation_epoch = self
            .session
            .context
            .ir_mutation_attempt_epoch()
            .map_err(|_| ProductionFinalGraphVerificationErrorV1::MutationEpochUnavailable)?
            .value();
        let report = ProductionFinalGraphVerificationReportV1 {
            final_graph: *self.canonical.identity(),
            final_epoch: self.final_epoch,
            bridge: self.bridge.clone(),
            execution_graph,
            capability,
            atomic_scope,
            resources,
            functions: reports.into_boxed_slice(),
        };
        Ok(ProductionVerifiedFinalGraphV1 {
            owner: self,
            verification_mutation_epoch,
            report,
        })
    }

    fn require_exact_live_graph(&mut self) -> Result<(), ProductionFinalGraphVerificationErrorV1> {
        let (canonical, bridge) = self
            .session
            .extract_canonical_kir_v13_o0(&self.graph)
            .map_err(ProductionFinalGraphVerificationErrorV1::Bridge)?;
        if canonical.identity() != self.canonical.identity()
            || canonical.canonical_bytes() != self.canonical.canonical_bytes()
            || !bridge.is_exact()
            || bridge.input() != self.bridge.input()
            || bridge.output() != self.bridge.output()
            || decode_module_v13(canonical.canonical_bytes()).ok().as_ref() != Some(&self.module)
        {
            return Err(ProductionFinalGraphVerificationErrorV1::GraphSubjectMismatch);
        }
        Ok(())
    }
}

/// Move-only custody released only after exact final-graph verification.
pub struct ProductionVerifiedFinalGraphV1 {
    owner: ProductionFinalGraphOwnerV1,
    verification_mutation_epoch: u64,
    report: ProductionFinalGraphVerificationReportV1,
}

impl ProductionVerifiedFinalGraphV1 {
    pub const fn report(&self) -> &ProductionFinalGraphVerificationReportV1 {
        &self.report
    }

    pub const fn canonical(&self) -> &VerifiedCanonicalKernelIrV13 {
        &self.owner.canonical
    }

    pub const fn module(&self) -> &Module {
        &self.owner.module
    }

    pub const fn final_epoch(&self) -> u64 {
        self.owner.final_epoch
    }

    /// Replays the exact V13 bridge while retaining the context and function
    /// owners. No Pliron handle escapes this custody boundary.
    pub fn revalidate_live(&mut self) -> Result<(), ProductionFinalGraphVerificationErrorV1> {
        let before = self
            .owner
            .session
            .context
            .ir_mutation_attempt_epoch()
            .map_err(|_| ProductionFinalGraphVerificationErrorV1::MutationEpochUnavailable)?
            .value();
        if before != self.verification_mutation_epoch {
            return Err(
                ProductionFinalGraphVerificationErrorV1::MutationAfterVerification {
                    expected_epoch: self.verification_mutation_epoch,
                    observed_epoch: before,
                },
            );
        }
        let (canonical, bridge) = self
            .owner
            .session
            .extract_canonical_kir_v13_o0(&self.owner.graph)
            .map_err(ProductionFinalGraphVerificationErrorV1::Bridge)?;
        let after = self
            .owner
            .session
            .context
            .ir_mutation_attempt_epoch()
            .map_err(|_| ProductionFinalGraphVerificationErrorV1::MutationEpochUnavailable)?
            .value();
        if after != self.verification_mutation_epoch {
            return Err(
                ProductionFinalGraphVerificationErrorV1::MutationAfterVerification {
                    expected_epoch: self.verification_mutation_epoch,
                    observed_epoch: after,
                },
            );
        }
        if canonical.identity() != self.owner.canonical.identity()
            || !bridge.is_exact()
            || bridge.input() != self.owner.bridge.input()
            || bridge.output() != self.owner.bridge.output()
            || decode_module_v13(canonical.canonical_bytes()).ok().as_ref()
                != Some(&self.owner.module)
        {
            return Err(ProductionFinalGraphVerificationErrorV1::GraphSubjectMismatch);
        }
        let execution_graph = kir_v13_execution_capability_witness_v1(&canonical)
            .map_err(ProductionFinalGraphVerificationErrorV1::Bridge)?;
        if &execution_graph != self.report.execution_graph() {
            return Err(ProductionFinalGraphVerificationErrorV1::GraphSubjectMismatch);
        }
        let atomic_scope = analyze_execution_capability_atomic_scope_v1(&self.owner.module)
            .map_err(|error| {
                ProductionFinalGraphVerificationErrorV1::AtomicScope(Box::new(error))
            })?;
        if &atomic_scope != self.report.atomic_scope() {
            return Err(ProductionFinalGraphVerificationErrorV1::GraphSubjectMismatch);
        }
        Ok(())
    }

    /// Executes and immediately revalidates the exact W4 witness while the
    /// owner-held Context and FuncOps are alive.
    #[allow(clippy::result_large_err)]
    pub fn execute_w4_capability_witness(
        &mut self,
        subject: ProductionW4FinalGraphSubjectV1,
        target: ProductionW4TargetResourceInputV1,
    ) -> Result<ProductionW4FinalGraphExecutionV1, ProductionFinalGraphVerificationErrorV1> {
        self.revalidate_live()?;
        let function_ids = self
            .owner
            .module
            .functions
            .iter()
            .filter(|function| function.body.is_some())
            .map(|function| function.id.clone())
            .collect::<Vec<_>>();
        let canonical = &self.owner.canonical;
        let module = &self.owner.module;
        let execution = self
            .owner
            .session
            .with_canonical_kir_v13_functions(&self.owner.graph, |context, functions| {
                if functions.len() != function_ids.len() {
                    return Err(ProductionFinalGraphVerificationErrorV1::GraphSubjectMismatch);
                }
                let live = functions
                    .iter()
                    .zip(&function_ids)
                    .map(|(function, id)| ProductionW4LiveFunctionV1::new(id, function))
                    .collect::<Vec<_>>();
                let execution = execute_production_w4_final_graph_capability_witness_v1(
                    canonical,
                    module,
                    subject.clone(),
                    target.clone(),
                    &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1,
                    context,
                    &live,
                )
                .map_err(|error| {
                    ProductionFinalGraphVerificationErrorV1::W4Execution(Box::new(error))
                })?;
                if let ProductionW4FinalGraphExecutionV1::Complete(witness) = &execution {
                    witness
                        .require_exact_w6_handoff_v1(
                            canonical,
                            module,
                            &subject,
                            &target,
                            &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1,
                            context,
                            &live,
                        )
                        .map_err(|error| {
                            ProductionFinalGraphVerificationErrorV1::W4Handoff(Box::new(error))
                        })?;
                }
                Ok(execution)
            })
            .map_err(ProductionFinalGraphVerificationErrorV1::Bridge)??;
        self.revalidate_live()?;
        Ok(execution)
    }

    pub fn into_parts(
        mut self,
    ) -> Result<
        (
            VerifiedCanonicalKernelIrV13,
            Module,
            ProductionFinalGraphVerificationReportV1,
        ),
        ProductionFinalGraphVerificationErrorV1,
    > {
        let before = self
            .owner
            .session
            .context
            .ir_mutation_attempt_epoch()
            .map_err(|_| ProductionFinalGraphVerificationErrorV1::MutationEpochUnavailable)?
            .value();
        if before != self.verification_mutation_epoch {
            return Err(
                ProductionFinalGraphVerificationErrorV1::MutationAfterVerification {
                    expected_epoch: self.verification_mutation_epoch,
                    observed_epoch: before,
                },
            );
        }
        let (canonical, bridge) = self
            .owner
            .session
            .extract_canonical_kir_v13_o0(&self.owner.graph)
            .map_err(ProductionFinalGraphVerificationErrorV1::Bridge)?;
        let after = self
            .owner
            .session
            .context
            .ir_mutation_attempt_epoch()
            .map_err(|_| ProductionFinalGraphVerificationErrorV1::MutationEpochUnavailable)?
            .value();
        if after != self.verification_mutation_epoch {
            return Err(
                ProductionFinalGraphVerificationErrorV1::MutationAfterVerification {
                    expected_epoch: self.verification_mutation_epoch,
                    observed_epoch: after,
                },
            );
        }
        if canonical.identity() != self.owner.canonical.identity()
            || !bridge.is_exact()
            || bridge.input() != self.owner.bridge.input()
            || bridge.output() != self.owner.bridge.output()
        {
            return Err(ProductionFinalGraphVerificationErrorV1::GraphSubjectMismatch);
        }
        let execution_graph = kir_v13_execution_capability_witness_v1(&canonical)
            .map_err(ProductionFinalGraphVerificationErrorV1::Bridge)?;
        if &execution_graph != self.report.execution_graph() {
            return Err(ProductionFinalGraphVerificationErrorV1::GraphSubjectMismatch);
        }
        let module = decode_module_v13(canonical.canonical_bytes())
            .map_err(|_| ProductionFinalGraphVerificationErrorV1::CanonicalDecode)?;
        if module != self.owner.module {
            return Err(ProductionFinalGraphVerificationErrorV1::GraphSubjectMismatch);
        }
        let atomic_scope =
            analyze_execution_capability_atomic_scope_v1(&module).map_err(|error| {
                ProductionFinalGraphVerificationErrorV1::AtomicScope(Box::new(error))
            })?;
        if &atomic_scope != self.report.atomic_scope() {
            return Err(ProductionFinalGraphVerificationErrorV1::GraphSubjectMismatch);
        }
        Ok((canonical, module, self.report))
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    #[cfg(test)]
    fn record_hostile_mutation_after_verification(&mut self) {
        let pointer = *self
            .owner
            .session
            .operations
            .get(&self.owner.graph.root().identity)
            .expect("test graph remains registered");
        drop(pointer.deref_mut(&self.owner.session.context));
    }
}

#[derive(Debug)]
pub enum ProductionFinalGraphVerificationErrorV1 {
    Canonical(VerifiedCanonicalKernelIrErrorV13),
    CanonicalDecode,
    DialectRegistration,
    Session(crate::ContextBuildError),
    Bridge(KirBridgeErrorV1),
    TargetSubjectMismatch,
    ScheduleLength {
        expected: usize,
        observed: usize,
    },
    ScheduleMismatch {
        position: usize,
    },
    OptionalScheduleStage {
        position: usize,
    },
    ResourceClosureMismatch,
    ResourceArithmeticOverflow,
    NoDefinedFunctions,
    AtomicScope(Box<ExecutionCapabilityAtomicScopeErrorV1>),
    FunctionCheck {
        function: FunctionId,
        source: Box<ProductionPlironPreloweringErrorV2>,
    },
    IncompleteFunctionReport {
        function: FunctionId,
    },
    W4Execution(Box<ProductionW4ExecutionErrorV1>),
    W4Handoff(Box<ProductionW4HandoffErrorV1>),
    GraphSubjectMismatch,
    MutationEpochUnavailable,
    MutationAfterVerification {
        expected_epoch: u64,
        observed_epoch: u64,
    },
}

impl ProductionFinalGraphVerificationErrorV1 {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Canonical(_) | Self::CanonicalDecode => "FE2O3-W4-FINAL-001",
            Self::DialectRegistration | Self::Session(_) | Self::Bridge(_) => "FE2O3-W4-FINAL-002",
            Self::TargetSubjectMismatch
            | Self::ResourceClosureMismatch
            | Self::ResourceArithmeticOverflow => "FE2O3-W4-FINAL-003",
            Self::ScheduleLength { .. }
            | Self::ScheduleMismatch { .. }
            | Self::OptionalScheduleStage { .. } => "FE2O3-W4-FINAL-004",
            Self::NoDefinedFunctions
            | Self::AtomicScope(_)
            | Self::FunctionCheck { .. }
            | Self::IncompleteFunctionReport { .. }
            | Self::W4Execution(_)
            | Self::W4Handoff(_) => "FE2O3-W4-FINAL-005",
            Self::GraphSubjectMismatch => "FE2O3-W4-FINAL-006",
            Self::MutationEpochUnavailable | Self::MutationAfterVerification { .. } => {
                "FE2O3-W4-FINAL-007"
            }
        }
    }
}

impl fmt::Display for ProductionFinalGraphVerificationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "error[{}]: ", self.code())?;
        match self {
            Self::Canonical(error) => write!(formatter, "final KIR V13 graph is invalid: {error}"),
            Self::CanonicalDecode => {
                formatter.write_str("final KIR V13 graph could not be decoded")
            }
            Self::DialectRegistration => {
                formatter.write_str("closed PLIRON dialect registration was rejected")
            }
            Self::Session(_) => formatter.write_str("closed PLIRON session construction failed"),
            Self::Bridge(_) => formatter.write_str("typed final-graph PLIRON bridge failed"),
            Self::TargetSubjectMismatch => formatter
                .write_str("W5 target contract does not name the exact final graph and epoch"),
            Self::ScheduleLength { expected, observed } => write!(
                formatter,
                "W4 schedule has {observed} stages; exactly {expected} are required"
            ),
            Self::ScheduleMismatch { position } => {
                write!(formatter, "W4 schedule differs at position {position}")
            }
            Self::OptionalScheduleStage { position } => write!(
                formatter,
                "W4 schedule position {position} is optional instead of mandatory"
            ),
            Self::ResourceClosureMismatch => formatter.write_str(
                "W5 target closure resource decisions do not exactly cover final KIR requirements",
            ),
            Self::ResourceArithmeticOverflow => {
                formatter.write_str("final KIR resource accounting overflowed")
            }
            Self::NoDefinedFunctions => {
                formatter.write_str("final KIR graph has no defined function to verify")
            }
            Self::AtomicScope(source) => write!(formatter, "{source}"),
            Self::FunctionCheck { function, .. } => write!(
                formatter,
                "mandatory PLIRON checks rejected final function {function}"
            ),
            Self::IncompleteFunctionReport { function } => write!(
                formatter,
                "mandatory PLIRON report for final function {function} is incomplete or reordered"
            ),
            Self::W4Execution(source) => write!(formatter, "W4 execution failed: {source}"),
            Self::W4Handoff(source) => write!(formatter, "W4 live-owner handoff failed: {source}"),
            Self::GraphSubjectMismatch => {
                formatter.write_str("verification result does not name the exact final graph")
            }
            Self::MutationEpochUnavailable => {
                formatter.write_str("PLIRON mutation-attempt epoch is unavailable")
            }
            Self::MutationAfterVerification {
                expected_epoch,
                observed_epoch,
            } => write!(
                formatter,
                "final graph was touched after verification (epoch {expected_epoch} -> {observed_epoch})"
            ),
        }
    }
}

impl Error for ProductionFinalGraphVerificationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Canonical(error) => Some(error),
            Self::Bridge(error) => Some(error),
            Self::AtomicScope(error) => Some(&**error),
            Self::FunctionCheck { source, .. } => Some(&**source),
            Self::W4Execution(error) => Some(&**error),
            Self::W4Handoff(error) => Some(&**error),
            _ => None,
        }
    }
}

fn require_exact_schedule(
    schedule: &[ProductionCapabilityAnalysisStageV1],
) -> Result<(), ProductionFinalGraphVerificationErrorV1> {
    if schedule.len() != PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1.len() {
        return Err(ProductionFinalGraphVerificationErrorV1::ScheduleLength {
            expected: PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1.len(),
            observed: schedule.len(),
        });
    }
    for (position, (observed, expected)) in schedule
        .iter()
        .zip(PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1.iter())
        .enumerate()
    {
        if observed != expected {
            return Err(ProductionFinalGraphVerificationErrorV1::ScheduleMismatch { position });
        }
        if !observed.executor().is_mandatory_final_graph_gate() {
            return Err(
                ProductionFinalGraphVerificationErrorV1::OptionalScheduleStage { position },
            );
        }
    }
    let passes = schedule
        .iter()
        .filter_map(|stage| match stage.executor() {
            ProductionCapabilityAnalysisExecutorV1::MandatoryPlironPass(pass) => Some(pass),
            _ => None,
        })
        .collect::<Vec<_>>();
    if passes != PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2 {
        return Err(ProductionFinalGraphVerificationErrorV1::ScheduleMismatch { position: 0 });
    }
    Ok(())
}

fn require_exact_function_report(
    function: &FunctionId,
    report: &ProductionPlironPreloweringReportV2,
) -> Result<(), ProductionFinalGraphVerificationErrorV1> {
    let exact_passes = report.pass_order() == &PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
        && report.preservation().certificates().len()
            == PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2.len()
        && report
            .preservation()
            .certificates()
            .iter()
            .zip(PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2)
            .all(|(certificate, pass)| certificate.pass() == pass)
        && report.preservation().is_exact_identity()
        && report.report_validation().stages().len()
            == PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2.len();
    if report.status() != KernelCheckStatusV1::Clean || !exact_passes {
        return Err(
            ProductionFinalGraphVerificationErrorV1::IncompleteFunctionReport {
                function: function.clone(),
            },
        );
    }
    Ok(())
}

fn require_exact_resource_closure(
    module: &Module,
    final_graph: &VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    contract: &ProductionFinalGraphTargetContractV1,
) -> Result<ProductionFinalGraphResourceReportV1, ProductionFinalGraphVerificationErrorV1> {
    if contract.final_graph() != final_graph || contract.final_epoch() != final_epoch {
        return Err(ProductionFinalGraphVerificationErrorV1::TargetSubjectMismatch);
    }
    let expected = expected_resources(module)?;
    let observed = contract
        .decisions()
        .iter()
        .filter_map(|decision| match decision.requirement() {
            TargetCapabilityRequirementV1::Resource(resource) => Some(resource),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    if expected != observed {
        return Err(ProductionFinalGraphVerificationErrorV1::ResourceClosureMismatch);
    }
    Ok(ProductionFinalGraphResourceReportV1 {
        final_graph: *final_graph,
        final_epoch,
        closure_identity: contract.closure_identity(),
        requirements: expected.into_iter().collect::<Vec<_>>().into_boxed_slice(),
    })
}

fn expected_resources(
    module: &Module,
) -> Result<BTreeSet<TargetResourceRequirementV1>, ProductionFinalGraphVerificationErrorV1> {
    let workgroups = module
        .kernels
        .iter()
        .map(|kernel| {
            let size = kernel
                .workgroup_size
                .ok_or(ProductionFinalGraphVerificationErrorV1::ResourceClosureMismatch)?;
            let invocations = size
                .x
                .checked_mul(size.y)
                .and_then(|value| value.checked_mul(size.z))
                .ok_or(ProductionFinalGraphVerificationErrorV1::ResourceArithmeticOverflow)?;
            Ok((size, invocations))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let maximum_workgroup_invocations = workgroups
        .iter()
        .map(|(_, invocations)| *invocations)
        .max()
        .ok_or(ProductionFinalGraphVerificationErrorV1::ResourceClosureMismatch)?;
    let mut expected = BTreeSet::from([TargetResourceRequirementV1::WorkgroupInvocationsAtMost(
        maximum_workgroup_invocations,
    )]);
    expected.extend(workgroups.into_iter().map(|(size, _)| {
        TargetResourceRequirementV1::WorkgroupDimensions {
            x: size.x,
            y: size.y,
            z: size.z,
        }
    }));
    let mut capabilities = module.effective_capabilities();
    capabilities.extend(
        module
            .functions
            .iter()
            .flat_map(|function| function.effective_capabilities()),
    );
    capabilities.extend(
        module
            .kernels
            .iter()
            .flat_map(|kernel| kernel.required_capabilities.iter().cloned()),
    );
    for capability in capabilities {
        if let TargetCapability::Execution(ExecutionCapabilityRequirementV1::Resource(resource)) =
            capability
        {
            expected.insert(match resource {
                ResourceCapabilityRequirementV1::WorkgroupInvocationsAtMost(value) => {
                    TargetResourceRequirementV1::WorkgroupInvocationsAtMost(value)
                }
                ResourceCapabilityRequirementV1::StaticWorkgroupMemoryBytesAtMost(value) => {
                    TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(value)
                }
                ResourceCapabilityRequirementV1::DynamicWorkgroupMemoryBytesAtMost(value) => {
                    TargetResourceRequirementV1::DynamicSharedMemoryBytesAtMost(value)
                }
                ResourceCapabilityRequirementV1::PrivateMemoryBytesPerInvocationAtMost(value) => {
                    TargetResourceRequirementV1::PrivateMemoryBytesPerInvocationAtMost(value)
                }
            });
        }
    }
    Ok(expected)
}

#[cfg(test)]
mod tests {
    use dialect_gpu::{CanonicalKirOperationAttr, IntrinsicOp};
    use dialect_kernel::CanonicalIdentityAttr;
    use fe2o3_kernel_ir::{
        BasicBlock, Function, IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent, Operation,
        OperationKind, Signature, Terminator, Type, ValueDef, ValueId, WorkgroupSize,
    };
    use fe2o3_target_spec::{
        TargetArchitectureFamilyV1, TargetArtifactFormatV1, TargetCapabilityDecisionOutcomeV1,
        TargetCapabilityQueryV1, TargetExecutionModelV1, TargetProfileSpecV1,
        TargetResourceRequirementV1, TargetVendorV1, query_target_capability_v1,
    };
    use pliron::{
        context::{Context, Ptr},
        linked_list::ContainsLinkedList,
        op::Op,
        operation::Operation as PlironOperation,
    };

    use super::*;

    #[derive(Clone, Copy)]
    struct SyntheticTarget(TargetCapabilityModelIdentityV1);

    impl TargetCapabilityQueryV1 for SyntheticTarget {
        fn model_identity(&self) -> TargetCapabilityModelIdentityV1 {
            self.0
        }

        fn query_outcome(
            &self,
            _requirement: TargetCapabilityRequirementV1,
        ) -> TargetCapabilityDecisionOutcomeV1 {
            TargetCapabilityDecisionOutcomeV1::Supported
        }
    }

    fn synthetic_target_model() -> TargetCapabilityModelIdentityV1 {
        let profile = TargetProfileSpecV1::from_static_parts(
            TargetVendorV1::Other,
            TargetArchitectureFamilyV1::Other,
            "synthetic-w4",
            None,
            None,
            TargetArtifactFormatV1::NativeObject,
            TargetExecutionModelV1::GpuGrid,
            None,
            &[],
        );
        TargetCapabilityModelIdentityV1::new(profile, "w4-test-v1").unwrap()
    }

    fn module() -> Module {
        let mut block = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("final-graph-owner");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        let mut kernel = Kernel::new(
            "kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
        module
    }

    fn intrinsic_module() -> Module {
        let mut block = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(0), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(1), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::launch_extent_1d()),
        ));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("final-graph-owner-intrinsics");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        let mut kernel = Kernel::new(
            "kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
        module
    }

    fn input_for(
        module: Module,
    ) -> (
        VerifiedCanonicalKernelIrV13,
        ProductionFinalGraphTargetContractV1,
    ) {
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
        let model = synthetic_target_model();
        let target = SyntheticTarget(model);
        let decisions = [
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::WorkgroupInvocationsAtMost(64),
            ),
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::WorkgroupDimensions { x: 64, y: 1, z: 1 },
            ),
        ]
        .into_iter()
        .map(|requirement| query_target_capability_v1(&target, requirement).unwrap());
        let contract = ProductionFinalGraphTargetContractV1::try_new(
            &canonical,
            8,
            *canonical.identity(),
            7,
            [9; 32],
            model,
            decisions,
        )
        .unwrap();
        (canonical, contract)
    }

    fn operation_tree(root: Ptr<PlironOperation>, context: &Context) -> Vec<Ptr<PlironOperation>> {
        let mut pending = vec![root];
        let mut operations = Vec::new();
        while let Some(operation) = pending.pop() {
            operations.push(operation);
            let mut children = Vec::new();
            for region in operation.deref(context).regions() {
                for block in region.deref(context).iter(context) {
                    children.extend(block.deref(context).iter(context));
                }
            }
            pending.extend(children.into_iter().rev());
        }
        operations
    }

    fn intrinsics(root: Ptr<PlironOperation>, context: &Context) -> Vec<IntrinsicOp> {
        operation_tree(root, context)
            .into_iter()
            .filter_map(|operation| PlironOperation::get_op::<IntrinsicOp>(operation, context))
            .collect()
    }

    fn require_hostile_intrinsic_graph_rejected(
        mutation: impl FnOnce(&mut Context, Ptr<PlironOperation>),
    ) {
        let (canonical, contract) = input_for(intrinsic_module());
        let mut owner = ProductionFinalGraphOwnerV1::try_new(canonical, 8, contract).unwrap();
        owner
            .session
            .with_canonical_kir_graph_mut_for_test(&owner.graph, mutation)
            .unwrap();
        assert!(owner.verify().is_err());
    }

    fn input() -> (
        VerifiedCanonicalKernelIrV13,
        ProductionFinalGraphTargetContractV1,
    ) {
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module()).unwrap();
        let model = synthetic_target_model();
        let target = SyntheticTarget(model);
        let invocation_decision = query_target_capability_v1(
            &target,
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::WorkgroupInvocationsAtMost(64),
            ),
        )
        .unwrap();
        let dimensions_decision = query_target_capability_v1(
            &target,
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::WorkgroupDimensions { x: 64, y: 1, z: 1 },
            ),
        )
        .unwrap();
        let contract = ProductionFinalGraphTargetContractV1::try_new(
            &canonical,
            8,
            *canonical.identity(),
            7,
            [9; 32],
            model,
            [invocation_decision, dimensions_decision],
        )
        .unwrap();
        (canonical, contract)
    }

    #[test]
    fn exact_final_graph_executes_all_sixteen_concerns_and_nine_checks() {
        let (canonical, contract) = input();
        let verified = ProductionFinalGraphOwnerV1::try_new(canonical, 8, contract)
            .unwrap()
            .verify()
            .unwrap();
        assert_eq!(verified.report().schedule().len(), 16);
        assert_eq!(verified.report().functions().len(), 1);
        assert_eq!(
            verified.report().functions()[0]
                .checks()
                .preservation()
                .certificates()
                .len(),
            9
        );
        assert!(!verified.grants_compiler_refinement_authority());
        assert!(!verified.grants_artifact_or_launch_authority());
        let (_, output, report) = verified.into_parts().unwrap();
        assert_eq!(output, module());
        assert_eq!(report.final_epoch(), 8);
    }

    #[test]
    fn omitted_schedule_stage_is_rejected() {
        let (canonical, contract) = input();
        let owner = ProductionFinalGraphOwnerV1::try_new(canonical, 8, contract).unwrap();
        let error = match owner.verify_with_schedule(
            &PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1
                [..PRODUCTION_CAPABILITY_ANALYSIS_SCHEDULE_V1.len() - 1],
        ) {
            Ok(_) => panic!("omitted stage was accepted"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            ProductionFinalGraphVerificationErrorV1::ScheduleLength {
                expected: 16,
                observed: 15
            }
        ));
    }

    #[test]
    fn mutation_after_verification_is_rejected_before_release() {
        let (canonical, contract) = input();
        let mut verified = ProductionFinalGraphOwnerV1::try_new(canonical, 8, contract)
            .unwrap()
            .verify()
            .unwrap();
        verified.record_hostile_mutation_after_verification();
        assert!(matches!(
            verified.into_parts(),
            Err(ProductionFinalGraphVerificationErrorV1::MutationAfterVerification { .. })
        ));
    }

    #[test]
    fn hostile_intrinsic_mutation_omission_reorder_and_stale_epoch_are_rejected() {
        require_hostile_intrinsic_graph_rejected(|context, root| {
            let intrinsic = intrinsics(root, context).remove(0);
            let mut contract = intrinsic.contract(context).unwrap();
            contract.kind = OperationKind::Intrinsic(IntrinsicOperation::launch_extent_1d());
            intrinsic.set_attr_gpu_kir_intrinsic_contract(
                context,
                CanonicalKirOperationAttr::new(&contract).unwrap(),
            );
        });
        require_hostile_intrinsic_graph_rejected(|context, root| {
            intrinsics(root, context)[0].get_operation().unlink(context);
        });
        require_hostile_intrinsic_graph_rejected(|context, root| {
            let operations = intrinsics(root, context);
            let first = operations[0].get_operation();
            first.unlink(context);
            first.insert_after(context, operations[1].get_operation());
        });
        require_hostile_intrinsic_graph_rejected(|context, root| {
            intrinsics(root, context)[0].set_attr_gpu_kir_intrinsic_graph_epoch(
                context,
                CanonicalIdentityAttr::from_bytes([0xfd; 32]),
            );
        });
    }

    #[test]
    fn wrong_epoch_and_omitted_resource_are_rejected() {
        let (canonical, contract) = input();
        assert!(matches!(
            ProductionFinalGraphOwnerV1::try_new(canonical, 9, contract),
            Err(ProductionFinalGraphVerificationErrorV1::TargetSubjectMismatch)
        ));

        let canonical = VerifiedCanonicalKernelIrV13::from_module(module()).unwrap();
        let model = synthetic_target_model();
        let target = SyntheticTarget(model);
        let irrelevant =
            query_target_capability_v1(&target, TargetCapabilityRequirementV1::SubgroupSize(64))
                .unwrap();
        let contract = ProductionFinalGraphTargetContractV1::try_new(
            &canonical,
            8,
            *canonical.identity(),
            7,
            [9; 32],
            model,
            [irrelevant],
        )
        .unwrap();
        assert!(matches!(
            ProductionFinalGraphOwnerV1::try_new(canonical, 8, contract)
                .unwrap()
                .verify(),
            Err(ProductionFinalGraphVerificationErrorV1::ResourceClosureMismatch)
        ));
    }
}
