pub(super) fn observe_for_test_v18(
    source: &Function,
    values: &[ValueId],
    location: FunctionOperationLocation,
) -> Vec<(
    std::result::Result<FormalAllocationIdentity, FormalMemoryIncompleteReason>,
    std::result::Result<(FormalAllocationIdentity, ByteExpression), FormalMemoryIncompleteReason>,
)> {
    let (definitions, _) = collect_definitions(source).unwrap();
    let types = collect_types(source);
    let parameters = formal_allocations(source);
    let roots = parameters
        .iter()
        .map(|row| (row.value, row.identity))
        .collect();
    let mut reasons = BTreeSet::new();
    let slots =
        private_slots::classify_eligible_private_slots(source, &definitions, &types, &mut reasons);
    let loads = private_slots::collect_private_load_sources(source, &definitions, &types, &slots);
    let mut cache = PointerDerivationCache::default();
    values
        .iter()
        .map(|&value| {
            let allocation = derive_pointer_allocation(
                value,
                &definitions,
                &types,
                &roots,
                &loads,
                &mut cache,
                location,
            );
            let expression = derive_pointer_expression(
                value,
                &definitions,
                &types,
                &roots,
                &loads,
                &mut cache,
                location,
            )
            .map(|row| (row.allocation, row.byte_offset.into_byte_expression()));
            (allocation, expression)
        })
        .collect()
}

pub(super) fn observe_access_for_test_v18(
    source: &Function,
    rank_one: bool,
    invocations: InvocationRange1d,
    requests: &[(
        FunctionOperationLocation,
        ValueId,
        FormalMemoryAccessKind,
        MemoryAccess,
        Option<ValueId>,
        bool,
    )],
) -> Vec<std::result::Result<FormalMemoryAccess, FormalMemoryIncompleteReason>> {
    let (definitions, controls) = collect_definitions(source).unwrap();
    let types = collect_types(source);
    let parameters = formal_allocations(source);
    let roots = parameters
        .iter()
        .map(|row| (row.value, row.identity))
        .collect();
    let mut reasons = BTreeSet::new();
    let slots =
        private_slots::classify_eligible_private_slots(source, &definitions, &types, &mut reasons);
    let loads = private_slots::collect_private_load_sources(source, &definitions, &types, &slots);
    let guarded =
        controls.map(|seed| GuardedAnalysisV1::new(seed, &definitions, source, rank_one).unwrap());
    let mut context =
        AccessDerivationContext::new(&definitions, &types, &parameters, &roots, &loads, guarded);
    requests
        .iter()
        .map(
            |&(location, pointer, kind, access, predicate, conservative)| {
                if conservative {
                    derive_conservative_guarded_access(
                        location,
                        pointer,
                        access,
                        invocations,
                        &mut context,
                    )
                } else {
                    match derive_access(
                        location,
                        pointer,
                        kind,
                        access,
                        invocations,
                        predicate,
                        &mut context,
                    ) {
                        Ok(value) => Ok(value),
                        Err(AccessDerivationError::Incomplete(reason)) => Err(reason),
                        Err(AccessDerivationError::Resource(error)) => {
                            panic!("reference access resource failure: {error:?}")
                        }
                    }
                }
            },
        )
        .collect()
}
