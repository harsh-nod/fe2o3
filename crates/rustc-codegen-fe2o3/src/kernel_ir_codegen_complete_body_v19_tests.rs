//! Synthetic canonical-only retainer controls, not live source or worker evidence.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    VerifiedCanonicalKernelIrModuleV19 as Owner,
};
#[path = "kernel_ir_codegen_complete_body_v19_fixtures.rs"]
mod fixtures;

fn owner(module: &Module) -> Owner {
    let mut work = Work::new(4_000_000);
    let mut budget = Budget::new(&mut work, 4_000_000);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v19(module, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    owner
}
fn emission(owner: &Owner) -> dialect_amdgcn::Gfx942CompleteBodyCanonicalEmissionV19 {
    let mut work = Work::new(4_000_000);
    let mut budget = Budget::new(&mut work, 4_000_000);
    let (emission, receipt) =
        dialect_amdgcn::lower_canonical_v19_compiler_module_to_gfx942_xnack_minus_llvm_ir(
            owner,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    emission
}
#[test]
fn exact_v19_relation_demotes_to_inert_text_without_changing_llvm_or_symbol_closure() {
    for module in [fixtures::single(), fixtures::diamond()] {
        let owner = owner(&module);
        let emission = emission(&owner);
        let expected = emission.llvm_ir().to_owned();
        let retained =
            retain_verified_complete_body_compiler_module_text_v19(&owner, emission).unwrap();
        assert_eq!(retained.llvm_ir(), expected);
        assert_eq!(retained.kernel_entries(), ["synthetic_entry"]);
        assert!(retained.internal_helpers().is_empty());
        assert!(retained.device_ffi_exports().is_empty());
        assert!(retained.external_declarations().is_empty());
        assert!(retained.descriptor_source_identity().is_none());
    }
}
#[test]
fn identical_llvm_with_different_canonical_identity_cannot_be_rebound() {
    let module = fixtures::single();
    let first = owner(&module);
    let mut other = module;
    let fe2o3_kernel_ir::OperationKind::Gfx942CompleteBodyDeclaration(declaration) =
        &mut other.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
    else {
        panic!("fixture declaration");
    };
    declaration.origin.raw_block += 1;
    let second = owner(&other);
    assert_ne!(first.identity(), second.identity());
    let emitted = emission(&first);
    assert_eq!(emitted.llvm_ir(), emission(&second).llvm_ir());
    assert_eq!(
        retain_verified_complete_body_compiler_module_text_v19(&second, emitted).unwrap_err(),
        CompilerModuleConstructionError::CompleteBodyIdentityMismatch,
    );
}
#[test]
fn distinct_logical_kernel_name_cannot_claim_the_emitted_entry_symbol() {
    let mut module = fixtures::single();
    module.kernels[0].id = fe2o3_kernel_ir::Kernel::new(
        "other_kernel",
        "synthetic_entry",
        fe2o3_kernel_ir::LaunchDomain::D1 {
            x: fe2o3_kernel_ir::LaunchExtent::Dynamic,
        },
    )
    .id;
    let owner = owner(&module);
    let emitted = emission(&owner);
    assert_eq!(
        retain_verified_complete_body_compiler_module_text_v19(&owner, emitted).unwrap_err(),
        CompilerModuleConstructionError::CompleteBodyIdentityMismatch,
    );
}
