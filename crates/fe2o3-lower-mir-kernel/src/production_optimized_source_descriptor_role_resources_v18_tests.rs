fn cleanup_header_oracle() -> usize {
    use std::panic::AssertUnwindSafe;
    type Panic = Box<dyn std::any::Any + Send>;
    size_of::<[Option<Panic>; 2]>()
        + size_of::<AssertUnwindSafe<[Option<Panic>; 2]>>()
        + 2 * size_of::<Panic>()
        + size_of::<AssertUnwindSafe<Panic>>()
        + 2 * size_of::<Result<(), Panic>>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<usize>()
        + size_of::<bool>()
}

fn header_oracle<T, E>() -> usize {
    let owned_rows = size_of::<Vec<DescriptorSourceRoleRowV18>>()
        + size_of::<(Vec<DescriptorSourceRoleRowV18>, usize)>()
        + 2 * size_of::<SourceOwnedResultV18<Vec<DescriptorSourceRoleRowV18>>>()
        + 2 * size_of::<SourceOwnedResultV18<(Vec<DescriptorSourceRoleRowV18>, usize)>>()
        + size_of::<
            std::thread::Result<SourceOwnedResultV18<(Vec<DescriptorSourceRoleRowV18>, usize)>>,
        >();
    let queries = size_of::<SourceOwnedResultV18<()>>()
        + size_of::<SourceOwnedResultV18<usize>>()
        + size_of::<SourceOwnedResultV18<Option<DescriptorSourceRoleV18>>>()
        + size_of::<Option<ProductionSliceAccessSiteV1>>()
        + size_of::<ProductionSliceAccessSiteV1>()
        + size_of::<SourceOwnedResultV18<Option<ProductionSliceAccessSiteV1>>>()
        + size_of::<DescriptorSourceRoleRowV18>()
        + size_of::<Option<&DescriptorSourceRoleRowV18>>()
        + size_of::<Option<&mut DescriptorSourceRoleRowV18>>()
        + size_of::<SourceOwnedResultV18<&DescriptorSourceRoleRowV18>>()
        + size_of::<SourceOwnedResultV18<&mut DescriptorSourceRoleRowV18>>()
        + size_of::<[(SliceOperation, SliceOperation, DescriptorSourceRoleV18); 4]>()
        + size_of::<
            std::array::IntoIter<(SliceOperation, SliceOperation, DescriptorSourceRoleV18), 4>,
        >()
        + 2 * size_of::<DescriptorAccessSummaryV18>()
        + 2 * size_of::<SourceOwnedResultV18<DescriptorAccessSummaryV18>>()
        + size_of::<Option<ProductionSemanticExpressionV2>>()
        + size_of::<ProductionSemanticExpressionV2>()
        + size_of::<SourceOwnedResultV18<Option<ProductionSemanticExpressionV2>>>()
        + size_of::<Option<SourcePhysicalAccessV18<'_>>>()
        + size_of::<SourcePhysicalAccessV18<'_>>()
        + size_of::<SourceOwnedResultV18<Option<SourcePhysicalAccessV18<'_>>>>()
        + size_of::<Option<SourcePhysicalPayloadV18<'_>>>()
        + size_of::<SourcePhysicalPayloadV18<'_>>()
        + size_of::<SourceOwnedResultV18<Option<SourcePhysicalPayloadV18<'_>>>>()
        + size_of::<ProductionSourceScalarInputV18<'_>>()
        + size_of::<SourceOwnedResultV18<ProductionSourceScalarInputV18<'_>>>()
        + size_of::<Type>()
        + size_of::<Result<Type, ProductionSemanticKirErrorV1>>()
        + size_of::<Constant>()
        + size_of::<Result<Constant, ProductionSemanticKirErrorV1>>();
    size_of::<[usize; 2]>()
        + size_of::<CheckedDescriptorSourceRolesV18<'_>>()
        + size_of::<std::thread::Result<Result<T, E>>>()
        + size_of::<Result<T, E>>()
        + size_of::<Option<SourceOwnedQueryFailureV18>>()
        + owned_rows
        + queries
        + global_header_oracle_v18()
        + cleanup_header_oracle()
}

#[test]
fn descriptor_role_headers_prepay_rows_queries_and_cleanup() {
    type Error = ProductionSourceOwnedViewErrorV18;
    let expected = header_oracle::<Vec<u64>, Error>();
    assert_eq!(
        descriptor_role_headers_v18::<Vec<u64>, Error>().unwrap(),
        expected
    );
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, expected - usize::from(short));
        let reserved =
            budget.reserve_storage(descriptor_role_headers_v18::<Vec<u64>, Error>().unwrap());
        if short {
            let Err(ArgumentResourceV1::Storage(error)) = reserved else {
                panic!("one-short exact header");
            };
            assert_eq!((error.actual(), error.limit()), (expected, expected - 1));
            assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
        } else {
            reserved.unwrap();
            assert_eq!(
                (budget.storage(), budget.peak_storage()),
                (expected, expected)
            );
            budget.release_storage(expected).unwrap();
        }
        assert_eq!(budget.work(), 0);
    }
}

#[test]
fn descriptor_role_outer_headers_prepay_aligned_callback_before_namespace() {
    let captured = [17u128; 3];
    let callback = move || std::hint::black_box(captured);
    let bytes = std::mem::size_of_val(&callback);
    let alignment = std::mem::align_of_val(&callback);
    type Error = ProductionSourceOwnedViewErrorV18;
    let expected = bytes
        + 2 * alignment
        + 8 * size_of::<Option<&OriginalEntryIndexV20<'_, '_>>>()
        + size_of::<DescriptorRoleScopeV18>()
        + size_of::<[usize; 2]>()
        + 2 * size_of::<SourceOwnedResultV18<usize>>()
        + size_of::<std::thread::Result<SourceOwnedResultV18<usize>>>()
        + size_of::<std::thread::Result<Result<Vec<u64>, Error>>>()
        + size_of::<Result<Vec<u64>, Error>>()
        + size_of::<Option<SourceOwnedQueryFailureV18>>()
        + size_of::<SourceOwnedResultV18<()>>()
        + size_of::<Option<&fe2o3_pliron::ProductionRankedKernelV1>>()
        + size_of::<(
            &ProductionSourceCorrespondenceV18<'_>,
            &ProductionOptimizedSourceCorrespondenceV18<'_>,
            usize,
            Option<&fe2o3_pliron::ProductionRankedKernelV1>,
            &mut ArgumentBudgetV1<'_>,
        )>()
        + size_of::<
            std::panic::AssertUnwindSafe<(
                &ProductionSourceCorrespondenceV18<'_>,
                &ProductionOptimizedSourceCorrespondenceV18<'_>,
                usize,
                Option<&fe2o3_pliron::ProductionRankedKernelV1>,
                &mut ArgumentBudgetV1<'_>,
            )>,
        >()
        + size_of::<(
            &ProductionSourceCorrespondenceV18<'_>,
            &ProductionOptimizedSourceCorrespondenceV18<'_>,
            usize,
        )>()
        + size_of::<(
            &CheckedDescriptorSourceRolesV18<'_>,
            &mut ArgumentBudgetV1<'_>,
        )>()
        + cleanup_header_oracle();
    assert_eq!(
        descriptor_role_outer_headers_v18::<Vec<u64>, Error>(bytes, alignment).unwrap(),
        expected
    );
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, expected - usize::from(short));
        let reserved = budget.reserve_storage(
            descriptor_role_outer_headers_v18::<Vec<u64>, Error>(bytes, alignment).unwrap(),
        );
        if short {
            let Err(ArgumentResourceV1::Storage(error)) = reserved else {
                panic!("one-short outer header");
            };
            assert_eq!((error.actual(), error.limit()), (expected, expected - 1));
            assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
        } else {
            reserved.unwrap();
            assert_eq!(
                (budget.storage(), budget.peak_storage()),
                (expected, expected)
            );
            budget.release_storage(expected).unwrap();
        }
        assert_eq!(budget.work(), 0);
    }
    assert!(matches!(
        descriptor_role_outer_headers_v18::<(), Error>(usize::MAX, alignment),
        Err(ArgumentResourceV1::Arithmetic)
    ));
}

#[test]
fn descriptor_write_definition_lookup_prepays_exact_borrowed_envelopes() {
    type Definition<'a> = &'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'a>;
    let expected = size_of::<Definition<'_>>()
        + size_of::<Option<Definition<'_>>>()
        + size_of::<Result<Option<Definition<'_>>, CanonicalKirInventoryErrorV1>>()
        + size_of::<Result<Option<Definition<'_>>, ProductionSemanticKirErrorV1>>()
        + size_of::<Result<Definition<'_>, ProductionSemanticKirErrorV1>>();
    assert_eq!(descriptor_write_lookup_headers_v18().unwrap(), expected);
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, expected - usize::from(short));
        let result = budget.reserve_storage(descriptor_write_lookup_headers_v18().unwrap());
        if short {
            let Err(ArgumentResourceV1::Storage(error)) = result else {
                panic!("descriptor write lookup one-short header");
            };
            assert_eq!((error.limit(), error.actual()), (expected - 1, expected));
            assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
        } else {
            result.unwrap();
            assert_eq!(
                (budget.storage(), budget.peak_storage()),
                (expected, expected)
            );
            budget.release_storage(expected).unwrap();
        }
        assert_eq!(budget.work(), 0);
    }
}

fn operation(index: u32) -> SliceOperation {
    SliceOperation {
        block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
            function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(3),
            block: 2,
        },
        operation: index,
    }
}

#[test]
fn descriptor_role_occurrence_index_has_independent_exact_and_one_short_work() {
    // Inert ordered coordinates test only: no source view or completion is
    // manufactured. The genuine source tests own all authority assertions.
    for count in [0usize, 1, 17, 256, 4096] {
        let rows: Vec<_> = (0..count)
            .map(|index| DescriptorSourceRoleRowV18 {
                output: operation(index as u32),
                input: None,
                instance: None,
                site: None,
                role: None,
                write_recipe_pending: false,
                global: None,
            })
            .collect();
        for target in [0, count / 2, count] {
            let (mut left, mut right, mut probes, mut right_moves) = (0, count, 0, 0);
            while left != right {
                probes += 1;
                let mid = left + (right - left) / 2;
                if mid < target {
                    left = mid + 1;
                    right_moves += 1;
                } else {
                    right = mid;
                }
            }
            // Same function and block compare all three coordinates. Each
            // probe has six fixed steps; advancing right pays one extra.
            let expected = 1 + 9 * probes + right_moves;
            for short in [false, true] {
                let limit = expected - usize::from(short);
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = ArgumentBudgetV1::new(&mut work, 0);
                let found = descriptor_role_index_v18(&rows, operation(target as u32), &mut budget);
                if short {
                    let Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(
                        error,
                    ))) = found
                    else {
                        panic!("exact occurrence-index work boundary");
                    };
                    assert_eq!(error.limit(), limit);
                    assert!(error.actual() > limit);
                } else {
                    assert_eq!(found.unwrap(), target);
                    assert_eq!(budget.work(), expected);
                }
                assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
            }
        }
    }
}
