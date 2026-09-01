use std::{error::Error, fmt};

use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_kernel_ir::{
    Module, TargetCapability, WaveWidth, gfx942_xnack_minus_target_capability,
    gfx950_xnack_minus_target_capability, verify_module,
};

use crate::ProductionTargetBoundKernelIrV1;

/// SHA-256 of `verus/target_binding_refinement_v1.rs`, enforced by the proof runner.
pub const TARGET_BINDING_REFINEMENT_MODEL_SHA256_V1: [u8; 32] = [
    0x9b, 0x7f, 0xb3, 0x7f, 0x7e, 0xe4, 0x2c, 0x4c, 0x46, 0xb6, 0x6b, 0xd7, 0x31, 0x86, 0xe7, 0x98,
    0xfa, 0x8a, 0xad, 0x13, 0xee, 0xe5, 0xfa, 0x56, 0x93, 0xea, 0x9d, 0x27, 0xd4, 0x29, 0xf4, 0xff,
];

/// Independent validation of the semantic boundary crossed by production target binding.
///
/// The target binder may add the selected processor capability and the Wave64 requirement to
/// the module, every kernel, and every kernel entry. This validator reconstructs that exact
/// result without calling the binder and rejects every other change to the Kernel IR.
#[derive(Debug)]
pub struct ValidatedProductionTargetSemanticBindingV1 {
    profile: ProductionAmdTargetProfileV1,
    kernel_count: usize,
}

impl ValidatedProductionTargetSemanticBindingV1 {
    pub const fn profile(&self) -> ProductionAmdTargetProfileV1 {
        self.profile
    }

    pub const fn kernel_count(&self) -> usize {
        self.kernel_count
    }

    pub const fn formal_model_sha256(&self) -> [u8; 32] {
        TARGET_BINDING_REFINEMENT_MODEL_SHA256_V1
    }

    /// The checked relation matches the input relation of the pinned Verus V1 theorem.
    pub const fn matches_formal_target_binding_relation_v1(&self) -> bool {
        true
    }

    /// LLVM, ISA, loading, runtime, and hardware behavior are outside this validation.
    pub const fn grants_later_stage_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum ProductionTargetSemanticValidationErrorV1 {
    InvalidNeutralModule(fe2o3_kernel_ir::VerificationErrors),
    InvalidTargetModule(fe2o3_kernel_ir::VerificationErrors),
    ProfileMismatch,
    MissingKernelEntry,
    NonCapabilitySemanticMutation,
}

impl fmt::Display for ProductionTargetSemanticValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNeutralModule(error) => {
                write!(formatter, "neutral Kernel IR is invalid: {error}")
            }
            Self::InvalidTargetModule(error) => {
                write!(formatter, "target-bound Kernel IR is invalid: {error}")
            }
            Self::ProfileMismatch => formatter.write_str(
                "target-bound Kernel IR profile does not match the requested validation profile",
            ),
            Self::MissingKernelEntry => formatter.write_str(
                "target semantic validation could not find an exact kernel entry function",
            ),
            Self::NonCapabilitySemanticMutation => formatter.write_str(
                "target binding changed Kernel IR outside the exact processor and Wave64 capability additions",
            ),
        }
    }
}

impl Error for ProductionTargetSemanticValidationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidNeutralModule(error) | Self::InvalidTargetModule(error) => Some(error),
            Self::ProfileMismatch
            | Self::MissingKernelEntry
            | Self::NonCapabilitySemanticMutation => None,
        }
    }
}

/// Validates that target binding preserved the complete target-neutral semantic module.
///
/// The reconstruction is intentionally separate from `bind_production_target_v1`: it performs
/// only the capability-set additions allowed by the formal V1 relation and then compares the
/// complete module. Any change to types, ABI, control flow, operations, effects, launch shape,
/// or existing capabilities is rejected.
pub fn validate_production_target_semantic_binding_v1(
    neutral: &Module,
    target: &ProductionTargetBoundKernelIrV1,
    profile: ProductionAmdTargetProfileV1,
) -> Result<ValidatedProductionTargetSemanticBindingV1, ProductionTargetSemanticValidationErrorV1> {
    verify_module(neutral)
        .map_err(ProductionTargetSemanticValidationErrorV1::InvalidNeutralModule)?;
    verify_module(target.module())
        .map_err(ProductionTargetSemanticValidationErrorV1::InvalidTargetModule)?;
    if target.profile() != profile {
        return Err(ProductionTargetSemanticValidationErrorV1::ProfileMismatch);
    }

    let processor = match profile {
        ProductionAmdTargetProfileV1::Gfx942 => gfx942_xnack_minus_target_capability(),
        ProductionAmdTargetProfileV1::Gfx950 => gfx950_xnack_minus_target_capability(),
    };
    let wave64 = TargetCapability::WaveWidth(WaveWidth::Wave64);
    let mut expected = neutral.clone();
    expected.required_capabilities.insert(processor.clone());
    expected.required_capabilities.insert(wave64.clone());

    let entries: Vec<_> = expected
        .kernels
        .iter_mut()
        .map(|kernel| {
            kernel.required_capabilities.insert(processor.clone());
            kernel.required_capabilities.insert(wave64.clone());
            kernel.entry.clone()
        })
        .collect();
    for entry in entries {
        let function = expected
            .functions
            .iter_mut()
            .find(|function| function.id == entry)
            .ok_or(ProductionTargetSemanticValidationErrorV1::MissingKernelEntry)?;
        function.required_capabilities.insert(processor.clone());
        function.required_capabilities.insert(wave64.clone());
    }

    if expected != *target.module() {
        return Err(ProductionTargetSemanticValidationErrorV1::NonCapabilitySemanticMutation);
    }

    Ok(ValidatedProductionTargetSemanticBindingV1 {
        profile,
        kernel_count: expected.kernels.len(),
    })
}

#[cfg(test)]
mod tests {
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Function, LaunchDomain, LaunchExtent, Module, Signature, Terminator,
        WorkgroupSize,
    };

    use super::*;
    use crate::bind_production_target_v1;
    use sha2::{Digest, Sha256};

    fn neutral_module() -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let function =
            Function::kernel_entry("entry", Signature::new(vec![], vec![]), vec![], vec![block]);
        let mut kernel = fe2o3_kernel_ir::Kernel::new(
            "kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        let mut module = Module::new("target-semantic-validation");
        module.functions.push(function);
        module.kernels.push(kernel);
        module
    }

    #[test]
    fn exact_production_binding_matches_formal_relation() {
        let neutral = neutral_module();
        let target = bind_production_target_v1(&neutral, ProductionAmdTargetProfileV1::Gfx942)
            .expect("target binding");
        let validated = validate_production_target_semantic_binding_v1(
            &neutral,
            &target,
            ProductionAmdTargetProfileV1::Gfx942,
        )
        .expect("semantic validation");
        assert_eq!(validated.kernel_count(), 1);
        assert_eq!(
            validated.formal_model_sha256(),
            TARGET_BINDING_REFINEMENT_MODEL_SHA256_V1
        );
        assert!(validated.matches_formal_target_binding_relation_v1());
        assert!(!validated.grants_later_stage_authority());
    }

    #[test]
    fn formal_model_identity_matches_the_pinned_theorem_source() {
        let actual: [u8; 32] =
            Sha256::digest(include_bytes!("../verus/target_binding_refinement_v1.rs")).into();
        assert_eq!(actual, TARGET_BINDING_REFINEMENT_MODEL_SHA256_V1);
    }

    #[test]
    fn wrong_profile_is_rejected() {
        let neutral = neutral_module();
        let target = bind_production_target_v1(&neutral, ProductionAmdTargetProfileV1::Gfx942)
            .expect("target binding");
        assert!(matches!(
            validate_production_target_semantic_binding_v1(
                &neutral,
                &target,
                ProductionAmdTargetProfileV1::Gfx950,
            ),
            Err(ProductionTargetSemanticValidationErrorV1::ProfileMismatch)
        ));
    }

    #[test]
    fn semantic_source_substitution_is_rejected() {
        let neutral = neutral_module();
        let target = bind_production_target_v1(&neutral, ProductionAmdTargetProfileV1::Gfx942)
            .expect("target binding");
        let mut substituted = neutral;
        substituted.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 1, 1));
        assert!(matches!(
            validate_production_target_semantic_binding_v1(
                &substituted,
                &target,
                ProductionAmdTargetProfileV1::Gfx942,
            ),
            Err(ProductionTargetSemanticValidationErrorV1::NonCapabilitySemanticMutation)
        ));
    }
}
