//! Ordered compiler effects, independent of physical memory effects.
//! Verification keys are locators, not proof authority.

use std::collections::BTreeSet;

use crate::{AddressSpace, MemoryEffect, MemoryEffectSummary, Operation, OperationKind, ValueId};

/// An inert zero-based key into the importing owner's contract catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VerificationContractKeyV12(u32);

impl VerificationContractKeyV12 {
    pub const fn new(index: u32) -> Self {
        Self(index)
    }
    pub const fn index(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkgroupPipelineEventKindV12 {
    Stage,
    Commit,
    Wait,
    Consume,
    Discard,
    Release,
}

/// Ordered compiler semantics with no physical memory or synchronization effect.
/// Structural verification does not authenticate the referenced catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VerificationContractOperationV12 {
    WorkgroupPipelineEvent {
        contract: VerificationContractKeyV12,
        kind: WorkgroupPipelineEventKindV12,
        storage: ValueId,
        epoch: ValueId,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompilerOrderingEffectV12 {
    OrderedVerificationContract,
    OrderedExecution,
    OrderedVerificationContractAndExecution,
}

/// Closed compiler-ordering summary, separate from physical memory effects.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerOrderingEffectSummaryV12 {
    ordered_verification_contract: bool,
    ordered_execution: bool,
}

impl CompilerOrderingEffectSummaryV12 {
    pub const fn empty() -> Self {
        Self {
            ordered_verification_contract: false,
            ordered_execution: false,
        }
    }
    pub const fn ordered_verification_contract() -> Self {
        Self {
            ordered_verification_contract: true,
            ordered_execution: false,
        }
    }
    pub const fn ordered_execution() -> Self {
        Self {
            ordered_verification_contract: false,
            ordered_execution: true,
        }
    }
    pub const fn is_empty(self) -> bool {
        !self.ordered_verification_contract && !self.ordered_execution
    }
    pub const fn has_ordered_verification_contract(self) -> bool {
        self.ordered_verification_contract
    }
    pub const fn has_ordered_execution(self) -> bool {
        self.ordered_execution
    }
    pub const fn union(self, other: Self) -> Self {
        Self {
            ordered_verification_contract: self.ordered_verification_contract
                || other.ordered_verification_contract,
            ordered_execution: self.ordered_execution || other.ordered_execution,
        }
    }
    /// Lossless summary, including coexistence of both ordered effect families.
    pub const fn effect(self) -> Option<CompilerOrderingEffectV12> {
        match (self.ordered_verification_contract, self.ordered_execution) {
            (false, false) => None,
            (true, false) => Some(CompilerOrderingEffectV12::OrderedVerificationContract),
            (false, true) => Some(CompilerOrderingEffectV12::OrderedExecution),
            (true, true) => {
                Some(CompilerOrderingEffectV12::OrderedVerificationContractAndExecution)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationEffectSummaryV12 {
    memory: MemoryEffectSummary,
    compiler_ordering: CompilerOrderingEffectSummaryV12,
}

impl OperationEffectSummaryV12 {
    pub fn new(
        memory: MemoryEffectSummary,
        compiler_ordering: CompilerOrderingEffectSummaryV12,
    ) -> Self {
        Self {
            memory,
            compiler_ordering,
        }
    }
    pub fn pure() -> Self {
        Self::new(
            MemoryEffectSummary::pure(),
            CompilerOrderingEffectSummaryV12::empty(),
        )
    }
    pub const fn memory(&self) -> &MemoryEffectSummary {
        &self.memory
    }
    pub const fn compiler_ordering(&self) -> CompilerOrderingEffectSummaryV12 {
        self.compiler_ordering
    }
    pub fn is_pure(&self) -> bool {
        self.memory.is_pure() && self.compiler_ordering.is_empty()
    }
    /// Physical effects only. Compiler-order consumers must query the other axis.
    pub fn effects(&self) -> &BTreeSet<MemoryEffect> {
        self.memory.effects()
    }
    pub fn reads(&self, space: AddressSpace) -> bool {
        self.memory.reads(space)
    }
    pub fn writes(&self, space: AddressSpace) -> bool {
        self.memory.writes(space)
    }
}

impl Operation {
    /// Local compiler effects. Calls require the transitive interprocedural analysis.
    pub fn compiler_ordering_effects_v12(&self) -> CompilerOrderingEffectSummaryV12 {
        match &self.kind {
            OperationKind::Execution(_) => CompilerOrderingEffectSummaryV12::ordered_execution(),
            OperationKind::Call { callee, arguments }
                if arguments.is_empty()
                    && matches!(
                        crate::AmdGpuDiagnosticOperation::intrinsic_descriptor_v1(callee),
                        Some(crate::AmdGpuDiagnosticIntrinsicDescriptorV1::Realtime64)
                    ) =>
            {
                CompilerOrderingEffectSummaryV12::ordered_execution()
            }
            OperationKind::VerificationContract(_) => {
                CompilerOrderingEffectSummaryV12::ordered_verification_contract()
            }
            _ => CompilerOrderingEffectSummaryV12::empty(),
        }
    }
    /// Local effects only, potentially incomplete for calls. Purity here is not
    /// permission to optimize: call sites require complete transitive memory and
    /// compiler-order summaries, alongside trap and convergence checks.
    pub fn combined_effect_summary_v12(&self) -> OperationEffectSummaryV12 {
        OperationEffectSummaryV12::new(self.effect_summary(), self.compiler_ordering_effects_v12())
    }
}
