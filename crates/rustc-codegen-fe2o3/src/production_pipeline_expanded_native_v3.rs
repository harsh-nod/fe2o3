//! One actual-source expanded final subject, constructed before first LLVM.
//! Staged target-bound integration; not a worker/default publication admission.
#![allow(clippy::result_large_err, reason = "Keep typed child errors inline.")]
use super::*;
use crate::compiler_descriptor::checked_output_policy3_v1::refined_forwarding_v1::loop_unroll_v1::expanded_v3 as descriptor;
use crate::production_pipeline::{CollectedRustStage, ProductionCompilation};
use descriptor::{E, R, pipeline, scoped};
#[cfg(test)]
use fe2o3_lower_mir_kernel::ProductionExpandedHistoryV1;
use fe2o3_lower_mir_kernel::{
    ProductionExpandedPrefixV1, ProductionOwnedExpandedContinuationV1 as Expanded,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExpandedNativeStorageV3(usize);
impl ExpandedNativeStorageV3 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Sole source/target/ranked/P7/F/U/scalar custody. The native text and reports
/// describe only owner.output(); the retained U is historical evidence.
struct ExpandedFinalNativeCustodyV3 {
    owner: Expanded,
    prefix_execution: Policy7ExecutionWitnessV1,
    history: PreparedRefinedForwardingHistoryClaimsV1,
    ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    bindings: AuthenticatedProductionBindings,
    llvm: String,
    profile: Profile,
}

/// Move-only final subject and fresh nominal V3 bytes. Numeric receipts never
/// authenticate origin, signing, compiler execution, linking or launch.
/// No mutable graph/table/native or arbitrary final-subject constructor exists.
pub(crate) struct ExpandedNativeProductionCompilationV3 {
    custody: ExpandedFinalNativeCustodyV3,
    wire: Vec<u8>,
    retained_floor: usize,
}

fn wrapper_header() -> R<usize> {
    size_of::<ExpandedNativeProductionCompilationV3>()
        .checked_sub(size_of::<Expanded>())
        .and_then(|n| n.checked_sub(size_of::<Policy7ExecutionWitnessV1>()))
        .and_then(|n| n.checked_sub(size_of::<PreparedRefinedForwardingHistoryClaimsV1>()))
        .and_then(|n| n.checked_sub(size_of::<String>()))
        .ok_or(Resource::Arithmetic.into())
}
fn final_f(owner: &Expanded) -> FinalSource<'_> {
    match owner.prefix() {
        ProductionExpandedPrefixV1::Direct(v) => FinalSource::Direct(v.prefix()),
        ProductionExpandedPrefixV1::Erased(v) => FinalSource::Erased(v.prefix()),
    }
}
#[cfg(test)]
fn unrolled(owner: &Expanded) -> &Graph {
    match owner.prefix() {
        ProductionExpandedPrefixV1::Direct(v) => v.output(),
        ProductionExpandedPrefixV1::Erased(v) => v.output(),
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Consumes the real collected transaction and uses its actual nominal
    /// import. The schedule and limits are closed, with no caller selector.
    /// Returned added storage is unreserved; pay before using the moved owner.
    pub(crate) fn prepare_expanded_native_v3(
        self,
        budget: &mut Budget<'_>,
    ) -> R<(
        ExpandedNativeProductionCompilationV3,
        ExpandedNativeStorageV3,
    )> {
        #[cfg(test)]
        emission_trace::reset();
        let floor = budget.storage();
        scoped(
            budget,
            || E::Panicked,
            move |budget| {
                let working_floor = budget.storage();
                budget.charge_work(3)?;
                let ranked = self.prepare_nominal_ranked_v3().map_err(E::Nominal)?;
                prepare_ranked(ranked, floor, working_floor, budget)
            },
        )
    }
}

fn prepare_ranked(
    ranked: RankedVerifiedProductionCompilation,
    input_floor: usize,
    working_floor: usize,
    budget: &mut Budget<'_>,
) -> R<(
    ExpandedNativeProductionCompilationV3,
    ExpandedNativeStorageV3,
)> {
    budget.reserve_storage(wrapper_header()?)?;
    let (prefix, ranked_verification, bindings) =
        ranked.prepare_native_prefix_v1(budget).map_err(pipeline)?;
    let erased = matches!(&prefix, Prefix6::Erased(_));
    let profile = bindings.rustc_target.profile();
    let (refinement, forwarding, unroll) = Expanded::prefix_limits_v1();
    let (seed, storage) =
        prepare_source_seed_v1(prefix, profile, refinement, forwarding, unroll, budget)
            .map_err(pipeline)?;
    budget.reserve_storage(storage.retained_storage())?;
    if budget.storage() < seed.retained_floor {
        return Err(Resource::Accounting.into());
    }
    #[cfg(test)]
    emission_trace::record(1);
    let history = PreparedRefinedForwardingHistoryClaimsV1::prepare_source_v1(
        seed.owner.source(),
        &seed.prefix_execution,
        budget,
    )
    .map_err(pipeline)?;
    budget.reserve_storage(history.retained_storage())?;
    let PreparedLoopUnrollSourceSeedV1 {
        owner,
        prefix_execution,
        profile,
        ..
    } = seed;
    let (owner, addition) = match owner {
        Unrolled::Direct(value) => value.continue_expanded_production_policy_v1(budget),
        Unrolled::Erased(value) => value.continue_expanded_production_policy_v1(budget),
    }
    .map_err(E::Expanded)?;
    budget.reserve_storage(addition.retained_storage())?;
    // E1 pays its own replacement of the active U header. The seed's enum,
    // profile and floor slack are dead; the new outer wrapper is already paid.
    budget.release_storage(source_seed_header(erased).map_err(pipeline)?)?;
    #[cfg(test)]
    emission_trace::record(2);
    let wire = descriptor::produce(
        &owner,
        &bindings.typed_descriptor_roots,
        &bindings.rustc_target,
        budget,
    )?;
    budget.reserve_storage(wire.capacity())?;
    #[cfg(test)]
    emission_trace::record(3);
    // No native producer is invoked above this point: source-owned U is moved
    // into E1, and the fresh descriptor has already checked the final subject.
    #[cfg(test)]
    emission_trace::record(4);
    let (llvm, storage) = lower_native(owner.output(), profile, budget).map_err(pipeline)?;
    budget.reserve_storage(storage)?;
    let retained = budget
        .storage()
        .checked_sub(working_floor)
        .ok_or(Resource::Accounting)?;
    let value = ExpandedNativeProductionCompilationV3 {
        custody: ExpandedFinalNativeCustodyV3 {
            owner,
            prefix_execution,
            history,
            ranked_verification,
            bindings,
            llvm,
            profile,
        },
        wire,
        retained_floor: input_floor
            .checked_add(retained)
            .ok_or(Resource::Arithmetic)?,
    };
    value.verify_equivalence(budget)?;
    Ok((value, ExpandedNativeStorageV3(retained)))
}

impl ExpandedFinalNativeCustodyV3 {
    fn verify(&self, wire: &[u8], floor: usize, budget: &mut Budget<'_>) -> R<()> {
        if budget.storage() < floor {
            return Err(Resource::Accounting.into());
        }
        scoped(
            budget,
            || E::Panicked,
            |budget| {
                budget.charge_work(5)?;
                if self
                    .bindings
                    .rustc_preflight_plan
                    .rustc_identity_inventory_sha256()
                    != self.bindings.rustc_identity_inventory.sha256()
                {
                    return Err(pipeline(ProductionPipelineError::RustcLineageMismatch));
                }
                if self.profile != self.bindings.rustc_target.profile()
                    || self.ranked_verification.root_count()
                        != self.owner.output().module().kernels.len()
                    || !self
                        .ranked_verification
                        .every_functional_verification_is_coherent()
                {
                    return Err(E::Mismatch("complete expanded target/ranked custody"));
                }
                let scratch = fe2o3_kernel_opt::POLICY7_EXECUTION_HEADER_BYTES_V1
                    .checked_add(size_of::<FinalSource<'_>>())
                    .ok_or(Resource::Arithmetic)?;
                budget.reserve_storage(scratch)?;
                self.prefix_execution
                    .check_history_v1(final_f(&self.owner).history(), budget)
                    .map_err(pipeline)?;
                budget.release_storage(scratch)?;
                self.history
                    .check_source_v1(final_f(&self.owner), &self.prefix_execution, budget)
                    .map_err(pipeline)?;
                // Descriptor reproduction replays this same E1 owner and its
                // final source/formal facts before reading the final view.
                let reproduced = descriptor::produce(
                    &self.owner,
                    &self.bindings.typed_descriptor_roots,
                    &self.bindings.rustc_target,
                    budget,
                )?;
                let bytes = reproduced
                    .capacity()
                    .checked_add(size_of::<Vec<u8>>())
                    .ok_or(Resource::Arithmetic)?;
                budget.reserve_storage(bytes)?;
                budget.charge_work(
                    reproduced
                        .len()
                        .checked_add(wire.len())
                        .ok_or(Resource::Arithmetic)?,
                )?;
                if reproduced.as_slice() != wire {
                    return Err(E::Mismatch("actual expanded-final V3 bytes"));
                }
                drop(reproduced);
                budget.release_storage(bytes)?;
                check_native_text_with_errors_v1(
                    self.owner.output(),
                    self.profile,
                    &self.llvm,
                    budget,
                    super::resource,
                    || super::mismatch("exact expanded-final native LLVM"),
                )
                .map_err(pipeline)?;
                Ok(())
            },
        )
    }
}

impl ExpandedNativeProductionCompilationV3 {
    pub(crate) fn output(&self) -> &Graph {
        self.custody.owner.output()
    }
    pub(crate) fn llvm_ir(&self) -> &str {
        &self.custody.llvm
    }
    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.wire
    }
    pub(crate) const fn retained_storage_floor_v3(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) const fn authenticates_execution(&self) -> bool {
        false
    }
    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> R<()> {
        self.custody.verify(&self.wire, self.retained_floor, budget)
    }
}

#[cfg(test)]
#[path = "production_pipeline_expanded_native_v3_tests.rs"]
mod tests;
#[path = "production_expanded_native_transport_v3.rs"]
pub(crate) mod transport;

#[cfg(test)]
mod emission_trace {
    std::thread_local! {
        static TRACE: std::cell::Cell<(usize, [u8; 4])> = const {
            std::cell::Cell::new((0, [0; 4]))
        };
    }
    pub(super) fn record(event: u8) {
        TRACE.with(|trace| {
            let (len, mut bytes) = trace.get();
            assert!(len < bytes.len(), "one actual expanded producer per reset");
            bytes[len] = event;
            trace.set((len + 1, bytes));
        });
    }
    pub(super) fn reset() {
        TRACE.with(|trace| trace.set((0, [0; 4])));
    }
    pub(super) fn get() -> (usize, [u8; 4]) {
        TRACE.with(|trace| trace.get())
    }
}
