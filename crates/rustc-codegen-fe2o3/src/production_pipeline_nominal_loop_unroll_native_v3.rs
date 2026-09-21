//! Explicit nominal source-to-U-to-LLVM continuation, without worker authority.
#![allow(
    clippy::result_large_err,
    reason = "Exact typed refusals without new error allocation."
)]

use super::*;
use crate::compiler_descriptor::checked_output_policy3_v1::refined_forwarding_v1::loop_unroll_v1::nominal_v3 as descriptor;
use crate::compiler_descriptor::nominal_v3::{
    NominalDescriptorErrorV3 as E, scoped as nominal_scope,
};
use crate::production_pipeline::{CollectedRustStage, ProductionCompilation};
use fe2o3_kernel_descriptor::DeviceDescriptorTableV3;

type R<T> = std::result::Result<T, E>;

/// Unreserved addition to the incoming caller floor, including original source,
/// actual U, full F/P7 claims and V3 backing. Drop the owner before refunding it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NominalLoopUnrollNativeStorageV3(usize);
impl NominalLoopUnrollNativeStorageV3 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

/// The sole actual U owner retains original nominal Rust source, target, ranked
/// and transaction custody. F claims remain F; the separate U descriptor binds
/// final U bytes and its freshly checked geometry. Nothing is a V1 wire token.
pub(crate) struct NominalLoopUnrollNativeProductionCompilationV3 {
    native: PreparedLoopUnrollNativeOutputV1,
    history: PreparedRefinedForwardingHistoryClaimsV1,
    ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    bindings: AuthenticatedProductionBindings,
    wire: Vec<u8>,
    retained_floor: usize,
}

fn wrapper_header() -> R<usize> {
    size_of::<NominalLoopUnrollNativeProductionCompilationV3>()
        .checked_sub(size_of::<PreparedLoopUnrollNativeOutputV1>())
        .and_then(|n| n.checked_sub(size_of::<PreparedRefinedForwardingHistoryClaimsV1>()))
        .ok_or(Resource::Arithmetic.into())
}

impl RankedVerifiedProductionCompilation {
    pub(crate) fn lower_nominal_bounded_loop_unroll_native_with_budget_v3(
        self,
        refinement: Limits,
        forwarding: ForwardingLimits,
        unroll: UnrollLimits,
        budget: &mut Budget<'_>,
    ) -> R<(
        NominalLoopUnrollNativeProductionCompilationV3,
        NominalLoopUnrollNativeStorageV3,
    )> {
        let floor = budget.storage();
        nominal_scope(budget, move |budget| {
            budget.reserve_storage(wrapper_header()?)?;
            let (prefix, ranked_verification, bindings) =
                self.prepare_native_prefix_v1(budget).map_err(E::Pipeline)?;
            let (native, receipt) = prepare(
                prefix,
                bindings.rustc_target.profile(),
                refinement,
                forwarding,
                unroll,
                budget,
            )
            .map_err(E::Pipeline)?;
            budget.reserve_storage(receipt.retained_storage())?;
            let history = PreparedRefinedForwardingHistoryClaimsV1::prepare_source_v1(
                native.owner.source(),
                &native.prefix_execution,
                budget,
            )
            .map_err(E::Pipeline)?;
            budget.reserve_storage(history.retained_storage())?;
            let wire = descriptor::produce(
                native.owner.descriptor(),
                &bindings.typed_descriptor_roots,
                &bindings.rustc_target,
                budget,
            )?;
            // The Vec header is already part of wrapper_header, not a second
            // active owner. Encoding's scratch reservation has been retired.
            budget.reserve_storage(wire.capacity())?;
            let value = NominalLoopUnrollNativeProductionCompilationV3 {
                native,
                history,
                ranked_verification,
                bindings,
                wire,
                retained_floor: budget.storage(),
            };
            value.verify_equivalence(budget)?;
            let retained = budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?;
            budget.charge_work(1)?;
            Ok((value, NominalLoopUnrollNativeStorageV3(retained)))
        })
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Actual nominal rustc import and general SSA/ranked checks, then consume
    /// those owners directly. No reconstruction from a source-only ABI receipt.
    pub(crate) fn prepare_nominal_loop_unroll_native_v3(
        self,
        refinement: Limits,
        forwarding: ForwardingLimits,
        unroll: UnrollLimits,
        budget: &mut Budget<'_>,
    ) -> R<(
        NominalLoopUnrollNativeProductionCompilationV3,
        NominalLoopUnrollNativeStorageV3,
    )> {
        nominal_scope(budget, move |budget| {
            budget.charge_work(3)?;
            self.import_semantic_mir_with_nominal_v35(true)
                .map_err(E::Pipeline)?
                .construct_semantic_middle_end()
                .map_err(E::Pipeline)?
                .construct_semantic_ssa()
                .map_err(E::Pipeline)?
                .materialize_target_neutral()
                .map_err(|e| E::Pipeline(*e))?
                .verify_general_kernel_checks()
                .map_err(E::Pipeline)?
                .lower_nominal_bounded_loop_unroll_native_with_budget_v3(
                    refinement, forwarding, unroll, budget,
                )
        })
    }
}

impl NominalLoopUnrollNativeProductionCompilationV3 {
    pub(crate) fn output(&self) -> &Graph {
        self.native.output()
    }
    pub(crate) fn forwarding_output(&self) -> &Graph {
        self.native.forwarding_output()
    }
    pub(crate) fn original(&self) -> R<&Graph> {
        self.native.original().map_err(E::Pipeline)
    }
    pub(crate) fn llvm_ir(&self) -> &str {
        self.native.llvm_ir()
    }
    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.wire
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) const fn authenticates_execution(&self) -> bool {
        false
    }

    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> R<()> {
        self.with_checked_table(budget, |_, _| Ok(()))
    }

    /// The callback cannot retain this local decoded view or mint native/worker
    /// authority. All source/U/history/profile joins precede its first use.
    pub(crate) fn with_checked_table<T>(
        &self,
        budget: &mut Budget<'_>,
        use_table: impl for<'a, 'w> FnOnce(&'a DeviceDescriptorTableV3<'w>, &mut Budget<'_>) -> R<T>,
    ) -> R<T> {
        if budget.storage() < self.retained_floor {
            return Err(Resource::Accounting.into());
        }
        nominal_scope(budget, |budget| {
            if self
                .bindings
                .rustc_preflight_plan
                .rustc_identity_inventory_sha256()
                != self.bindings.rustc_identity_inventory.sha256()
            {
                return Err(E::Pipeline(ProductionPipelineError::RustcLineageMismatch));
            }
            if self.native.profile != self.bindings.rustc_target.profile()
                || self.ranked_verification.root_count() != self.output().module().kernels.len()
                || !self
                    .ranked_verification
                    .every_functional_verification_is_coherent()
            {
                return Err(E::Mismatch("complete nominal U target/ranked custody"));
            }
            self.history
                .check_source_v1(
                    self.native.owner.source(),
                    &self.native.prefix_execution,
                    budget,
                )
                .map_err(E::Pipeline)?;
            self.native
                .verify_equivalence(budget)
                .map_err(E::Pipeline)?;
            let reproduced = descriptor::produce(
                self.native.owner.descriptor(),
                &self.bindings.typed_descriptor_roots,
                &self.bindings.rustc_target,
                budget,
            )?;
            let reproduced_storage = reproduced
                .capacity()
                .checked_add(size_of::<Vec<u8>>())
                .ok_or(Resource::Arithmetic)?;
            budget.reserve_storage(reproduced_storage)?;
            budget.charge_work(
                reproduced
                    .len()
                    .checked_add(self.wire.len())
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if reproduced != self.wire {
                return Err(E::Mismatch("actual U-derived V3 bytes"));
            }
            drop(reproduced);
            budget.release_storage(reproduced_storage)?;
            let result = descriptor::check_table(
                self.native.owner.descriptor(),
                &self.wire,
                budget,
                use_table,
            )?;
            budget.charge_work(1)?;
            Ok(result)
        })
    }

    #[cfg(test)]
    pub(crate) fn source_test_selection_v3(&self) -> (bool, Option<u8>) {
        match &self.native.owner {
            Unrolled::Direct(v) => (
                false,
                v.continuation().origins().selection.map(|v| v.iterations),
            ),
            Unrolled::Erased(v) => (
                true,
                v.continuation().origins().selection.map(|v| v.iterations),
            ),
        }
    }

    #[cfg(test)]
    pub(crate) fn source_test_mutations_v3(&mut self, budget: &mut Budget<'_>) -> R<()> {
        let floor = budget.storage();
        let last = self
            .wire
            .len()
            .checked_sub(1)
            .ok_or(E::Mismatch("nonempty V3 wire"))?;
        self.wire[last] ^= 1;
        let wire_result = self.verify_equivalence(budget);
        self.wire[last] ^= 1;
        if !matches!(wire_result, Err(E::Mismatch("actual U-derived V3 bytes"))) {
            return Err(E::Mismatch("mutated actual U wire refusal"));
        }
        let original = self.native.llvm.as_bytes()[0];
        // One same-size ASCII byte mutation preserves backing and pays no new
        // capacity. No unsafe String access or duplicate native owner is used.
        self.native.llvm.replace_range(..1, "!");
        let llvm_result = self.verify_equivalence(budget);
        let restored = [original];
        self.native
            .llvm
            .replace_range(..1, std::str::from_utf8(&restored).unwrap());
        if !matches!(llvm_result, Err(E::Pipeline(_))) {
            return Err(E::Mismatch("mutated actual U LLVM refusal"));
        }
        if budget.storage() != floor {
            return Err(Resource::Accounting.into());
        }
        self.verify_equivalence(budget)
    }
}

#[cfg(test)]
pub(crate) fn test_wrapper_header_v3() -> usize {
    wrapper_header().unwrap()
}
