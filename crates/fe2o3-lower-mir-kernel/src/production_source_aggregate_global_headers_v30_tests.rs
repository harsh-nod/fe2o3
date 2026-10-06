pub(super) fn aggregate_global_local_header_oracle_v30() -> usize {
    use std::mem::{align_of, size_of};
    #[allow(dead_code)]
    struct Fields {
        premises: Vec<ProductionMixedSliceRuntimePremiseV26>,
        occurrences: Vec<ProductionMixedRuntimeOccurrenceV26>,
        coordinates: AggregateCoordinateTransportV30,
        premise_columns: usize,
    }
    assert_eq!(
        size_of::<Fields>(),
        size_of::<AggregateGlobalTransportV30>()
    );
    assert_eq!(
        align_of::<Fields>(),
        align_of::<AggregateGlobalTransportV30>()
    );
    type Runtime = (
        Vec<ProductionMixedSliceRuntimePremiseV26>,
        Vec<ProductionMixedRuntimeOccurrenceV26>,
    );
    type BackingHeaders = (
        Vec<SliceDefinition>,
        Vec<SliceOperation>,
        Vec<AggregateEdgeV30>,
        Vec<AggregateFunctionV30>,
        Vec<Option<(SliceOperation, SliceOperation)>>,
        Vec<fe2o3_kernel_ir::ExplicitLaunchExtent>,
        Vec<bool>,
    );
    type EndpointLocals = (
        GlobalSourceAccessPairV18,
        GlobalSourceAccessEndpointV18,
        GlobalSourceLogicalEndpointV18,
        [SliceDefinition; 9],
        ProductionMixedSliceRuntimePremiseV26,
        ProductionMixedRuntimeOccurrenceV26,
        GlobalSourceCfgGuardV85,
    );
    type ReturnFrames = (
        Result<Fields, ProductionAggregateSourceErrorV30>,
        SourceOwnedResultV18<Runtime>,
        SourceOwnedResultV18<Vec<Option<(SliceOperation, SliceOperation)>>>,
        SourceOwnedResultV18<Vec<fe2o3_kernel_ir::ExplicitLaunchExtent>>,
        SourceOwnedResultV18<GlobalSourceAccessPairV18>,
        SourceOwnedResultV18<Option<GlobalSourceAccessEndpointV18>>,
        SourceOwnedResultV18<[SliceDefinition; 9]>,
        SourceOwnedResultV18<usize>,
        SourceOwnedResultV18<()>,
        SourceOwnedResultV18<GlobalSourceCfgGuardV85>,
    );
    type QueryFrames<'a> = (
        Result<
            Option<&'a fe2o3_kernel_ir::CanonicalConditionalSliceParameterV26>,
            fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
        >,
        Result<
            Option<&'a fe2o3_kernel_ir::CanonicalConditionalSliceAccessV26>,
            fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
        >,
        Result<
            Option<(
                fe2o3_kernel_ir::ExplicitLaunchExtent,
                fe2o3_kernel_ir::FormalIndexWidth,
                usize,
                usize,
            )>,
            fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
        >,
        SourceOwnedResultV18<&'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'a>>,
        SourceOwnedResultV18<bool>,
        Result<
            fe2o3_kernel_ir::CanonicalGuardedGlobalReadOutcomeV18<'a, 'a>,
            fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
        >,
        Result<
            fe2o3_kernel_ir::CanonicalGuardedGlobalStoreOutcomeV24<'a, 'a>,
            fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
        >,
    );
    type CallFrames<'a> = (
        &'a Fields,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a AggregateSourceStageV30<'a>,
        &'a mut ArgumentBudgetV1<'a>,
        &'a [fe2o3_kernel_ir::ExplicitLaunchExtent],
        [&'a (); 12],
        [usize; 20],
        [Option<SliceDefinition>; 3],
    );
    size_of::<Fields>()
        + size_of::<Runtime>()
        + size_of::<BackingHeaders>()
        + align_of::<BackingHeaders>()
        + size_of::<EndpointLocals>()
        + align_of::<EndpointLocals>()
        + size_of::<ReturnFrames>()
        + align_of::<ReturnFrames>()
        + size_of::<QueryFrames<'_>>()
        + align_of::<QueryFrames<'_>>()
        + size_of::<CallFrames<'_>>()
        + align_of::<CallFrames<'_>>()
}
