//! Startup accounting only; no source, account or Worker admission.
use super::*;
use crate::ConditionalWorkerOperationQuoteV5 as Operation;
use fe2o3_artifact_transaction::CompilerModuleHandoffCustodyQuotaV5 as Custody;
use fe2o3_compiler_ffi::MAX_INERT_REFINED_FORWARDING_STORAGE_V1 as INPUT_CEILING;

/// Finite maxima for the actual original-account preflight/execution adapters.
/// The caller separately funds source recovery, consumption, finalization and
/// persistence. Quotes may exceed an operation's unchanged local ceiling; they
/// neither guarantee admission of maximum-sized inputs nor raise that ceiling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalFirstBuildWorkerStartupQuoteV2 {
    preflight: Operation,
    execution: Operation,
    prepared_retained: usize,
    evidence_additional_retained: usize,
}

impl ConditionalFirstBuildWorkerStartupQuoteV2 {
    /// `configuration_storage` must be the actual configuration's published
    /// retained quote, including spare capacities. The enclosing native owner
    /// must validate output/producer against `custody`'s bounded input shape.
    /// No caller-supplied work limit, account or synthetic source is accepted.
    pub fn for_original_account(
        configuration_storage: usize,
        custody: Custody,
    ) -> Result<Self, Error> {
        let engine = Quote::admitted_limits_upper_bound()
            .map_err(|e| failure("startup engine resources", e))?;
        let binding = Binding::original_operation_upper_bound()?;
        // A successful local revalidation can retain no more than the existing
        // input ceiling. A larger owner refuses before entering its body. The
        // filesystem cost still comes from the existing bounded custody plan.
        let ordinary = custody.currentness_revalidation_quota();
        let current = Operation::new(
            sum(&[ordinary.work(), Budget::STORAGE_WINDOW_WORK_V1])?,
            sum(&[
                ordinary.scratch(),
                INPUT_CEILING,
                Budget::STORAGE_WINDOW_SCRATCH_V1,
            ])?,
        );
        let prepared_retained = sum(&[
            engine.preflight_storage(),
            size_of::<Prepared>(),
            configuration_storage,
        ])?;
        let evidence_additional_retained = sum(&[
            engine.returned_retained_storage(),
            size_of::<Evidence>(),
            configuration_storage,
        ])?;
        let preflight_inner_work = sum(&[
            8,                                                    // original account capture
            8 + crate::MAX_LINK_INPUTS + crate::MAX_LINK_OPTIONS, // configuration census
            ENTRY_WORK,
            current.work().checked_mul(2).ok_or(Resource::Arithmetic)?,
            binding.work(),
            crate::native_worker_engine::PREFLIGHT_ENTRY_WORK,
            engine.preflight_work(),
        ])?;
        // After engine preparation its returned reservation coexists with the
        // second actual currentness check. Earlier binding/engine scopes have
        // been restored, so use max only across these nonoverlapping scopes.
        let preflight_peak = binding
            .additional_storage()
            .max(sum(&[prepared_retained, current.additional_storage()])?)
            .max(sum(&[engine.preflight_storage(), size_of::<Prepared>()])?);
        let preflight =
            AccountMode::original_operation_quote(sum(&[INPUT_CEILING, configuration_storage])?)?
                .nested(Operation::new(
                preflight_inner_work,
                sum(&[FRAME, preflight_peak])?,
            ))?;

        // Source and prepared owners stay prepaid through execution. Worker
        // metadata is bounded by its share of the actual configuration quote.
        let execution_inputs = sum(&[INPUT_CEILING, prepared_retained, configuration_storage])?;
        let execution =
            AccountMode::original_operation_quote(execution_inputs)?.nested(Operation::new(
                sum(&[ENTRY_WORK, binding.work(), engine.execution_work()])?,
                sum(&[
                    FRAME,
                    binding
                        .additional_storage()
                        .max(sum(&[engine.execution_storage(), size_of::<Evidence>()])?),
                ])?,
            ))?;
        Ok(Self {
            preflight,
            execution,
            prepared_retained,
            evidence_additional_retained,
        })
    }
    pub const fn preflight(self) -> Operation {
        self.preflight
    }
    pub const fn execution(self) -> Operation {
        self.execution
    }
    pub const fn prepared_retained_storage(self) -> usize {
        self.prepared_retained
    }
    pub const fn evidence_additional_retained_storage(self) -> usize {
        self.evidence_additional_retained
    }
}

fn sum(values: &[usize]) -> Result<usize, Resource> {
    values
        .iter()
        .try_fold(0usize, |n, v| n.checked_add(*v).ok_or(Resource::Arithmetic))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_adapters_include_engine_and_live_configuration_without_wrapping() {
        let custody =
            fe2o3_artifact_transaction::compiler_module_handoff_custody_quota_for_limit_v5(1024)
                .unwrap();
        let small =
            ConditionalFirstBuildWorkerStartupQuoteV2::for_original_account(97, custody).unwrap();
        let large =
            ConditionalFirstBuildWorkerStartupQuoteV2::for_original_account(98, custody).unwrap();
        let engine = Quote::admitted_limits_upper_bound().unwrap();
        assert!(small.preflight().work() > engine.preflight_work());
        assert!(small.execution().work() > engine.execution_work());
        assert!(small.preflight().additional_storage() > engine.preflight_storage());
        assert!(small.execution().additional_storage() > engine.execution_storage());
        assert_eq!(
            large.prepared_retained_storage(),
            small.prepared_retained_storage() + 1
        );
        assert_eq!(
            large.evidence_additional_retained_storage(),
            small.evidence_additional_retained_storage() + 1
        );
        assert_eq!(
            large.preflight().additional_storage(),
            small.preflight().additional_storage() + 2
        );
        assert_eq!(
            large.execution().additional_storage(),
            small.execution().additional_storage() + 2
        );
        assert!(matches!(
            ConditionalFirstBuildWorkerStartupQuoteV2::for_original_account(usize::MAX, custody),
            Err(Error::Resource(Resource::Arithmetic))
        ));
    }
}
