use std::error::Error;
use std::fmt;

use fe2o3_amd_target::{
    PRODUCTION_AMDHSA_LLVM_DATA_LAYOUT_V1, PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1,
    ProductionAmdTargetProfileV1,
};
use fe2o3_kernel_ir::{
    FunctionId, KernelId, Module, TargetCapability, VerificationErrors, WaveWidth,
    gfx942_xnack_minus_target_capability, gfx950_xnack_minus_target_capability, verify_module,
};
use sha2::{Digest, Sha256};

use crate::{
    AMDGPU_TRIPLE, GFX942_XNACK_MINUS_DATA_LAYOUT, ProductionReplayKernelIrIdentityV1,
    ProductionReplayKernelIrVersionV1,
};

const STRUCTURAL_BINDING_DOMAIN_V1: &[u8] = b"FE2O3/PRODUCTION-TARGET-STRUCTURAL-BINDING/V1\0";

/// Exact, name-independent coordinate counts retained by production target binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionTargetStructuralCountsV1 {
    functions: u64,
    defined_bodies: u64,
    blocks: u64,
    operations: u64,
}

impl ProductionTargetStructuralCountsV1 {
    pub const fn functions(self) -> u64 {
        self.functions
    }

    pub const fn defined_bodies(self) -> u64 {
        self.defined_bodies
    }

    pub const fn blocks(self) -> u64 {
        self.blocks
    }

    pub const fn operations(self) -> u64 {
        self.operations
    }
}

/// Exact canonical identities and coordinate shape joined by the sole production target binder.
///
/// This record proves only that function, block, and operation ordinals were retained while the
/// binder added its target capabilities. It is not a semantic-refinement or execution proof.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionTargetStructuralBindingV1 {
    identity: [u8; 32],
    profile: ProductionAmdTargetProfileV1,
    version: ProductionReplayKernelIrVersionV1,
    neutral_kernel_ir: ProductionReplayKernelIrIdentityV1,
    target_bound_kernel_ir: ProductionReplayKernelIrIdentityV1,
    counts: ProductionTargetStructuralCountsV1,
}

impl ProductionTargetStructuralBindingV1 {
    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }

    pub const fn profile(self) -> ProductionAmdTargetProfileV1 {
        self.profile
    }

    pub const fn version(self) -> ProductionReplayKernelIrVersionV1 {
        self.version
    }

    pub const fn neutral_kernel_ir(self) -> ProductionReplayKernelIrIdentityV1 {
        self.neutral_kernel_ir
    }

    pub const fn target_bound_kernel_ir(self) -> ProductionReplayKernelIrIdentityV1 {
        self.target_bound_kernel_ir
    }

    pub const fn counts(self) -> ProductionTargetStructuralCountsV1 {
        self.counts
    }

    pub const fn preserves_function_block_operation_coordinates(self) -> bool {
        true
    }

    pub const fn proves_semantic_refinement(self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(self) -> bool {
        false
    }
}

/// The deterministic target-bound Kernel IR produced from one neutral module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionTargetBoundKernelIrV1 {
    profile: ProductionAmdTargetProfileV1,
    module: Module,
    kernel_ids: Box<[KernelId]>,
}

impl ProductionTargetBoundKernelIrV1 {
    pub const fn profile(&self) -> ProductionAmdTargetProfileV1 {
        self.profile
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    /// Returns every exact kernel identity in canonical module order.
    pub fn kernel_ids(&self) -> &[KernelId] {
        &self.kernel_ids
    }

    /// Consumes target-bound custody into the module and ordered kernel roster.
    pub fn into_parts(self) -> (Module, Box<[KernelId]>) {
        (self.module, self.kernel_ids)
    }

    pub(crate) fn admit_exact_structural_binding_v1(
        &self,
        neutral_module: &Module,
        neutral_kernel_ir: ProductionReplayKernelIrIdentityV1,
        target_bound_kernel_ir: ProductionReplayKernelIrIdentityV1,
    ) -> Result<ProductionTargetStructuralBindingV1, ProductionTargetStructuralBindingErrorV1> {
        if neutral_kernel_ir.version() != target_bound_kernel_ir.version()
            || !same_coordinate_shape(neutral_module, &self.module)
        {
            return Err(ProductionTargetStructuralBindingErrorV1::CoordinateShapeMismatch);
        }
        let counts = structural_counts(neutral_module)?;
        let mut digest = Sha256::new();
        digest.update((STRUCTURAL_BINDING_DOMAIN_V1.len() as u32).to_le_bytes());
        digest.update(STRUCTURAL_BINDING_DOMAIN_V1);
        digest.update([match self.profile {
            ProductionAmdTargetProfileV1::Gfx942 => 1,
            ProductionAmdTargetProfileV1::Gfx950 => 2,
        }]);
        digest.update([match neutral_kernel_ir.version() {
            ProductionReplayKernelIrVersionV1::V8 => 8,
            ProductionReplayKernelIrVersionV1::V9 => 9,
        }]);
        for identity in [neutral_kernel_ir, target_bound_kernel_ir] {
            digest.update(identity.sha256());
            digest.update(identity.byte_len().to_le_bytes());
        }
        for count in [
            counts.functions,
            counts.defined_bodies,
            counts.blocks,
            counts.operations,
        ] {
            digest.update(count.to_le_bytes());
        }
        for function in &neutral_module.functions {
            match &function.body {
                Some(body) => {
                    digest.update([1]);
                    digest.update((body.blocks.len() as u64).to_le_bytes());
                    for block in &body.blocks {
                        digest.update((block.operations.len() as u64).to_le_bytes());
                    }
                }
                None => digest.update([0]),
            }
        }
        Ok(ProductionTargetStructuralBindingV1 {
            identity: digest.finalize().into(),
            profile: self.profile,
            version: neutral_kernel_ir.version(),
            neutral_kernel_ir,
            target_bound_kernel_ir,
            counts,
        })
    }
}

fn same_coordinate_shape(neutral: &Module, target: &Module) -> bool {
    neutral.functions.len() == target.functions.len()
        && neutral
            .functions
            .iter()
            .zip(&target.functions)
            .all(|(neutral, target)| match (&neutral.body, &target.body) {
                (None, None) => true,
                (Some(neutral), Some(target)) => {
                    neutral.blocks.len() == target.blocks.len()
                        && neutral
                            .blocks
                            .iter()
                            .zip(&target.blocks)
                            .all(|(neutral, target)| {
                                neutral.operations.len() == target.operations.len()
                            })
                }
                _ => false,
            })
}

fn structural_counts(
    module: &Module,
) -> Result<ProductionTargetStructuralCountsV1, ProductionTargetStructuralBindingErrorV1> {
    let functions = u64::try_from(module.functions.len())
        .map_err(|_| ProductionTargetStructuralBindingErrorV1::CoordinateCountOverflow)?;
    let (defined_bodies, blocks, operations) = module.functions.iter().try_fold(
        (0_u64, 0_u64, 0_u64),
        |(defined_bodies, blocks, operations), function| {
            let Some(body) = &function.body else {
                return Ok((defined_bodies, blocks, operations));
            };
            let defined_bodies = defined_bodies
                .checked_add(1)
                .ok_or(ProductionTargetStructuralBindingErrorV1::CoordinateCountOverflow)?;
            let blocks = blocks
                .checked_add(u64::try_from(body.blocks.len()).map_err(|_| {
                    ProductionTargetStructuralBindingErrorV1::CoordinateCountOverflow
                })?)
                .ok_or(ProductionTargetStructuralBindingErrorV1::CoordinateCountOverflow)?;
            let operations = body
                .blocks
                .iter()
                .try_fold(operations, |operations, block| {
                    operations
                        .checked_add(u64::try_from(block.operations.len()).map_err(|_| {
                            ProductionTargetStructuralBindingErrorV1::CoordinateCountOverflow
                        })?)
                        .ok_or(ProductionTargetStructuralBindingErrorV1::CoordinateCountOverflow)
                })?;
            Ok((defined_bodies, blocks, operations))
        },
    )?;
    Ok(ProductionTargetStructuralCountsV1 {
        functions,
        defined_bodies,
        blocks,
        operations,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionTargetStructuralBindingErrorV1 {
    CoordinateShapeMismatch,
    CoordinateCountOverflow,
}

/// Closed failures for the production neutral-KIR target-binding transform.
#[derive(Debug)]
pub enum ProductionTargetBindingErrorV1 {
    KernelClosure { observed: usize },
    MissingEntry { entry: FunctionId },
    InvalidTargetBoundModule(VerificationErrors),
}

impl fmt::Display for ProductionTargetBindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::KernelClosure { observed } => write!(
                formatter,
                "production target binding requires at least one exact kernel, observed {observed}"
            ),
            Self::MissingEntry { entry } => write!(
                formatter,
                "production target binding cannot find kernel entry {entry}"
            ),
            Self::InvalidTargetBoundModule(error) => {
                write!(
                    formatter,
                    "production target-bound Kernel IR is invalid: {error}"
                )
            }
        }
    }
}

impl Error for ProductionTargetBindingErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidTargetBoundModule(error) => Some(error),
            Self::KernelClosure { .. } | Self::MissingEntry { .. } => None,
        }
    }
}

/// Applies the sole production target transform to a target-neutral Kernel IR module.
///
/// The transform adds only the exact processor and Wave64 requirements to the
/// module, every kernel, and each kernel's entry function. It then verifies
/// the complete result before returning target-bound custody.
pub fn bind_production_target_v1(
    neutral_module: &Module,
    profile: ProductionAmdTargetProfileV1,
) -> Result<ProductionTargetBoundKernelIrV1, ProductionTargetBindingErrorV1> {
    let mut module = neutral_module.clone();
    let target = match profile {
        ProductionAmdTargetProfileV1::Gfx942 => gfx942_xnack_minus_target_capability(),
        ProductionAmdTargetProfileV1::Gfx950 => gfx950_xnack_minus_target_capability(),
    };
    let wave = TargetCapability::WaveWidth(WaveWidth::Wave64);

    module.required_capabilities.insert(target.clone());
    module.required_capabilities.insert(wave.clone());

    let observed = module.kernels.len();
    if module.kernels.is_empty() {
        return Err(ProductionTargetBindingErrorV1::KernelClosure { observed });
    }
    let mut kernel_ids = Vec::with_capacity(module.kernels.len());
    for kernel in &mut module.kernels {
        kernel.required_capabilities.insert(target.clone());
        kernel.required_capabilities.insert(wave.clone());
        kernel_ids.push(kernel.id.clone());
        let entry_id = kernel.entry.clone();
        let entry = module
            .functions
            .iter_mut()
            .find(|function| function.id == entry_id)
            .ok_or_else(|| ProductionTargetBindingErrorV1::MissingEntry {
                entry: entry_id.clone(),
            })?;
        entry.required_capabilities.insert(target.clone());
        entry.required_capabilities.insert(wave.clone());
    }

    verify_module(&module).map_err(ProductionTargetBindingErrorV1::InvalidTargetBoundModule)?;
    Ok(ProductionTargetBoundKernelIrV1 {
        profile,
        module,
        kernel_ids: kernel_ids.into_boxed_slice(),
    })
}

/// Closed failures for exact production LLVM target-header binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionLlvmLayoutBindingErrorV1 {
    NonCanonicalTargetHeader,
    Overflow,
    ResourceLimit,
    AllocationFailure,
}

impl fmt::Display for ProductionLlvmLayoutBindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonCanonicalTargetHeader => formatter
                .write_str("verified AMDGPU lowering did not retain one canonical target header"),
            Self::Overflow => formatter.write_str("LLVM target-header binding length overflowed"),
            Self::ResourceLimit => {
                formatter.write_str("LLVM target-header binding exceeds its byte limit")
            }
            Self::AllocationFailure => {
                formatter.write_str("LLVM target-header binding allocation failed")
            }
        }
    }
}

impl Error for ProductionLlvmLayoutBindingErrorV1 {}

/// Retains the historical production LLVM V1 layout for exact replay compatibility.
///
/// The input must contain exactly one canonical AMDGPU target header. The
/// returned text is suitable for exact replay by an independent verifier; it
/// does not grant object-generation, linking, publication, or execution authority.
pub fn bind_production_upstream_llvm_layout_v1(
    dialect_llvm_ir: &str,
) -> Result<String, ProductionLlvmLayoutBindingErrorV1> {
    bind_exact_llvm_layout_v1(
        dialect_llvm_ir,
        PRODUCTION_AMDHSA_LLVM_DATA_LAYOUT_V1,
        crate::MAX_COMPILER_MODULE_TEXT_BYTES,
    )
}

pub(crate) fn bind_historical_replay_llvm_layout_v1(
    dialect_llvm_ir: &str,
) -> Result<String, ProductionLlvmLayoutBindingErrorV1> {
    bind_exact_llvm_layout_v1(
        dialect_llvm_ir,
        PRODUCTION_AMDHSA_LLVM_DATA_LAYOUT_V1,
        crate::MAX_PRODUCTION_LEGACY_REPLAY_LLVM_TEXT_BYTES_V1,
    )
}

/// Rebinds deterministic dialect LLVM to the layout measured from the LLVM 22 Worker.
///
/// This additive surface is for physical Worker input. It does not change the byte meaning of the
/// historical V1 binder or serialized V1 policies.
pub fn bind_production_llvm22_worker_layout_v1(
    dialect_llvm_ir: &str,
) -> Result<String, ProductionLlvmLayoutBindingErrorV1> {
    bind_exact_llvm_layout_v1(
        dialect_llvm_ir,
        PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1,
        crate::MAX_PRODUCTION_SEMANTIC_ANCHOR_LLVM_TEXT_BYTES_V1,
    )
}

fn bind_exact_llvm_layout_v1(
    dialect_llvm_ir: &str,
    bound_layout: &str,
    max_bytes: usize,
) -> Result<String, ProductionLlvmLayoutBindingErrorV1> {
    const TRIPLE_HEADER: &str = "target triple = \"amdgcn-amd-amdhsa\"\n";
    const LAYOUT_HEADER: &str = "target datalayout = \"";
    const HEADER_SUFFIX: &str = "\"\n\n";
    debug_assert_eq!(AMDGPU_TRIPLE, "amdgcn-amd-amdhsa");
    let body = dialect_llvm_ir
        .strip_prefix(TRIPLE_HEADER)
        .and_then(|text| text.strip_prefix(LAYOUT_HEADER))
        .and_then(|text| text.strip_prefix(GFX942_XNACK_MINUS_DATA_LAYOUT))
        .and_then(|text| text.strip_prefix(HEADER_SUFFIX))
        .ok_or(ProductionLlvmLayoutBindingErrorV1::NonCanonicalTargetHeader)?;
    if dialect_llvm_ir.matches("target triple =").count() != 1
        || dialect_llvm_ir.matches("target datalayout =").count() != 1
    {
        return Err(ProductionLlvmLayoutBindingErrorV1::NonCanonicalTargetHeader);
    }
    let output_len = TRIPLE_HEADER
        .len()
        .checked_add(LAYOUT_HEADER.len())
        .and_then(|length| length.checked_add(bound_layout.len()))
        .and_then(|length| length.checked_add(HEADER_SUFFIX.len()))
        .and_then(|length| length.checked_add(body.len()))
        .ok_or(ProductionLlvmLayoutBindingErrorV1::Overflow)?;
    if output_len > max_bytes {
        return Err(ProductionLlvmLayoutBindingErrorV1::ResourceLimit);
    }
    let mut bound = String::new();
    bound
        .try_reserve_exact(output_len)
        .map_err(|_| ProductionLlvmLayoutBindingErrorV1::AllocationFailure)?;
    bound.push_str(TRIPLE_HEADER);
    bound.push_str(LAYOUT_HEADER);
    bound.push_str(bound_layout);
    bound.push_str(HEADER_SUFFIX);
    bound.push_str(body);
    if bound.len() != output_len
        || dialect_llvm_ir.matches("target triple =").count() != 1
        || dialect_llvm_ir.matches("target datalayout =").count() != 1
    {
        return Err(ProductionLlvmLayoutBindingErrorV1::NonCanonicalTargetHeader);
    }
    Ok(bound)
}

#[cfg(test)]
mod tests {
    use fe2o3_kernel_ir::{
        AddressSpace, BarrierSemantics, BasicBlock, BlockId, Convergence, Function, LaunchDomain,
        LaunchExtent, MemoryOrdering, Module, Operation, OperationKind, ScalarType, Signature,
        SynchronizationScope, Terminator, Type, ValueDef, ValueId, WorkgroupBarrier,
        WorkgroupMemory, WorkgroupMemoryExtent, WorkgroupSize,
        gfx942_xnack_minus_target_capability,
    };

    use super::*;

    fn neutral_module() -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let function =
            Function::kernel_entry("entry", Signature::new(vec![], vec![]), vec![], vec![block]);
        let mut kernel = fe2o3_kernel_ir::Kernel::new(
            "kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        let mut module = Module::new("production-refinement-test");
        module.functions.push(function);
        module.kernels.push(kernel);
        module
    }

    fn neutral_workgroup_collective_module() -> Module {
        let pointer = Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Workgroup,
            fe2o3_kernel_ir::AccessMode::ReadWrite,
        );
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = vec![
            Operation::new(
                vec![ValueDef::new(ValueId(0), pointer)],
                OperationKind::WorkgroupMemory(WorkgroupMemory {
                    element: Type::Scalar(ScalarType::U32),
                    extent: WorkgroupMemoryExtent::Static(64),
                    alignment: 4,
                }),
            ),
            Operation::new(
                vec![],
                OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                    memory_scope: SynchronizationScope::Workgroup,
                    semantics: BarrierSemantics::new(
                        MemoryOrdering::AcquireRelease,
                        [AddressSpace::Workgroup],
                    ),
                    convergence: Convergence::uniform(SynchronizationScope::Workgroup),
                }),
            ),
        ];
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut function = Function::kernel_entry(
            "workgroup_collective_entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        );
        function.required_capabilities = function.derived_capabilities();
        let mut kernel = fe2o3_kernel_ir::Kernel::new(
            "workgroup_collective",
            "workgroup_collective_entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        kernel.required_capabilities = function.required_capabilities.clone();
        let mut module = Module::new("production-neutral-workgroup-collective");
        module.required_capabilities = function.required_capabilities.clone();
        module.functions.push(function);
        module.kernels.push(kernel);
        module
    }

    #[test]
    fn target_binding_is_exact_and_does_not_mutate_neutral_input() {
        let neutral = neutral_module();
        let bound = bind_production_target_v1(&neutral, ProductionAmdTargetProfileV1::Gfx942)
            .expect("target binding succeeds");
        let target = gfx942_xnack_minus_target_capability();
        let wave = TargetCapability::WaveWidth(WaveWidth::Wave64);

        assert!(neutral.required_capabilities.is_empty());
        assert_eq!(bound.profile(), ProductionAmdTargetProfileV1::Gfx942);
        assert_eq!(bound.kernel_ids(), &[KernelId::new("kernel")]);
        assert!(bound.module().required_capabilities.contains(&target));
        assert!(bound.module().required_capabilities.contains(&wave));
        assert!(
            bound.module().kernels[0]
                .required_capabilities
                .contains(&target)
        );
        assert!(
            bound.module().kernels[0]
                .required_capabilities
                .contains(&wave)
        );
        assert!(
            bound.module().functions[0]
                .required_capabilities
                .contains(&target)
        );
        assert!(
            bound.module().functions[0]
                .required_capabilities
                .contains(&wave)
        );
    }

    #[test]
    fn neutral_workgroup_memory_and_uniform_barriers_bind_and_lower_for_both_targets() {
        let neutral = neutral_workgroup_collective_module();
        for profile in [
            ProductionAmdTargetProfileV1::Gfx942,
            ProductionAmdTargetProfileV1::Gfx950,
        ] {
            let bound = bind_production_target_v1(&neutral, profile).unwrap();
            let llvm = match profile {
                ProductionAmdTargetProfileV1::Gfx942 => {
                    crate::lower_compiler_module_to_gfx942_xnack_minus_llvm_ir(bound.module())
                }
                ProductionAmdTargetProfileV1::Gfx950 => {
                    crate::lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(bound.module())
                }
            }
            .unwrap();
            assert!(llvm.contains("internal addrspace(3) global [64 x i32] undef, align 4"));
            assert!(llvm.contains("fence syncscope(\"workgroup\") release"));
            assert!(llvm.contains("call void asm sideeffect \"s_barrier\", \"\"()"));
            assert!(llvm.contains("fence syncscope(\"workgroup\") acquire"));
        }

        let gfx942 =
            bind_production_target_v1(&neutral, ProductionAmdTargetProfileV1::Gfx942).unwrap();
        assert!(
            crate::lower_compiler_module_to_gfx950_xnack_minus_llvm_ir(gfx942.module()).is_err()
        );
        let gfx950 =
            bind_production_target_v1(&neutral, ProductionAmdTargetProfileV1::Gfx950).unwrap();
        assert!(
            crate::lower_compiler_module_to_gfx942_xnack_minus_llvm_ir(gfx950.module()).is_err()
        );
    }

    #[test]
    fn target_binding_rejects_empty_and_binds_every_kernel_in_order() {
        let mut neutral = neutral_module();
        neutral.kernels.clear();
        assert!(matches!(
            bind_production_target_v1(&neutral, ProductionAmdTargetProfileV1::Gfx942),
            Err(ProductionTargetBindingErrorV1::KernelClosure { observed: 0 })
        ));

        let mut neutral = neutral_module();
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        neutral.functions.push(Function::kernel_entry(
            "second_entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        let mut second = fe2o3_kernel_ir::Kernel::new(
            "second_kernel",
            "second_entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        );
        second.workgroup_size = Some(WorkgroupSize::new(128, 1, 1));
        neutral.kernels.push(second);
        let bound = bind_production_target_v1(&neutral, ProductionAmdTargetProfileV1::Gfx942)
            .expect("multi-kernel target binding succeeds");
        assert_eq!(
            bound.kernel_ids(),
            &[KernelId::new("kernel"), KernelId::new("second_kernel")]
        );
        assert!(bound.module().kernels.iter().all(|kernel| {
            kernel
                .required_capabilities
                .contains(&TargetCapability::WaveWidth(WaveWidth::Wave64))
        }));
    }

    #[test]
    fn target_binding_error_surface_remains_exhaustively_matchable() {
        fn classify(error: &ProductionTargetBindingErrorV1) -> u8 {
            match error {
                ProductionTargetBindingErrorV1::KernelClosure { .. } => 1,
                ProductionTargetBindingErrorV1::MissingEntry { .. } => 2,
                ProductionTargetBindingErrorV1::InvalidTargetBoundModule(_) => 3,
            }
        }

        assert_eq!(
            classify(&ProductionTargetBindingErrorV1::KernelClosure { observed: 0 }),
            1
        );
        assert_eq!(
            classify(&ProductionTargetBindingErrorV1::MissingEntry {
                entry: FunctionId::new("missing"),
            }),
            2
        );
    }

    #[test]
    fn llvm_layout_binding_requires_the_exact_unique_header() {
        let dialect = format!(
            "target triple = \"{AMDGPU_TRIPLE}\"\ntarget datalayout = \"{GFX942_XNACK_MINUS_DATA_LAYOUT}\"\n\ndefine void @kernel() {{\n  ret void\n}}\n"
        );
        let expected = format!(
            "target triple = \"{AMDGPU_TRIPLE}\"\ntarget datalayout = \"{PRODUCTION_AMDHSA_LLVM_DATA_LAYOUT_V1}\"\n\ndefine void @kernel() {{\n  ret void\n}}\n"
        );
        assert_eq!(
            bind_production_upstream_llvm_layout_v1(&dialect).unwrap(),
            expected
        );
        let worker_expected = format!(
            "target triple = \"{AMDGPU_TRIPLE}\"\ntarget datalayout = \"{PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1}\"\n\ndefine void @kernel() {{\n  ret void\n}}\n"
        );
        assert_eq!(
            bind_production_llvm22_worker_layout_v1(&dialect).unwrap(),
            worker_expected
        );

        for hostile in [
            dialect.replacen("target triple", "source triple", 1),
            dialect.replacen("target datalayout", "source datalayout", 1),
            format!("{dialect}target triple = \"{AMDGPU_TRIPLE}\"\n"),
            format!("{dialect}target datalayout = \"{GFX942_XNACK_MINUS_DATA_LAYOUT}\"\n"),
        ] {
            assert_eq!(
                bind_production_upstream_llvm_layout_v1(&hostile),
                Err(ProductionLlvmLayoutBindingErrorV1::NonCanonicalTargetHeader)
            );
            assert_eq!(
                bind_production_llvm22_worker_layout_v1(&hostile),
                Err(ProductionLlvmLayoutBindingErrorV1::NonCanonicalTargetHeader)
            );
        }
    }
}
