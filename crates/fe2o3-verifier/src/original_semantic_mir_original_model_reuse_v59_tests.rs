#[test]
fn original_source_typed_prefix_imports_complete_exact_root_models_once() {
    for consensus in [false, true] {
        let result = run_mode(LIMIT, LIMIT, false, consensus, |text| {
            let (parent, rest) = text.split_once("mod typed_prefix_v49 {").unwrap();
            let (prefix, _) = rest.split_once("mod forwarding_v46 {").unwrap();
            let expected: std::collections::BTreeSet<_> = parent
                .split("spec fn ")
                .skip(1)
                .map(|definition| definition.split_once('(').unwrap().0)
                .filter(|name| {
                    (0..2).any(|root| {
                        *name == format!("byte_inputs_{root}_v55")
                            || ["block_step", "micro_begin", "micro_step", "micro_finish"]
                                .iter()
                                .any(|kind| *name == format!("byte_{kind}_{root}_v30"))
                            || ["operation", "block", "control"]
                                .iter()
                                .any(|kind| name.starts_with(&format!("byte_{kind}_{root}_")))
                    })
                })
                .collect();
            let mut imported = std::collections::BTreeSet::new();
            let mut targets = std::collections::BTreeSet::new();
            let mut inputs = 0;
            let mut operations = 0;
            let mut blocks = 0;
            let mut controls = 0;
            let mut dispatchers = 0;
            for line in prefix.lines() {
                let Some((source, target)) = line.trim().split_once(" as ") else {
                    continue;
                };
                if !source.starts_with("byte_") {
                    continue;
                }
                let target = target.strip_suffix(',').unwrap();
                assert!(imported.insert(source), "duplicate source alias: {source}");
                assert!(targets.insert(target), "duplicate target alias: {target}");
                assert_eq!(parent.matches(&format!("spec fn {source}(")).count(), 1);
                // Nested forwarding models have their own namespace numbering.
                assert!(!prefix.contains(&format!("spec fn {source}(")));
                assert!(!prefix.contains(&format!("spec fn {target}(")));
                if source.starts_with("byte_inputs_") {
                    inputs += 1;
                } else if source.starts_with("byte_operation_") {
                    operations += 1;
                } else if source.starts_with("byte_block_step_")
                    || source.starts_with("byte_micro_")
                {
                    dispatchers += 1;
                } else if source.starts_with("byte_block_") {
                    blocks += 1;
                } else if source.starts_with("byte_control_") {
                    controls += 1;
                } else {
                    panic!("unexpected original interpreter alias: {source}");
                }
            }
            assert_eq!(imported, expected);
            assert_eq!(inputs, 2);
            assert_eq!(dispatchers, 8);
            assert!(blocks >= 2);
            assert_eq!(blocks, controls);
            if consensus {
                assert!(operations > 0);
            }
            assert_eq!(
                operations,
                parent.matches("spec fn byte_operation_").count()
            );
            assert_eq!(
                text.matches("proof fn typed_prefix_step_relation_").count(),
                2
            );
            assert_eq!(
                text.matches("proof fn typed_final_native_source_trace_")
                    .count(),
                2
            );
            assert_eq!(
                text.matches("proof fn invocation_source_observation_extensionality_")
                    .count(),
                2
            );
            assert!(!text.contains("assume("));
        });
        result.0.unwrap();
    }
}

#[test]
fn original_source_generation_headers_cover_the_retained_index_and_borrow() {
    use crate::mixed_optimizer_refinement_v26::semantics::byte_function_v30::ByteFunctionV30;
    use fe2o3_kernel_analysis::{
        CanonicalKirPrivateByteAnalysisV38, CanonicalKirPrivateByteLimitsV38,
        CanonicalKirPrivateByteStorageV38, CanonicalKirPrivateMemoryErrorV1,
    };

    fn retained<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    type Index<'a, 'g, 'v, 's> = EmittedByteFunctionsV55<'a, 'g, slots::SourceSlots<'v, 's>>;
    assert_eq!(
        size_of::<Option<&Index<'_, '_, '_, '_>>>(),
        size_of::<&()>()
    );
    let index = size_of::<Option<Index<'_, '_, '_, '_>>>()
        + 2 * size_of::<Result<Option<Index<'_, '_, '_, '_>>>>()
        + size_of::<&Index<'_, '_, '_, '_>>()
        + 2 * size_of::<Result<&Index<'_, '_, '_, '_>>>()
        + size_of::<[usize; 2]>();
    let existing = retained::<&fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>>()
        + retained::<&mut Writer<'_, '_>>()
        + retained::<super::super::invocations::InvocationPlan<'_, '_>>()
        + retained::<slots::SourceSlots<'_, '_>>()
        + retained::<TargetContracts<'_, '_>>()
        + retained::<slots::SourceTagPairsV40<'_, '_, '_, '_, '_>>()
        + retained::<&TargetContracts<'_, '_>>()
        + retained::<&slots::SourceTagPairsV40<'_, '_, '_, '_, '_>>()
        + retained::<source_function::SourceByteProgram<'_, '_, '_>>()
        + retained::<byte_bindings::SourceByteBindings<'_, '_, '_>>()
        + retained::<Vec<ByteFunctionV30<'_, '_, slots::SourceSlots<'_, '_>>>>()
        + retained::<&[ExplicitLaunchExtent]>()
        + retained::<FormalIndexWidth>()
        + retained::<EndiannessV2>()
        + retained::<paired::PairedInvocations<'_, '_, '_>>()
        + retained::<CanonicalKirPrivateByteAnalysisV38<'_, '_>>()
        + retained::<CanonicalKirPrivateByteStorageV38>()
        + retained::<CanonicalKirPrivateByteLimitsV38>()
        + size_of::<
            std::result::Result<
                (
                    CanonicalKirPrivateByteAnalysisV38<'_, '_>,
                    CanonicalKirPrivateByteStorageV38,
                ),
                CanonicalKirPrivateMemoryErrorV1,
            >,
        >()
        + retained::<[usize; 6]>()
        + retained::<Option<SemanticKernelLaunchBoundsV1>>()
        + retained::<SemanticWorkgroupDimensionsV1>()
        + retained::<[u32; 3]>()
        + size_of::<[&(); 7]>()
        + size_of::<[usize; 8]>();
    assert_eq!(generation_headers_v36(), existing + index);
}

#[test]
fn original_source_typed_prefix_reuse_keeps_exact_budgets_and_caller_floor() {
    use sha2::Digest as _;
    let observe = |work, storage, fingerprint: &mut (usize, [u8; 32])| {
        run_mode(work, storage, false, true, |text| {
            *fingerprint = (text.len(), sha2::Sha256::digest(text.as_bytes()).into());
        })
    };
    let mut fingerprint = (0, [0; 32]);
    let measured = observe(LIMIT, LIMIT, &mut fingerprint);
    measured.0.unwrap();
    assert_eq!(measured.2, super::super::invocations::tests::FLOOR);
    let mut repeated = (0, [0; 32]);
    let exact = observe(measured.1, measured.3, &mut repeated);
    exact.0.unwrap();
    assert_eq!(fingerprint, repeated);
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    for work_short in [false, true] {
        let work = measured.1 - usize::from(work_short);
        let storage = measured.3 - usize::from(!work_short);
        let mut rejected = (0, [0; 32]);
        let denied = observe(work, storage, &mut rejected);
        let error = denied
            .0
            .expect_err("one-short original-model reuse must refuse");
        let mut chain: &(dyn std::error::Error + 'static) = &error;
        let resource = loop {
            if let Some(resource) = chain.downcast_ref::<Resource>() {
                break *resource;
            }
            chain = chain
                .source()
                .unwrap_or_else(|| panic!("missing resource: {error:?}"));
        };
        match resource {
            Resource::Work(limit) if work_short => {
                assert_eq!(limit.limit(), work);
                assert_eq!(limit.actual(), measured.1);
            }
            Resource::Storage(limit) if !work_short => {
                assert_eq!(limit.limit(), storage);
                assert_eq!(limit.actual(), measured.3);
            }
            other => panic!("wrong reuse resource: {other:?}"),
        }
    }
}
