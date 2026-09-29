use super::*;
use crate::native_spawn::RootTaskObservationV2 as View;

#[test]
fn original_trace_observation_preserves_custody_and_accounting() {
    for mode in [
        "observe-view",
        "observe-exact",
        "observe-short-work",
        "observe-short-storage",
        "observe-terminal",
    ] {
        subprocess(mode);
    }
}

#[test]
#[ignore = "requires isolated native pidfd_getfd permission"]
fn original_root_descriptor_observation() {
    assert_eq!(std::env::var("FE2O3_RUN_NATIVE_TESTS").as_deref(), Ok("1"));
    subprocess("observe-descriptor");
}

pub(super) fn run(mode: &str, service: &mut Service, b: &mut Budget<'_>) {
    let slot = service.reserve_launch(b).unwrap().into_slot();
    let (child, gate, witness) = spawn(slot);
    let retained = Owner::STORAGE + Owner::ROOT_TRACE_GROWTH;
    b.reserve_storage(retained).unwrap();
    let mut trace = child.into_root_trace(b).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let custody = service.report().unwrap();

    match mode {
        "observe-exact" | "observe-short-work" | "observe-short-storage" => {
            let short_work = usize::from(mode == "observe-short-work");
            let short_storage = usize::from(mode == "observe-short-storage");
            let pressure = LIMIT - retained - View::VIEW_SCRATCH + short_storage;
            b.reserve_storage(pressure).unwrap();
            b.charge_work(LIMIT - b.work() - (View::VIEW_WORK - short_work))
                .unwrap();
            let entry = b.storage();
            let mut called = false;
            let result = trace.with_task_observation::<_, Error>(b, |view, _| {
                called = true;
                assert_eq!(view.pid(), trace.pid());
                assert_eq!(view.retained_storage(), retained);
                Ok(())
            });
            match mode {
                "observe-exact" => {
                    result.unwrap();
                    assert!(called);
                    assert_eq!(b.work(), LIMIT);
                    assert_eq!(b.peak_storage(), LIMIT);
                }
                "observe-short-work" => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                    assert!(
                        called,
                        "late continuity refusal must suppress callback success"
                    );
                    assert_eq!(b.failed_work(), Some(LIMIT + 1));
                }
                _ => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                    assert!(!called);
                    assert_eq!(b.failed_storage(), Some(LIMIT + 1));
                }
            }
            assert_eq!(b.storage(), entry);
            b.release_storage(pressure).unwrap();
        }
        "observe-view" => {
            let mut foreign_work = Work::new(LIMIT);
            let mut foreign = Budget::new(&mut foreign_work, LIMIT);
            foreign.reserve_storage(retained).unwrap();
            assert!(matches!(
                trace.with_task_observation::<(), Error>(&mut foreign, |_, _| panic!(
                    "foreign ledger callback"
                )),
                Err(Error::Resource(Resource::Accounting))
            ));
            b.release_storage(1).unwrap();
            assert!(matches!(
                trace.with_task_observation::<(), Error>(b, |_, _| panic!("unfunded callback")),
                Err(Error::Resource(Resource::Accounting))
            ));
            b.reserve_storage(1).unwrap();
            let thread = trace.origin;
            trace.origin.2 = std::thread::spawn(|| std::thread::current().id())
                .join()
                .unwrap();
            assert!(matches!(
                trace.with_task_observation::<(), Error>(b, |_, _| panic!(
                    "foreign thread callback"
                )),
                Err(Error::State(_))
            ));
            trace.origin = thread;
            trace
                .with_task_observation::<_, Error>(b, |view, b| {
                    view.validate_continuity(b)?;
                    assert!(matches!(
                        view.duplicate_descriptor(-1, b),
                        Err(Error::State(_))
                    ));
                    assert!(matches!(
                        view.validate_continuity(&mut foreign),
                        Err(Error::Resource(Resource::Accounting))
                    ));
                    Ok(())
                })
                .unwrap();
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _: Result<()> =
                        trace.with_task_observation(b, |_, _| panic!("observation unwind fixture"));
                }))
                .is_err()
            );
            assert!(matches!(
                trace.with_task_observation::<_, Error>(b, |_, b| {
                    b.release_storage(b.storage())?;
                    Ok(())
                }),
                Err(Error::Resource(Resource::Accounting))
            ));
            trace
                .with_task_observation::<_, Error>(b, |_, _| Ok(()))
                .unwrap();
        }
        "observe-descriptor" => {
            trace
                .with_task_observation::<_, Error>(b, |view, b| {
                    let before = b.work();
                    let (file, charge) = view.duplicate_descriptor(0, b)?;
                    assert_eq!(b.work() - before, View::DESCRIPTOR_WORK);
                    b.reserve_storage(charge.additional_storage())?;
                    let flags = rustix::io::fcntl_getfd(&file).unwrap();
                    assert!(flags.contains(rustix::io::FdFlags::CLOEXEC));
                    let observed = rustix::fs::fstat(&file).unwrap();
                    let original = rustix::fs::fstat(&gate).unwrap();
                    assert_eq!(
                        (observed.st_dev, observed.st_ino),
                        (original.st_dev, original.st_ino)
                    );
                    assert!(matches!(
                        view.duplicate_descriptor(i32::MAX, b),
                        Err(Error::Io {
                            source: Errno::BADF,
                            ..
                        })
                    ));
                    drop(file);
                    b.release_storage(charge.additional_storage())?;
                    Ok(())
                })
                .unwrap();
        }
        "observe-terminal" => {
            rustix::io::write(&gate, b"go\n").unwrap();
            assert_eq!(next(&mut trace, b), Event::Exec);
            trace.resume(b).unwrap();
            assert_eq!(next(&mut trace, b), Event::Exited(0));
            assert!(matches!(
                trace.with_task_observation::<(), Error>(b, |_, _| panic!("terminal callback")),
                Err(Error::State(_))
            ));
        }
        _ => panic!("unknown observation fixture: {mode}"),
    }
    assert!(ledger == b.work_ledger_identity_v1());
    assert_eq!(b.storage(), retained);
    if mode != "observe-terminal" {
        assert_eq!(service.report().unwrap(), custody);
        assert!(trace.child.record().unwrap().retains_spawn_lease());
    }
    assert_ne!(trace.cancel(), Poll::Quarantined);
    assert!(matches!(
        trace.with_task_observation::<(), Error>(b, |_, _| panic!("cancelled callback")),
        Err(Error::State(_)) | Err(Error::Resource(Resource::Work(_)))
    ));
    drop(trace);
    drain(service);
    require_reaped(&witness);
}
