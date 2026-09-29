//! Bounded negative/preflight mechanics only. No approved runtime or backing owner
//! is manufactured; positive fixed-origin custody remains an integration obligation.
use super::*;
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_rustc_invocation::{CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcUnitV2};

const RUNTIME_CHARGE: usize = 719;
const PREFIX: usize = 31;

fn descriptor() -> Descriptor {
    let mut cwd = String::with_capacity(8192);
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

#[test]
fn preflight_prepays_actual_capacity_walk_on_the_original_budget() {
    let descriptor = descriptor();
    let input = RUNTIME_CHARGE + descriptor.retained_storage_bytes().unwrap();
    assert!(input >= RUNTIME_CHARGE + 8192);
    let mut work = Work::new(PREFIX + MEASURE_WORK);
    let floor = PREFIX + input;
    let mut b = Budget::new(&mut work, floor + CompilerInvocationBacking::FRAME_STORAGE);
    b.reserve_storage(floor).unwrap();
    b.charge_work(PREFIX).unwrap();
    let identity = b.work_ledger_identity_v1();
    assert_eq!(
        measure_inputs(RUNTIME_CHARGE, &descriptor, &mut b).unwrap(),
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
fn preflight_refuses_short_work_scratch_and_source_floor_without_refunding_history() {
    let descriptor = descriptor();
    let input = RUNTIME_CHARGE + descriptor.retained_storage_bytes().unwrap();
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
            measure_inputs(RUNTIME_CHARGE, &descriptor, &mut b),
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
    let descriptor = descriptor();
    let mut work = Work::new(MEASURE_WORK);
    let mut b = Budget::new(&mut work, PREFIX + CompilerInvocationBacking::FRAME_STORAGE);
    b.reserve_storage(PREFIX).unwrap();
    assert!(b.charge_work(usize::MAX).is_err());
    assert!(b.reserve_storage(usize::MAX).is_err());
    let failed_work = b.failed_work();
    let failed_storage = b.failed_storage();
    assert!(matches!(
        measure_inputs(usize::MAX, &descriptor, &mut b),
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
fn full_charge_keeps_all_four_sources_staged_data_and_complete_original_input() {
    let (input, invocation, rustc, interpreter, backend, proc_macro) =
        (1001, 2003, 3007, 4009, 5011, 6013);
    let charge =
        retained_storage_for(input, invocation, rustc, interpreter, backend, proc_macro).unwrap();
    assert_eq!(
        charge.additional_storage(),
        invocation
            + rustc
            + interpreter
            + backend
            + proc_macro
            + CompilerInvocationBacking::ENVELOPE
    );
    assert_eq!(
        charge.retained_storage(),
        input + charge.additional_storage()
    );
    let duplicate_charge = transfer_storage(rustc, interpreter, backend, proc_macro).unwrap();
    assert_eq!(
        staged_floor(charge.retained_storage(), duplicate_charge).unwrap(),
        input
            + invocation
            + 2 * (rustc + interpreter + backend + proc_macro)
            + CompilerInvocationBacking::ENVELOPE
    );
    // The whole owner can stay in the native child's retained cleanup state.
    fn send_static<T: Send + 'static>() {}
    send_static::<CompilerInvocationBacking>();
}

#[test]
fn every_retention_addition_refuses_arithmetic_overflow() {
    for (input, invocation, rustc, interpreter, backend, proc_macro) in [
        (usize::MAX, 1, 1, 1, 1, 1),
        (1, usize::MAX, 1, 1, 1, 1),
        (1, 1, usize::MAX, 1, 1, 1),
        (1, 1, 1, usize::MAX, 1, 1),
        (1, 1, 1, 1, usize::MAX, 1),
        (1, 1, 1, 1, 1, usize::MAX),
    ] {
        assert!(matches!(
            retained_storage_for(input, invocation, rustc, interpreter, backend, proc_macro),
            Err(CompilerInvocationBackingError::Resource(
                Resource::Arithmetic
            ))
        ));
    }
    assert!(staged_floor(usize::MAX, 1).is_err());
    for (rustc, interpreter, backend, proc_macro) in [
        (usize::MAX, 1, 1, 1),
        (1, usize::MAX, 1, 1),
        (1, 1, usize::MAX, 1),
        (1, 1, 1, usize::MAX),
    ] {
        assert!(transfer_storage(rustc, interpreter, backend, proc_macro).is_err());
    }
}
