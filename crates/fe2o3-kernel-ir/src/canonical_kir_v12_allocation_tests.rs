use super::*;
use crate::{
    BasicBlock, BlockId, DiagnosticCode, Function, FunctionId, FunctionRole, Kernel, LaunchDomain,
    LaunchExtent, Signature, Terminator,
};

fn source_module() -> Module {
    let mut identifier = String::with_capacity(4096);
    identifier.push_str("canonical-owned-string");
    let mut module = Module::new(identifier);
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::internal_helper(
        FunctionId::new("entry-with-a-borrowed-role-key"),
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    function.role = FunctionRole::KernelEntry;
    module.kernels.push(Kernel::new(
        "kernel",
        function.id.clone(),
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    module.functions.push(function);
    module
}

#[test]
fn exact_owner_preserves_v12_bytes_and_rejects_an_older_envelope() {
    let source = source_module();
    let bytes = encode_module_v12(&source).unwrap();
    let (owner, decoded) =
        VerifiedCanonicalKernelIrV12::from_canonical_bytes_with_module(bytes.clone()).unwrap();
    assert_eq!(decoded, source);
    assert_eq!(owner.canonical_bytes(), bytes);
    owner.revalidate().unwrap();
    assert_eq!(
        VerifiedCanonicalKernelIrV12::from_module(source)
            .unwrap()
            .identity(),
        owner.identity()
    );
    let old = crate::encode_module_v11(&Module::new("older")).unwrap();
    assert_eq!(
        VerifiedCanonicalKernelIrV12::from_canonical_bytes(old).unwrap_err(),
        VerifiedCanonicalKernelIrErrorV12::NotExactV12 { version: 11 }
    );
}

#[test]
fn complete_owner_admission_preserves_prefixes_and_exact_work_storage_limits() {
    const WORK_PREFIX: usize = 11;
    const STORAGE_PREFIX: usize = 7;
    let source = source_module();
    let source_capacity = source.id.retained_capacity_bytes();
    assert!(source_capacity > source.id.as_str().len());
    let expected = VerifiedCanonicalKernelIrV12::from_module(source.clone()).unwrap();

    let mut baseline_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    baseline_work.charge_work(WORK_PREFIX).unwrap();
    let (owner, receipt, peak) = {
        let mut resources =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut baseline_work, usize::MAX);
        resources.reserve_storage(STORAGE_PREFIX).unwrap();
        let (owner, receipt) =
            VerifiedCanonicalKernelIrV12::from_module_ref_with_verification_budget_v12(
                &source,
                &mut resources,
            )
            .unwrap();
        assert_eq!(resources.storage(), STORAGE_PREFIX);
        assert_eq!(resources.failed_storage(), None);
        (owner, receipt, resources.peak_storage())
    };
    assert_eq!(owner, expected);
    assert_eq!(
        receipt.retained_storage(),
        std::mem::size_of::<VerifiedCanonicalKernelIrV12>() + owner.canonical_bytes().len()
    );
    assert!(peak >= STORAGE_PREFIX + receipt.retained_storage());
    assert_eq!(source.id.retained_capacity_bytes(), source_capacity);
    let required_work = baseline_work.work();

    let mut exact_work = CanonicalKernelIrWorkBudgetV1::new(required_work);
    exact_work.charge_work(WORK_PREFIX).unwrap();
    {
        let mut resources =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut exact_work, peak);
        resources.reserve_storage(STORAGE_PREFIX).unwrap();
        let (exact, exact_receipt) =
            VerifiedCanonicalKernelIrV12::from_module_ref_with_verification_budget_v12(
                &source,
                &mut resources,
            )
            .unwrap();
        assert_eq!(exact, owner);
        assert_eq!(exact_receipt, receipt);
        assert_eq!(resources.storage(), STORAGE_PREFIX);
        assert_eq!(resources.peak_storage(), peak);
    }
    assert_eq!(exact_work.work(), required_work);

    let mut storage_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    storage_work.charge_work(WORK_PREFIX).unwrap();
    {
        let mut resources =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut storage_work, peak - 1);
        resources.reserve_storage(STORAGE_PREFIX).unwrap();
        let error = VerifiedCanonicalKernelIrV12::from_module_ref_with_verification_budget_v12(
            &source,
            &mut resources,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            CanonicalKernelIrReplayAdmissionErrorV12::Resource(
                CanonicalKernelIrVerificationResourceErrorV1::Storage(_)
            ) | CanonicalKernelIrReplayAdmissionErrorV12::Decode(KernelIrDecodeError::Resource(
                CanonicalKernelIrVerificationResourceErrorV1::Storage(_)
            ))
        ));
        assert_eq!(resources.storage(), STORAGE_PREFIX);
        assert_eq!(resources.failed_storage(), Some(peak));
    }

    let mut short_work = CanonicalKernelIrWorkBudgetV1::new(required_work - 1);
    short_work.charge_work(WORK_PREFIX).unwrap();
    {
        let mut resources =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut short_work, peak);
        resources.reserve_storage(STORAGE_PREFIX).unwrap();
        let error = VerifiedCanonicalKernelIrV12::from_module_ref_with_verification_budget_v12(
            &source,
            &mut resources,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            CanonicalKernelIrReplayAdmissionErrorV12::Canonical(
                MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit(_)
            )
        ));
        assert_eq!(resources.storage(), STORAGE_PREFIX);
    }
    assert_eq!(short_work.failed_work(), Some(required_work));
    assert_eq!(source.id.retained_capacity_bytes(), source_capacity);
    assert_eq!(encode_module_v12(&source).unwrap(), owner.canonical_bytes());
}

#[test]
fn complete_admission_rejects_wire_valid_but_semantically_invalid_input() {
    let source = Module::new("");
    let bytes = encode_module_v12(&source).unwrap();
    assert_eq!(decode_module_v12(&bytes).unwrap(), source);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut resources = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    resources.reserve_storage(13).unwrap();
    let error = VerifiedCanonicalKernelIrV12::from_module_ref_with_verification_budget_v12(
        &source,
        &mut resources,
    )
    .unwrap_err();
    let CanonicalKernelIrReplayAdmissionErrorV12::Verification(errors) = error else {
        panic!("semantic verification must reject the invalid identity");
    };
    assert!(errors.contains(DiagnosticCode::InvalidIdentity));
    assert_eq!(resources.storage(), 13);
}

fn empty_owner_sweep_module(invalid: bool) -> Module {
    let mut identifier = String::with_capacity(4096);
    if !invalid {
        identifier.push('m');
    }
    Module::new(identifier)
}

fn empty_owner_sweep_peak(invalid: bool) -> usize {
    let identifier_bytes = if invalid { 0 } else { 1 };
    // Header20, ID length4, three empty roster counts12, then the ID payload.
    let wire_bytes = 36 + identifier_bytes;
    let diagnostics = if invalid {
        // One fixed Diagnostic row and its 33-byte message; the owned ID is empty.
        std::mem::size_of::<crate::Diagnostic>().div_ceil(std::mem::size_of::<usize>()) + 33
    } else {
        0
    };
    std::mem::size_of::<VerifiedCanonicalKernelIrV12>()
        + wire_bytes
        + std::mem::size_of::<Module>()
        + identifier_bytes
        + diagnostics
}

#[test]
fn full_owner_work_prefix_sweep_reaches_decoder_diagnostics_and_hash() {
    const WORK_PREFIX: usize = 11;
    const STORAGE_PREFIX: usize = 7;
    // Independently derived empty-module paths, not limits sampled from a run:
    // Count10 + Encode(Count10 + wire + patch4) +
    // Decode(read wire + 2*ID bytes + Count10 + Compare(wire + patch4 + tokens10 + end1)).
    const VALID_PREVERIFY: usize = 162;
    const INVALID_PREVERIFY: usize = 157;
    // Valid: preverify162 + semantic5 + equality37 + hash(4+39+2+8+37).
    const VALID_COMPLETE: usize = 294;
    // Invalid: preverify157 + count(5+1+33+5) +
    // materialize(5+1+33 + ID copy(5+2) + message(33+33+1) + 5) + finish1.
    const INVALID_COMPLETE: usize = 320;
    // Both literal paths above fit this independently chosen envelope. Sweeping
    // every limit also covers rejected charges between the named boundaries.
    const SWEEP_WORK: usize = 512;

    for invalid in [false, true] {
        let source = empty_owner_sweep_module(invalid);
        let snapshot = source.clone();
        let source_capacity = source.id.retained_capacity_bytes();
        let expected_bytes = encode_module_v12(&source).unwrap();
        let expected_errors = verify_module(&source).err();
        assert_eq!(expected_errors.is_some(), invalid);
        assert_eq!(expected_bytes.len(), if invalid { 36 } else { 37 });
        assert!(source_capacity > source.id.as_str().len());
        let peak = STORAGE_PREFIX + empty_owner_sweep_peak(invalid);
        let complete = if invalid {
            INVALID_COMPLETE
        } else {
            VALID_COMPLETE
        };
        let mut saw_encoder_denial = false;
        let mut saw_decoder_denial = false;
        let mut saw_semantic_denial = false;
        let mut saw_owned_diagnostic_denial = false;
        let mut saw_hash_denial = false;
        let mut saw_terminal = false;

        for seeded_history in [false, true] {
            for allowance in 0..=SWEEP_WORK {
                let limit = WORK_PREFIX + allowance;
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                work.charge_work(WORK_PREFIX).unwrap();
                let prior_work_failure = seeded_history.then_some(limit + 1);
                if seeded_history {
                    assert!(work.charge_work(allowance + 1).is_err());
                    assert_eq!(work.work(), WORK_PREFIX);
                }
                let mut resources =
                    CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, peak);
                resources.reserve_storage(STORAGE_PREFIX).unwrap();
                let prior_storage_failure = seeded_history.then_some(peak + 1);
                if seeded_history {
                    assert!(
                        resources
                            .reserve_storage(peak - STORAGE_PREFIX + 1)
                            .is_err()
                    );
                    assert_eq!(resources.storage(), STORAGE_PREFIX);
                }

                let result =
                    VerifiedCanonicalKernelIrV12::from_module_ref_with_verification_budget_v12(
                        &source,
                        &mut resources,
                    );
                assert_eq!(resources.storage(), STORAGE_PREFIX);
                assert!((WORK_PREFIX..=limit).contains(&resources.work()));
                assert!((STORAGE_PREFIX..=peak).contains(&resources.peak_storage()));
                assert_eq!(resources.failed_storage(), prior_storage_failure);
                let rejected = match result {
                    Ok((owner, receipt)) => {
                        assert!(!invalid);
                        assert!(allowance >= complete);
                        assert_eq!(owner.canonical_bytes(), expected_bytes);
                        assert_eq!(
                            receipt.retained_storage(),
                            std::mem::size_of::<VerifiedCanonicalKernelIrV12>() + 37
                        );
                        assert_eq!(resources.work(), WORK_PREFIX + VALID_COMPLETE);
                        assert_eq!(resources.peak_storage(), peak);
                        saw_terminal = true;
                        None
                    }
                    Err(CanonicalKernelIrReplayAdmissionErrorV12::Verification(errors)) => {
                        assert!(invalid);
                        assert!(allowance >= complete);
                        assert_eq!(Some(&errors), expected_errors.as_ref());
                        assert_eq!(resources.work(), WORK_PREFIX + INVALID_COMPLETE);
                        assert_eq!(resources.peak_storage(), peak);
                        saw_terminal = true;
                        None
                    }
                    Err(error) => {
                        assert!(allowance < complete, "{error:?}");
                        let failure = match error {
                            CanonicalKernelIrReplayAdmissionErrorV12::Encode(
                                KernelIrEncodeError::WorkLimit(failure),
                            ) => {
                                saw_encoder_denial = true;
                                failure
                            }
                            CanonicalKernelIrReplayAdmissionErrorV12::Decode(
                                KernelIrDecodeError::WorkLimit(failure),
                            )
                            | CanonicalKernelIrReplayAdmissionErrorV12::Decode(
                                KernelIrDecodeError::Encode(KernelIrEncodeError::WorkLimit(
                                    failure,
                                )),
                            ) => {
                                saw_decoder_denial = true;
                                let encoded_prefix = if invalid { 60 } else { 61 };
                                if allowance == encoded_prefix {
                                    assert_eq!(resources.work(), WORK_PREFIX + encoded_prefix);
                                    assert_eq!(failure.actual(), WORK_PREFIX + encoded_prefix + 8);
                                }
                                failure
                            }
                            CanonicalKernelIrReplayAdmissionErrorV12::Resource(
                                CanonicalKernelIrVerificationResourceErrorV1::Work(failure),
                            ) => {
                                let preverify = if invalid {
                                    INVALID_PREVERIFY
                                } else {
                                    VALID_PREVERIFY
                                };
                                assert!(resources.work() >= WORK_PREFIX + preverify);
                                saw_semantic_denial = true;
                                if invalid && allowance == INVALID_COMPLETE - 1 {
                                    // Finish denies only after the complete diagnostic String
                                    // exists. Returning the error must drop it and restore floor.
                                    assert_eq!(
                                        resources.work(),
                                        WORK_PREFIX + INVALID_COMPLETE - 1
                                    );
                                    assert_eq!(failure.actual(), WORK_PREFIX + INVALID_COMPLETE);
                                    assert_eq!(resources.peak_storage(), peak);
                                    saw_owned_diagnostic_denial = true;
                                }
                                failure
                            }
                            CanonicalKernelIrReplayAdmissionErrorV12::Canonical(
                                MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit(failure),
                            ) => {
                                assert!(!invalid);
                                assert_eq!(resources.work(), WORK_PREFIX + 204);
                                assert_eq!(failure.actual(), WORK_PREFIX + VALID_COMPLETE);
                                saw_hash_denial = true;
                                failure
                            }
                            unexpected => panic!("unexpected full-owner rejection: {unexpected:?}"),
                        };
                        assert_eq!(failure.limit(), limit);
                        assert!(failure.actual() > limit);
                        Some(failure.actual())
                    }
                };
                assert_eq!(
                    resources.work_budget_v1().failed_work(),
                    prior_work_failure.or(rejected)
                );
                assert_eq!(source, snapshot);
                assert_eq!(source.id.retained_capacity_bytes(), source_capacity);
            }
        }
        assert!(saw_encoder_denial);
        assert!(saw_decoder_denial);
        assert!(saw_semantic_denial);
        assert_eq!(saw_owned_diagnostic_denial, invalid);
        assert_eq!(saw_hash_denial, !invalid);
        assert!(saw_terminal);
        assert_eq!(encode_module_v12(&source).unwrap(), expected_bytes);
    }
}

#[test]
fn full_owner_storage_prefix_sweep_keeps_failures_and_owned_error_accounting() {
    const WORK_PREFIX: usize = 11;
    const STORAGE_PREFIX: usize = 7;
    // The independent work derivation in the preceding test bounds both paths
    // by 320; storage sweeps use 512 and must never incur a fresh work failure.
    const WORK_ALLOWANCE: usize = 512;

    for invalid in [false, true] {
        let source = empty_owner_sweep_module(invalid);
        let snapshot = source.clone();
        let source_capacity = source.id.retained_capacity_bytes();
        let expected_bytes = encode_module_v12(&source).unwrap();
        let expected_errors = verify_module(&source).err();
        let expected_peak = empty_owner_sweep_peak(invalid);
        let mut saw_decoder_denial = false;
        let mut saw_diagnostic_denial = false;
        let mut saw_terminal = false;

        for seeded_history in [false, true] {
            for allowance in 0..=expected_peak {
                let storage_limit = STORAGE_PREFIX + allowance;
                let work_limit = WORK_PREFIX + WORK_ALLOWANCE;
                let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                work.charge_work(WORK_PREFIX).unwrap();
                let prior_work_failure = seeded_history.then_some(work_limit + 1);
                if seeded_history {
                    assert!(work.charge_work(WORK_ALLOWANCE + 1).is_err());
                    assert_eq!(work.work(), WORK_PREFIX);
                }
                let mut resources =
                    CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
                resources.reserve_storage(STORAGE_PREFIX).unwrap();
                let prior_storage_failure = seeded_history.then_some(storage_limit + 1);
                if seeded_history {
                    assert!(resources.reserve_storage(allowance + 1).is_err());
                    assert_eq!(resources.storage(), STORAGE_PREFIX);
                }

                let result =
                    VerifiedCanonicalKernelIrV12::from_module_ref_with_verification_budget_v12(
                        &source,
                        &mut resources,
                    );
                assert_eq!(resources.storage(), STORAGE_PREFIX);
                assert!((WORK_PREFIX..=work_limit).contains(&resources.work()));
                assert!((STORAGE_PREFIX..=storage_limit).contains(&resources.peak_storage()));
                assert_eq!(resources.work_budget_v1().failed_work(), prior_work_failure);
                let rejected = match result {
                    Ok((owner, receipt)) => {
                        assert!(!invalid);
                        assert_eq!(allowance, expected_peak);
                        assert_eq!(owner.canonical_bytes(), expected_bytes);
                        assert_eq!(
                            receipt.retained_storage(),
                            std::mem::size_of::<VerifiedCanonicalKernelIrV12>() + 37
                        );
                        assert_eq!(resources.work(), WORK_PREFIX + 294);
                        assert_eq!(resources.peak_storage(), STORAGE_PREFIX + expected_peak);
                        saw_terminal = true;
                        None
                    }
                    Err(CanonicalKernelIrReplayAdmissionErrorV12::Verification(errors)) => {
                        assert!(invalid);
                        assert_eq!(allowance, expected_peak);
                        assert_eq!(Some(&errors), expected_errors.as_ref());
                        assert_eq!(resources.work(), WORK_PREFIX + 320);
                        assert_eq!(resources.peak_storage(), STORAGE_PREFIX + expected_peak);
                        saw_terminal = true;
                        None
                    }
                    Err(error) => {
                        assert!(allowance < expected_peak, "{error:?}");
                        let failure = match error {
                            CanonicalKernelIrReplayAdmissionErrorV12::Decode(
                                KernelIrDecodeError::Resource(
                                    CanonicalKernelIrVerificationResourceErrorV1::Storage(failure),
                                ),
                            ) => {
                                saw_decoder_denial = true;
                                failure
                            }
                            CanonicalKernelIrReplayAdmissionErrorV12::Resource(
                                CanonicalKernelIrVerificationResourceErrorV1::Storage(failure),
                            ) => {
                                if invalid && allowance == expected_peak - 1 {
                                    // After preverify157, first diagnostic pass44 and the
                                    // second-pass copy/materialization precharges113.
                                    assert_eq!(resources.work(), WORK_PREFIX + 314);
                                    assert_eq!(failure.actual(), STORAGE_PREFIX + expected_peak);
                                    saw_diagnostic_denial = true;
                                }
                                failure
                            }
                            unexpected => panic!("unexpected full-owner rejection: {unexpected:?}"),
                        };
                        assert_eq!(failure.limit(), storage_limit);
                        assert!(failure.actual() > storage_limit);
                        Some(failure.actual())
                    }
                };
                assert_eq!(
                    resources.failed_storage(),
                    prior_storage_failure.or(rejected)
                );
                assert_eq!(source, snapshot);
                assert_eq!(source.id.retained_capacity_bytes(), source_capacity);
            }
        }
        // Only the nonempty identity requests a decoder String buffer.
        assert_eq!(saw_decoder_denial, !invalid);
        assert_eq!(saw_diagnostic_denial, invalid);
        assert!(saw_terminal);
        assert_eq!(encode_module_v12(&source).unwrap(), expected_bytes);
    }
}
