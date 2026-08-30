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

use crate::{AMDGPU_TRIPLE, GFX942_XNACK_MINUS_DATA_LAYOUT};

/// The deterministic target-bound Kernel IR produced from one neutral module.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionTargetBoundKernelIrV1 {
    profile: ProductionAmdTargetProfileV1,
    module: Module,
    kernel_id: KernelId,
}

impl ProductionTargetBoundKernelIrV1 {
    pub const fn profile(&self) -> ProductionAmdTargetProfileV1 {
        self.profile
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn kernel_id(&self) -> &KernelId {
        &self.kernel_id
    }

    pub fn into_parts(self) -> (Module, KernelId) {
        (self.module, self.kernel_id)
    }
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
                "production target binding requires exactly one kernel, observed {observed}"
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
/// module, its sole kernel, and that kernel's entry function. It then verifies
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
    let [kernel] = module.kernels.as_mut_slice() else {
        return Err(ProductionTargetBindingErrorV1::KernelClosure { observed });
    };
    kernel.required_capabilities.insert(target.clone());
    kernel.required_capabilities.insert(wave.clone());
    let kernel_id = kernel.id.clone();
    let entry_id = kernel.entry.clone();

    let entry = module
        .functions
        .iter_mut()
        .find(|function| function.id == entry_id)
        .ok_or_else(|| ProductionTargetBindingErrorV1::MissingEntry {
            entry: entry_id.clone(),
        })?;
    entry.required_capabilities.insert(target);
    entry.required_capabilities.insert(wave);

    verify_module(&module).map_err(ProductionTargetBindingErrorV1::InvalidTargetBoundModule)?;
    Ok(ProductionTargetBoundKernelIrV1 {
        profile,
        module,
        kernel_id,
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
        BasicBlock, BlockId, Function, LaunchDomain, LaunchExtent, Module, Signature, Terminator,
        WorkgroupSize, gfx942_xnack_minus_target_capability,
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

    #[test]
    fn target_binding_is_exact_and_does_not_mutate_neutral_input() {
        let neutral = neutral_module();
        let bound = bind_production_target_v1(&neutral, ProductionAmdTargetProfileV1::Gfx942)
            .expect("target binding succeeds");
        let target = gfx942_xnack_minus_target_capability();
        let wave = TargetCapability::WaveWidth(WaveWidth::Wave64);

        assert!(neutral.required_capabilities.is_empty());
        assert_eq!(bound.profile(), ProductionAmdTargetProfileV1::Gfx942);
        assert_eq!(bound.kernel_id(), &KernelId::new("kernel"));
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
    fn target_binding_rejects_non_singleton_kernel_closure() {
        let mut neutral = neutral_module();
        neutral.kernels.clear();
        assert!(matches!(
            bind_production_target_v1(&neutral, ProductionAmdTargetProfileV1::Gfx942),
            Err(ProductionTargetBindingErrorV1::KernelClosure { observed: 0 })
        ));

        let mut neutral = neutral_module();
        neutral.kernels.push(neutral.kernels[0].clone());
        assert!(matches!(
            bind_production_target_v1(&neutral, ProductionAmdTargetProfileV1::Gfx942),
            Err(ProductionTargetBindingErrorV1::KernelClosure { observed: 2 })
        ));
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
