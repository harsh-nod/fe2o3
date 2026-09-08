//! Cross-workgroup scope checks over canonical KIR V13 execution atomics.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use fe2o3_kernel_ir::{
    ExecutionAtomicKindV1, ExecutionCapabilityOperationV1, ExecutionCapabilitySourceV1,
    ExecutionMemoryAddressSpaceV1, ExecutionMemoryScopeV1, FunctionId, KernelId, LaunchExtent,
    Module, OperationKind,
};

/// Stable diagnostic required when a typed atomic cannot order every possible conflict.
pub const EXECUTION_CAPABILITY_ATOMIC_SCOPE_DIAGNOSTIC_V1: &str = "FE2O3-CAP-ANALYSIS001";

/// Authority-free summary of the exact KIR V13 atomic-scope traversal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionCapabilityAtomicScopeReportV1 {
    checked_kernel_roots: usize,
    checked_atomic_operations: usize,
}

impl ExecutionCapabilityAtomicScopeReportV1 {
    pub const fn checked_kernel_roots(&self) -> usize {
        self.checked_kernel_roots
    }

    pub const fn checked_atomic_operations(&self) -> usize {
        self.checked_atomic_operations
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExecutionCapabilityAtomicScopeErrorV1 {
    MissingKernelEntry {
        kernel: KernelId,
        entry: FunctionId,
    },
    MissingWorkgroupSize {
        kernel: KernelId,
    },
    InsufficientScope {
        kernel: KernelId,
        function: FunctionId,
        block: u32,
        source: ExecutionCapabilitySourceV1,
        scope: ExecutionMemoryScopeV1,
        kind: ExecutionAtomicKindV1,
    },
}

impl ExecutionCapabilityAtomicScopeErrorV1 {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InsufficientScope { .. } => EXECUTION_CAPABILITY_ATOMIC_SCOPE_DIAGNOSTIC_V1,
            Self::MissingKernelEntry { .. } | Self::MissingWorkgroupSize { .. } => {
                "FE2O3-CAP-ANALYSIS002"
            }
        }
    }
}

impl fmt::Display for ExecutionCapabilityAtomicScopeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "error[{}]: ", self.code())?;
        match self {
            Self::MissingKernelEntry { kernel, entry } => write!(
                formatter,
                "kernel {kernel} names missing KIR entry function {entry}"
            ),
            Self::MissingWorkgroupSize { kernel } => write!(
                formatter,
                "kernel {kernel} has no exact workgroup size for atomic-scope analysis"
            ),
            Self::InsufficientScope {
                kernel,
                function,
                block,
                source,
                scope,
                kind,
            } => write!(
                formatter,
                "insufficient atomic scope at final-graph analysis: {kind:?} in kernel {kernel}, function {function}, block {block} uses {scope:?} scope for a global write/RMW reachable by multiple workgroups (source block {}, source operation {:02x?})",
                source.block, source.operation,
            ),
        }
    }
}

impl Error for ExecutionCapabilityAtomicScopeErrorV1 {}

/// Rejects global write/RMW atomics whose synchronization scope cannot order
/// conflicts between workgroups in any reachable kernel root.
///
/// The caller must supply a module decoded from an already verified canonical
/// KIR V13 owner. This pass is deliberately target-neutral: target support is
/// checked separately, while this pass checks the language memory contract.
pub fn analyze_execution_capability_atomic_scope_v1(
    module: &Module,
) -> Result<ExecutionCapabilityAtomicScopeReportV1, ExecutionCapabilityAtomicScopeErrorV1> {
    let mut checked_atomic_operations = 0usize;
    for kernel in &module.kernels {
        let workgroup = kernel.workgroup_size.ok_or_else(|| {
            ExecutionCapabilityAtomicScopeErrorV1::MissingWorkgroupSize {
                kernel: kernel.id.clone(),
            }
        })?;
        let may_span_workgroups = kernel
            .domain
            .extents()
            .zip([workgroup.x, workgroup.y, workgroup.z])
            .any(|(extent, workgroup_extent)| match extent {
                LaunchExtent::Dynamic => true,
                LaunchExtent::Static(global_extent) => global_extent > workgroup_extent,
            });

        let mut pending = vec![kernel.entry.clone()];
        let mut visited = BTreeSet::new();
        while let Some(function_id) = pending.pop() {
            if !visited.insert(function_id.clone()) {
                continue;
            }
            let function = module.function(&function_id).ok_or_else(|| {
                ExecutionCapabilityAtomicScopeErrorV1::MissingKernelEntry {
                    kernel: kernel.id.clone(),
                    entry: function_id.clone(),
                }
            })?;
            let Some(body) = &function.body else {
                continue;
            };
            for block in &body.blocks {
                for operation in &block.operations {
                    match &operation.kind {
                        OperationKind::Call { callee, .. } if module.function(callee).is_some() => {
                            pending.push(callee.clone());
                        }
                        OperationKind::ExecutionCapability(contract) => {
                            let ExecutionCapabilityOperationV1::Atomic {
                                kind,
                                address_space,
                                scope,
                                ..
                            } = contract.operation
                            else {
                                continue;
                            };
                            if kind.atomic_kind().is_none() {
                                continue;
                            }
                            checked_atomic_operations = checked_atomic_operations.saturating_add(1);
                            let writes = !matches!(kind, ExecutionAtomicKindV1::Load);
                            let scope_is_too_narrow = matches!(
                                scope,
                                ExecutionMemoryScopeV1::Workgroup
                                    | ExecutionMemoryScopeV1::Subgroup
                            );
                            if may_span_workgroups
                                && writes
                                && address_space == ExecutionMemoryAddressSpaceV1::Global
                                && scope_is_too_narrow
                            {
                                return Err(
                                    ExecutionCapabilityAtomicScopeErrorV1::InsufficientScope {
                                        kernel: kernel.id.clone(),
                                        function: function.id.clone(),
                                        block: block.id.0,
                                        source: contract.source,
                                        scope,
                                        kind,
                                    },
                                );
                            }
                        }
                        OperationKind::Call { .. }
                        | OperationKind::Constant(_)
                        | OperationKind::Intrinsic(_)
                        | OperationKind::MemoryIntrinsic(_)
                        | OperationKind::Unary { .. }
                        | OperationKind::Binary { .. }
                        | OperationKind::Compare { .. }
                        | OperationKind::Cast { .. }
                        | OperationKind::Select { .. }
                        | OperationKind::Alloca { .. }
                        | OperationKind::SliceLength { .. }
                        | OperationKind::SliceData { .. }
                        | OperationKind::GetElementPointer { .. }
                        | OperationKind::Load { .. }
                        | OperationKind::GuardedLoad { .. }
                        | OperationKind::GuardedStore { .. }
                        | OperationKind::Store { .. }
                        | OperationKind::Barrier(_)
                        | OperationKind::Atomic(_)
                        | OperationKind::Fence(_)
                        | OperationKind::WorkgroupBarrier(_)
                        | OperationKind::WorkgroupMemory(_)
                        | OperationKind::Matrix(_)
                        | OperationKind::Gfx950LdsTranspose(_)
                        | OperationKind::Wave(_)
                        | OperationKind::InlineAssembly(_)
                        | OperationKind::KernelContextIssue(_)
                        | OperationKind::GlobalCapabilityBind(_)
                        | OperationKind::GlobalCapabilityIndex(_) => {}
                    }
                }
            }
        }
    }

    Ok(ExecutionCapabilityAtomicScopeReportV1 {
        checked_kernel_roots: module.kernels.len(),
        checked_atomic_operations,
    })
}

#[cfg(test)]
mod tests {
    use fe2o3_kernel_ir::{
        BasicBlock, ExecutionCapabilityOpV1, ExecutionCapabilityProvenanceV1,
        ExecutionCapabilitySignatureV1, ExecutionMemoryOrderingV1, ExecutionSafetyObligationsV1,
        ExecutionTypeIdentityV1, Function, Kernel, LaunchDomain, Operation, Signature, Terminator,
        WorkgroupSize, required_execution_obligations_v1,
    };

    use super::*;

    fn identity(byte: u8) -> ExecutionTypeIdentityV1 {
        ExecutionTypeIdentityV1::new([byte; 32])
    }

    fn atomic(scope: ExecutionMemoryScopeV1) -> Operation {
        let operation = ExecutionCapabilityOperationV1::Atomic {
            kind: ExecutionAtomicKindV1::FetchAdd,
            authority: identity(1),
            location_input: identity(2),
            location: identity(3),
            element: identity(4),
            operand: Some(identity(5)),
            replacement: None,
            result: identity(6),
            value_type: fe2o3_kernel_ir::ScalarType::U32,
            address_space: ExecutionMemoryAddressSpaceV1::Global,
            scope,
            success: Some(ExecutionMemoryOrderingV1::Relaxed),
            failure: None,
        };
        Operation::new(
            vec![],
            OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
                operands: vec![],
                signature: ExecutionCapabilitySignatureV1::new(
                    &[identity(1), identity(2), identity(5)],
                    identity(6),
                )
                .unwrap(),
                provenance: ExecutionCapabilityProvenanceV1 {
                    root: FunctionId::new("entry"),
                    kernel_binding: [7; 32],
                    frontend_unit: [8; 32],
                    kernel_marker: [9; 32],
                    target_brand: [10; 32],
                    launch_brand: [11; 32],
                    issuance: [12; 32],
                },
                workgroup_brand: Some([13; 32]),
                epoch_before: Some([14; 32]),
                epoch_after: None,
                obligations: ExecutionSafetyObligationsV1::from_bits(
                    required_execution_obligations_v1(&operation),
                ),
                source: ExecutionCapabilitySourceV1 {
                    function: [15; 32],
                    operation: [16; 32],
                    block: 3,
                },
                operation,
            }),
        )
    }

    fn module(scope: ExecutionMemoryScopeV1, global_extent: u32) -> Module {
        let mut helper_block = BasicBlock::new(fe2o3_kernel_ir::BlockId(1));
        helper_block.operations.push(atomic(scope));
        helper_block.terminator = Some(Terminator::Return { values: vec![] });

        let mut entry_block = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
        entry_block.operations.push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: FunctionId::new("helper"),
                arguments: vec![],
            },
        ));
        entry_block.terminator = Some(Terminator::Return { values: vec![] });

        let mut module = Module::new("atomic-scope-analysis");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![entry_block],
        ));
        module.functions.push(Function::internal_helper(
            "helper",
            Signature::new(vec![], vec![]),
            vec![],
            vec![helper_block],
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

    #[test]
    fn cross_workgroup_global_write_requires_device_or_system_scope() {
        let error = analyze_execution_capability_atomic_scope_v1(&module(
            ExecutionMemoryScopeV1::Workgroup,
            128,
        ))
        .unwrap_err();
        assert!(matches!(
            error,
            ExecutionCapabilityAtomicScopeErrorV1::InsufficientScope {
                scope: ExecutionMemoryScopeV1::Workgroup,
                kind: ExecutionAtomicKindV1::FetchAdd,
                ..
            }
        ));
        assert!(
            error
                .to_string()
                .contains(EXECUTION_CAPABILITY_ATOMIC_SCOPE_DIAGNOSTIC_V1)
        );

        for scope in [
            ExecutionMemoryScopeV1::Device,
            ExecutionMemoryScopeV1::System,
        ] {
            let report = analyze_execution_capability_atomic_scope_v1(&module(scope, 128)).unwrap();
            assert_eq!(report.checked_kernel_roots(), 1);
            assert_eq!(report.checked_atomic_operations(), 1);
        }
    }

    #[test]
    fn workgroup_scope_is_sufficient_for_a_single_workgroup() {
        let report = analyze_execution_capability_atomic_scope_v1(&module(
            ExecutionMemoryScopeV1::Workgroup,
            64,
        ))
        .unwrap();
        assert_eq!(report.checked_atomic_operations(), 1);
    }
}
