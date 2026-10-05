fn source_domain_join_header_oracle_v30() -> usize {
    // Nine thin argument/query borrows plus the actual three return envelopes.
    // No helper result is substituted for the independently enumerated frame.
    type Frame<'a> = (
        [&'a (); 9],
        Result<
            &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
            fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
        >,
        SourceOwnedResultV18<()>,
        Result<(), PendingGlobalReadConditionErrorV18>,
        GlobalSourceCfgGuardV85,
        SourceOwnedResultV18<GlobalSourceCfgGuardV85>,
    );
    2 * (size_of::<Frame<'_>>() + std::mem::align_of::<Frame<'_>>())
}

#[test]
fn source_global_domain_join_frames_have_exact_and_one_short_storage() {
    let expected = source_domain_join_header_oracle_v30();
    assert_eq!(source_global_domain_join_headers_v30().unwrap(), expected);
    for (limit, pass) in [(expected, true), (expected - 1, false)] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        match (
            pass,
            budget.reserve_storage(source_global_domain_join_headers_v30().unwrap()),
        ) {
            (true, Ok(())) => {
                assert_eq!(budget.storage(), expected);
                budget.release_storage(expected).unwrap();
            }
            (false, Err(ArgumentResourceV1::Storage(error))) => {
                assert_eq!(error.actual(), expected);
                assert_eq!(error.limit(), expected - 1);
            }
            _ => panic!("shared concrete endpoint join frame admission"),
        }
        assert_eq!(budget.storage(), 0);
    }
}
