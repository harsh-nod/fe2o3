use super::{Fixture, ProvisionedStaticExecutableMeasurementV1 as Provisioned};
use crate::{AdmittedIssuerProgramV2 as Program, IssuerProgramAdmissionErrorV2 as Error};
use ed25519_dalek::SigningKey;
use fe2o3_compiler_closure_capability::CompilerExecutionPolicyCapabilityV2 as Cap;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV2 as Policy,
    sealed_static_issuer_runtime_measurement_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableErrorV2 as ImageError,
    ProtectedStaticExecutableMeasurementV1 as ImageMeasurement,
    ProtectedStaticExecutableOperationV2 as Op, ProtectedStaticExecutableV2 as Image,
};

fn policy(f: &Fixture, runtime: Measurement) -> Cap {
    policy_for(f.issuer_measurement(), runtime)
}
fn policy_for(executable: Measurement, runtime: Measurement) -> Cap {
    let mut w = Work::new(100_000);
    let mut b = Budget::new(&mut w, 100_000);
    let (p, s) = Policy::new(
        7,
        executable,
        runtime,
        SigningKey::from_bytes(&[0x51; 32])
            .verifying_key()
            .to_bytes(),
        SigningKey::from_bytes(&[0x52; 32])
            .verifying_key()
            .to_bytes(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    Cap::create(p, &mut b).unwrap().0
}
fn measurement(f: &Fixture) -> ImageMeasurement {
    let m = f.measurement();
    ImageMeasurement::new(m.sha256(), m.byte_len(), 128 * 1024 * 1024).unwrap()
}
fn floor(f: &Fixture, p: &Cap) -> usize {
    2 * Image::file_storage(measurement(f)).unwrap() + p.retained_storage()
}
fn work(f: &Fixture) -> usize {
    Program::WORK
        + 2 * Cap::IO_WORK
        + 2 * Image::quota(measurement(f), Op::Admit).unwrap().work()
        + 2 * Image::quota(measurement(f), Op::Revalidate).unwrap().work()
}

#[test]
fn native_program_seals_aliased_sources_as_distinct_objects_with_exact_accounting() {
    let f = Fixture::new("native-program");
    let p = policy(&f, sealed_static_issuer_runtime_measurement_v1());
    let expected = p.policy().identity();
    let floor = floor(&f, &p);
    let mut w = Work::new(1_000_000_000);
    let mut b = Budget::new(&mut w, 10_000_000);
    b.reserve_storage(floor).unwrap();
    let source = f.open();
    let duplicate = source.try_clone().unwrap();
    let (program, delta) =
        Program::provision(source, f.measurement(), duplicate, p, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), work(&f));
    b.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(b.storage(), program.retained_storage());
    assert_eq!(program.policy().identity(), expected);
    assert_ne!(
        program.launcher_object_identity(),
        program.issuer_object_identity()
    );
    program.revalidate(&mut b).unwrap();
    let (launcher, s) = program.try_clone_launcher_for_launch(&mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    program
        .revalidate_launcher_clone(&launcher, &mut b)
        .unwrap();
    assert!(program.revalidate_issuer_clone(&launcher, &mut b).is_err());
    let (issuer, s) = program.try_clone_issuer_for_launch(&mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    program.revalidate_issuer_clone(&issuer, &mut b).unwrap();
    let text = format!("{program:?}");
    assert!(text.contains("none"));
    assert!(!text.contains("/proc/"));
    assert!(!text.contains("image: File"));
    drop((launcher, issuer));
    b.release_storage(2 * Image::file_storage(measurement(&f)).unwrap())
        .unwrap();
    let retained = program.retained_storage();
    drop(program);
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), 0);
}

#[test]
fn program_resource_refusals_restore_consumed_floor_without_refunding_work() {
    let f = Fixture::new("native-program-bounds");
    let mut peak = 0;
    for mode in 0..5 {
        let p = policy(&f, sealed_static_issuer_runtime_measurement_v1());
        let full = floor(&f, &p);
        let mut w = Work::new(work(&f) - usize::from(mode == 1));
        let mut b = Budget::new(
            &mut w,
            if mode == 2 {
                peak - 1
            } else if mode == 4 {
                peak
            } else {
                10_000_000
            },
        );
        let floor = full - usize::from(mode == 3);
        b.reserve_storage(floor).unwrap();
        let result = Program::provision(f.open(), f.measurement(), f.open(), p, &mut b);
        assert_eq!(b.storage(), floor);
        match mode {
            0 => {
                result.unwrap();
                peak = b.peak_storage();
                assert_eq!(b.work(), work(&f));
            }
            1 => {
                assert!(matches!(
                    result,
                    Err(Error::Image(ImageError::Resource(Resource::Work(_))))
                ));
                assert!(b.work() < work(&f));
            }
            2 => {
                assert!(matches!(
                    result,
                    Err(Error::Image(ImageError::Resource(Resource::Storage(_))))
                ));
                assert_eq!(b.failed_storage(), Some(peak));
            }
            3 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                assert_eq!(b.work(), 8);
            }
            _ => {
                result.unwrap();
                assert_eq!(b.peak_storage(), peak);
                assert_eq!(b.work(), work(&f));
            }
        }
    }
}

#[test]
fn native_policy_runtime_precedes_launcher_errors_and_bad_measurement_refuses() {
    let f = Fixture::new("native-program-precedence");
    let wrong = Provisioned::new([0x99; 32], f.measurement().byte_len()).unwrap();
    for wrong_runtime in [true, false] {
        let runtime = if wrong_runtime {
            Measurement::new([3; 32], 123).unwrap()
        } else {
            sealed_static_issuer_runtime_measurement_v1()
        };
        let p = policy(&f, runtime);
        let mut w = Work::new(1_000_000_000);
        let mut b = Budget::new(&mut w, 10_000_000);
        b.reserve_storage(floor(&f, &p)).unwrap();
        let result = Program::provision(f.open(), wrong, f.open(), p, &mut b);
        if wrong_runtime {
            assert!(matches!(result, Err(Error::RuntimePolicyMismatch)));
            assert_eq!(b.work(), Program::WORK + Cap::IO_WORK);
        } else {
            assert!(matches!(result, Err(Error::Image(_))));
        }
    }
}

#[test]
fn program_transfer_requires_the_whole_borrowed_owner_floor() {
    let f = Fixture::new("native-program-transfer-floor");
    let p = policy(&f, sealed_static_issuer_runtime_measurement_v1());
    let mut w = Work::new(1_000_000_000);
    let mut b = Budget::new(&mut w, 10_000_000);
    b.reserve_storage(floor(&f, &p)).unwrap();
    let (program, _) = Program::provision(f.open(), f.measurement(), f.open(), p, &mut b).unwrap();
    let mut w = Work::new(1_000_000_000);
    let mut b = Budget::new(&mut w, 10_000_000);
    b.reserve_storage(program.retained_storage() - 1).unwrap();
    assert!(matches!(
        program.try_clone_launcher_for_launch(&mut b),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(b.work(), 8);
}

#[test]
fn unequal_images_cover_every_borrowed_program_operation_at_exact_limits() {
    let l = Fixture::new("native-program-unequal-launcher");
    let i = Fixture::with_code("native-program-unequal-issuer", &[0x90, 0x90, 0xc3]);
    let p = policy(&i, sealed_static_issuer_runtime_measurement_v1());
    let mut w = Work::new(1_000_000_000);
    let mut b = Budget::new(&mut w, 10_000_000);
    b.reserve_storage(
        p.retained_storage()
            + Image::file_storage(measurement(&l)).unwrap()
            + Image::file_storage(measurement(&i)).unwrap(),
    )
    .unwrap();
    let (program, delta) =
        Program::provision(l.open(), l.measurement(), i.open(), p, &mut b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    for operation in 0..5 {
        let mut creation_w = Work::new(1_000_000_000);
        let mut creation_b = Budget::new(&mut creation_w, 10_000_000);
        creation_b
            .reserve_storage(program.retained_storage())
            .unwrap();
        let file = match operation {
            3 => Some(
                program
                    .try_clone_launcher_for_launch(&mut creation_b)
                    .unwrap()
                    .0,
            ),
            4 => Some(
                program
                    .try_clone_issuer_for_launch(&mut creation_b)
                    .unwrap()
                    .0,
            ),
            _ => None,
        };
        let m = if operation == 2 || operation == 4 {
            measurement(&i)
        } else {
            measurement(&l)
        };
        let required = program.retained_storage()
            + if file.is_some() {
                Image::file_storage(m).unwrap()
            } else {
                0
            };
        let total = Program::WORK
            + if operation == 0 {
                Cap::IO_WORK
                    + Image::quota(measurement(&l), Op::Revalidate)
                        .unwrap()
                        .work()
                    + Image::quota(measurement(&i), Op::Revalidate)
                        .unwrap()
                        .work()
            } else {
                Image::quota(m, Op::Transfer).unwrap().work()
            };
        let mut peak = 0;
        for mode in 0..5 {
            let floor = if mode == 3 {
                required - 1
            } else {
                required + 19
            };
            let limit = match mode {
                2 => peak - 1,
                4 => peak,
                _ => 10_000_000,
            };
            let mut w = Work::new(total - usize::from(mode == 1));
            let mut b = Budget::new(&mut w, limit);
            b.reserve_storage(floor).unwrap();
            let result = match operation {
                0 => program.revalidate(&mut b),
                1 => program.try_clone_launcher_for_launch(&mut b).map(|_| ()),
                2 => program.try_clone_issuer_for_launch(&mut b).map(|_| ()),
                3 => program.revalidate_launcher_clone(file.as_ref().unwrap(), &mut b),
                _ => program.revalidate_issuer_clone(file.as_ref().unwrap(), &mut b),
            };
            assert_eq!(b.storage(), floor);
            match mode {
                0 => {
                    result.unwrap();
                    peak = b.peak_storage();
                    assert_eq!(b.work(), total);
                }
                1 => {
                    assert!(matches!(
                        result,
                        Err(Error::Image(ImageError::Resource(Resource::Work(_))))
                    ));
                    assert!(b.work() < total);
                }
                2 => {
                    assert!(matches!(
                        result,
                        Err(Error::Image(ImageError::Resource(Resource::Storage(_))))
                    ));
                    assert_eq!(b.failed_storage(), Some(peak));
                }
                3 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                    assert_eq!(b.work(), 8);
                }
                _ => {
                    result.unwrap();
                    assert_eq!(b.peak_storage(), peak);
                    assert_eq!(b.work(), total);
                }
            }
        }
    }
}

#[test]
fn launcher_precedes_oversize_issuer_measurement_and_consumed_sources_close() {
    use std::os::unix::fs::MetadataExt;
    let f = Fixture::new("native-program-size-precedence");
    let oversized =
        Measurement::new([3; 32], crate::MAX_PROVISIONED_EXECUTABLE_BYTES_V1 + 1).unwrap();
    for bad_launcher in [true, false] {
        let p = policy_for(oversized, sealed_static_issuer_runtime_measurement_v1());
        let issuer_floor = Image::file_storage(
            ImageMeasurement::new(
                oversized.sha256(),
                oversized.byte_len(),
                oversized.byte_len(),
            )
            .unwrap(),
        )
        .unwrap();
        let mut w = Work::new(1_000_000_000);
        let mut b = Budget::new(&mut w, 200_000_000);
        b.reserve_storage(
            p.retained_storage() + Image::file_storage(measurement(&f)).unwrap() + issuer_floor,
        )
        .unwrap();
        let witness = f.open();
        let identity = witness.metadata().unwrap();
        let references = || {
            std::fs::read_dir("/proc/self/fd")
                .unwrap()
                .filter_map(|e| std::fs::metadata(e.ok()?.path()).ok())
                .filter(|m| m.dev() == identity.dev() && m.ino() == identity.ino())
                .count()
        };
        let source = f.open();
        let issuer = f.open();
        assert_eq!(references(), 3);
        let expected = if bad_launcher {
            Provisioned::new([0x99; 32], f.measurement().byte_len()).unwrap()
        } else {
            f.measurement()
        };
        let result = Program::provision(source, expected, issuer, p, &mut b);
        if bad_launcher {
            assert!(matches!(result, Err(Error::Image(_))));
        } else {
            assert!(matches!(
                result,
                Err(Error::Measurement(
                    crate::IssuerProgramAdmissionErrorV1::InvalidMeasurement
                ))
            ));
        }
        assert_eq!(references(), 1);
    }
}
