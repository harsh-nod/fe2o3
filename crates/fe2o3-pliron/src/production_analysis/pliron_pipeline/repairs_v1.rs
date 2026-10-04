/// Whether a compiler repair can be applied without another semantic choice.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KernelCheckRepairApplicabilityV1 {
    MachineApplicable,
    HasPlaceholders,
    Manual,
}

/// Stable category for a production kernel-check repair.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KernelCheckRepairActionV1 {
    RepairStructure,
    RepairControlFlow,
    RepairTensorLayout,
    GuardMemoryAccess,
    SelectSupportedAtomic,
    PartitionOrSynchronizeAccess,
    CorrectHierarchyOwnership,
    MakeBarrierControlUniform,
    RepairPipelineProtocol,
    InitializeAndPublishWorkgroupMemory,
    MatchReferenceSemantics,
    SatisfyTargetContract,
    PreservePassSemantics,
}

impl KernelCheckRepairActionV1 {
    pub const fn code(self) -> &'static str {
        match self {
            Self::RepairStructure => "FE2O3-FIX-STRUCTURE",
            Self::RepairControlFlow => "FE2O3-FIX-CFG",
            Self::RepairTensorLayout => "FE2O3-FIX-LAYOUT",
            Self::GuardMemoryAccess => "FE2O3-FIX-BOUNDS",
            Self::SelectSupportedAtomic => "FE2O3-FIX-ATOMIC",
            Self::PartitionOrSynchronizeAccess => "FE2O3-FIX-RACE",
            Self::CorrectHierarchyOwnership => "FE2O3-FIX-OWNERSHIP",
            Self::MakeBarrierControlUniform => "FE2O3-FIX-BARRIER",
            Self::RepairPipelineProtocol => "FE2O3-FIX-PIPELINE",
            Self::InitializeAndPublishWorkgroupMemory => "FE2O3-FIX-WORKGROUP",
            Self::MatchReferenceSemantics => "FE2O3-FIX-SEMANTIC",
            Self::SatisfyTargetContract => "FE2O3-FIX-TARGET",
            Self::PreservePassSemantics => "FE2O3-FIX-PASS-PRESERVATION",
        }
    }
}

/// One bounded structured repair attached to a production compiler error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelCheckRepairV1 {
    pass: KernelCheckPassKindV1,
    action: KernelCheckRepairActionV1,
    applicability: KernelCheckRepairApplicabilityV1,
    message: String,
}

impl KernelCheckRepairV1 {
    fn new(
        pass: KernelCheckPassKindV1,
        action: KernelCheckRepairActionV1,
        applicability: KernelCheckRepairApplicabilityV1,
        message: impl Into<String>,
    ) -> Self {
        Self {
            pass,
            action,
            applicability,
            message: message.into(),
        }
    }

    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        self.pass
    }

    pub const fn action(&self) -> KernelCheckRepairActionV1 {
        self.action
    }

    pub const fn applicability(&self) -> KernelCheckRepairApplicabilityV1 {
        self.applicability
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for KernelCheckRepairV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "help[{}] ({:?}): {}",
            self.action().code(),
            self.applicability(),
            self.message(),
        )
    }
}

pub fn kernel_check_repair_for_pass_v1(pass: KernelCheckPassKindV1) -> KernelCheckRepairV1 {
    let (action, message) = match pass {
        KernelCheckPassKindV1::TensorLayout => (
            KernelCheckRepairActionV1::RepairTensorLayout,
            "keep each value in the fragment ABI required by its consumer; otherwise insert a checked conversion or reload whose source projection creates a new compiler-derived layout root",
        ),
        KernelCheckPassKindV1::MemoryBounds => (
            KernelCheckRepairActionV1::GuardMemoryAccess,
            "guard every path to the access with the failed index < extent relation, or use a checked access with a defined tail value",
        ),
        KernelCheckPassKindV1::AtomicLegality => (
            KernelCheckRepairActionV1::SelectSupportedAtomic,
            "use an ordering, scope, width, and address space supported by the target capability, or replace the atomic with an ownership-preserving non-atomic design",
        ),
        KernelCheckPassKindV1::RaceFreedom => (
            KernelCheckRepairActionV1::PartitionOrSynchronizeAccess,
            "map every conflicting write to an injective affine coordinate over all active invocation axes and retain finite guards needed to prove no integer wrap; otherwise use a legal atomic or establish the required synchronization edge",
        ),
        KernelCheckPassKindV1::HierarchicalOwnership => (
            KernelCheckRepairActionV1::CorrectHierarchyOwnership,
            "make the lane, subgroup, workgroup, and grid ownership sets disjoint and cover the declared output domain exactly",
        ),
        KernelCheckPassKindV1::BarrierConvergence => (
            KernelCheckRepairActionV1::MakeBarrierControlUniform,
            "move the barrier to control flow uniform at its execution scope, or restructure the branch so every required participant reaches the same barrier",
        ),
        KernelCheckPassKindV1::PipelineProtocol => (
            KernelCheckRepairActionV1::RepairPipelineProtocol,
            "use slot = epoch % buffer_count; stage then commit each future epoch, wait then consume or discard it, and release every slot before reuse; prime and drain the full prefetch window",
        ),
        KernelCheckPassKindV1::WorkgroupMemory => (
            KernelCheckRepairActionV1::InitializeAndPublishWorkgroupMemory,
            "initialize each workgroup-memory element before reading it and publish producer writes with the required barrier and memory ordering",
        ),
        KernelCheckPassKindV1::SemanticRefinement => (
            KernelCheckRepairActionV1::MatchReferenceSemantics,
            "make GPU output coordinates, guards, values, and numerical policy match the safe Rust reference under its Verus preconditions and invariants, then regenerate exact-boundary proof evidence",
        ),
        KernelCheckPassKindV1::Structural => (
            KernelCheckRepairActionV1::RepairStructure,
            "repair the malformed Kernel IR operation, type, attribute, or ownership relation before running the production PLIRON checks",
        ),
        KernelCheckPassKindV1::ControlFlow => (
            KernelCheckRepairActionV1::RepairControlFlow,
            "repair undefined successors, edge arguments, unreachable executable regions, or irreducible control flow before running dataflow checks",
        ),
    };
    KernelCheckRepairV1::new(
        pass,
        action,
        KernelCheckRepairApplicabilityV1::HasPlaceholders,
        message,
    )
}

fn launch_contract_repair_v1() -> KernelCheckRepairV1 {
    KernelCheckRepairV1::new(
        KernelCheckPassKindV1::Structural,
        KernelCheckRepairActionV1::SatisfyTargetContract,
        KernelCheckRepairApplicabilityV1::HasPlaceholders,
        "use a target-supported grid, workgroup, subgroup, and LDS footprint; bind each global allocation origin to a sufficiently large aligned host descriptor, and guard dynamic launch facts at runtime",
    )
}

pub fn pass_preservation_repair_for_error_v1(
    error: &PlironPassPreservationErrorV1,
) -> KernelCheckRepairV1 {
    let (pass, action, message) = match error {
        PlironPassPreservationErrorV1::StructuralIdentityChanged { pass, .. }
        | PlironPassPreservationErrorV1::StaleInputIdentity { pass, .. } => (
            *pass,
            KernelCheckRepairActionV1::PreservePassSemantics,
            "compiler maintainer: remove the persistent structural mutation from the named analysis pass; a transforming pass must instead re-enter correctness verification under a separately validated semantic-refinement contract",
        ),
        PlironPassPreservationErrorV1::MutationAttempted {
            pass: Some(pass), ..
        }
        | PlironPassPreservationErrorV1::MutationEpochUnavailable {
            pass: Some(pass), ..
        } => (
            *pass,
            KernelCheckRepairActionV1::PreservePassSemantics,
            "compiler maintainer: make the named analysis stage use immutable PLIRON queries only; move transformations before verification and restart the fixed pipeline from a fresh snapshot",
        ),
        PlironPassPreservationErrorV1::StaleMutationEpoch { pass, .. } => (
            *pass,
            KernelCheckRepairActionV1::PreservePassSemantics,
            "compiler maintainer: discard stale analysis capabilities and restart the complete fixed pipeline from the current verified PLIRON identity",
        ),
        PlironPassPreservationErrorV1::AnalysisPanicked { pass } => (
            *pass,
            KernelCheckRepairActionV1::PreservePassSemantics,
            "compiler maintainer: replace the panic in the named analysis stage with a typed fail-closed diagnostic and rerun the fixed pipeline",
        ),
        PlironPassPreservationErrorV1::IdentityUnavailable { source_code, .. }
            if *source_code == "FE2O3-PRESERVE-001" =>
        {
            (
                KernelCheckPassKindV1::Structural,
                KernelCheckRepairActionV1::RepairStructure,
                "lower every operation, attribute, and type into the closed production ranked PLIRON subset before running preservation checks",
            )
        }
        PlironPassPreservationErrorV1::IdentityUnavailable { source_code, .. }
            if *source_code == "FE2O3-PRESERVE-002" =>
        {
            (
                KernelCheckPassKindV1::Structural,
                KernelCheckRepairActionV1::RepairStructure,
                "split or simplify the function so structural identity construction remains within its audited resource bounds",
            )
        }
        PlironPassPreservationErrorV1::IdentityUnavailable { .. } => (
            KernelCheckPassKindV1::Structural,
            KernelCheckRepairActionV1::RepairStructure,
            "repair malformed PLIRON structure or its registered deterministic printer before constructing a structural identity",
        ),
        _ => (
            KernelCheckPassKindV1::Structural,
            KernelCheckRepairActionV1::PreservePassSemantics,
            "compiler maintainer: restore the fixed pass manifest, order, and sealed checkpoint state before accepting this pipeline",
        ),
    };
    KernelCheckRepairV1::new(
        pass,
        action,
        KernelCheckRepairApplicabilityV1::Manual,
        message,
    )
}

pub fn report_validation_repair_for_error_v1(
    _error: &ProductionAnalysisReportValidationErrorV1,
) -> KernelCheckRepairV1 {
    KernelCheckRepairV1::new(
        KernelCheckPassKindV1::Structural,
        KernelCheckRepairActionV1::PreservePassSemantics,
        KernelCheckRepairApplicabilityV1::Manual,
        "compiler maintainer: discard the report and rerun the fixed analysis sequence on the exact live PLIRON checkpoint with the sealed implementation and configuration",
    )
}

pub fn tensor_layout_repair_for_error_v1(
    error: &PlironTensorLayoutCheckErrorV1,
) -> KernelCheckRepairV1 {
    let message = error
        .report()
        .findings()
        .iter()
        .find_map(|finding| {
            let PlironTensorLayoutFindingV1::Dataflow(issue) = finding else {
                return None;
            };
            match issue.as_ref() {
                PlironTensorLayoutDataflowIssueV1::ConsumerMismatch {
                    producer,
                    consumer,
                    consumer_profile,
                    operand,
                    ..
                } if *operand == fe2o3_kernel_ir::TensorOperandRoleV1::Accumulator => Some(format!(
                "at block {} op {}, use an accumulator ABI compatible with producer profile {:?} instead of consumer profile {:?}, or insert an explicit checked conversion before the consumer",
                consumer.block, consumer.operation, producer.profile, consumer_profile,
            )),
                PlironTensorLayoutDataflowIssueV1::ConsumerMismatch {
                    producer,
                    consumer,
                    consumer_profile,
                    operand,
                    ..
                } => Some(format!(
                "before block {} op {}, convert or checked-reload producer profile {:?}'s accumulator into the {:?} fragment ABI required by consumer profile {:?}; source projection must retain the conversion as a new compiler-derived root",
                consumer.block, consumer.operation, producer.profile, operand, consumer_profile,
            )),
                PlironTensorLayoutDataflowIssueV1::MergeConflict { first, second, .. } => Some(format!(
                "convert the value from block {} op {} or block {} op {} so both CFG producers reach the join with one identical fragment layout",
                first.producer.block,
                first.producer.operation,
                second.producer.block,
                second.producer.operation,
            )),
            }
        })
        .unwrap_or_else(|| {
            kernel_check_repair_for_pass_v1(KernelCheckPassKindV1::TensorLayout).message
        });
    KernelCheckRepairV1::new(
        KernelCheckPassKindV1::TensorLayout,
        KernelCheckRepairActionV1::RepairTensorLayout,
        KernelCheckRepairApplicabilityV1::HasPlaceholders,
        message,
    )
}

fn write_repairs(
    formatter: &mut fmt::Formatter<'_>,
    repairs: &[KernelCheckRepairV1],
) -> fmt::Result {
    for repair in repairs {
        write!(formatter, "\n{repair}")?;
    }
    Ok(())
}
