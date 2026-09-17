use super::*;
use crate::generated_source::GeneratedNativeInputErrorV1 as InputError;
use crate::{
    KfdRuntimeBackendV1, RuntimeBackendFailureV1, RuntimeBackendV1,
    RuntimeGfx942GeneratedReservationErrorV1 as SourceError, RuntimeGfx942GeneratedSourceV1,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn generated_native_inputs_borrow_original_program_and_complete_bytes_across_control_transfer() {
    let (hsaco, projection) = source_projection();
    let authority = source_authority(&projection);
    let expected = RuntimeGfx942GeneratedSourceV1::new(&projection, &hsaco, &authority)
        .validate(7)
        .unwrap();
    let identity = projection.identity();
    let expected_abi = fe2o3_amdhsa_loader::validate(
        &hsaco,
        fe2o3_amdhsa_loader::AdmittedProfile::Gfx942XnackOffCov6,
    )
    .unwrap()
    .bind_kernel("vecadd")
    .unwrap()
    .reconcile_dispatch_abi(
        expected.dispatch_contract_sha256,
        &[fe2o3_amdhsa_loader::KernelGlobalBufferAbiV1::new(
            0,
            "a_ptr",
            0,
            4,
            fe2o3_hsaco::ArgumentAccess::ReadWrite,
        )],
    )
    .unwrap()
    .dispatch_abi_identity();
    assert!(expected_abi.is_some());
    let mut storage = projection.into_generated_storage_v1();
    let original = storage
        .buffers()
        .iter()
        .map(|buffer| (buffer.bytes().as_ptr(), buffer.bytes().to_vec()))
        .collect::<Vec<_>>();
    let buffer_roster = storage.buffers().as_ptr();
    let mut control = None;
    let mut calls = 0;
    for transferred in [false, true] {
        if transferred {
            assert!(storage.transfer_control_into(&mut control));
        }
        let source =
            RuntimeGfx942GeneratedSourceV1::from_generated_storage(&storage, &hsaco, &authority);
        source
            .with_native_inputs_v1(7, &expected, |program, buffers| {
                calls += 1;
                assert_eq!(program.identity_inputs(), identity);
                assert_eq!(program.dispatch_abi_identity(), expected_abi);
                assert_eq!(program.dispatch_pointee_alignment(0), Some(4));
                assert_eq!(
                    program.dispatch_actual_access(0),
                    Some(fe2o3_hsaco::ArgumentAccess::ReadWrite)
                );
                assert_eq!(program.selected_kernel().name(), "vecadd");
                let segment = program
                    .envelope()
                    .materialization()
                    .copy_phase()
                    .segment(fe2o3_amdhsa_loader::SegmentOrdinal::First)
                    .source()
                    .bytes();
                let range = hsaco.as_ptr_range();
                assert!(segment.as_ptr() >= range.start && segment.as_ptr() < range.end);
                assert_eq!(buffers.as_ptr(), buffer_roster);
                assert_eq!(buffers.len(), 3);
                for (buffer, (address, bytes)) in buffers.iter().zip(&original) {
                    assert_eq!(buffer.bytes().as_ptr(), *address);
                    assert_eq!(buffer.bytes(), bytes);
                }
                Ok(())
            })
            .unwrap()
            .unwrap();
        assert_eq!(storage.control_available(), !transferred);
        assert_eq!(control.is_some(), transferred);
        assert!(source.validate(7).unwrap().matches(&expected));
    }
    assert_eq!(calls, 2);
    assert!(!storage.transfer_control_into(&mut None));
    assert_eq!(control.as_ref().unwrap().buffer_count(), 1);
    assert_eq!(storage.timeout_milliseconds(), 4321);
}

#[test]
fn generated_native_inputs_reject_every_reserved_roster_coordinate_before_callback() {
    let (hsaco, projection) = source_projection();
    let authority = source_authority(&projection);
    let storage = projection.into_generated_storage_v1();
    let source =
        RuntimeGfx942GeneratedSourceV1::from_generated_storage(&storage, &hsaco, &authority);
    for axis in 0..10 {
        let mut expected = source.validate(7).unwrap();
        match axis {
            0 => expected.source_identity = std::sync::Arc::new(()),
            1 => expected.count -= 1,
            2 => expected.readback_bytes += 1,
            3 => expected.fixup_count += 1,
            4 => expected.dispatch_contract_sha256[0] ^= 1,
            5 => expected.buffers[0].as_mut().unwrap().ordinal = 1,
            6 => expected.buffers[1].as_mut().unwrap().bytes += 1,
            7 => {
                expected.buffers[2].as_mut().unwrap().access =
                    crate::Gfx942RuntimeBufferAccessV1::ReadOnly
            }
            8 => expected.buffers[0] = None,
            _ => expected.buffers[3] = expected.buffers[0],
        }
        assert!(
            matches!(
                source.with_native_inputs_v1(7, &expected, |_, _| {
                    panic!("invalid roster reached callback")
                }),
                Err(InputError::Source(SourceError::InvalidRoster))
            ),
            "axis {axis}"
        );
        assert!(storage.control_available());
    }
}

#[test]
fn generated_native_inputs_reject_artifact_substitution_before_callback() {
    let (hsaco, projection) = source_projection();
    let authority = source_authority(&projection);
    let expected = RuntimeGfx942GeneratedSourceV1::new(&projection, &hsaco, &authority)
        .validate(7)
        .unwrap();
    let storage = projection.into_generated_storage_v1();
    for axis in 0..3 {
        let mut changed = hsaco.clone();
        match axis {
            0 => changed[0] ^= 1,
            1 => changed.push(0),
            _ => changed = storage.executable_image().to_vec(),
        }
        let source =
            RuntimeGfx942GeneratedSourceV1::from_generated_storage(&storage, &changed, &authority);
        assert!(matches!(
            source.with_native_inputs_v1(7, &expected, |_, _| {
                panic!("substituted artifact reached callback")
            }),
            Err(InputError::Source(SourceError::ArtifactMismatch))
        ));
        assert!(storage.control_available());
    }
}

#[test]
fn generated_native_inputs_reject_authority_substitution_and_staleness_before_callback() {
    let (hsaco, projection) = source_projection();
    let good = source_authority(&projection);
    let expected = RuntimeGfx942GeneratedSourceV1::new(&projection, &hsaco, &good)
        .validate(7)
        .unwrap();
    for axis in 0..6 {
        let mut authority = source_authority(&projection);
        match axis {
            0 => authority.object[0] ^= 1,
            1 => authority.length += 1,
            2 => authority.kernel = "other",
            3 => authority.dispatch[0] ^= 1,
            4 => authority.device += 1,
            _ => authority.current.set(false),
        }
        let source = RuntimeGfx942GeneratedSourceV1::new(&projection, &hsaco, &authority);
        let error = source
            .with_native_inputs_v1(7, &expected, |_, _| panic!("invalid authority callback"))
            .unwrap_err();
        if axis < 5 {
            assert!(matches!(
                error,
                InputError::Source(SourceError::AuthorityMismatch)
            ));
        } else {
            assert!(matches!(
                error,
                InputError::Source(SourceError::AuthorityNotCurrent)
            ));
        }
    }
}

fn rejected_error() -> crate::KfdRuntimeBackendErrorV1 {
    match KfdRuntimeBackendV1::mock()
        .create_stream_v1(u64::MAX)
        .unwrap_err()
    {
        RuntimeBackendFailureV1::Rejected(error) => error,
        other => panic!("expected pure backend rejection: {other:?}"),
    }
}

#[test]
fn generated_native_inputs_preserve_each_callback_failure_class_after_closing_check() {
    let (hsaco, projection) = source_projection();
    let authority = source_authority(&projection);
    let storage = projection.into_generated_storage_v1();
    let source =
        RuntimeGfx942GeneratedSourceV1::from_generated_storage(&storage, &hsaco, &authority);
    let expected = source.validate(7).unwrap();
    for axis in 0..3 {
        let error = rejected_error();
        let same = error.clone();
        let mut calls = 0;
        let result = source
            .with_native_inputs_v1(7, &expected, |_, _| {
                calls += 1;
                Err(match axis {
                    0 => RuntimeBackendFailureV1::Rejected(error),
                    1 => RuntimeBackendFailureV1::Quiescent(error),
                    _ => RuntimeBackendFailureV1::Terminal(error),
                })
            })
            .unwrap()
            .unwrap_err();
        match (axis, result) {
            (0, RuntimeBackendFailureV1::Rejected(error))
            | (1, RuntimeBackendFailureV1::Quiescent(error))
            | (2, RuntimeBackendFailureV1::Terminal(error)) => assert_eq!(error, same),
            other => panic!("changed failure class: {other:?}"),
        }
        assert_eq!(calls, 1);
        assert!(storage.control_available());
    }
}

#[test]
fn generated_native_inputs_closing_staleness_overrides_callback_success_and_failure() {
    let (hsaco, projection) = source_projection();
    let authority = source_authority(&projection);
    let storage = projection.into_generated_storage_v1();
    for failed in [false, true] {
        authority.current.set(true);
        let source =
            RuntimeGfx942GeneratedSourceV1::from_generated_storage(&storage, &hsaco, &authority);
        let expected = source.validate(7).unwrap();
        let mut calls = 0;
        let result = source.with_native_inputs_v1(7, &expected, |_, _| {
            calls += 1;
            authority.current.set(false);
            if failed {
                Err(RuntimeBackendFailureV1::Rejected(rejected_error()))
            } else {
                Ok(())
            }
        });
        assert!(matches!(
            result,
            Err(InputError::Source(SourceError::AuthorityNotCurrent))
        ));
        assert_eq!(calls, 1);
        assert!(storage.control_available());
    }
}

#[test]
fn generated_native_inputs_propagate_first_callback_panic_without_consuming_storage() {
    let (hsaco, projection) = source_projection();
    let authority = source_authority(&projection);
    let storage = projection.into_generated_storage_v1();
    let source =
        RuntimeGfx942GeneratedSourceV1::from_generated_storage(&storage, &hsaco, &authority);
    let expected = source.validate(7).unwrap();
    authority.checks.set(0);
    authority.fault.set(Some(CurrentnessFault::PanicAt(3)));
    let original = storage.buffers().as_ptr();
    let mut calls = 0;
    let result = catch_unwind(AssertUnwindSafe(|| {
        source.with_native_inputs_v1(7, &expected, |_, _| {
            calls += 1;
            authority.current.set(false);
            std::panic::panic_any(1729u64)
        })
    }));
    assert_eq!(result.unwrap_err().downcast_ref::<u64>(), Some(&1729));
    assert_eq!(calls, 1);
    assert_eq!(authority.checks.get(), 2);
    assert!(storage.control_available());
    assert_eq!(storage.buffers().as_ptr(), original);
}

#[test]
fn generated_native_inputs_opening_currentness_error_and_panic_precede_callback() {
    let (hsaco, projection) = source_projection();
    let authority = source_authority(&projection);
    let storage = projection.into_generated_storage_v1();
    let source =
        RuntimeGfx942GeneratedSourceV1::from_generated_storage(&storage, &hsaco, &authority);
    let expected = source.validate(7).unwrap();
    for at in [1, 2] {
        for panic in [false, true] {
            authority.checks.set(0);
            authority.fault.set(Some(if panic {
                CurrentnessFault::PanicAt(at)
            } else {
                CurrentnessFault::ErrorAt(at)
            }));
            let calls = Cell::new(0);
            let result = catch_unwind(AssertUnwindSafe(|| {
                source.with_native_inputs_v1(7, &expected, |_, _| {
                    calls.set(calls.get() + 1);
                    Ok(())
                })
            }));
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<&str>(),
                    Some(&"currentness panic")
                );
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(InputError::Source(SourceError::AuthorityNotCurrent))
                ));
            }
            assert_eq!(calls.get(), 0);
            assert_eq!(authority.checks.get(), at);
            assert!(storage.control_available());
        }
    }
}

#[test]
fn generated_native_inputs_closing_currentness_panic_preserves_success_and_error_custody() {
    let (hsaco, projection) = source_projection();
    let authority = source_authority(&projection);
    let storage = projection.into_generated_storage_v1();
    let source =
        RuntimeGfx942GeneratedSourceV1::from_generated_storage(&storage, &hsaco, &authority);
    let expected = source.validate(7).unwrap();
    let original = storage.buffers().as_ptr();
    for failed in [false, true] {
        authority.checks.set(0);
        authority.fault.set(Some(CurrentnessFault::PanicAt(3)));
        let mut calls = 0;
        let result = catch_unwind(AssertUnwindSafe(|| {
            source.with_native_inputs_v1(7, &expected, |_, _| {
                calls += 1;
                if failed {
                    Err(RuntimeBackendFailureV1::Rejected(rejected_error()))
                } else {
                    Ok(())
                }
            })
        }));
        assert_eq!(
            result.unwrap_err().downcast_ref::<&str>(),
            Some(&"currentness panic")
        );
        assert_eq!(calls, 1);
        assert_eq!(authority.checks.get(), 3);
        assert!(storage.control_available());
        assert_eq!(storage.buffers().as_ptr(), original);
    }
}
