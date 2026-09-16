#[derive(Debug)]
pub(crate) enum CheckedOutputMemoryTargetErrorV1 {
    Resource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1),
    Canonical(fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12),
    Optimization(fe2o3_pliron::KirNeutralOptimizationErrorV1),
    Transition(fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1),
    Join(crate::production_ranked_projection_v1::CheckedOutputModuleJoinErrorV1),
    LocalRelation(crate::production_ranked_projection_v1::CheckedOutputLocalRelationErrorV1),
}

impl fmt::Display for CheckedOutputMemoryTargetErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Canonical(error) => error.fmt(f),
            Self::Optimization(error) => error.fmt(f),
            Self::Transition(error) => error.fmt(f),
            Self::Join(error) => error.fmt(f),
            Self::LocalRelation(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CheckedOutputMemoryTargetErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Canonical(error) => Some(error),
            Self::Optimization(error) => Some(error),
            Self::Transition(error) => Some(error),
            Self::Join(error) => Some(error),
            Self::LocalRelation(error) => Some(error),
        }
    }
}

fn checked_output_pipeline_resource_v1(
    error: fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1,
) -> ProductionPipelineError {
    ProductionPipelineError::CheckedOutputMemoryTarget(CheckedOutputMemoryTargetErrorV1::Resource(
        error,
    ))
}

const CHECKED_OUTPUT_PIPELINE_CALLBACK_ERROR_V1: &str = "checked-output production callback failed";

/// The production stage and genuine-source tests share this one binder /
/// policy-3 execution / complete-memory continuation. It takes no alternate O.
/// The source receipt must already be retained in this same caller ledger.
/// All new B/O owners drop before their accounting returns to that floor.
/// Binder raw temporaries, projector, formal engine and native LLVM retain their
/// existing separate limits; this is not whole-compiler allocator metering.
#[allow(clippy::too_many_arguments)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "#271 historical checked-output differential endpoint"
    )
)]
pub(crate) fn with_checked_output_memory_target_v1<T>(
    materialized: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    root_inputs: &[crate::production_ranked_projection_v1::ProductionRankedRootInputV1],
    references: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    next: impl for<'formal, 'scope, 'source, 'output> FnOnce(
        &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
        &[fe2o3_lower_mir_kernel::ProductionScopedCompleteFormalMemoryV1<
            'formal,
            'scope,
            'source,
            'output,
        >],
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<T, ProductionPipelineError>,
) -> Result<T, ProductionPipelineError> {
    use crate::production_ranked_projection_v1::CheckedOutputModuleJoinErrorV1 as Join;
    use CheckedOutputMemoryTargetErrorV1 as Error;
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    use fe2o3_lower_mir_kernel::{
        ProductionScopedFormalMemoryErrorV1 as Formal, ProductionSourceOutputErrorV1 as Output,
    };
    let error = ProductionPipelineError::CheckedOutputMemoryTarget;
    let mut callback_error = None;
    with_checked_output_target_endpoint_v1(
        materialized,
        profile,
        budget,
        |bound, checked, budget| {
            let result =
                crate::production_ranked_projection_v1::with_projected_checked_output_module_v1(
                    materialized,
                    bound,
                    checked,
                    profile,
                    root_inputs,
                    references,
                    budget,
                    |formals, budget| match next(bound, checked, formals, budget) {
                        Ok(value) => Ok(value),
                        Err(error) => {
                            callback_error = Some(error);
                            Err(Formal::SourceOutput(Output::Invalid(
                                CHECKED_OUTPUT_PIPELINE_CALLBACK_ERROR_V1,
                            )))
                        }
                    },
                );
            match result {
                Err(Join::Formal(Formal::SourceOutput(Output::Invalid(
                    CHECKED_OUTPUT_PIPELINE_CALLBACK_ERROR_V1,
                )))) => Err(callback_error
                    .take()
                    .unwrap_or_else(|| checked_output_pipeline_resource_v1(Resource::Accounting))),
                other => other.map_err(|e| error(Error::Join(e))),
            }
        },
    )
}

fn with_checked_output_target_endpoint_v1<T>(
    materialized: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    next: impl FnOnce(
        &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<T, ProductionPipelineError>,
) -> Result<T, ProductionPipelineError> {
    use CheckedOutputMemoryTargetErrorV1 as Error;
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    let error = ProductionPipelineError::CheckedOutputMemoryTarget;
    let source_bytes = materialized
        .executable_storage()
        .retained_storage()
        .checked_add(materialized.assert_origin_storage().payload_storage())
        .ok_or_else(|| checked_output_pipeline_resource_v1(Resource::Arithmetic))?;
    let floor = budget.storage();
    if floor < source_bytes {
        return Err(checked_output_pipeline_resource_v1(Resource::Accounting));
    }
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        budget
            .charge_work(1)
            .map_err(checked_output_pipeline_resource_v1)?;
        // Existing binder raw clone has its own domain, and is dropped before
        // B is retained. Neither B nor O is cloned or optimized a second time.
        let (bound, bound_storage) = {
            let raw = dialect_amdgcn::bind_production_target_v1(
                materialized.executable().module(),
                profile,
            )
            .map_err(ProductionPipelineError::TargetBinding)?;
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::
                from_module_ref_with_verification_budget_v12(raw.module(), budget)
                .map_err(|e| error(Error::Canonical(e)))?
        };
        budget
            .reserve_storage(bound_storage.retained_storage())
            .map_err(checked_output_pipeline_resource_v1)?;
        let observed = fe2o3_pliron::optimize_native_neutral_kernel_ir_policy3_v1(&bound, budget)
            .map_err(|e| error(Error::Optimization(e)))?;
        budget
            .reserve_storage(observed.storage().retained_storage())
            .map_err(checked_output_pipeline_resource_v1)?;
        let checked = observed
            .try_check_and_finish_v1(budget)
            .map_err(|e| error(Error::Transition(e)))?;
        budget
            .reserve_storage(checked.storage().retained_storage())
            .map_err(checked_output_pipeline_resource_v1)?;
        next(&bound, &checked, budget)
    }));
    let released = budget
        .storage()
        .checked_sub(floor)
        .ok_or_else(|| checked_output_pipeline_resource_v1(Resource::Accounting))?;
    budget
        .release_storage(released)
        .map_err(checked_output_pipeline_resource_v1)?;
    match outcome {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// Local production continuation, never a final owner or transferable receipt.
/// It borrows the original transaction and the complete actual-O formal roster.
struct CheckedOutputMemoryPreparedProductionCompilation<
    'transaction,
    'formal,
    'scope,
    'source,
    'output,
> {
    source: &'transaction fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    bindings: &'transaction AuthenticatedProductionBindings,
    checked: &'transaction fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    formals: &'transaction [fe2o3_lower_mir_kernel::ProductionScopedCompleteFormalMemoryV1<
        'formal,
        'scope,
        'source,
        'output,
    >],
}

impl CheckedOutputMemoryPreparedProductionCompilation<'_, '_, '_, '_, '_> {
    fn lower_inert_target_v1(
        self,
        budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<String, ProductionPipelineError> {
        self.validate_inert_target_geometry_v1(budget)?;
        lower_checked_output_native_text_v1(self.checked, self.bindings.rustc_target.profile())
    }

    fn validate_inert_target_geometry_v1(
        &self,
        budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), ProductionPipelineError> {
        let semantic = self.source.semantic_ssa().source_semantic();
        let output = self.checked.owner();
        budget
            .charge_work(4)
            .map_err(checked_output_pipeline_resource_v1)?;
        if self.formals.len() != self.bindings.typed_descriptor_roots.len()
            || self.formals.len() != semantic.roots().len()
            || self.formals.len() != output.module().kernels.len()
        {
            return Err(ProductionPipelineError::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
            ));
        }
        for ((typed_root, source_id), formal) in self
            .bindings
            .typed_descriptor_roots
            .iter()
            .zip(semantic.roots())
            .zip(self.formals)
        {
            budget
                .charge_work(6)
                .map_err(checked_output_pipeline_resource_v1)?;
            let function = semantic.functions().get(source_id.index() as usize).ok_or(
                ProductionPipelineError::Geometry(
                    crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
                ),
            )?;
            let entry = function
                .kernel_entry()
                .ok_or(ProductionPipelineError::Geometry(
                    crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
                ))?;
            let export = entry.export_symbol().as_bytes();
            let typed_export = typed_root.entry_symbol().as_bytes();
            let actual_export = formal.kernel().id.as_str().as_bytes();
            let obligation_export = formal.obligations().kernel().as_str().as_bytes();
            let visits = export
                .len()
                .checked_mul(3)
                .and_then(|n| n.checked_add(typed_export.len()))
                .and_then(|n| n.checked_add(actual_export.len()))
                .and_then(|n| n.checked_add(obligation_export.len()))
                .and_then(|n| n.checked_add(32))
                .ok_or_else(|| {
                    checked_output_pipeline_resource_v1(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
                    )
                })?;
            budget
                .charge_work(visits)
                .map_err(checked_output_pipeline_resource_v1)?;
            if formal.selected_root() != *source_id
                || !std::ptr::eq(formal.output(), output)
                || entry.kernel_binding_identity().as_bytes() != &typed_root.kernel_binding_bytes()
                || export != typed_export
                || export != actual_export
                || export != obligation_export
            {
                return Err(ProductionPipelineError::Geometry(
                    crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
                ));
            }
            let launch = typed_root.source_launch().ok_or(ProductionPipelineError::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::NonExactDescriptorWorkgroup,
            ))?;
            // Retain the established geometry/capability engine and its own
            // scan/allocation domain. Only the new full roster join is indexed.
            crate::production_geometry_v1::derive_production_geometry_v1(
                output.module(),
                formal.kernel().id.as_str(),
                function,
                launch,
                self.bindings.rustc_target.profile().device_target(),
            )
            .map_err(ProductionPipelineError::Geometry)?;
        }
        Ok(())
    }
}

/// Native L borrows this exact owner; its result is inert LLVM text only.
pub(crate) fn lower_checked_output_native_text_v1(
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
) -> Result<String, ProductionPipelineError> {
    use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
    let text = match profile {
        Profile::Gfx942 => dialect_amdgcn::
            lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(
                checked.owner(),
            ),
        Profile::Gfx950 => dialect_amdgcn::
            lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(
                checked.owner(),
            ),
    }.map_err(ProductionPipelineError::TargetLowering)?;
    dialect_amdgcn::bind_production_llvm22_worker_layout_v1(&text)
        .map_err(ProductionPipelineError::UpstreamLlvmLayoutBinding)
}

/// Held private production continuation. Original reference and transaction
/// custody remain in bindings; this emits no worker/final/publication owner.
#[expect(dead_code, reason = "#271 source-owned checked-output attachment")]
fn with_source_checked_output_descriptor_text_v1<T>(
    bindings: &AuthenticatedProductionBindings,
    source: &fe2o3_lower_mir_kernel::ProductionBorrowedRankedCorrespondenceV1<'_>,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    formals: &[fe2o3_lower_mir_kernel::ProductionScopedCompleteFormalMemoryV1<'_, '_, '_, '_>],
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    next: impl FnOnce(
        &crate::kernel_ir_codegen::InertCompilerModuleTextV1,
        &fe2o3_compiler_ffi::CompilerDescriptorSourceV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<T, ProductionPipelineError>,
) -> Result<T, ProductionPipelineError> {
    with_checked_output_descriptor_text_v1(
        source,
        checked,
        formals,
        &bindings.typed_descriptor_roots,
        bindings.rustc_target.profile(),
        bindings.rustc_target.device_target(),
        bindings.transaction.compiler_ffi_envelope.as_ref(),
        budget,
        next,
    )
}

/// Shared inert core for the private collected-source continuation and genuine
/// source tests. There is no alternate Module or caller LLVM input. Native L,
/// descriptor encoding and LLVM buffers retain their established domains.
#[allow(clippy::too_many_arguments)]
pub(crate) fn with_checked_output_descriptor_text_v1<T>(
    source: &fe2o3_lower_mir_kernel::ProductionBorrowedRankedCorrespondenceV1<'_>,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    formals: &[fe2o3_lower_mir_kernel::ProductionScopedCompleteFormalMemoryV1<'_, '_, '_, '_>],
    typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    target: fe2o3_compiler_ffi::DeviceTargetV1,
    source_envelope: Option<&fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    next: impl FnOnce(
        &crate::kernel_ir_codegen::InertCompilerModuleTextV1,
        &fe2o3_compiler_ffi::CompilerDescriptorSourceV1,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<T, ProductionPipelineError>,
) -> Result<T, ProductionPipelineError> {
    use crate::production_worker_handoff::ProductionWorkerHandoffError as Handoff;
    let source_error = |error| {
        ProductionPipelineError::DescriptorEvidence(
            crate::compiler_descriptor::CompilerDescriptorError::CheckedOutputSource(error),
        )
    };
    let worker = ProductionPipelineError::WorkerHandoff;
    let floor = budget.storage();
    let mut callback_entered = false;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for formal in formals {
            formal
                .require_borrowed_source_v1(source, budget)
                .map_err(source_error)?;
        }
        let module = checked.owner().module();
        crate::compiler_module_contract::validate_exact_target_binding(target, module)
            .map_err(|error| worker(Handoff::from(error)))?;
        let text = lower_checked_output_native_text_v1(checked, profile)?;
        let compiler_module =
            crate::kernel_ir_codegen::retain_production_compiler_module_text_v1(module, text)
                .map_err(|error| worker(Handoff::CompilerModule(error)))?;
        let envelope =
            crate::production_worker_handoff::derive_checked_output_compiler_ffi_envelope_v1(
                target,
                module,
                &compiler_module,
                source_envelope,
                *checked.owner().canonical().identity().digest(),
            )
            .map_err(worker)?;
        crate::compiler_module_contract::validate_exact_target_binding(envelope.target(), module)
            .map_err(|error| worker(Handoff::from(error)))?;
        crate::compiler_module_contract::validate_envelope_module_roles(
            &envelope,
            &compiler_module,
        )
        .map_err(|error| worker(Handoff::from(error)))?;
        let descriptor = crate::compiler_descriptor::construct_checked_output_source_descriptor_v1(
            &envelope,
            &compiler_module,
            typed_roots,
            source,
            checked,
            formals,
            budget,
        )
        .map_err(ProductionPipelineError::DescriptorEvidence)?;
        let compiler_module = crate::kernel_ir_codegen::bind_compiler_descriptor_source_v1(
            compiler_module,
            &descriptor,
        )
        .map_err(|error| worker(Handoff::CompilerModule(error)))?;
        callback_entered = true;
        next(&compiler_module, &descriptor, budget)
    }));
    // Inert text/descriptor temporaries have dropped. Recheck the live scope
    // even on callback Err/unwind. Every postflight resource/liveness failure,
    // including Work exhaustion, takes precedence over the callback outcome.
    let live = if callback_entered {
        formals.iter().try_for_each(|formal| {
            formal
                .require_borrowed_source_v1(source, budget)
                .map_err(source_error)
        })
    } else {
        Ok(())
    };
    let released = budget.storage().checked_sub(floor).ok_or_else(|| {
        checked_output_pipeline_resource_v1(
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting,
        )
    })?;
    budget
        .release_storage(released)
        .map_err(checked_output_pipeline_resource_v1)?;
    live?;
    match outcome {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Extraction-only observation before any source-functional execution.
    /// Original references stay borrowed from the consumed transaction. The
    /// callback result is flattened only after both existing scope postflights.
    pub(crate) fn with_collected_shape_observation_v1(
        self,
        next: impl FnOnce(
            &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
            &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
            &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
            &[crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1],
            fe2o3_amd_target::ProductionAmdTargetProfileV1,
            &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        self.with_collected_shape_inputs_v1(
            |source, bound, checked, _inputs, references, profile, budget| {
                next(
                    source,
                    bound,
                    checked,
                    references.as_slice(),
                    profile,
                    budget,
                )
            },
        )
    }

    // Diagnostic-only lending of the original stage's inputs. No source proof
    // capability is constructed, and the old shape callback remains an adapter.
    pub(crate) fn with_collected_shape_inputs_v1(
        self,
        next: impl FnOnce(
            &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
            &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
            &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
            &[crate::production_ranked_projection_v1::ProductionRankedRootInputV1],
            &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
            fe2o3_amd_target::ProductionAmdTargetProfileV1,
            &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        ) -> Result<(), String>,
    ) -> Result<(), String> {
        if !self.stage.transaction.compiler_custody.is_extraction_only() {
            return Err("collected shape observation requires extraction-only custody".to_owned());
        }
        self.import_semantic_mir()
            .and_then(|stage| stage.construct_semantic_middle_end())
            .and_then(|stage| stage.construct_semantic_ssa())
            .map_err(|error| error.to_string())?
            .with_captured_materialized_target_neutral_v1(|stage, budget| {
                with_checked_output_target_endpoint_v1(
                    &stage.materialized,
                    stage.bindings.rustc_target.profile(),
                    budget,
                    |bound, checked, budget| {
                        Ok(next(
                            &stage.materialized,
                            bound,
                            checked,
                            &stage.ranked_roots,
                            &stage.bindings.reference_effect_bindings,
                            stage.bindings.rustc_target.profile(),
                            budget,
                        ))
                    },
                )
                .map_err(Box::new)
            })
            .map_err(|error| error.to_string())?
    }

    fn with_source_checked_output_transaction_v1<T>(
        self,
        next: impl for<'scope> FnOnce(
            &MaterializedNeutralProductionCompilation,
            &crate::production_ranked_projection_v1::SourceRankedCustodyV1<'scope>,
            &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
            &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
            &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        ) -> Result<T, ProductionPipelineError>,
    ) -> Result<T, ProductionPipelineError> {
        self.import_semantic_mir()?
            .construct_semantic_middle_end()?
            .construct_semantic_ssa()?
            .with_captured_materialized_target_neutral_v1(|stage, budget| {
                crate::production_ranked_projection_v1::with_source_ranked_custody_v1(
                    &stage.materialized,
                    &stage.ranked_roots,
                    &stage.bindings.reference_effect_bindings,
                    budget,
                    |source, budget| {
                        with_checked_output_target_endpoint_v1(
                            &stage.materialized,
                            stage.bindings.rustc_target.profile(),
                            budget,
                            |bound, checked, budget| next(stage, source, bound, checked, budget),
                        )
                    },
                )
                .map_err(Box::new)
            })
            .map_err(ProductionPipelineError::from)
    }

    /// Historical nondefault String hook; it retains its original checks and L.
    /// Source/O functional equivalence and final attachment are not established.
    #[expect(dead_code, reason = "#271 checked-output final attachment pending")]
    pub(crate) fn lower_checked_output_memory_target_v1(
        self,
    ) -> Result<String, ProductionPipelineError> {
        self.with_source_checked_output_transaction_v1(|stage, source, bound, checked, budget| {
            use crate::production_ranked_projection_v1::CheckedOutputModuleJoinErrorV1 as Join;
            use fe2o3_lower_mir_kernel::{
                ProductionScopedFormalMemoryErrorV1 as Formal,
                ProductionSourceOutputErrorV1 as Output,
            };
            let mut callback_error = None;
            let result =
                crate::production_ranked_projection_v1::with_source_checked_output_module_v1(
                    source,
                    bound,
                    checked,
                    stage.bindings.rustc_target.profile(),
                    budget,
                    |formals, budget| {
                        let result = CheckedOutputMemoryPreparedProductionCompilation {
                            source: &stage.materialized,
                            bindings: &stage.bindings,
                            checked,
                            formals,
                        }
                        .lower_inert_target_v1(budget);
                        result.map_err(|error| {
                            callback_error = Some(error);
                            Formal::SourceOutput(Output::Invalid(
                                CHECKED_OUTPUT_PIPELINE_CALLBACK_ERROR_V1,
                            ))
                        })
                    },
                );
            match result {
                Err(Join::Formal(Formal::SourceOutput(Output::Invalid(
                    CHECKED_OUTPUT_PIPELINE_CALLBACK_ERROR_V1,
                )))) => Err(callback_error.take().unwrap_or_else(|| {
                    checked_output_pipeline_resource_v1(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting,
                    )
                })),
                other => other.map_err(|error| {
                    ProductionPipelineError::CheckedOutputMemoryTarget(
                        CheckedOutputMemoryTargetErrorV1::Join(error),
                    )
                }),
            }
        })
    }

    /// Held local composition only. Nothing is published, transported or given
    /// authority. The callback preserves the ledger and all live reservations;
    /// side-effect copies require their own explicit custody/accounting contract.
    #[expect(dead_code, reason = "#271 local relation handoff remains nondefault")]
    pub(crate) fn with_checked_output_local_relations_v1(
        self,
        next: impl FnOnce(
            &fe2o3_kernel_opt::CheckedCanonicalPolicy3ExecutionReceiptV1<'_, '_>,
            &dialect_amdgcn::ReplayedNativeV12TextDescriptorRelationV1<'_, '_, '_, '_>,
            &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        ) -> Result<(), ProductionPipelineError>,
    ) -> Result<(), ProductionPipelineError> {
        self.with_source_checked_output_transaction_v1(|stage, source, bound, checked, budget| {
            use crate::production_ranked_projection_v1::CheckedOutputModuleJoinErrorV1 as Join;
            use fe2o3_lower_mir_kernel::{ProductionScopedFormalMemoryErrorV1 as Formal, ProductionSourceOutputErrorV1 as Output};
            let mut callback_error = None;
            let result = crate::production_ranked_projection_v1::with_source_checked_output_module_catalog_v1(
                source, bound, checked, stage.bindings.rustc_target.profile(), budget,
                |formals, input_catalog, catalog, budget| {
                    let result = (|| {
                        CheckedOutputMemoryPreparedProductionCompilation {
                            source: &stage.materialized, bindings: &stage.bindings,
                            checked, formals,
                        }.validate_inert_target_geometry_v1(budget)?;
                        crate::production_ranked_projection_v1::with_source_checked_output_local_relations_v1(
                            source, bound, checked, input_catalog, catalog, formals,
                            &stage.bindings.typed_descriptor_roots,
                            stage.bindings.rustc_target.profile(),
                            stage.bindings.rustc_target.device_target(),
                            stage.bindings.transaction.compiler_ffi_envelope.as_ref(),
                            budget, next,
                        )
                    })();
                    result.map_err(|error| {
                        callback_error = Some(error);
                        Formal::SourceOutput(Output::Invalid(CHECKED_OUTPUT_PIPELINE_CALLBACK_ERROR_V1))
                    })
                },
            );
            match result {
                Err(Join::Formal(Formal::SourceOutput(Output::Invalid(CHECKED_OUTPUT_PIPELINE_CALLBACK_ERROR_V1)))) =>
                    Err(callback_error.take().unwrap_or_else(|| checked_output_pipeline_resource_v1(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting))),
                other => other.map_err(|error| ProductionPipelineError::CheckedOutputMemoryTarget(CheckedOutputMemoryTargetErrorV1::Join(error))),
            }
        })
    }
}
