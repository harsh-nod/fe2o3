mod mixed_prefix_sealed_v29 {
    pub trait Sealed {}
}

/// Borrowed inspection of an actual checked nominal prefix. Only the sealed
/// source handoffs construct this view; it cannot replace their custody.
pub struct ProductionCheckedMixedPrefixViewV29<'owner> {
    owner: &'owner fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
    input_audit: &'owner [u8],
    occurrences: &'owner fe2o3_pliron::KirNeutralOccurrenceRowsV1,
    execution: ProductionMixedPrefixExecutionViewV29<'owner>,
}
impl<'owner> ProductionCheckedMixedPrefixViewV29<'owner> {
    /// Prepaid call/query/return frames for either closed nominal inspection.
    /// Shared consumers reserve this before inspecting, separately from any
    /// retained canonical owner or witness backing.
    pub fn inspection_storage_v29() -> Result<usize, ArgumentResourceV1> {
        type Call<'a, H, O> = (&'a H, &'a ArgumentBudgetV1<'a>, &'a O);
        type Historical<'a> = Call<
            'a,
            ProductionConditionalMixedPureCseOutputHandoffV26<'a, 'a>,
            fe2o3_pliron::CheckedNeutralKernelIrOwnerMixedPureCseV18,
        >;
        type Fixedpoint<'a> = Call<
            'a,
            ProductionConditionalMixedFixedpointOutputHandoffV29<'a, 'a>,
            fe2o3_pliron::CheckedNeutralKernelIrOwnerMixedFixedpointV18,
        >;
        argument_sum_v1(&[
            size_of::<Self>(),
            size_of::<ProductionMixedPrefixExecutionViewV29<'_>>(),
            size_of::<SourceOwnedResultV18<Self>>(),
            size_of::<Historical<'_>>().max(size_of::<Fixedpoint<'_>>()),
            std::mem::align_of::<Historical<'_>>().max(std::mem::align_of::<Fixedpoint<'_>>()),
            size_of::<
                SourceOwnedResultV18<&fe2o3_pliron::CheckedNeutralKernelIrOwnerMixedPureCseV18>,
            >()
            .max(size_of::<
                SourceOwnedResultV18<&fe2o3_pliron::CheckedNeutralKernelIrOwnerMixedFixedpointV18>,
            >()),
        ])
    }
    /// Actual retained canonical endpoint, not a reconstructed graph.
    pub fn owner(&self) -> &'owner fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18 {
        self.owner
    }
    /// Exact original canonical bytes retained by consuming adoption.
    pub fn input_audit_bytes(&self) -> &'owner [u8] {
        self.input_audit
    }
    /// Complete checked transition rows for this actual optimization.
    pub fn occurrences(&self) -> &'owner fe2o3_pliron::KirNeutralOccurrenceRowsV1 {
        self.occurrences
    }
    /// Full actual execution witness, including every Policy11 round.
    pub fn execution(&self) -> ProductionMixedPrefixExecutionViewV29<'owner> {
        self.execution
    }
}

/// Inspection only; neither caller-constructible nor an authority receipt.
#[derive(Clone, Copy)]
pub struct ProductionMixedPrefixExecutionViewV29<'owner> {
    policy: u16,
    schema: u16,
    bytes: &'owner [u8],
}
impl<'owner> ProductionMixedPrefixExecutionViewV29<'owner> {
    /// Original nominal policy; old receipts are never relabeled.
    pub fn policy_version(self) -> u16 {
        self.policy
    }
    /// The canonical graph schema of both closed implementations.
    pub fn graph_schema(self) -> u16 {
        self.schema
    }
    /// Unabridged canonical witness bytes, not only a terminal-round digest.
    pub fn canonical_bytes(self) -> &'owner [u8] {
        self.bytes
    }
}

/// Closed source prefix contract implemented only for actual Policy10 and
/// Policy11 handoffs. Generic consumers retain the nominal implementation.
/// This does not permit callers to supply a policy or create a prefix view.
pub trait ProductionMixedPrefixOwnerV29<
    'view,
    'source,
    R: ProductionContinuationOccurrenceV90 = ProductionMixedRuntimeOccurrenceV26,
>: mixed_prefix_sealed_v29::Sealed + 'view
{
    /// Fixed nominal policy identity.
    const POLICY_VERSION: u16;
    /// Original source owner whose live ledger owns this output.
    fn source_owned_v29(&self) -> &'view ProductionSourceOwnedViewV18<'source>;
    /// Checks the source and exact ledger custody.
    fn check_prefix_v29(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()>;
    /// Checks custody for cleanup without replacing any earlier query refusal.
    fn prefix_custody_v29(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()>;
    /// Inspects the checked output while retaining the nominal owner.
    fn checked_prefix_v29(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionCheckedMixedPrefixViewV29<'_>>;
    /// Rejoins this handoff to its original semantic owner.
    fn check_original_source(
        &self,
        source: &ProductionSemanticSsaOwnerV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()>;
    /// Replays the captured original complete argument ABI.
    fn check_original_argument_abi_v26(
        &self,
        abi: ProductionKernelArgumentAbiInputV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()>;
    /// Borrows original runtime premises with custody checked.
    fn runtime_premises(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[ProductionMixedSliceRuntimePremiseV26]>;
    /// Borrows original exact runtime occurrences with custody checked.
    fn runtime_occurrences(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<&[R]>;
    /// Borrows the source launch requirements.
    fn launch_context(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(
        &[fe2o3_kernel_ir::ExplicitLaunchExtent],
        fe2o3_kernel_ir::FormalIndexWidth,
    )>;
    /// Checks the exact retained storage floor.
    fn observe_retained_storage_v18(
        &self,
        required: usize,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()>;
}

macro_rules! mixed_prefix_owner_v29 {
    ($handoff:ident, $policy:literal) => {
        mixed_prefix_owner_v29!($handoff, $policy, ProductionMixedRuntimeOccurrenceV26);
    };
    ($handoff:ident, $policy:literal, $occurrence:ty) => {
        impl mixed_prefix_sealed_v29::Sealed for $handoff<'_, '_> {}
        impl<'view, 'source> ProductionMixedPrefixOwnerV29<'view, 'source, $occurrence>
            for $handoff<'view, 'source>
        {
            const POLICY_VERSION: u16 = $policy;
            fn source_owned_v29(&self) -> &'view ProductionSourceOwnedViewV18<'source> {
                self.owned.source
            }
            fn check_prefix_v29(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
                self.owned.check(budget)
            }
            fn prefix_custody_v29(
                &self,
                budget: &ArgumentBudgetV1<'_>,
            ) -> SourceOwnedResultV18<()> {
                self.owned.custody(budget)
            }
            fn checked_prefix_v29(
                &self,
                budget: &ArgumentBudgetV1<'_>,
            ) -> SourceOwnedResultV18<ProductionCheckedMixedPrefixViewV29<'_>> {
                let checked = self.output(budget)?;
                Ok(ProductionCheckedMixedPrefixViewV29 {
                    owner: checked.owner(),
                    input_audit: checked.input_audit_bytes(),
                    occurrences: checked.occurrences(),
                    execution: ProductionMixedPrefixExecutionViewV29 {
                        policy: checked.execution().policy_version(),
                        schema: checked.execution().graph_schema(),
                        bytes: checked.execution().canonical_bytes(),
                    },
                })
            }
            fn check_original_source(
                &self,
                source: &ProductionSemanticSsaOwnerV1,
                budget: &mut ArgumentBudgetV1<'_>,
            ) -> SourceOwnedResultV18<()> {
                self.check_original_source(source, budget)
            }
            fn check_original_argument_abi_v26(
                &self,
                abi: ProductionKernelArgumentAbiInputV18<'_>,
                budget: &mut ArgumentBudgetV1<'_>,
            ) -> SourceOwnedResultV18<()> {
                self.check_original_argument_abi_v26(abi, budget)
            }
            fn runtime_premises(
                &self,
                budget: &ArgumentBudgetV1<'_>,
            ) -> SourceOwnedResultV18<&[ProductionMixedSliceRuntimePremiseV26]> {
                self.runtime_premises(budget)
            }
            fn runtime_occurrences(
                &self,
                budget: &ArgumentBudgetV1<'_>,
            ) -> SourceOwnedResultV18<&[$occurrence]> {
                self.runtime_occurrences(budget)
            }
            fn launch_context(
                &self,
                budget: &ArgumentBudgetV1<'_>,
            ) -> SourceOwnedResultV18<(
                &[fe2o3_kernel_ir::ExplicitLaunchExtent],
                fe2o3_kernel_ir::FormalIndexWidth,
            )> {
                self.launch_context(budget)
            }
            fn observe_retained_storage_v18(
                &self,
                required: usize,
                budget: &ArgumentBudgetV1<'_>,
            ) -> SourceOwnedResultV18<()> {
                self.observe_retained_storage_v18(required, budget)
            }
        }
    };
}
mixed_prefix_owner_v29!(ProductionConditionalMixedPureCseOutputHandoffV26, 10);
mixed_prefix_owner_v29!(ProductionConditionalMixedFixedpointOutputHandoffV29, 11);
mixed_prefix_owner_v29!(
    ProductionConditionalPredicatedFixedpointOutputHandoffV89,
    11,
    ProductionMixedRuntimeOccurrenceV89
);

include!("production_source_predicated_continuation_v90.rs");

#[cfg(test)]
mod mixed_prefix_frame_tests_v29 {
    use super::*;
    #[test]
    fn mixed_prefix_inspection_frames_cover_closed_call_query_and_return_envelopes() {
        type ExecutionFields<'a> = (u16, u16, &'a [u8]);
        type ViewFields<'a> = (
            &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
            &'a [u8],
            &'a fe2o3_pliron::KirNeutralOccurrenceRowsV1,
            ExecutionFields<'a>,
        );
        // Receiver, budget and checked-owner local are three thin borrows.
        type CallFields<'a> = (&'a (), &'a (), &'a ());
        assert_eq!(
            size_of::<ProductionMixedPrefixExecutionViewV29<'_>>(),
            size_of::<ExecutionFields<'_>>()
        );
        assert_eq!(
            size_of::<ProductionCheckedMixedPrefixViewV29<'_>>(),
            size_of::<ViewFields<'_>>()
        );
        assert_eq!(
            size_of::<SourceOwnedResultV18<ProductionCheckedMixedPrefixViewV29<'_>>>(),
            size_of::<SourceOwnedResultV18<ViewFields<'_>>>()
        );
        for actual in [
            size_of::<
                SourceOwnedResultV18<&fe2o3_pliron::CheckedNeutralKernelIrOwnerMixedPureCseV18>,
            >(),
            size_of::<
                SourceOwnedResultV18<&fe2o3_pliron::CheckedNeutralKernelIrOwnerMixedFixedpointV18>,
            >(),
        ] {
            assert_eq!(actual, size_of::<SourceOwnedResultV18<&()>>());
        }
        let expected = size_of::<ViewFields<'_>>()
            + size_of::<ExecutionFields<'_>>()
            + size_of::<SourceOwnedResultV18<ViewFields<'_>>>()
            + size_of::<CallFields<'_>>()
            + std::mem::align_of::<CallFields<'_>>()
            + size_of::<SourceOwnedResultV18<&()>>();
        assert_eq!(
            ProductionCheckedMixedPrefixViewV29::inspection_storage_v29().unwrap(),
            expected
        );
    }
}
