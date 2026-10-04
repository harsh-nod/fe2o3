//! Inert sealed-descriptor accounting, not cargo authorship or native execution.
//! The separate ignored preparation tests require genuine installed approval;
//! these local fixtures cannot manufacture an approved runtime or backing.
use super::*;
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_rustc_invocation::{CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcUnitV2};

const RUNTIME_CHARGE: usize = 719;
const PREFIX: usize = 31;

fn descriptor() -> Descriptor {
    descriptor_with_cwd_capacity(8192)
}

fn descriptor_with_cwd_capacity(capacity: usize) -> Descriptor {
    let mut cwd = String::with_capacity(capacity);
    cwd.push_str("/workspace/project");
    let v2 = RustcInvocationDescriptorV2::new(
        [0x11; 32],
        [0x22; 32],
        RustcUnitV2::new(
            cwd,
            vec![
                "/toolchains/rustc".into(),
                "-Zcodegen-backend=/proc/./self/fd/198".into(),
            ],
        )
        .unwrap(),
        CompileEnvironmentV2::from_child_environment([
            ("FE2O3_TARGET".into(), "gfx942:xnack-".into()),
            ("FE2O3_HSACO_DIR".into(), "/proc/self/fd/197".into()),
        ])
        .unwrap(),
    )
    .unwrap();
    let closure = CompilerClosureV2::new(
        [0x31; 32], [0x32; 32], [0x33; 32], [0x11; 32], [0x35; 32], [0x22; 32],
    )
    .unwrap();
    Descriptor::new(v2, closure).unwrap()
}

fn capture() -> Capture {
    Capture::create(descriptor()).unwrap()
}

#[test]
fn preflight_keeps_full_sealed_owner_on_the_original_budget() {
    let capture = capture();
    let input = RUNTIME_CHARGE + capture.native_retained_storage().unwrap();
    assert!(input > RUNTIME_CHARGE + capture.descriptor().retained_storage_bytes().unwrap());
    let mut work = Work::new(PREFIX + MEASURE_WORK);
    let floor = PREFIX + input;
    let mut b = Budget::new(&mut work, floor + CompilerInvocationBacking::FRAME_STORAGE);
    b.reserve_storage(floor).unwrap();
    b.charge_work(PREFIX).unwrap();
    let identity = b.work_ledger_identity_v1();
    assert_eq!(
        measure_inputs(RUNTIME_CHARGE, &capture, &mut b).unwrap(),
        input
    );
    assert!(identity == b.work_ledger_identity_v1());
    assert_eq!(b.storage(), floor);
    assert_eq!(
        b.peak_storage(),
        floor + CompilerInvocationBacking::FRAME_STORAGE
    );
    assert_eq!(b.work(), PREFIX + MEASURE_WORK);
}

#[test]
fn preparation_input_floor_includes_the_complete_consumed_output_owner() {
    let capture = capture();
    let input = RUNTIME_CHARGE + capture.native_retained_storage().unwrap();
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(
        &mut work,
        input + Output::STORAGE + CompilerInvocationBacking::FRAME_STORAGE,
    );
    b.reserve_storage(input).unwrap();
    assert!(matches!(
        measure_inputs(RUNTIME_CHARGE + Output::STORAGE, &capture, &mut b),
        Err(CompilerInvocationBackingError::Resource(
            Resource::Accounting
        ))
    ));
    assert_eq!(b.storage(), input);
    b.reserve_storage(Output::STORAGE).unwrap();
    assert_eq!(
        measure_inputs(RUNTIME_CHARGE + Output::STORAGE, &capture, &mut b).unwrap(),
        input + Output::STORAGE
    );
}

#[test]
fn preflight_refuses_short_work_scratch_and_source_floor_without_refunding_history() {
    let capture = capture();
    let input = RUNTIME_CHARGE + capture.native_retained_storage().unwrap();
    for case in 0..4 {
        let floor = input - usize::from(case == 0);
        let work_limit = match case {
            1 => ENTRY - 1,
            2 => MEASURE_WORK - 1,
            _ => MEASURE_WORK,
        };
        let limit = floor + CompilerInvocationBacking::FRAME_STORAGE - usize::from(case == 3);
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, limit);
        b.reserve_storage(floor).unwrap();
        let identity = b.work_ledger_identity_v1();
        assert!(matches!(
            measure_inputs(RUNTIME_CHARGE, &capture, &mut b),
            Err(CompilerInvocationBackingError::Resource(_))
        ));
        assert!(identity == b.work_ledger_identity_v1());
        assert_eq!(b.storage(), floor);
        match case {
            0 => {
                assert_eq!(b.work(), MEASURE_WORK);
                // The temporary frame must not cover this missing input byte.
                assert!(b.peak_storage() > input);
                assert!(b.failed_storage().is_none());
            }
            1 | 2 => assert!(b.failed_work().is_some()),
            _ => assert_eq!(
                b.failed_storage(),
                Some(floor + CompilerInvocationBacking::FRAME_STORAGE)
            ),
        }
    }
}

#[test]
fn preflight_overflow_restores_entry_storage_and_preserves_first_denials() {
    let capture = capture();
    let mut work = Work::new(MEASURE_WORK);
    let mut b = Budget::new(&mut work, PREFIX + CompilerInvocationBacking::FRAME_STORAGE);
    b.reserve_storage(PREFIX).unwrap();
    assert!(b.charge_work(usize::MAX).is_err());
    assert!(b.reserve_storage(usize::MAX).is_err());
    let failed_work = b.failed_work();
    let failed_storage = b.failed_storage();
    assert!(matches!(
        measure_inputs(usize::MAX, &capture, &mut b),
        Err(CompilerInvocationBackingError::Resource(
            Resource::Arithmetic
        ))
    ));
    assert_eq!(b.storage(), PREFIX);
    assert_eq!(b.work(), MEASURE_WORK);
    assert_eq!(b.failed_work(), failed_work);
    assert_eq!(b.failed_storage(), failed_storage);
}

#[test]
fn preflight_rejects_oversized_sealed_owner_and_preserves_original_account_history() {
    let ordinary = capture();
    let bound = ordinary.native_retained_storage().unwrap();
    let oversized = Capture::create(descriptor_with_cwd_capacity(2 * bound)).unwrap();
    assert_eq!(oversized.descriptor(), ordinary.descriptor());
    assert!(oversized.descriptor().retained_storage_bytes().unwrap() > bound);
    // Separately fund the exact supplied decoded allocations and the maximum
    // sealed backing. This negative must reach the capacity guard, not fail
    // because its budget ran out or its claimed original owner was underfunded.
    let floor = ordinary.native_retained_storage().unwrap()
        + oversized.descriptor().retained_storage_bytes().unwrap()
        + Capture::NATIVE_MAX_RETAINED_STORAGE
        + RUNTIME_CHARGE
        + Output::STORAGE;
    let limit = floor + CompilerInvocationBacking::FRAME_STORAGE;
    let mut work = Work::new(PREFIX + 2 * MEASURE_WORK);
    let mut b = Budget::new(&mut work, limit);
    b.reserve_storage(floor).unwrap();
    b.charge_work(PREFIX).unwrap();
    assert!(b.charge_work(usize::MAX).is_err());
    assert!(b.reserve_storage(usize::MAX).is_err());
    let denials = (b.failed_work(), b.failed_storage());
    let ledger = b.work_ledger_identity_v1();
    let address = &b as *const Budget<'_>;
    assert!(matches!(
        measure_inputs(RUNTIME_CHARGE + Output::STORAGE, &oversized, &mut b),
        Err(CompilerInvocationBackingError::Capture(
            CaptureError::Rejected("retained invocation capacity exceeds native owner bound")
        ))
    ));
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), PREFIX + MEASURE_WORK);
    assert_eq!(b.peak_storage(), limit);
    assert_eq!(
        measure_inputs(RUNTIME_CHARGE + Output::STORAGE, &ordinary, &mut b).unwrap(),
        RUNTIME_CHARGE + Output::STORAGE + bound
    );
    assert_eq!(b.work(), PREFIX + 2 * MEASURE_WORK);
    assert_eq!((b.failed_work(), b.failed_storage()), denials);
    assert!(ledger == b.work_ledger_identity_v1());
    assert_eq!(address, &b as *const Budget<'_>);
    drop(oversized);
    drop(ordinary);
    assert_eq!(b.storage(), floor);
    b.release_storage(floor).unwrap();
}

#[test]
fn received_sealed_input_keeps_original_fd_charge_plus_native_admission_growth() {
    // Synthetic process text in a real sealed file is still only inert input.
    let source = capture();
    let source_storage = source.native_retained_storage().unwrap();
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, usize::MAX);
    b.reserve_storage(source_storage).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let address = &b as *const Budget<'_>;
    let (file, file_charge) = source.try_clone_for_transfer_native(&mut b).unwrap();
    assert_eq!(
        file_charge.additional_storage(),
        Capture::NATIVE_FILE_STORAGE
    );
    b.reserve_storage(file_charge.additional_storage()).unwrap();
    let (received, growth) = Capture::from_file_native(file, &mut b).unwrap();
    b.reserve_storage(growth.additional_storage()).unwrap();
    let full = received.native_retained_storage().unwrap();
    assert_eq!(
        full,
        Capture::NATIVE_FILE_STORAGE + growth.additional_storage()
    );
    assert_eq!(received.descriptor(), source.descriptor());
    drop(source);
    b.release_storage(source_storage).unwrap();
    b.reserve_storage(RUNTIME_CHARGE + Output::STORAGE).unwrap();
    let input = full + RUNTIME_CHARGE + Output::STORAGE;
    assert_eq!(b.storage(), input);
    received.revalidate_native(&mut b).unwrap();
    assert_eq!(
        measure_inputs(RUNTIME_CHARGE + Output::STORAGE, &received, &mut b).unwrap(),
        input
    );
    // Admission growth alone, and the former plain descriptor charge, must
    // both fail the outer floor even when temporary frames would cover it.
    for reserved in [
        growth.additional_storage(),
        received.descriptor().retained_storage_bytes().unwrap(),
    ] {
        let missing = full.checked_sub(reserved).unwrap();
        assert!(missing > 0);
        b.release_storage(missing).unwrap();
        assert!(matches!(
            measure_inputs(RUNTIME_CHARGE + Output::STORAGE, &received, &mut b),
            Err(CompilerInvocationBackingError::Resource(
                Resource::Accounting
            ))
        ));
        assert_eq!(b.storage(), input - missing);
        b.reserve_storage(missing).unwrap();
    }
    assert!(ledger == b.work_ledger_identity_v1());
    assert_eq!(address, &b as *const Budget<'_>);
    let spent = b.work();
    drop(received);
    assert_eq!(b.storage(), input);
    assert_eq!(b.work(), spent);
    b.release_storage(input).unwrap();
}

#[test]
fn full_charge_keeps_complete_inventory_transfer_and_original_input() {
    let (input, invocation, sources, staged_four) = (1001, 2003, 30007, 19040);
    let charge = retained_storage_for(input, invocation, sources).unwrap();
    assert_eq!(
        charge.additional_storage(),
        invocation + sources + CompilerInvocationBacking::ENVELOPE
    );
    assert_eq!(
        charge.retained_storage(),
        input + charge.additional_storage()
    );
    assert_eq!(
        staged_floor(charge.retained_storage(), staged_four).unwrap(),
        input + invocation + sources + staged_four + CompilerInvocationBacking::ENVELOPE
    );
    // The whole owner can stay in the native child's retained cleanup state.
    fn send_static<T: Send + 'static>() {}
    send_static::<CompilerInvocationBacking>();
}

#[test]
fn every_retention_addition_refuses_arithmetic_overflow() {
    for (input, invocation, sources) in [(usize::MAX, 1, 1), (1, usize::MAX, 1), (1, 1, usize::MAX)]
    {
        assert!(matches!(
            retained_storage_for(input, invocation, sources),
            Err(CompilerInvocationBackingError::Resource(
                Resource::Arithmetic
            ))
        ));
    }
    assert!(staged_floor(usize::MAX, 1).is_err());
}

#[path = "compiler_invocation_backing_production_tests.rs"]
mod production;
