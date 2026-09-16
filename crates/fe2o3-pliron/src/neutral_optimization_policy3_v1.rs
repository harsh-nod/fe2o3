//! Explicit policy-3 native continuation. Historical V1 entrypoints never
//! construct these owners, and these owners do not authorize code generation.

use super::{
    KirCheckedNeutralOptimizationErrorV1, KirCheckedNeutralOptimizationStorageV1,
    KirNeutralOccurrenceRowsV1, KirNeutralOptimizationErrorV1 as Error,
    KirNeutralOptimizationStorageV1, KirNeutralOwnedOriginStorageV1,
    checked_neutral_optimization_v1::{CheckedParts, ObservedParts, check_and_finish_parts},
};
use crate::{
    KirBridgeOptimizedReceiptV1, KirOptimizationMapPolicy3V12, PlironOptimizationReportV1,
    fixed_policy_v3::{FixedPolicy, Policy3ExecutionWitnessV1},
};
use fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{
    convert::Infallible,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

/// Actual eight-pass output with immutable execution evidence; not yet checked
/// for semantic preservation. The original B remains borrowed without copying.
pub struct KirNeutralOptimizationOutputPolicy3V1<'input> {
    parts: ObservedParts<'input, KirOptimizationMapPolicy3V12, Policy3ExecutionWitnessV1>,
}

/// The same actual O after independent F2NTR1 local-rule checking, with distinct
/// trusted execution evidence. No V1 owner conversion or old-policy cast exists.
pub struct CheckedNeutralKernelIrOwnerPolicy3V1 {
    parts: CheckedParts<KirOptimizationMapPolicy3V12, Policy3ExecutionWitnessV1>,
}

fn wrapper_remainder<T>() -> Option<usize> {
    let mut bytes = size_of::<T>();
    for header in [
        size_of::<Owner>(),
        size_of::<PlironOptimizationReportV1>(),
        size_of::<KirBridgeOptimizedReceiptV1>(),
        size_of::<KirOptimizationMapPolicy3V12>(),
        size_of::<KirNeutralOccurrenceRowsV1>(),
    ] {
        bytes = bytes.checked_sub(header)?;
    }
    // The execution record is intentionally included: it is not separately
    // charged by any component receipt and stays owned through checking.
    Some(bytes)
}
fn observed_wrapper() -> Option<usize> {
    wrapper_remainder::<KirNeutralOptimizationOutputPolicy3V1<'_>>()
}
fn checked_wrapper() -> Option<usize> {
    wrapper_remainder::<CheckedNeutralKernelIrOwnerPolicy3V1>()
}

impl<'input> KirNeutralOptimizationOutputPolicy3V1<'input> {
    pub const fn input(&self) -> &'input Owner {
        self.parts.input
    }
    pub const fn owner(&self) -> &Owner {
        &self.parts.owner
    }
    pub const fn report(&self) -> &PlironOptimizationReportV1 {
        &self.parts.report
    }
    pub const fn map(&self) -> &KirOptimizationMapPolicy3V12 {
        &self.parts.map
    }
    pub const fn occurrences(&self) -> &KirNeutralOccurrenceRowsV1 {
        &self.parts.occurrences
    }
    pub const fn execution(&self) -> &Policy3ExecutionWitnessV1 {
        &self.parts.extra
    }
    pub const fn storage(&self) -> KirNeutralOptimizationStorageV1 {
        self.parts.storage
    }

    /// Consumes the observed receipt. Success transfers a checked owner receipt;
    /// all failures drop rejected custody before restoring the caller floor.
    pub fn try_check_and_finish_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<CheckedNeutralKernelIrOwnerPolicy3V1, KirCheckedNeutralOptimizationErrorV1> {
        self.try_check_and_finish_with_v1(budget, |_, _| Ok::<_, Infallible>(((), 0)))
            .map(|(owner, (), _)| owner)
    }
    /// The owned callback result cannot retain the temporary checked view.
    ///
    /// ```compile_fail
    /// use fe2o3_pliron::KirNeutralOptimizationOutputPolicy3V1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape_view(observed: KirNeutralOptimizationOutputPolicy3V1<'_>, budget: &mut Budget<'_>) {
    ///     let _ = observed.try_check_and_finish_with_v1(budget, |checked, _| {
    ///         Ok::<_, ()>((checked.output(), 0))
    ///     });
    /// }
    /// ```
    ///
    /// Capturing the borrowed original input does not bypass that boundary.
    ///
    /// ```compile_fail
    /// use fe2o3_pliron::KirNeutralOptimizationOutputPolicy3V1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape_input(observed: KirNeutralOptimizationOutputPolicy3V1<'_>, budget: &mut Budget<'_>) {
    ///     let input = observed.input();
    ///     let _ = observed.try_check_and_finish_with_v1(budget, |_, _| {
    ///         Ok::<_, ()>((input, 0))
    ///     });
    /// }
    /// ```
    pub fn try_check_and_finish_with_v1<T: 'static, E: 'static, F>(
        self,
        budget: &mut Budget<'_>,
        origins: F,
    ) -> Result<
        (
            CheckedNeutralKernelIrOwnerPolicy3V1,
            T,
            KirNeutralOwnedOriginStorageV1,
        ),
        KirCheckedNeutralOptimizationErrorV1<E>,
    >
    where
        F: for<'view, 'inventory, 'graph_input, 'output, 'rows, 'work> FnOnce(
            &'view CheckedCanonicalKirTransitionV1<'inventory, 'graph_input, 'output, 'rows>,
            &mut Budget<'work>,
        ) -> Result<
            (T, usize),
            E,
        >,
    {
        check_and_finish_parts(
            self.parts,
            budget,
            origins,
            observed_wrapper,
            checked_wrapper,
            FixedPolicy::Checked3,
        )
        .map(|(parts, origin, receipt)| {
            (
                CheckedNeutralKernelIrOwnerPolicy3V1 { parts },
                origin,
                receipt,
            )
        })
    }
}

impl CheckedNeutralKernelIrOwnerPolicy3V1 {
    pub const fn owner(&self) -> &Owner {
        &self.parts.owner
    }
    pub const fn report(&self) -> &PlironOptimizationReportV1 {
        &self.parts.report
    }
    pub const fn bridge(&self) -> &KirBridgeOptimizedReceiptV1 {
        &self.parts.bridge
    }
    pub const fn map(&self) -> &KirOptimizationMapPolicy3V12 {
        &self.parts.map
    }
    pub const fn occurrences(&self) -> &KirNeutralOccurrenceRowsV1 {
        &self.parts.occurrences
    }
    pub const fn execution(&self) -> &Policy3ExecutionWitnessV1 {
        &self.parts.extra
    }
    pub fn native_input_audit_bytes(&self) -> &[u8] {
        &self.parts.input_history
    }
    pub const fn storage(&self) -> KirCheckedNeutralOptimizationStorageV1 {
        self.parts.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Executes the literal policy-3 roster on one private imported candidate and
/// extracts the actual output once. Prepaid capture/profile bounds and dynamic
/// CSE charges all use this same ledger. Reserve the returned transfer receipt
/// before further allocation; the original input remains borrowed and excluded.
pub fn optimize_native_neutral_kernel_ir_policy3_v1<'input>(
    input: &'input Owner,
    budget: &mut Budget<'_>,
) -> Result<KirNeutralOptimizationOutputPolicy3V1<'input>, Error> {
    let floor = budget.storage();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let (mut graph, witness) =
            crate::kir_bridge_v1::import_native_neutral_v1(input, budget).map_err(Error::Bridge)?;
        let limits = graph
            .neutral_occurrence_limits_v1(budget)
            .and_then(|limits| limits.for_policy3())
            .map_err(Error::Occurrences)?;
        budget.charge_work(limits.work().map_err(Error::Occurrences)?)?;
        budget.reserve_storage(limits.storage().map_err(Error::Occurrences)?)?;
        let capture = graph
            .begin_neutral_occurrence_capture_for_policy_v1(limits, FixedPolicy::Checked3)
            .map_err(Error::Occurrences)?;
        let execution = graph.execute_native_policy3_v1(budget, &capture, limits);
        let execution = match execution {
            Err(error @ crate::PlironOptimizationErrorV12::Resources(_)) => {
                return Err(Error::Execution(error));
            }
            other => other,
        };
        // A nonpanicking observer failure cannot escape as a partial capture.
        if let Some(error) = capture.failure() {
            return Err(Error::Occurrences(error));
        }
        let (report, profile, cse_work) = execution.map_err(Error::Execution)?;
        budget.reserve_storage(profile.retained_storage())?;
        let (owner, bridge, map, extracted) = graph
            .extract_admitted_canonical_with_policy3_map_v1(budget, &witness)
            .map_err(Error::Extraction)?;
        budget.reserve_storage(extracted.retained_storage())?;
        let scratch = limits.storage().map_err(Error::Occurrences)?;
        budget.reserve_storage(scratch)?;
        let roster = capture
            .with_roster_meter(|meter| graph.neutral_live_roster_v1(limits.nodes, meter))
            .map_err(Error::Occurrences)?;
        let occurrences = capture
            .finish_policy3(&graph.session.context, &roster, &map, owner.module())
            .map_err(Error::Occurrences)?;
        let rows_storage = occurrences.retained_storage().map_err(Error::Occurrences)?;
        drop(roster);
        budget.release_storage(
            scratch
                .checked_sub(rows_storage)
                .ok_or(Resource::Accounting)?,
        )?;
        let wrapper = observed_wrapper().ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(wrapper)?;
        let execution = Policy3ExecutionWitnessV1::from_execution(
            input,
            &owner,
            &report,
            &map,
            crate::fixed_policy_v3::ExecutionProfileV1 {
                resources: profile,
                registered_nodes: limits.nodes,
                cse_work,
            },
            budget,
        )?;
        let retained = extracted
            .retained_storage()
            .checked_add(profile.retained_storage())
            .and_then(|n| n.checked_add(rows_storage))
            .and_then(|n| n.checked_add(wrapper))
            .ok_or(Resource::Arithmetic)?;
        let output = KirNeutralOptimizationOutputPolicy3V1 {
            parts: ObservedParts {
                input,
                owner,
                report,
                bridge,
                map,
                occurrences,
                extra: execution,
                storage: KirNeutralOptimizationStorageV1 { retained },
            },
        };
        drop(capture);
        drop(witness);
        drop(graph);
        Ok(output)
    }));
    let result = match result {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::Panicked)
        }
    };
    // Graph/session/capture and all rejected owners have dropped before release.
    if let Err(error) = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)
        .and_then(|release| budget.release_storage(release))
    {
        drop(result);
        return Err(error.into());
    }
    result
}
