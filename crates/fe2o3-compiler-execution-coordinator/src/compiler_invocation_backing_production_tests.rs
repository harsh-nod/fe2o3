//! Explicit installed-runtime integration lane; run with --ignored --test-threads=1.
//! Requires the real root-owned immutable policy-v2, profile-v3, manifest-v1 and
//! complete code tree at their production paths, plus writable temporary storage.
//! Missing admission fails; no installed-input mutation or immutability stub.
//! Invocations are inert fixture-created sealed descriptors, not cargo-authored
//! captures. Successful backing preparation retains data plus genuine runtime
//! approval; it cannot establish capture authorship, runtime enforcement or exec.
//! No spawn/enforcement claim. Terminal means owner Drop, not process/pool retirement.
use super::*;
use fe2o3_build_authority::CompilerRuntimeRoleV1 as Role;
use fe2o3_compiler_closure_capability::{
    CompilerApprovalErrorV2 as ApprovalError,
    RetainedCompilerRuntimeExecTransferChargeV1 as TransferCharge,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[path = "compiler_invocation_backing_production_fixture.rs"]
mod fixture;
use fixture::{HARNESS, STORAGE, WORK, account, admit, history};

#[test]
#[ignore = "requires installed immutable approved compiler runtime; run serially"]
fn native_executable_inventory_is_bound_to_original_backing_and_account() {
    use crate::native_runtime_inventory::{
        Error as InventoryError, NativeCompilerExecutableInventory as Inventory,
    };
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    account(|b| {
        let (inputs, witness) = admit(b, false);
        let (owner, charge) = inputs.prepare(b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        let floor = b.storage();
        let inventory = Inventory::capture(&owner, b).unwrap();
        assert_eq!(b.storage(), floor);
        let retained = inventory.retained_storage();
        assert!(retained <= Inventory::MAX_STORAGE);
        b.reserve_storage(retained).unwrap();
        inventory.revalidate(&owner, b).unwrap();
        assert_eq!(
            inventory.ranges().count(),
            owner.inventory_sources().entries().len() - 1
        );
        let mut other_work = Work::new(usize::MAX);
        let mut other = Budget::new(&mut other_work, usize::MAX);
        other.reserve_storage(b.storage()).unwrap();
        assert!(matches!(
            inventory.revalidate(&owner, &mut other),
            Err(InventoryError::Resource(Resource::Accounting))
        ));
        witness.assert_live();
        let before_drop = history(b);
        drop(inventory);
        assert_eq!(history(b), before_drop);
        b.release_storage(retained).unwrap();
        drop(owner);
        witness.assert_dropped();
        b.release_storage(charge.retained_storage()).unwrap();
    });
}

#[test]
#[ignore = "requires installed immutable approved compiler runtime; run serially"]
fn approved_runtime_retains_received_inert_capture_and_drops_on_original_account() {
    account(|b| {
        let (inputs, witness) = admit(b, false);
        let input_storage = inputs.storage;
        let floor = b.storage();
        let (owner, charge) = inputs.prepare(b).unwrap();
        assert_eq!(b.storage(), floor);
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(b.storage(), HARNESS + owner.retained_storage());
        assert_eq!(
            charge.retained_storage(),
            input_storage + charge.additional_storage()
        );
        assert_eq!(
            input_storage,
            owner.runtime().required_retained_storage()
                + owner.capture.native_retained_storage().unwrap()
                + Output::STORAGE
        );
        witness.assert_capture_retained(&owner);
        let sources = owner.inventory_sources();
        let expected_sources = size_of::<(Sources, TransferCharge)>()
            + usize::try_from(owner.runtime().manifest().total_file_bytes()).unwrap();
        assert_eq!(sources.retained_storage(), expected_sources);
        assert_eq!(
            charge.additional_storage(),
            owner.invocation().retained_storage()
                + expected_sources
                + CompilerInvocationBacking::ENVELOPE
        );
        assert_eq!(
            sources.entries().len(),
            owner.runtime().manifest().entries().len()
        );
        let mut libraries = 0;
        for ((actual, file), expected) in
            sources.entries().zip(owner.runtime().manifest().entries())
        {
            assert_eq!(actual, expected);
            assert_eq!(file.metadata().unwrap().len(), expected.length);
            libraries += usize::from(actual.role == Role::SharedLibrary);
        }
        assert!(libraries > 0);
        owner.revalidate(b).unwrap();
        witness.assert_live();
        let floor = b.storage();
        let before_drop = history(b);
        drop(owner);
        witness.assert_dropped();
        assert_eq!(b.storage(), floor);
        assert_eq!(history(b), before_drop);
        b.release_storage(charge.retained_storage()).unwrap();
    });
}

#[test]
#[ignore = "requires installed immutable approved compiler runtime; run serially"]
fn approved_runtime_backing_revalidates_the_retained_sealed_invocation() {
    account(|b| {
        let (inputs, witness) = admit(b, false);
        let (owner, charge) = inputs.prepare(b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        witness.assert_capture_retained(&owner);
        owner.revalidate(b).unwrap();
        let floor = b.storage();
        let before = history(b);
        witness.invocation_mode(rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR);
        let refused = owner.revalidate(b);
        witness.invocation_mode(rustix::fs::Mode::RUSR);
        assert!(matches!(
            refused,
            Err(CompilerInvocationBackingError::Capture(
                CaptureError::Rejected(" is not an exact regular mode-0400 file")
            ))
        ));
        assert_eq!(b.storage(), floor);
        assert!(b.work() > before.0);
        assert_eq!((b.failed_work(), b.failed_storage()), (before.2, before.3));
        owner.revalidate(b).unwrap();
        witness.assert_capture_retained(&owner);
        drop(owner);
        witness.assert_dropped();
        b.release_storage(charge.retained_storage()).unwrap();
    });
}

#[test]
#[ignore = "requires installed immutable approved compiler runtime; run serially"]
fn approved_runtime_preparation_rejects_changed_sealed_invocation_and_drops_inputs() {
    account(|b| {
        let (inputs, witness) = admit(b, false);
        let input_storage = inputs.storage;
        let floor = b.storage();
        let before = history(b);
        witness.invocation_mode(rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR);
        let refused = inputs.prepare(b);
        witness.invocation_mode(rustix::fs::Mode::RUSR);
        assert!(matches!(
            refused,
            Err(CompilerInvocationBackingError::Capture(
                CaptureError::Rejected(" is not an exact regular mode-0400 file")
            ))
        ));
        witness.assert_dropped();
        assert_eq!(b.storage(), floor);
        assert!(b.work() > before.0);
        assert_eq!((b.failed_work(), b.failed_storage()), (before.2, before.3));
        b.release_storage(input_storage).unwrap();
    });
}

#[test]
#[ignore = "requires installed immutable approved compiler runtime; run serially"]
fn approved_runtime_preparation_native_work_denial_drops_inputs_on_original_account() {
    account(|b| {
        let (inputs, witness) = admit(b, false);
        let input_storage = inputs.storage;
        // Fund local preparation and measurement, then leave native
        // revalidation one short of its eight-unit entry charge.
        b.charge_work(
            WORK - b.work() - CompilerInvocationBacking::LOCAL_WORK - MEASURE_WORK - 8 + 1,
        )
        .unwrap();
        let spent = b.work();
        let floor = b.storage();
        assert!(matches!(
            inputs.prepare(b),
            Err(CompilerInvocationBackingError::Capture(
                CaptureError::Resource(Resource::Work(_))
            ))
        ));
        assert_eq!(
            b.work(),
            spent + CompilerInvocationBacking::LOCAL_WORK + MEASURE_WORK
        );
        assert_eq!(b.failed_work(), Some(WORK + 1));
        assert!(b.failed_storage().is_none());
        witness.assert_dropped();
        assert_eq!(b.storage(), floor);
        b.release_storage(input_storage).unwrap();
    });
}

#[test]
#[ignore = "requires installed immutable approved compiler runtime; run serially"]
fn approved_runtime_backing_rejects_source_and_account_substitution() {
    account(|b| {
        let (inputs, witness) = admit(b, false);
        let (owner, charge) = inputs.prepare(b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        let original_history = history(b);
        // A separate account is used only to prove refusal, never as a retry.
        account(|foreign| {
            foreign.reserve_storage(owner.retained_storage()).unwrap();
            assert!(matches!(
                owner.revalidate(foreign),
                Err(CompilerInvocationBackingError::Output(
                    OutputError::Resource(Resource::Accounting)
                ))
            ));
            assert!(matches!(
                owner
                    .runtime()
                    .validate_inventory_transfer(owner.inventory_sources(), foreign),
                Err(RuntimeError::Resource(Resource::Accounting))
            ));
            foreign.release_storage(owner.retained_storage()).unwrap();
        });
        assert_eq!(history(b), original_history);
        // Admitted rustc requires executable mode; the backend requires mode 0444.
        // A shared-library role alone would not exclude a backend inode alias.
        let rustc = owner.rustc_source();
        assert!(matches!(
            owner
                .runtime()
                .validate_codegen_backend_load_transfer(rustc, b),
            Err(RuntimeError::Mismatch(_))
        ));
        // Mutate only this owned duplicate's descriptor flags, not its shared
        // open-file description or the immutable installed inode.
        let (_, library) = owner
            .inventory_sources()
            .entries()
            .find(|(entry, _)| entry.role == Role::SharedLibrary)
            .unwrap();
        let flags = rustix::io::fcntl_getfd(library).unwrap();
        rustix::io::fcntl_setfd(library, rustix::io::FdFlags::empty()).unwrap();
        let refused = owner.revalidate(b);
        rustix::io::fcntl_setfd(library, flags).unwrap();
        assert!(matches!(
            refused,
            Err(CompilerInvocationBackingError::Runtime(
                RuntimeError::Mismatch(_)
            ))
        ));
        owner.revalidate(b).unwrap();
        witness.assert_live();
        assert!(b.failed_work().is_none());
        assert!(b.failed_storage().is_none());
        drop(owner);
        witness.assert_dropped();
        b.release_storage(charge.retained_storage()).unwrap();
    });
}

#[test]
#[ignore = "requires installed immutable approved compiler runtime; run serially"]
fn approved_runtime_preparation_rejects_changed_closure_and_drops_inputs() {
    account(|b| {
        let (inputs, witness) = admit(b, true);
        let input_storage = inputs.storage;
        let floor = b.storage();
        let spent = b.work();
        assert!(matches!(
            inputs.prepare(b),
            Err(CompilerInvocationBackingError::Runtime(
                RuntimeError::Approval(ApprovalError::Mismatch(
                    "compiler differs from root-approved closure"
                ))
            ))
        ));
        witness.assert_dropped();
        assert_eq!(b.storage(), floor);
        assert!(b.work() > spent);
        assert!(b.failed_work().is_none());
        assert!(b.failed_storage().is_none());
        b.release_storage(input_storage).unwrap();
    });
}

#[test]
#[ignore = "requires installed immutable approved compiler runtime; run serially"]
fn approved_runtime_preparation_one_short_funding_closes_consumed_inputs() {
    for storage_short in [false, true] {
        account(|b| {
            let (inputs, witness) = admit(b, false);
            let input_storage = inputs.storage;
            let padding = if storage_short {
                // Logical external pressure, not an allocated buffer or new cap.
                STORAGE - b.storage() - CompilerInvocationBacking::FRAME_STORAGE + 1
            } else {
                b.charge_work(WORK - b.work() - CompilerInvocationBacking::LOCAL_WORK + 1)
                    .unwrap();
                0
            };
            b.reserve_storage(padding).unwrap();
            let floor = b.storage();
            let spent = b.work();
            let result = inputs.prepare(b);
            if storage_short {
                assert!(matches!(
                    result,
                    Err(CompilerInvocationBackingError::Resource(Resource::Storage(
                        _
                    )))
                ));
                assert_eq!(
                    b.work(),
                    spent + CompilerInvocationBacking::LOCAL_WORK + MEASURE_WORK
                );
                assert_eq!(b.failed_storage(), Some(STORAGE + 1));
            } else {
                assert!(matches!(
                    result,
                    Err(CompilerInvocationBackingError::Resource(Resource::Work(_)))
                ));
                assert_eq!(b.work(), spent);
                assert_eq!(b.failed_work(), Some(WORK + 1));
            }
            witness.assert_dropped();
            assert_eq!(b.storage(), floor);
            b.release_storage(input_storage + padding).unwrap();
        });
    }
}

#[test]
#[ignore = "requires installed immutable approved compiler runtime; run serially"]
fn approved_runtime_backing_outer_error_and_unwind_close_without_refunding_history() {
    for unwind in [false, true] {
        account(|b| {
            let (inputs, witness) = admit(b, false);
            let input_storage = inputs.storage;
            let floor = b.storage();
            assert!(b.charge_work(WORK).is_err());
            assert!(b.reserve_storage(STORAGE).is_err());
            let denials = (b.failed_work(), b.failed_storage());
            let spent = b.work();
            let mut installed = false;
            let result = catch_unwind(AssertUnwindSafe(|| {
                b.with_prepaid_scope(input_storage, 8, 8, 0, |b| {
                    let (owner, charge) = inputs.prepare(b)?;
                    b.reserve_storage(charge.additional_storage())?;
                    witness.assert_live();
                    installed = true;
                    if unwind {
                        panic!("approved backing outer unwind");
                    }
                    b.reserve_storage(STORAGE)?;
                    drop(owner);
                    Ok::<(), CompilerInvocationBackingError>(())
                })
            }));
            assert!(
                installed,
                "the complete genuine backing must precede failure"
            );
            if unwind {
                let panic = result.err().expect("expected the explicit owner unwind");
                assert_eq!(
                    panic.downcast_ref::<&str>(),
                    Some(&"approved backing outer unwind")
                );
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(CompilerInvocationBackingError::Resource(Resource::Storage(
                        _
                    )))
                ));
            }
            witness.assert_dropped();
            assert_eq!(b.storage(), floor);
            assert!(b.work() > spent);
            assert_eq!((b.failed_work(), b.failed_storage()), denials);
            b.release_storage(input_storage).unwrap();
        });
    }
}
