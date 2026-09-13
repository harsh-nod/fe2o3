use super::*;

fn borrowed_empty_owner_work_prefixes_v1() -> Vec<usize> {
    // Exact V12 schema: ten sizing tokens, then header/ID/three counts.
    let fields = [8, 2, 2, 4, 4, 4, 1, 4, 4, 4];
    let mut charges = vec![1; 10];
    charges.extend(fields);
    charges.push(4); // Producer patches the declared module length.
    charges.extend([8, 2]); // Exact V12 magic/version gate.
    charges.extend([8, 2, 2, 4, 4, 4, 1, 2, 4, 4, 4]);
    // Decoder comparison visits each schema chunk. The placeholder is checked
    // only by the final length patch; text decoding above pays validation/copy2.
    for (ordinal, field) in fields.into_iter().enumerate() {
        charges.push(field);
        if ordinal != 3 {
            charges.push(1);
        }
    }
    charges.extend([4, 1, 1]); // Length patch, its query, final length query.
    charges.extend([5, 37, 90]); // Empty verifier, source equality, framed hash.
    charges
}

#[test]
fn borrowed_resource_owner_matches_literal_prefixes_and_owned_api() {
    const WORK: usize = 284;
    const FLOOR: usize = 7;
    const PRE_VERIFIER: usize = 152;
    let charges = borrowed_empty_owner_work_prefixes_v1();
    assert_eq!(charges.iter().sum::<usize>(), WORK);
    assert_eq!(
        charges[..charges.len() - 3].iter().sum::<usize>(),
        PRE_VERIFIER
    );
    assert_eq!(
        4 + VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_DOMAIN_V1.len() + 2 + 8,
        53
    );
    let module = Module::new("m");
    let original = module.clone();
    let expected = VerifiedCanonicalKernelIrV12::from_module(module.clone()).unwrap();
    for available in 0..=WORK {
        let limit = FLOOR + available;
        let mut expected_work = FLOOR;
        let mut denied = None;
        for charge in &charges {
            let actual = expected_work + charge;
            if actual > limit {
                denied = Some(actual);
                break;
            }
            expected_work = actual;
        }
        let mut borrowed_work = CanonicalKernelIrWorkBudgetV1::new(limit);
        borrowed_work.charge_work(FLOOR).unwrap();
        let borrowed = VerifiedCanonicalKernelIrV12::from_module_ref_with_resource_budget_v1(
            &module,
            &mut borrowed_work,
            0,
        );
        let mut owned_work = CanonicalKernelIrWorkBudgetV1::new(limit);
        owned_work.charge_work(FLOOR).unwrap();
        let owned = VerifiedCanonicalKernelIrV12::from_module_with_resource_budget_v1(
            module.clone(),
            &mut owned_work,
            0,
        );
        assert_eq!(borrowed, owned, "available={available}");
        assert_eq!(borrowed_work.work(), expected_work, "available={available}");
        assert_eq!(borrowed_work.failed_work(), denied, "available={available}");
        assert_eq!(owned_work.work(), expected_work);
        assert_eq!(owned_work.failed_work(), denied);
        assert_eq!(module, original);
        match borrowed {
            Ok((owner, receipt)) => {
                assert_eq!(available, WORK);
                assert_eq!(owner, expected);
                assert_eq!(receipt.work(), 5);
                assert_eq!(receipt.peak_storage(), 0);
            }
            Err(MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit(error)) => {
                assert!(expected_work < FLOOR + PRE_VERIFIER);
                assert_eq!(Some(error.actual()), denied);
                assert_eq!(error.limit(), limit);
            }
            Err(MeteredVerifiedCanonicalKernelIrErrorV12::VerificationWorkLimit {
                error,
                receipt,
            }) => {
                assert!(expected_work >= FLOOR + PRE_VERIFIER);
                assert_eq!(Some(error.actual()), denied);
                assert_eq!(error.limit(), limit);
                assert_eq!(
                    receipt.work(),
                    if available < PRE_VERIFIER + 5 { 0 } else { 5 }
                );
                assert_eq!(receipt.peak_storage(), 0);
            }
            other => panic!("unexpected borrowed result at {available}: {other:?}"),
        }
    }
}

fn borrowed_resource_fixture_v1() -> Module {
    let mut module = Module::new("borrowed-canonical");
    module.functions.push(crate::Function::internal_helper(
        "f",
        crate::Signature::new(vec![], vec![]),
        vec![],
        vec![crate::BasicBlock {
            id: crate::BlockId(0),
            parameters: vec![],
            operations: vec![crate::Operation::effect_free(
                crate::ValueDef::new(crate::ValueId(0), crate::Type::INDEX),
                crate::OperationKind::Constant(crate::Constant::Index(1)),
            )],
            terminator: Some(crate::Terminator::Return { values: vec![] }),
        }],
    ));
    module
}

#[test]
fn borrowed_resource_owner_preserves_verification_errors_and_storage_prefixes() {
    let valid = borrowed_resource_fixture_v1();
    let mut wrong_role = valid.clone();
    wrong_role.functions[0].role = crate::FunctionRole::KernelEntry;
    let mut wrong_type = valid.clone();
    wrong_type.functions[0].body.as_mut().unwrap().blocks[0].operations[0].results[0].ty =
        crate::Type::Scalar(crate::ScalarType::Bool);
    let mut duplicate = valid.clone();
    let operation = duplicate.functions[0].body.as_ref().unwrap().blocks[0].operations[0].clone();
    duplicate.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(operation);
    let mut wrong_dominance = valid.clone();
    wrong_dominance.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            0,
            crate::Operation::effect_free(
                crate::ValueDef::new(crate::ValueId(1), crate::Type::INDEX),
                crate::OperationKind::Binary {
                    op: crate::BinaryOp::Add,
                    lhs: crate::ValueId(0),
                    rhs: crate::ValueId(0),
                },
            ),
        );
    verify_module(&valid).unwrap();
    assert!(
        verify_module(&duplicate)
            .unwrap_err()
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == crate::DiagnosticCode::DuplicateValue)
    );
    assert!(
        verify_module(&wrong_dominance)
            .unwrap_err()
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == crate::DiagnosticCode::NonDominatingUse)
    );
    for (ordinal, module) in [
        valid,
        wrong_role,
        wrong_type,
        duplicate,
        wrong_dominance,
        Module::new(""),
    ]
    .into_iter()
    .enumerate()
    {
        let original = module.clone();
        let unmetered = VerifiedCanonicalKernelIrV12::from_module(module.clone());
        assert_eq!(unmetered.is_ok(), ordinal == 0);
        let mut measuring = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let measured = VerifiedCanonicalKernelIrV12::from_module_ref_with_resource_budget_v1(
            &module,
            &mut measuring,
            usize::MAX,
        );
        match (&unmetered, &measured) {
            (Ok(expected), Ok((actual, _))) => assert_eq!(expected, actual),
            (
                Err(VerifiedCanonicalKernelIrErrorV12::Verification(expected)),
                Err(MeteredVerifiedCanonicalKernelIrErrorV12::Verification { error, .. }),
            ) => {
                assert_eq!(expected, error);
            }
            (Err(expected), Err(MeteredVerifiedCanonicalKernelIrErrorV12::Canonical(actual))) => {
                assert_eq!(expected, actual);
            }
            other => panic!("unmetered mismatch for fixture {ordinal}: {other:?}"),
        }
        let peak = match &measured {
            Ok((_, receipt)) => receipt.peak_storage(),
            Err(error) => error
                .verification_receipt()
                .map_or(0, |receipt| receipt.peak_storage()),
        };
        for storage in 0..=peak {
            let mut borrowed_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let borrowed = VerifiedCanonicalKernelIrV12::from_module_ref_with_resource_budget_v1(
                &module,
                &mut borrowed_work,
                storage,
            );
            let mut owned_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let owned = VerifiedCanonicalKernelIrV12::from_module_with_resource_budget_v1(
                module.clone(),
                &mut owned_work,
                storage,
            );
            assert_eq!(borrowed, owned, "fixture={ordinal}, storage={storage}");
            assert_eq!(borrowed_work.work(), owned_work.work());
            assert_eq!(borrowed_work.failed_work(), owned_work.failed_work());
            assert_eq!(module, original);
            if storage == peak {
                assert_eq!(borrowed, measured);
                assert_eq!(borrowed_work.work(), measuring.work());
            }
        }
    }
}

#[test]
fn borrowed_resource_owner_retains_no_source_borrow_or_validation_cache() {
    let owner = {
        let mut module = borrowed_resource_fixture_v1();
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let (owner, _) = VerifiedCanonicalKernelIrV12::from_module_ref_with_resource_budget_v1(
            &module,
            &mut budget,
            usize::MAX,
        )
        .unwrap();
        module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].results[0].ty =
            crate::Type::Scalar(crate::ScalarType::Bool);
        let mut fresh = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        assert!(matches!(
            VerifiedCanonicalKernelIrV12::from_module_ref_with_resource_budget_v1(
                &module,
                &mut fresh,
                usize::MAX,
            ),
            Err(MeteredVerifiedCanonicalKernelIrErrorV12::Verification { .. })
        ));
        owner
    };
    owner.revalidate().unwrap();
    let mut other = borrowed_resource_fixture_v1();
    other.id = "different-owner".into();
    let mut budget = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let (different, _) = VerifiedCanonicalKernelIrV12::from_module_ref_with_resource_budget_v1(
        &other,
        &mut budget,
        usize::MAX,
    )
    .unwrap();
    assert_ne!(owner.identity(), different.identity());
}
