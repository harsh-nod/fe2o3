//! Closed checked-domain adapter for the shared native execution service.
//! Operation shapes are descriptive; each exact scoped domain owner remains
//! borrowed for the complete native epoch and all report queries.
use fe2o3_kernel_ir::{
    CanonicalConditionalSliceAccessV26, CanonicalConditionalSliceDomainV26 as Domain,
    CanonicalGuardedGlobalReadErrorV1 as Error,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirFunctionCoordinateV1 as Function, CanonicalKirOperationCoordinateV1 as Operation,
    CanonicalSelectedSliceAccessV30, CheckedCanonicalConditionalSliceDomainsV26 as Single,
    CheckedCanonicalSelectedSliceDomainsV30 as Selected, ExplicitLaunchExtent, FormalIndexWidth,
    ValueId, VerifiedCanonicalKernelIrModuleV18 as Owner,
};

type Result<T> = std::result::Result<T, Error>;

pub(crate) fn conditional_query_headers_v30() -> std::result::Result<usize, Resource> {
    use std::mem::size_of;
    type Query<'a> = (
        NativeConditionalDomainsV30<'a>,
        &'a mut Budget<'a>,
        Function,
        Operation,
        &'a Owner,
        Option<&'a CanonicalConditionalSliceAccessV26>,
        Option<&'a CanonicalSelectedSliceAccessV30>,
        Option<(bool, ValueId)>,
        (bool, ValueId),
        Option<(ExplicitLaunchExtent, FormalIndexWidth, usize, usize)>,
        (ExplicitLaunchExtent, FormalIndexWidth, usize, usize),
        std::ops::Range<usize>,
        [usize; 4],
    );
    size_of::<Query<'_>>()
        .checked_add(
            size_of::<Result<Query<'_>>>()
                .checked_mul(2)
                .ok_or(Resource::Arithmetic)?,
        )
        .ok_or(Resource::Arithmetic)
}

#[derive(Clone, Copy)]
pub(crate) enum NativeConditionalDomainsV30<'a> {
    Legacy(&'a Single<'a, 'a>),
    Selected(&'a Selected<'a, 'a>),
}

impl<'a> NativeConditionalDomainsV30<'a> {
    pub(crate) const fn runtime_requirements_are_discharged(self) -> bool {
        false
    }
    pub(crate) const fn grants_artifact_or_launch_authority(self) -> bool {
        false
    }
    pub(crate) fn owner(self, budget: &mut Budget<'_>) -> Result<&'a Owner> {
        match self {
            Self::Legacy(rows) => rows.owner(budget),
            Self::Selected(rows) => rows.owner(budget),
        }
    }
    pub(crate) fn refuse_retained_custody(self) -> Error {
        match self {
            Self::Legacy(rows) => rows.refuse_retained_custody(),
            Self::Selected(rows) => rows.refuse_retained_custody(),
        }
    }
    pub(crate) fn function_conditions(
        self,
        function: Function,
        budget: &mut Budget<'_>,
    ) -> Result<Option<(ExplicitLaunchExtent, FormalIndexWidth, usize, usize)>> {
        match self {
            Self::Legacy(rows) => rows.function_conditions(function, budget),
            Self::Selected(rows) => rows.function_conditions(function, budget).map(Some),
        }
    }
    pub(crate) fn access_at(
        self,
        operation: Operation,
        budget: &mut Budget<'_>,
    ) -> Result<Option<(bool, ValueId)>> {
        match self {
            Self::Legacy(rows) => {
                Ok(rows
                    .access_at(operation, budget)?
                    .map(|row| match row.domain() {
                        Domain::Read(domain) => (false, domain.pointer()),
                        Domain::Store(domain) => (true, domain.pointer()),
                    }))
            }
            Self::Selected(rows) => Ok(rows
                .access_at(operation, budget)?
                .map(|row| (row.writing(), row.pointer()))),
        }
    }
    pub(crate) fn access_count(self, budget: &mut Budget<'_>) -> Result<usize> {
        match self {
            Self::Legacy(rows) => rows.access_count(budget),
            Self::Selected(rows) => {
                let mut count = 0usize;
                for function in 0..rows.function_count(budget)? {
                    budget.charge_work(3)?;
                    let function =
                        Function(u32::try_from(function).map_err(|_| Resource::Arithmetic)?);
                    let (_, _, reads, writes) = rows.function_conditions(function, budget)?;
                    count = count
                        .checked_add(reads)
                        .and_then(|n| n.checked_add(writes))
                        .ok_or(Resource::Arithmetic)?;
                }
                Ok(count)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_native_domain_query_frames_match_independent_field_and_result_oracle() {
        use std::mem::size_of;
        // The sum is intentionally assembled from the separate live query
        // fields, not from the implementation's private Query alias.
        type Fields<'a> = (
            NativeConditionalDomainsV30<'a>,
            &'a mut Budget<'a>,
            Function,
            Operation,
            &'a Owner,
            Option<&'a CanonicalConditionalSliceAccessV26>,
            Option<&'a CanonicalSelectedSliceAccessV30>,
            Option<(bool, ValueId)>,
            (bool, ValueId),
            Option<(ExplicitLaunchExtent, FormalIndexWidth, usize, usize)>,
            (ExplicitLaunchExtent, FormalIndexWidth, usize, usize),
            std::ops::Range<usize>,
            [usize; 4],
        );
        assert_eq!(
            conditional_query_headers_v30().unwrap(),
            size_of::<Fields<'_>>() + 2 * size_of::<std::result::Result<Fields<'_>, Error>>()
        );
    }
}
