//! Inert descriptors only; no runtime, approved owner, Command or process fixture.
use super::*;
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcUnitV2, ValidationError,
    decode_descriptor_v3, encode_descriptor_v3,
};
use std::{ffi::OsString, os::unix::ffi::OsStringExt};

const PREFIX: usize = 29;
const BACKEND: &str = "-Zcodegen-backend=/proc/./self/fd/198";

fn descriptor(
    arguments: Vec<String>,
    entries: Vec<(OsString, OsString)>,
) -> RustcInvocationDescriptorV3 {
    let v2 = RustcInvocationDescriptorV2::new(
        [0x11; 32],
        [0x22; 32],
        RustcUnitV2::new("/workspace/project", arguments).unwrap(),
        CompileEnvironmentV2::from_child_environment(entries).unwrap(),
    )
    .unwrap();
    let closure = CompilerClosureV2::new(
        [0x31; 32], [0x32; 32], [0x33; 32], [0x11; 32], [0x35; 32], [0x22; 32],
    )
    .unwrap();
    RustcInvocationDescriptorV3::new(v2, closure).unwrap()
}

fn environment() -> Vec<(OsString, OsString)> {
    [
        ("FE2O3_TARGET", "gfx942:xnack-"),
        ("FE2O3_HSACO_DIR", "/proc/self/fd/197"),
    ]
    .map(|(key, value)| (key.into(), value.into()))
    .into()
}

fn fixture() -> RustcInvocationDescriptorV3 {
    let mut entries = environment();
    entries.extend([
        ("Z_INPUT".into(), "x=y\nvalue".into()),
        ("A_EMPTY".into(), "".into()),
        ("LANG".into(), "C.UTF-8".into()),
    ]);
    descriptor(
        [
            "/toolchains/rustc",
            "--crate-name",
            "exact_unit",
            "",
            "same",
            "same",
            "space and \"quote\"",
            "\u{03bb}.rs",
            BACKEND,
        ]
        .map(str::to_owned)
        .into(),
        entries,
    )
}

fn stage(descriptor: &RustcInvocationDescriptorV3) -> StagedRustcInvocationV1 {
    let mut work = Work::new(StagedRustcInvocationV1::STAGING_WORK);
    let source = descriptor.retained_storage_bytes().unwrap();
    let mut budget = Budget::new(&mut work, source + StagedRustcInvocationV1::STAGING_SCRATCH);
    budget.reserve_storage(source).unwrap();
    let (staged, charge) = StagedRustcInvocationV1::stage(descriptor, &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    staged
}

#[test]
fn preserves_exact_argv_complete_environment_and_untranslated_cwd() {
    let descriptor = fixture();
    let encoded = encode_descriptor_v3(&descriptor).unwrap();
    let staged = stage(&descriptor);
    assert_eq!(staged.arguments().len(), descriptor.rustc().argv().len());
    for (staged, original) in staged.arguments().iter().zip(descriptor.rustc().argv()) {
        assert_eq!(staged.as_bytes(), original.as_bytes());
        assert_eq!(staged.as_bytes_with_nul().last(), Some(&0));
    }
    assert_eq!(
        staged.arguments().first().unwrap().as_bytes(),
        b"/toolchains/rustc"
    );
    assert_eq!(
        staged.arguments().last().unwrap().as_bytes(),
        BACKEND.as_bytes()
    );
    assert_eq!(
        staged.environment().len(),
        descriptor.compile_environment().entries().len()
    );
    for (staged, original) in staged
        .environment()
        .iter()
        .zip(descriptor.compile_environment().entries())
    {
        assert_eq!(
            staged.as_bytes(),
            format!("{}={}", original.key(), original.value()).as_bytes()
        );
    }
    assert_eq!(staged.environment()[0].as_bytes(), b"A_EMPTY=");
    assert_eq!(staged.working_directory().to_bytes(), b"/workspace/project");
    assert_eq!(encode_descriptor_v3(&descriptor).unwrap(), encoded);
}

#[test]
fn source_and_result_backing_overlap_on_original_ledger() {
    let descriptor = fixture();
    let source = descriptor.retained_storage_bytes().unwrap();
    let floor = PREFIX + source;
    let mut work = Work::new(StagedRustcInvocationV1::STAGING_WORK + 5);
    let mut budget = Budget::new(&mut work, floor + StagedRustcInvocationV1::STAGING_SCRATCH);
    budget.charge_work(5).unwrap();
    budget.reserve_storage(floor).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let (staged, charge) = StagedRustcInvocationV1::stage(&descriptor, &mut budget).unwrap();
    assert!(identity == budget.work_ledger_identity_v1());
    assert_eq!(budget.work(), 5 + StagedRustcInvocationV1::STAGING_WORK);
    assert_eq!(budget.storage(), floor);
    assert_eq!(
        budget.peak_storage(),
        floor + StagedRustcInvocationV1::STAGING_SCRATCH
    );
    let measured = size_of::<(StagedRustcInvocationV1, RustcInvocationStagingChargeV1)>()
        + (staged.arguments.capacity() + staged.environment.capacity()) * size_of::<CString>()
        + staged
            .arguments
            .iter()
            .chain(&staged.environment)
            .map(|s| s.as_bytes_with_nul().len())
            .sum::<usize>()
        + staged.working_directory.as_bytes_with_nul().len();
    assert_eq!(charge.additional_storage(), measured);
    assert_eq!(staged.retained_storage(), measured);
    budget.reserve_storage(measured).unwrap();
    assert_eq!(budget.storage(), floor + measured);
    drop(staged);
    budget.release_storage(measured).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn short_source_floor_work_and_scratch_fail_on_same_budget() {
    let descriptor = fixture();
    let source = descriptor.retained_storage_bytes().unwrap();
    for case in 0..4 {
        let floor = source - usize::from(case == 0);
        let work_limit = match case {
            1 => MEASURE_WORK - 1,
            2 => StagedRustcInvocationV1::STAGING_WORK - 1,
            _ => StagedRustcInvocationV1::STAGING_WORK,
        };
        let storage_limit =
            floor + StagedRustcInvocationV1::STAGING_SCRATCH - usize::from(case == 3);
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let identity = budget.work_ledger_identity_v1();
        assert!(matches!(
            StagedRustcInvocationV1::stage(&descriptor, &mut budget),
            Err(RustcInvocationStagingErrorV1::Resource(_))
        ));
        assert!(identity == budget.work_ledger_identity_v1());
        assert_eq!(budget.storage(), floor);
        if case == 0 {
            assert_eq!(budget.work(), MEASURE_WORK);
        } else if case < 3 {
            assert!(budget.failed_work().is_some());
        } else {
            assert_eq!(
                budget.failed_storage(),
                Some(floor + StagedRustcInvocationV1::STAGING_SCRATCH)
            );
        }
    }
}

#[test]
fn first_denial_history_survives_successful_staging() {
    let descriptor = fixture();
    let source = descriptor.retained_storage_bytes().unwrap();
    let mut work = Work::new(StagedRustcInvocationV1::STAGING_WORK);
    let mut budget = Budget::new(&mut work, source + StagedRustcInvocationV1::STAGING_SCRATCH);
    budget.reserve_storage(source).unwrap();
    assert!(budget.charge_work(usize::MAX).is_err());
    assert!(budget.reserve_storage(usize::MAX).is_err());
    let failed_work = budget.failed_work();
    let failed_storage = budget.failed_storage();
    let (staged, _) = StagedRustcInvocationV1::stage(&descriptor, &mut budget).unwrap();
    drop(staged);
    assert_eq!(budget.failed_work(), failed_work);
    assert_eq!(budget.failed_storage(), failed_storage);
    assert_eq!(budget.storage(), source);
}

#[test]
fn descriptor_boundaries_reject_nul_non_utf8_and_cwd_aliases() {
    for arguments in [
        vec!["/rustc\0x".into(), BACKEND.into()],
        vec!["/rustc".into(), "x\0y".into(), BACKEND.into()],
    ] {
        assert!(RustcUnitV2::new("/workspace/project", arguments).is_err());
    }
    for cwd in [
        "relative",
        "/workspace/./project",
        "/workspace//project",
        "/workspace/../project",
        "/workspace/\0project",
    ] {
        assert!(RustcUnitV2::new(cwd, vec!["/rustc".into(), BACKEND.into()]).is_err());
    }
    for (key, value) in [
        (OsString::from_vec(vec![0xff]), "value".into()),
        ("KEY".into(), OsString::from_vec(vec![0xff])),
        ("KEY\0".into(), "value".into()),
        ("KEY".into(), "value\0".into()),
    ] {
        assert!(CompileEnvironmentV2::from_child_environment([(key, value)]).is_err());
    }
    let descriptor = fixture();
    for field in [b"exact_unit".as_slice(), b"/workspace/project".as_slice()] {
        let mut bytes = encode_descriptor_v3(&descriptor).unwrap();
        let offset = bytes
            .windows(field.len())
            .position(|window| window == field)
            .unwrap();
        bytes[offset] = 0xff;
        assert!(decode_descriptor_v3(&bytes).is_err());
    }
    let mut total = 0;
    assert!(c_string(&[b"x\0y"], &mut total).is_err());
    assert_eq!(total, 0);
}

#[test]
fn maximum_counts_and_field_lengths_preserve_every_byte() {
    let mut argv = vec![String::new(); MAX_RUSTC_ARGUMENTS_V2];
    argv[0] = "/rustc".into();
    argv[1] = "x".repeat(MAX_ARGUMENT_BYTES_V2);
    *argv.last_mut().unwrap() = BACKEND.into();
    let mut entries = environment();
    for index in entries.len()..MAX_COMPILE_ENVIRONMENT_ENTRIES_V2 {
        entries.push((format!("INPUT_{index:04}").into(), "".into()));
    }
    entries[2].1 = "v".repeat(MAX_ENVIRONMENT_VALUE_BYTES_V2).into();
    let staged = stage(&descriptor(argv, entries));
    assert_eq!(staged.arguments().len(), MAX_RUSTC_ARGUMENTS_V2);
    assert_eq!(
        staged.environment().len(),
        MAX_COMPILE_ENVIRONMENT_ENTRIES_V2
    );
    assert_eq!(
        staged.arguments()[1].as_bytes().len(),
        MAX_ARGUMENT_BYTES_V2
    );
    let value = "v".repeat(MAX_ENVIRONMENT_VALUE_BYTES_V2);
    assert!(
        staged
            .environment()
            .iter()
            .any(|s| s.as_bytes().ends_with(value.as_bytes()))
    );
    assert!(matches!(
        RustcUnitV2::new("/", vec![String::new(); MAX_RUSTC_ARGUMENTS_V2 + 1]),
        Err(ValidationError::TooMany { .. })
    ));
}

#[test]
fn cumulative_byte_bound_refuses_before_constructing_next_c_string() {
    let mut total = MAX_C_BYTES - 2;
    assert_eq!(c_string(&[b"x"], &mut total).unwrap().as_bytes(), b"x");
    assert_eq!(total, MAX_C_BYTES);
    assert!(c_string(&[b""], &mut total).is_err());
    assert_eq!(total, MAX_C_BYTES);
    assert!(bounded_vec::<CString>(usize::MAX).is_err());
}
