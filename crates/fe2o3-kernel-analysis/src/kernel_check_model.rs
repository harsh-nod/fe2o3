//! Shared status vocabulary for the sole PLIRON kernel-check pipeline.

/// One mandatory analysis pass in the workload-neutral compiler pipeline.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KernelCheckPassKindV1 {
    Structural,
    ControlFlow,
    MemoryBounds,
    TensorLayout,
    AtomicLegality,
    RaceFreedom,
    HierarchicalOwnership,
    BarrierConvergence,
    WorkgroupMemory,
    SemanticRefinement,
}

impl KernelCheckPassKindV1 {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Structural => "kernel-structural-v1",
            Self::ControlFlow => "kernel-control-flow-v1",
            Self::MemoryBounds => "kernel-memory-bounds-v1",
            Self::TensorLayout => "kernel-tensor-layout-v1",
            Self::AtomicLegality => "kernel-atomic-legality-v1",
            Self::RaceFreedom => "kernel-race-freedom-v1",
            Self::HierarchicalOwnership => "kernel-hierarchical-ownership-v1",
            Self::BarrierConvergence => "kernel-barrier-convergence-v1",
            Self::WorkgroupMemory => "kernel-workgroup-memory-v1",
            Self::SemanticRefinement => "kernel-semantic-refinement-v1",
        }
    }
}

/// Conservative outcome of one analysis pass.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum KernelCheckStatusV1 {
    Clean,
    Incomplete,
    Rejected,
}

impl KernelCheckStatusV1 {
    /// Combines pass evidence conservatively, with rejection taking precedence.
    pub const fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Rejected, _) | (_, Self::Rejected) => Self::Rejected,
            (Self::Incomplete, _) | (_, Self::Incomplete) => Self::Incomplete,
            (Self::Clean, Self::Clean) => Self::Clean,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_join_is_the_conservative_lattice() {
        assert_eq!(
            KernelCheckStatusV1::Clean.join(KernelCheckStatusV1::Incomplete),
            KernelCheckStatusV1::Incomplete
        );
        assert_eq!(
            KernelCheckStatusV1::Incomplete.join(KernelCheckStatusV1::Rejected),
            KernelCheckStatusV1::Rejected
        );
    }

    #[test]
    fn retired_kernel_ir_pipeline_is_absent_from_the_public_module_graph() {
        let root = include_str!("lib.rs");
        assert!(!root.contains("kernel_check_pipeline"));
        assert!(!root.contains("run_general_kernel_checks"));
    }
}
