// Included inside lowering. Legacy identity-pair APIs never admit V12 by
// rebuilding an executable; the native path retains the actual owner borrow.
#[derive(Clone, Copy)]
enum SemanticAnchorInputV1<'a> {
    Historical(ProductionSemanticAnchorKirIdentityV1),
    Native(&'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12),
}

impl ProductionSemanticAnchorKirIdentityV1 {
    /// Reads the actual V12 owner's already verified canonical identity.
    /// This inert identity does not authorize a legacy module/identity pair.
    pub fn from_v12(owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12) -> Self {
        Self {
            version: 12,
            sha256: *owner.canonical().identity().digest(),
            byte_len: owner.canonical().identity().canonical_length(),
        }
    }
}

impl SemanticAnchorInputV1<'_> {
    fn validate(
        self,
        module: &Module,
    ) -> Result<ProductionSemanticAnchorKirIdentityV1, LoweringErrors> {
        match self {
            Self::Historical(identity) => {
                validate_semantic_anchor_identity_v1(module, identity)?;
                Ok(identity)
            }
            Self::Native(owner) => {
                if !std::ptr::eq(module, owner.module()) {
                    return Err(LoweringErrors::one(
                        LoweringLocation::module(module),
                        LoweringDiagnosticCode::SemanticAnchorIdentityMismatch,
                        "native semantic anchors require the actual borrowed V12 executable owner",
                    ));
                }
                Ok(ProductionSemanticAnchorKirIdentityV1::from_v12(owner))
            }
        }
    }
}

/// Lowers the actual admitted V12 module for exact gfx942:xnack- with anchors.
/// No replacement executable, legacy canonicalization or optimizer is run.
/// The existing complete-module feature preflight, exact target/call-graph
/// checks, helper-coverage absence policy and text bounds apply unchanged.
/// Returned LLVM remains inert; this is not source/formal/lineage admission.
/// The existing lowering engine has its own resource policy, not the canonical
/// optimizer ledger. The caller retains the actual checked owner and its floor.
pub fn lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
) -> Result<String, LoweringErrors> {
    lower_compiler_module_to_llvm_ir_for_target(
        owner.module(),
        LoweringTarget::Gfx942XnackMinusV1,
        None,
        Some(SemanticAnchorInputV1::Native(owner)),
        true,
    )
}

/// Gfx950 counterpart of the actual-owner V12 compiler-module entry point.
pub fn lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
) -> Result<String, LoweringErrors> {
    lower_compiler_module_to_llvm_ir_for_target(
        owner.module(),
        LoweringTarget::Gfx950XnackMinusV1,
        None,
        Some(SemanticAnchorInputV1::Native(owner)),
        true,
    )
}

#[cfg(test)]
mod native_v12_identity_tests {
    use super::*;
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work, LaunchDomain, LaunchExtent, Signature,
        VerifiedCanonicalKernelIrModuleV12 as Owner,
    };

    #[test]
    fn native_anchor_validation_rejects_a_foreign_same_bytes_owner() {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("native_owner_identity");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            "kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        ));
        let mut work = Work::new(1_000_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000_000);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(29).unwrap();
        let (first, first_storage) =
            Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
        budget
            .reserve_storage(first_storage.retained_storage())
            .unwrap();
        let (second, second_storage) =
            Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
        budget
            .reserve_storage(second_storage.retained_storage())
            .unwrap();
        assert_eq!(
            first.canonical().canonical_bytes(),
            second.canonical().canonical_bytes()
        );
        let floor = budget.storage();
        let before = budget.work();
        assert!(
            SemanticAnchorInputV1::Native(&first)
                .validate(second.module())
                .unwrap_err()
                .contains(LoweringDiagnosticCode::SemanticAnchorIdentityMismatch)
        );
        assert_eq!(
            SemanticAnchorInputV1::Native(&first)
                .validate(first.module())
                .unwrap(),
            ProductionSemanticAnchorKirIdentityV1::from_v12(&first)
        );
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), before);
        assert_eq!(budget.failed_storage(), None);
        drop(second);
        budget
            .release_storage(second_storage.retained_storage())
            .unwrap();
        drop(first);
        budget
            .release_storage(first_storage.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), 29);
        assert_eq!(work.failed_work(), None);
    }
}
