//! Fresh source-owned recipe application; child of existing Linux eligibility.
//! The normal adapter never accepts an owner, receipt, pass list or callback.
use super::local_order_join as join;
use super::*;
use crate::collector::source_census_v1::bitselect_feasibility::retained::local_order::capture_local_order;
use crate::source_local_order_recipe_api_v1::{
    self as api, Intent, SourceLocalOrderIdentityObservationV1 as Identity,
    SourceLocalOrderRecipeEvidenceV1 as Evidence, SourceLocalOrderRecipeFailurePhaseV1 as Phase,
    SourceLocalOrderRecipeFailureV1 as Failure, SourceLocalOrderRecipeOutputV1 as Output,
    SourceLocalOrderRecipeRequestV1 as Request, SourceLocalOrderSourceBindingModeV1 as SourceMode,
};
use crate::source_local_order_recipe_v1 as codec;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirOperationCoordinateV1, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_lower_mir_kernel::SourceU32LocalOrderRequestV1;

#[path = "source_local_order_recipe_relation_v1.rs"]
mod relation;

fn eligibility(message: String) -> Failure {
    Failure::new(Phase::Eligibility, message)
}
fn continuation(error: impl std::fmt::Display) -> Failure {
    api::failure(Phase::Continuation, error)
}
fn currentness(message: String) -> Failure {
    Failure::new(Phase::SourceCurrentness, message)
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn compile_source_local_order_recipe_v1(
        self,
        mut input: RetainedInput,
        request: &Request,
    ) -> Result<Output, Failure> {
        let tcx = self.stage.tcx;
        let mut captured =
            capture_local_order(tcx, &self.stage.closure, &input).map_err(eligibility)?;
        captured
            .meter
            .storage(request.retained_input_storage())
            .map_err(eligibility)?;
        api::require_current_source_revision(
            request.expected_current_source_sha256(),
            &captured.original_sha256,
        )?;
        let identities = captured.identities;
        let binding = codec::InstanceBinding {
            function: *identities.function().as_bytes(),
            item: *identities.item_definition().as_bytes(),
            monomorphization: *identities.monomorphization().as_bytes(),
            generic_types: *identities.generic_type_arguments().as_bytes(),
            const_arguments: *identities.const_generic_arguments().as_bytes(),
        };
        let preference = match request.intent() {
            Intent::Create { preference, .. } => *preference,
            Intent::Replay { recipe, .. } => recipe
                .bind(binding, captured.original_sha256)
                .map_err(|error| api::failure(Phase::RecipeBinding, error))?,
        };
        let ranked = self.verify_general_kernel_checks().map_err(continuation)?;
        let mut work = Work::new(
            usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
                .map_err(continuation)?,
        );
        let mut budget = Budget::new(
            &mut work,
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        );
        budget
            .reserve_storage(request.retained_input_storage())
            .map_err(continuation)?;
        // A Create recipe lives on this callback stack; reserve its fixed header
        // before constructing it. Replay borrows the request's already-owned copy.
        budget
            .reserve_storage(std::mem::size_of::<Option<codec::Recipe>>())
            .map_err(continuation)?;
        let prefix = ranked
            .prepare_source_local_order_prefix_v1(&mut budget)
            .map_err(continuation)?;
        captured.recheck(tcx, &mut input).map_err(currentness)?;
        let joined = join::exact_join(&mut captured, prefix.admitted.source_semantic_kir())
            .map_err(eligibility)?;
        let original = prefix
            .admitted
            .source_semantic_kir()
            .pre_ranked_executable()
            .ok_or_else(|| eligibility("local-order recipe actual original owner absent".into()))?;
        let selected = joined
            .bound_region(original, &mut captured.meter)
            .map_err(eligibility)?;
        let n = *original.canonical().identity();
        let i = *prefix.admitted.output().canonical().identity();
        let semantic_sha256 = *prefix
            .admitted
            .source_semantic_kir()
            .semantic()
            .semantic()
            .semantic_sha256()
            .as_bytes();
        let origin = codec::Origin {
            source: captured.original_sha256,
            semantic: semantic_sha256,
            bound: *prefix.admitted.bound().canonical().identity().digest(),
        };
        let created = match request.intent() {
            Intent::Create {
                constraint,
                source_binding,
                ..
            } => Some(codec::Recipe::new(
                binding,
                origin,
                preference,
                *constraint,
                match source_binding {
                    SourceMode::ExactRevision => codec::SourceBinding::ExactRevision {
                        expected_source_sha256: captured.original_sha256,
                        expected_original: codec::ProgramIdentity::from_verified(&n),
                        expected_prefix: codec::ProgramIdentity::from_verified(&i),
                    },
                    SourceMode::RebindCurrent => codec::SourceBinding::RebindCurrent {},
                },
            )),
            Intent::Replay { .. } => None,
        };
        let recipe = match (&created, request.intent()) {
            (Some(recipe), Intent::Create { .. }) | (None, Intent::Replay { recipe, .. }) => recipe,
            _ => {
                return Err(eligibility(
                    "local-order recipe intent custody mismatch".into(),
                ));
            }
        };
        recipe
            .bind(binding, captured.original_sha256)
            .and_then(|_| recipe.check_current_program(&n, &i))
            .map_err(|error| api::failure(Phase::RecipeBinding, error))?;
        let first = selected.first_operation;
        let operations = [0_u32, 1, 2].map(|offset| {
            first
                .checked_add(offset)
                .map(|operation| CanonicalKirOperationCoordinateV1 {
                    block: selected.block,
                    operation,
                })
        });
        let [Some(xor), Some(or), Some(and)] = operations else {
            return Err(eligibility(
                "local-order recipe source coordinate overflow".into(),
            ));
        };
        let continued = crate::production_pipeline::source_local_order_v1::continue_admitted_source_local_order_v1(
            prefix, SourceU32LocalOrderRequestV1 {
                expected_source: n, operations: [xor, or, and], preference: preference.preference(),
            }, &mut budget,
        ).map_err(continuation)?;
        let floor = budget.storage();
        continued
            .verify_equivalence(&mut budget)
            .map_err(continuation)?;
        if budget.storage() != floor {
            return Err(continuation(
                "local-order recipe replay changed storage floor",
            ));
        }
        let (actual_relation, results) = relation::observe(
            continued.input(),
            continued.output(),
            continued.region(),
            continued.transition_receipt(),
            &mut budget,
        )
        .map_err(continuation)?;
        let outcome = recipe
            .evaluate_constraint(actual_relation)
            .map_err(|error| api::failure(Phase::Constraint, error))?;
        let descriptor = continued.descriptor_source();
        if descriptor.table().producer().version().as_str() != api::DESCRIPTOR_PRODUCER {
            return Err(continuation(
                "local-order recipe descriptor producer mismatch",
            ));
        }
        let llvm = continued.llvm_ir().map_err(continuation)?;
        if llvm.is_empty() || llvm.len() > api::LLVM_BYTE_CAP {
            return Err(continuation("local-order recipe LLVM observation bound"));
        }
        let [formal] = continued.admitted().kernels() else {
            return Err(continuation(
                "local-order recipe expected one fresh formal report",
            ));
        };
        let receipt = continued.transition_receipt();
        let rows = receipt.candidate();
        let transition_rows = [
            rows.functions.len(),
            rows.blocks.len(),
            rows.segments.len(),
            rows.operations.len(),
            rows.definitions.len(),
            rows.definition_outputs.len(),
            rows.uses.len(),
            rows.edges.len(),
            rows.edge_arguments.len(),
        ];
        let source_binding_mode = match recipe.source_binding() {
            codec::SourceBinding::ExactRevision { .. } => SourceMode::ExactRevision,
            codec::SourceBinding::RebindCurrent {} => SourceMode::RebindCurrent,
        };
        // Fixed-size evidence + bounded output copies are charged before those
        // copies. Actual capacity excess, if any, is charged before retaining it.
        budget
            .reserve_storage(std::mem::size_of::<Output>())
            .map_err(continuation)?;
        budget
            .charge_work(
                llvm.len()
                    .checked_add(256 + 128)
                    .ok_or_else(|| continuation("local-order recipe output work overflow"))?,
            )
            .map_err(continuation)?;
        let mut created_recipe = None;
        let recipe_sha256;
        if request.is_create() {
            budget
                .reserve_storage(codec::BYTE_CAP)
                .map_err(continuation)?;
            budget
                .charge_work(codec::BYTE_CAP * 2)
                .map_err(continuation)?;
            let bytes = recipe.encode().map_err(continuation)?;
            if bytes.capacity() > codec::BYTE_CAP {
                budget
                    .reserve_storage(bytes.capacity() - codec::BYTE_CAP)
                    .map_err(continuation)?;
            }
            recipe_sha256 = Sha256::digest(&bytes).into();
            created_recipe = Some(bytes);
        } else {
            let bytes = request
                .replay_recipe_bytes()
                .ok_or_else(|| continuation("local-order replay bytes unavailable"))?;
            budget.charge_work(bytes.len()).map_err(continuation)?;
            recipe_sha256 = Sha256::digest(bytes).into();
        }
        budget.reserve_storage(llvm.len()).map_err(continuation)?;
        budget.charge_work(llvm.len()).map_err(continuation)?;
        let llvm_bytes =
            api::copy_bytes(llvm.as_bytes(), api::LLVM_BYTE_CAP).map_err(continuation)?;
        if llvm_bytes.capacity() > llvm.len() {
            budget
                .reserve_storage(llvm_bytes.capacity() - llvm.len())
                .map_err(continuation)?;
        }
        let llvm = String::from_utf8(llvm_bytes).map_err(continuation)?;
        let region = continued.region();
        let evidence = Evidence {
            source_sha256: captured.original_sha256,
            source_initializer: [
                captured.initializer.normalized_start,
                captured.initializer.normalized_end,
                captured.initializer.original_start,
                captured.initializer.original_end,
            ],
            semantic_sha256,
            instance_axes: [
                binding.function,
                binding.item,
                binding.monomorphization,
                binding.generic_types,
                binding.const_arguments,
            ],
            original: Identity::from_current(&n),
            input: Identity::from_current(&i),
            output: Identity::from_current(continued.output().canonical().identity()),
            requested_order: recipe.preference().into(),
            requested_relation: recipe.constraint().relation.into(),
            strength: recipe.constraint().strength.into(),
            source_binding_mode,
            actual_relation: actual_relation.into(),
            constraint_outcome: outcome.into(),
            region: [
                region.block.function.0,
                region.block.block,
                region.first_operation,
                region.operation_count,
            ],
            output_result_order: results,
            prefix_execution_bytes: *continued
                .admitted()
                .prefix()
                .checked_output()
                .execution()
                .canonical_bytes(),
            transition_sha256: *receipt.digest(),
            transition_bytes: receipt.canonical_bytes().len(),
            transition_rows,
            fresh_formal_counts: [
                formal.allocations().len(),
                formal.accesses().len(),
                formal.bounds_requirements().len(),
                formal.runtime_alias_requirements().len(),
                formal.inter_invocation_conflicts().len(),
            ],
            llvm_sha256: Sha256::digest(llvm.as_bytes()).into(),
            descriptor_sha256: *descriptor.identity().sha256(),
            recipe_sha256,
            canonical_work: budget.work(),
            canonical_peak_storage: budget.peak_storage(),
            created: request.is_create(),
        };
        #[cfg(test)]
        api::test_support::observe_current(continued.output(), &evidence)
            .map_err(|message| Failure::new(Phase::Observation, message))?;
        // This is after the optional test observer. Mutation/failure there cannot
        // expose a previous successful source result or unchecked output text.
        captured.recheck(tcx, &mut input).map_err(currentness)?;
        // Live source witness, exact I/L owners and original bindings stay on
        // this stack until final currentness and bounded inert conversion finish.
        Ok(Output {
            llvm,
            created_recipe,
            evidence,
        })
    }
}
