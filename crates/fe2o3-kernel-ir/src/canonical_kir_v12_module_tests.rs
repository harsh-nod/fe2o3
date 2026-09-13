use super::*;
use crate::{AccessMode, AddressSpace, Function, Signature, TargetCapability, Type};

const WORK_PREFIX: usize = 11;
const STORAGE_PREFIX: usize = 7;
const WORK_ENVELOPE: usize = 100_000;
const STORAGE_ENVELOPE: usize = 1_000_000;

struct Probe {
    result: Result<
        (
            VerifiedCanonicalKernelIrModuleV12,
            CanonicalKernelIrReplayStorageV12,
        ),
        CanonicalKernelIrReplayAdmissionErrorV12,
    >,
    work: usize,
    peak: usize,
    rejected_work: Option<usize>,
    rejected_storage: Option<usize>,
}

fn admit(source: &Module, work_allowance: usize, storage_allowance: usize, seeded: bool) -> Probe {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK_PREFIX + work_allowance);
    work.charge_work(WORK_PREFIX).unwrap();
    let (result, peak, rejected_storage) = {
        let mut resources = CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            STORAGE_PREFIX + storage_allowance,
        );
        resources.reserve_storage(STORAGE_PREFIX).unwrap();
        if seeded {
            assert!(resources.charge_work(usize::MAX).is_err());
            assert!(resources.reserve_storage(usize::MAX).is_err());
        }
        let result =
            VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                source,
                &mut resources,
            );
        assert_eq!(resources.storage(), STORAGE_PREFIX);
        (result, resources.peak_storage(), resources.failed_storage())
    };
    Probe {
        result,
        work: work.work(),
        peak,
        rejected_work: work.failed_work(),
        rejected_storage,
    }
}

fn borrowed_source(invalid: bool) -> Module {
    let mut id = String::with_capacity(4096);
    if !invalid {
        id.push('m');
    }
    Module::new(id)
}

fn empty_retained(invalid: bool) -> usize {
    // Header20 + ID length4 + three empty roster counts12, then the ID payload.
    let id_bytes = usize::from(!invalid);
    std::mem::size_of::<VerifiedCanonicalKernelIrModuleV12>() + 36 + id_bytes + id_bytes
}

fn empty_peak(invalid: bool) -> usize {
    empty_retained(invalid)
        + if invalid {
            // One verifier-local Diagnostic row and its 33-byte owned message.
            std::mem::size_of::<crate::Diagnostic>().div_ceil(std::mem::size_of::<usize>()) + 33
        } else {
            0
        }
}

#[test]
fn connected_owner_empty_module_has_independent_exact_and_one_under_limits() {
    let source = borrowed_source(false);
    let snapshot = source.clone();
    let source_capacity = source.id.retained_capacity_bytes();
    // Count10 + Encode(Count10 + wire37 + patch4) = 61.
    // Decode(wire37 + ID validation/copy2 + Count10 +
    //        Compare(wire37 + patch4 + tokens10 + end1)) = 101.
    // Semantic5 + equality37 + hash(4 + domain39 + policy2 + length8 + wire37) = 132.
    const COMPLETE_WORK: usize = 294;
    const BEFORE_HASH: usize = 204;
    const BEFORE_ID_ALLOCATION: usize = 61 + 20 + 4 + 1 + 2;
    let retained = empty_retained(false);
    assert_eq!(encode_module_v12(&source).unwrap().len(), 37);

    let exact = admit(&source, COMPLETE_WORK, retained, false);
    assert_eq!(exact.work, WORK_PREFIX + COMPLETE_WORK);
    assert_eq!(exact.peak, STORAGE_PREFIX + retained);
    assert_eq!(exact.rejected_work, None);
    assert_eq!(exact.rejected_storage, None);
    let (owner, receipt) = exact.result.unwrap();
    assert_eq!(receipt.retained_storage(), retained);
    assert_eq!(owner.module(), &source);
    assert_eq!(
        owner.canonical().canonical_bytes(),
        encode_module_v12(&source).unwrap()
    );
    assert_eq!(
        owner.canonical().identity(),
        VerifiedCanonicalKernelIrV12::from_module(source.clone())
            .unwrap()
            .identity()
    );

    let short_work = admit(&source, COMPLETE_WORK - 1, retained, false);
    assert!(matches!(
        short_work.result,
        Err(CanonicalKernelIrReplayAdmissionErrorV12::Canonical(
            MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit(error)
        )) if error.actual() == WORK_PREFIX + COMPLETE_WORK
            && error.limit() == WORK_PREFIX + COMPLETE_WORK - 1
    ));
    assert_eq!(short_work.work, WORK_PREFIX + BEFORE_HASH);
    assert_eq!(short_work.peak, STORAGE_PREFIX + retained);
    assert_eq!(short_work.rejected_work, Some(WORK_PREFIX + COMPLETE_WORK));
    assert_eq!(short_work.rejected_storage, None);

    let short_storage = admit(&source, COMPLETE_WORK, retained - 1, false);
    assert!(matches!(
        short_storage.result,
        Err(CanonicalKernelIrReplayAdmissionErrorV12::Decode(KernelIrDecodeError::Resource(
            CanonicalKernelIrVerificationResourceErrorV1::Storage(error)
        ))) if error.actual() == STORAGE_PREFIX + retained
            && error.limit() == STORAGE_PREFIX + retained - 1
    ));
    assert_eq!(short_storage.work, WORK_PREFIX + BEFORE_ID_ALLOCATION);
    assert_eq!(short_storage.peak, STORAGE_PREFIX + retained - 1);
    assert_eq!(
        short_storage.rejected_storage,
        Some(STORAGE_PREFIX + retained)
    );
    assert_eq!(short_storage.rejected_work, None);
    assert_eq!(source, snapshot);
    assert_eq!(source.id.retained_capacity_bytes(), source_capacity);
}

#[test]
fn connected_owner_keeps_nested_inverse_payload_independent_of_source_owners() {
    let mut source = borrowed_source(false);
    source.functions.push(Function::declaration(
        "f",
        Signature::new(
            vec![Type::pointer(
                Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadOnly),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            )],
            vec![],
        ),
    ));
    source.functions.reserve(17);
    source.functions[0].signature.parameters.reserve(19);
    let snapshot = source.clone();
    let source_id_capacity = source.id.retained_capacity_bytes();
    let source_function_capacity = source.functions.capacity();
    let source_parameter_capacity = source.functions[0].signature.parameters.capacity();

    // Module37 + function ID5 + signature counts8 + pointer/slice/scalar8 +
    // absent body1 + function capability count4 = 63 wire bytes.
    // Inverse heap: Function row + module/function ID bytes2 + signature Type
    // row + a Box<Type> for each of pointer and slice (three Type rows total).
    const WIRE: usize = 63;
    let retained = std::mem::size_of::<VerifiedCanonicalKernelIrModuleV12>()
        + WIRE
        + std::mem::size_of::<Function>()
        + 2
        + 3 * std::mem::size_of::<Type>();
    let probe = admit(&source, WORK_ENVELOPE, STORAGE_ENVELOPE, false);
    let (owner, receipt) = probe.result.unwrap();
    assert_eq!(receipt.retained_storage(), retained);
    assert_eq!(owner.module(), &source);
    assert_eq!(owner.canonical().canonical_bytes().len(), WIRE);
    assert_eq!(owner.module().id.retained_capacity_bytes(), 1);
    assert_eq!(owner.module().functions.capacity(), 1);
    assert_eq!(
        owner.module().functions[0].signature.parameters.capacity(),
        1
    );
    assert_ne!(
        owner.module().id.as_str().as_ptr(),
        source.id.as_str().as_ptr()
    );
    assert_ne!(owner.module().functions.as_ptr(), source.functions.as_ptr());
    assert_ne!(
        owner.module().functions[0].signature.parameters.as_ptr(),
        source.functions[0].signature.parameters.as_ptr()
    );
    assert_eq!(source, snapshot);
    assert_eq!(source.id.retained_capacity_bytes(), source_id_capacity);
    assert_eq!(source.functions.capacity(), source_function_capacity);
    assert_eq!(
        source.functions[0].signature.parameters.capacity(),
        source_parameter_capacity
    );

    source.functions.clear();
    source.id = crate::ModuleId::new("different");
    drop(source);
    assert_eq!(owner.module(), &snapshot);
    assert_eq!(
        decode_module_v12(owner.canonical().canonical_bytes()).unwrap(),
        snapshot
    );
    owner.canonical().revalidate().unwrap();
}

#[test]
fn connected_owner_receipt_retains_extension_strings_and_tree_payload_only() {
    let mut source = borrowed_source(false);
    let mut namespace = String::with_capacity(4096);
    namespace.push_str("vendor");
    let mut name = String::with_capacity(4096);
    name.push_str("name");
    source
        .required_capabilities
        .insert(TargetCapability::Extension { namespace, name });
    let snapshot = source.clone();
    // One extension adds tag1 + namespace length4/payload6 + name length4/payload4.
    const WIRE: usize = 37 + 1 + 4 + 6 + 4 + 4;
    // The decoder's documented pinned-tree bound: three nodes with eleven
    // inline keys and fourteen pointer-sized fields, plus four key payloads.
    let key = std::mem::size_of::<TargetCapability>();
    let tree = 3 * (11 * key + 14 * std::mem::size_of::<usize>()) + 4 * key;
    let retained =
        std::mem::size_of::<VerifiedCanonicalKernelIrModuleV12>() + WIRE + 1 + tree + 6 + 4;
    let probe = admit(&source, WORK_ENVELOPE, STORAGE_ENVELOPE, false);
    assert!(probe.peak >= STORAGE_PREFIX + retained + 6 + 4);
    let (owner, receipt) = probe.result.unwrap();
    assert_eq!(owner.canonical().canonical_bytes().len(), WIRE);
    assert_eq!(receipt.retained_storage(), retained);
    assert_eq!(owner.module(), &snapshot);
    let TargetCapability::Extension { namespace, name } =
        owner.module().required_capabilities.first().unwrap()
    else {
        panic!("extension must survive the inverse");
    };
    assert_eq!(namespace.capacity(), 6);
    assert_eq!(name.capacity(), 4);
    source.required_capabilities.clear();
    drop(source);
    assert_eq!(owner.module(), &snapshot);
}

#[test]
fn connected_owner_transfer_reservation_enforces_coexisting_owner_floor() {
    let source = borrowed_source(false);
    let retained = empty_retained(false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK_ENVELOPE);
    work.charge_work(WORK_PREFIX).unwrap();
    let mut resources = CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut work,
        STORAGE_PREFIX + 2 * retained - 1,
    );
    resources.reserve_storage(STORAGE_PREFIX).unwrap();
    let (first, receipt) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &source,
            &mut resources,
        )
        .unwrap();
    assert_eq!(receipt.retained_storage(), retained);
    assert_eq!(resources.storage(), STORAGE_PREFIX);
    resources
        .reserve_storage(receipt.retained_storage())
        .unwrap();

    let error = VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
        &source,
        &mut resources,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        CanonicalKernelIrReplayAdmissionErrorV12::Decode(KernelIrDecodeError::Resource(
            CanonicalKernelIrVerificationResourceErrorV1::Storage(error)
        )) if error.actual() == STORAGE_PREFIX + 2 * retained
    ));
    assert_eq!(resources.storage(), STORAGE_PREFIX + retained);
    assert_eq!(
        resources.failed_storage(),
        Some(STORAGE_PREFIX + 2 * retained)
    );
    assert_eq!(first.module(), &source);
    drop(first);
    resources
        .release_storage(receipt.retained_storage())
        .unwrap();

    let (replacement, replacement_receipt) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &source,
            &mut resources,
        )
        .unwrap();
    assert_eq!(resources.storage(), STORAGE_PREFIX);
    assert_eq!(
        resources.failed_storage(),
        Some(STORAGE_PREFIX + 2 * retained)
    );
    assert_eq!(replacement_receipt, receipt);
    drop(replacement);
}

#[test]
fn connected_owner_prefix_sweeps_cover_failures_without_borrowing_the_source() {
    // Independent empty-module paths: valid294 (derived above), invalid320:
    // preverify157 + diagnostic count44 + materialization118 + finish1.
    // 512 is a bounded sweep envelope, not a sampled successful-run oracle.
    const COMPLETE_VALID: usize = 294;
    const COMPLETE_INVALID: usize = 320;
    const SWEEP_WORK: usize = 512;
    for invalid in [false, true] {
        let source = borrowed_source(invalid);
        let snapshot = source.clone();
        let source_capacity = source.id.retained_capacity_bytes();
        let expected_errors = verify_module(&source).err();
        let complete = if invalid {
            COMPLETE_INVALID
        } else {
            COMPLETE_VALID
        };
        let peak = empty_peak(invalid);

        for seeded in [false, true] {
            for allowance in 0..=SWEEP_WORK {
                let probe = admit(&source, allowance, peak, seeded);
                assert!(probe.work >= WORK_PREFIX);
                assert!(probe.work <= WORK_PREFIX + allowance);
                assert!(probe.peak >= STORAGE_PREFIX);
                assert!(probe.peak <= STORAGE_PREFIX + peak);
                assert_eq!(probe.rejected_storage, seeded.then_some(usize::MAX));
                if allowance < complete {
                    assert!(probe.result.is_err());
                    assert!(!matches!(
                        probe.result,
                        Err(CanonicalKernelIrReplayAdmissionErrorV12::Verification(_))
                    ));
                    if seeded {
                        assert_eq!(probe.rejected_work, Some(usize::MAX));
                    } else {
                        assert!(
                            probe
                                .rejected_work
                                .is_some_and(|actual| { actual > WORK_PREFIX + allowance })
                        );
                    }
                } else {
                    assert_eq!(probe.work, WORK_PREFIX + complete);
                    assert_eq!(probe.peak, STORAGE_PREFIX + peak);
                    assert_eq!(probe.rejected_work, seeded.then_some(usize::MAX));
                    match probe.result {
                        Ok((owner, receipt)) => {
                            assert!(!invalid);
                            assert_eq!(owner.module(), &source);
                            assert_eq!(receipt.retained_storage(), empty_retained(false));
                        }
                        Err(CanonicalKernelIrReplayAdmissionErrorV12::Verification(errors)) => {
                            assert!(invalid);
                            assert_eq!(Some(errors), expected_errors);
                        }
                        Err(error) => {
                            panic!("sufficient budget rejected the semantic path: {error}")
                        }
                    }
                }
                assert_eq!(source, snapshot);
                assert_eq!(source.id.retained_capacity_bytes(), source_capacity);
            }

            for allowance in 0..=peak {
                let probe = admit(&source, complete, allowance, seeded);
                assert!(probe.work >= WORK_PREFIX);
                assert!(probe.work <= WORK_PREFIX + complete);
                assert!(probe.peak >= STORAGE_PREFIX);
                assert!(probe.peak <= STORAGE_PREFIX + allowance);
                assert_eq!(probe.rejected_work, seeded.then_some(usize::MAX));
                if allowance < peak {
                    assert!(matches!(
                        probe.result,
                        Err(CanonicalKernelIrReplayAdmissionErrorV12::Resource(
                            CanonicalKernelIrVerificationResourceErrorV1::Storage(_)
                        )) | Err(CanonicalKernelIrReplayAdmissionErrorV12::Decode(
                            KernelIrDecodeError::Resource(
                                CanonicalKernelIrVerificationResourceErrorV1::Storage(_)
                            )
                        ))
                    ));
                    if seeded {
                        assert_eq!(probe.rejected_storage, Some(usize::MAX));
                    } else {
                        assert!(
                            probe
                                .rejected_storage
                                .is_some_and(|actual| { actual > STORAGE_PREFIX + allowance })
                        );
                    }
                } else {
                    assert_eq!(probe.work, WORK_PREFIX + complete);
                    assert_eq!(probe.peak, STORAGE_PREFIX + peak);
                    assert_eq!(probe.rejected_storage, seeded.then_some(usize::MAX));
                    match probe.result {
                        Ok((owner, receipt)) => {
                            assert!(!invalid);
                            assert_eq!(owner.module(), &source);
                            assert_eq!(receipt.retained_storage(), empty_retained(false));
                        }
                        Err(CanonicalKernelIrReplayAdmissionErrorV12::Verification(errors)) => {
                            assert!(invalid);
                            assert_eq!(Some(errors), expected_errors);
                        }
                        Err(error) => panic!("exact storage rejected the semantic path: {error}"),
                    }
                }
                assert_eq!(source, snapshot);
                assert_eq!(source.id.retained_capacity_bytes(), source_capacity);
            }
        }
    }
}
