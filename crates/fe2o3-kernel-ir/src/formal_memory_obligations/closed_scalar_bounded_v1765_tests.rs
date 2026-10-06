fn roots_module(count: usize) -> Module {
    let template = scalar_module();
    let mut module = Module::new("bounded_formal_roots");
    for ordinal in 0..count {
        let mut function = template.functions[0].clone();
        function.id = FunctionId::from(format!("entry_{ordinal}"));
        let mut kernel = template.kernels[0].clone();
        kernel.id = KernelId::from(format!("root_{ordinal}"));
        kernel.entry = function.id.clone();
        module.functions.push(function);
        module.kernels.push(kernel);
    }
    module
}

fn with_bounded_owner<T>(
    module: &Module,
    run: impl FnOnce(&VerifiedCanonicalKernelIrModuleV18) -> T,
) -> T {
    let mut work = Work::new(500_000_000);
    let mut budget = Budget::new(&mut work, 20_000_000);
    budget.reserve_storage(37).unwrap();
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            StorageLayoutLimitsV1 {
                rows: 64,
                edges: 256,
                containment_depth: 32,
                object_bytes: 4096,
            },
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let before = (budget.work(), budget.storage(), budget.peak_storage());
    let result = run(&owner);
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        before
    );
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 37);
    result
}

#[test]
fn closed_scalar_formal_bounded_scope_shares_actual_owner_effects_and_limits_root_replay() {
    let module = roots_module(8);
    with_bounded_owner(&module, |owner| {
        let mut scope = CanonicalClosedScalarFormalScopeV18::new(owner).unwrap();
        assert!(std::ptr::eq(scope.owner, owner));
        assert_eq!(scope.effects.functions().len(), 8);
        let effects = &scope.effects as *const _;
        for (ordinal, kernel) in owner.module().kernels.iter().enumerate() {
            let report = scope
                .derive(
                    &kernel.id,
                    ExplicitLaunchExtent::Exact {
                        rank: 1,
                        extents: [64, 1, 1],
                    },
                    FormalIndexWidth::Bits64,
                )
                .unwrap();
            assert!(report.is_complete());
            assert_eq!(report.obligations().kernel(), &kernel.id);
            assert_eq!(scope.remaining, 7 - ordinal);
            assert_eq!(&scope.effects as *const _, effects);
        }
        assert!(matches!(
            scope.derive(
                &owner.module().kernels[0].id,
                ExplicitLaunchExtent::Exact {
                    rank: 1,
                    extents: [64, 1, 1]
                },
                FormalIndexWidth::Bits64
            ),
            Err(CanonicalClosedScalarFormalErrorV18::Unsupported(
                "formal root replay bound"
            ))
        ));
    });
}

#[test]
fn closed_scalar_formal_bounded_scope_rejects_duplicate_root_and_uncovered_function() {
    let mut module = roots_module(2);
    module.kernels[1].entry = module.kernels[0].entry.clone();
    // The uncovered definition must not falsely declare itself a kernel entry.
    module.functions[1].role = crate::FunctionRole::InternalHelper;
    // The generic owner can structurally verify repeated exports of an entry.
    // This final closed subset additionally requires exact one-to-one coverage.
    with_bounded_owner(&module, |owner| {
        assert_eq!(owner.module().functions.len(), 2);
        assert_eq!(owner.module().kernels.len(), 2);
        assert_eq!(
            owner.module().kernels[0].entry,
            owner.module().kernels[1].entry
        );
        assert_ne!(owner.module().kernels[0].id, owner.module().kernels[1].id);
        assert!(matches!(
            CanonicalClosedScalarFormalScopeV18::new(owner),
            Err(CanonicalClosedScalarFormalErrorV18::Unsupported(
                "duplicate root entry"
            ))
        ));
    });
    let mut module = roots_module(2);
    module.kernels.pop();
    module.functions[1].role = crate::FunctionRole::InternalHelper;
    with_bounded_owner(&module, |owner| {
        assert_eq!(owner.module().functions.len(), 2);
        assert_eq!(owner.module().kernels.len(), 1);
        assert_eq!(
            owner.module().functions[1].role,
            crate::FunctionRole::InternalHelper
        );
        assert!(matches!(
            CanonicalClosedScalarFormalScopeV18::new(owner),
            Err(CanonicalClosedScalarFormalErrorV18::Unsupported(
                "non-root functions"
            ))
        ));
    });
}

#[test]
fn closed_scalar_formal_work_bound_equation_has_exact_short_and_overflow_controls() {
    // For nodes=1024 the exact logarithm is 12, so 84 replay slots plus
    // indexing cost 1_044_480; the next slot exceeds the fixed 1_048_576 cap.
    assert_eq!(checked_work_v18(1024, 84).unwrap(), 1_044_480);
    assert!(matches!(
        checked_work_v18(1024, 85),
        Err(CanonicalClosedScalarFormalErrorV18::Unsupported(
            "formal module work bound"
        ))
    ));
    assert!(matches!(
        checked_work_v18(1, usize::MAX),
        Err(CanonicalClosedScalarFormalErrorV18::Unsupported(
            "formal work arithmetic"
        ))
    ));
    assert!(matches!(
        checked_work_v18(usize::MAX, 1),
        Err(CanonicalClosedScalarFormalErrorV18::Unsupported(
            "formal module work bound"
        ))
    ));
    let mut nodes = MAX_SCALAR_FORMAL_NODES_V18 - 1;
    add_nodes_v18(&mut nodes, 1).unwrap();
    assert_eq!(nodes, 65_536);
    assert!(matches!(
        add_nodes_v18(&mut nodes, 1),
        Err(CanonicalClosedScalarFormalErrorV18::Unsupported(
            "formal module node bound"
        ))
    ));
}

#[test]
fn closed_scalar_formal_genuine_many_roots_refuse_before_index_and_effect_construction() {
    let module = roots_module(128);
    with_bounded_owner(&module, |owner| {
        assert!(matches!(
            CanonicalClosedScalarFormalScopeV18::new(owner),
            Err(CanonicalClosedScalarFormalErrorV18::Unsupported(
                "formal module work bound"
            ))
        ));
    });
    with_bounded_owner(&roots_module(1), |owner| {
        let mut scope = CanonicalClosedScalarFormalScopeV18::new(owner).unwrap();
        let oversized = KernelId::from("x".repeat(scope.nodes + 1));
        assert!(matches!(
            scope.derive(
                &oversized,
                ExplicitLaunchExtent::Exact {
                    rank: 1,
                    extents: [64, 1, 1]
                },
                FormalIndexWidth::Bits64
            ),
            Err(CanonicalClosedScalarFormalErrorV18::Unsupported(
                "formal query identity bound"
            ))
        ));
        assert_eq!(scope.remaining, 0);
    });
}
