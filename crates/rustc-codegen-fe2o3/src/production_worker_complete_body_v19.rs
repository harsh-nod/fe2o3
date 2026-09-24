//! Ordinary inert compiler-module preparation from the new source-owned V19
//! continuation. No protected finalizer, legacy lineage or artifact authority.
use super::*;

pub(crate) fn prepare_complete_body_worker_handoff_v19(
    authenticated: crate::production_pipeline::AuthenticatedCompleteBodyTargetModuleV19,
) -> Result<PreparedProductionWorkerHandoff, ProductionWorkerHandoffError> {
    let (checked, target, emission, typed_roots, observed_source_envelope, mut ledger) =
        authenticated.into_parts();
    // The exact source and emission reservations remain in the same owned
    // ledger through the final descriptor join. Existing compiler-module/FFI
    // encoders have their established separate allocation bounds, not RSS caps.
    ledger.with_budget(|budget| {
        let canonical = checked.executable();
        let module = canonical.module();
        if target.to_string() != "gfx942:xnack-"
            || emission.canonical_identity() != canonical.identity()
        {
            return Err(ProductionWorkerHandoffError::MissingProductionBindings);
        }
        validate_exact_target_binding(target, module)?;
        let compiler_module =
            crate::kernel_ir_codegen::retain_verified_complete_body_compiler_module_text_v19(
                canonical, emission,
            ).map_err(ProductionWorkerHandoffError::CompilerModule)?;
        let envelope = derive_production_compiler_ffi_envelope(
            target, module, &compiler_module, observed_source_envelope,
            *canonical.identity().digest(),
        )?;
        validate_exact_target_binding(envelope.target(), module)?;
        validate_envelope_module_roles(&envelope, &compiler_module)?;
        let descriptor_source =
            crate::compiler_descriptor::complete_body_v19::construct_complete_body_v19_descriptor_source(
                &envelope, &compiler_module, &typed_roots, &checked, budget,
            ).map_err(ProductionWorkerHandoffError::CompilerDescriptor)?;
        // The existing binder owns its generated descriptor globals. LLVM
        // enters that binder only via the canonical emission's consuming API;
        // no caller-selected text, generic V12 replay or old schema is accepted.
        assemble_production_worker_handoff(target, envelope, compiler_module, descriptor_source)
    })
}
