use super::*;
use crate::native_spawn::RootTaskObservationV2 as View;
use std::os::unix::fs::MetadataExt;

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
fn same_trace_observation_builds_late_custody_without_exclusive_trace_borrow() {
    subprocess("observe-late-builder");
    subprocess("observe-late-builder-refused");
}

#[test]
#[ignore = "requires isolated native pidfd_getfd permission"]
fn original_root_descriptor_observation() {
    assert_eq!(std::env::var("FE2O3_RUN_NATIVE_TESTS").as_deref(), Ok("1"));
    subprocess("observe-descriptor");
}

#[test]
fn original_root_observation_terminal_callback_preserves_wait_custody() {
    subprocess("observe-callback-terminal");
}

#[test]
#[ignore = "requires isolated native pidfd_getfd permission"]
fn original_root_descriptor_observation_exact_quote() {
    assert_eq!(std::env::var("FE2O3_RUN_NATIVE_TESTS").as_deref(), Ok("1"));
    subprocess("observe-descriptor-exact");
}

#[test]
#[ignore = "requires isolated native pidfd_getfd permission"]
fn original_root_descriptor_observation_second_continuity_refusal_closes_fd() {
    assert_eq!(std::env::var("FE2O3_RUN_NATIVE_TESTS").as_deref(), Ok("1"));
    subprocess("observe-descriptor-short-work");
}

pub(super) fn run(mode: &str, service: &mut Service, b: &mut Budget<'_>) {
    if mode.starts_with("observe-late-builder") {
        late_builder(service, b, mode.ends_with("-refused"));
        return;
    }
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
                    // Damage only the scoped frame, not the live trace's floor.
                    b.release_storage(1)?;
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
        "observe-descriptor-exact" | "observe-descriptor-short-work" => {
            descriptor_quote(&trace, &gate, b, mode == "observe-descriptor-short-work");
            assert_eq!(trace.held, Event::Pending);
        }
        "observe-callback-terminal" => {
            let entry = b.storage();
            let before = b.work();
            let drops = Arc::new(AtomicUsize::new(0));
            let terminal = Event::Signaled {
                signal: libc::SIGKILL,
                core_dumped: false,
            };
            let result = trace.with_task_observation::<_, Error>(b, |_, _| {
                rustix::process::pidfd_send_signal(&witness, Signal::KILL).unwrap();
                let deadline = Instant::now() + Duration::from_secs(5);
                loop {
                    if let Some(status) = terminal_without_reaping(&witness) {
                        assert_eq!(status, terminal);
                        break;
                    }
                    assert!(
                        Instant::now() < deadline,
                        "original child did not terminate"
                    );
                    std::thread::sleep(Duration::from_millis(1));
                }
                Ok(Backing(Arc::clone(&drops)))
            });
            assert!(matches!(result, Err(Error::State(_))));
            assert_eq!(drops.load(Ordering::SeqCst), 1, "late success escaped");
            // The final trace check rejects the terminal child before is_live.
            assert_eq!(b.work() - before, View::VIEW_WORK - Owner::OPERATION_WORK);
            assert_eq!(b.storage(), entry);
            assert_eq!((b.failed_work(), b.failed_storage()), (None, None));
            assert_eq!(trace.held, Event::Pending);
            assert_eq!(service.report().unwrap(), custody);
            assert!(trace.child.record().unwrap().retains_spawn_lease());
            assert_eq!(terminal_without_reaping(&witness), Some(terminal));
            assert_eq!(next(&mut trace, b), terminal);
            assert!(!trace.child.record().unwrap().retains_spawn_lease());
            require_reaped(&witness);
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
    if !matches!(mode, "observe-terminal" | "observe-callback-terminal") {
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

struct ObservationPayload {
    _lease: Backing,
    token: Option<Backing>,
    drops: Arc<AtomicUsize>,
}

// SAFETY: inert bounded counters, no native lock or authority; preparation is
// independent and all partial states are complete drop-only owners.
unsafe impl crate::cleanup_bridge::LateRetainedPayloadV2 for ObservationPayload {
    const RETIRE_WORK: usize = 128;
    const RETIRE_SCRATCH: usize = 128;
    type Prepared = ();
    fn try_prepare_retirement(&self) -> Option<()> {
        Some(())
    }
    fn retire(self, _: ()) {
        drop(self);
    }
}

// SAFETY: only adds one inert owner after actual same-trace continuity checking;
// the borrowed view is neither stored nor exported, even on late refusal.
unsafe impl crate::cleanup_bridge::LateRetainedBuildV2<&View<'_, '_>> for ObservationPayload {
    const BUILD_WORK: usize = 64;
    const BUILD_SCRATCH: usize = 128;
    type Error = Error;
    fn build(&mut self, view: &View<'_, '_>, b: &mut Budget<'_>) -> Result<()> {
        if self.token.is_some() {
            return Err(Error::State("duplicate fixture token"));
        }
        view.validate_continuity(b)?;
        self.token = Some(Backing(self.drops.clone()));
        Ok(())
    }
}

fn late_builder(service: &mut Service, b: &mut Budget<'_>, refuse: bool) {
    let slot = service.reserve_launch(b).unwrap().into_slot();
    let (child, _gate, witness) = spawn(slot);
    b.reserve_storage(Owner::STORAGE + Owner::ROOT_TRACE_GROWTH)
        .unwrap();
    let mut trace = child.into_root_trace(b).unwrap();
    let drops = Arc::new(AtomicUsize::new(0));
    // SAFETY: covers this fixture's entire inline and shared atomic storage.
    let (holder, charge) =
        unsafe { trace.reserve_late_custody::<ObservationPayload>(service, 1024, b) }.unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    trace
        .prepare_late_attachment(&holder, b)
        .unwrap()
        .commit(ObservationPayload {
            _lease: Backing(drops.clone()),
            token: None,
            drops: drops.clone(),
        });
    let floor = b.storage();
    // This is a real scoped view from the SAME trace, never a fabricated view.
    let result = trace.with_task_observation::<_, Error>(b, |view, b| {
        // SAFETY: the prepaid fixture builder only validates and adds an inert owner.
        unsafe { trace.build_late_custody(&holder, view, b) }?;
        if refuse {
            b.release_storage(1)?;
        }
        Ok(())
    });
    if refuse {
        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    } else {
        result.unwrap();
    }
    assert_eq!(b.storage(), floor);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(trace.child.record().unwrap().retains_spawn_lease());
    assert_ne!(trace.cancel(), Poll::Quarantined);
    drop(trace);
    drop(holder);
    b.release_storage(charge.additional_storage()).unwrap();
    drain(service);
    require_reaped(&witness);
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}

fn terminal_without_reaping(witness: &OwnedFd) -> Option<Event> {
    rustix::process::waitid(
        WaitId::PidFd(witness.as_fd()),
        WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
    )
    .unwrap()
    .map(|status| decode(status).unwrap())
}

fn pipe_references(gate: &OwnedFd) -> usize {
    // Only this fixture's gate and any duplicated stdin refer to this pipe in
    // the isolated parent process; the child's descriptor is in another table.
    let stat = rustix::fs::fstat(gate).unwrap();
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter(
            |entry| match std::fs::metadata(entry.as_ref().unwrap().path()) {
                Ok(m) => (m.dev(), m.ino()) == (stat.st_dev, stat.st_ino),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
                Err(e) => panic!("inspect original root pipe references: {e}"),
            },
        )
        .count()
}

fn descriptor_quote(
    trace: &RootTaskTraceV2<'_>,
    gate: &OwnedFd,
    b: &mut Budget<'_>,
    short_work: bool,
) {
    let scratch = View::VIEW_SCRATCH - View::CONTINUITY_SCRATCH + View::DESCRIPTOR_SCRATCH;
    let pressure = LIMIT - trace.retained_storage() - scratch;
    b.reserve_storage(pressure).unwrap();
    if !short_work {
        b.charge_work(LIMIT - b.work() - View::VIEW_WORK - View::DESCRIPTOR_WORK)
            .unwrap();
    }
    let entry = b.storage();
    assert_eq!(pipe_references(gate), 1);
    let result = trace.with_task_observation::<_, Error>(b, |view, b| {
        let frame = b.storage();
        let before = b.work();
        let (file, charge) = view.duplicate_descriptor(0, b)?;
        assert_eq!(b.work() - before, View::DESCRIPTOR_WORK);
        assert_eq!(
            b.storage(),
            frame,
            "returned descriptor charge is unreserved"
        );
        assert_eq!(b.peak_storage(), LIMIT);
        assert_eq!(
            charge.additional_storage(),
            std::mem::size_of::<(File, crate::native_spawn::ProtectedServiceSpawnStorageV2)>()
        );
        b.reserve_storage(charge.additional_storage())?;
        assert!(
            rustix::io::fcntl_getfd(&file)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        let observed = rustix::fs::fstat(&file).unwrap();
        let original = rustix::fs::fstat(gate).unwrap();
        assert_eq!(
            (observed.st_dev, observed.st_ino),
            (original.st_dev, original.st_ino)
        );
        assert_eq!(pipe_references(gate), 2);
        drop(file);
        b.release_storage(charge.additional_storage())?;
        assert_eq!(b.storage(), frame);
        assert_eq!(pipe_references(gate), 1);
        if !short_work {
            return Ok(());
        }

        b.charge_work(LIMIT - b.work() - (View::DESCRIPTOR_WORK - 1))?;
        let before = b.work();
        let result = view.duplicate_descriptor(0, b);
        match &result {
            Err(Error::Resource(Resource::Work(limit))) => {
                assert_eq!(limit.actual(), LIMIT + 1);
                assert_eq!(limit.limit(), LIMIT);
            }
            other => panic!("expected second-continuity work refusal: {other:?}"),
        }
        // This prefix includes both trace checks and the second child check's
        // entry charge: pidfd_getfd succeeded before that last charge refused.
        assert_eq!(
            b.work() - before,
            View::DESCRIPTOR_WORK - Owner::OPERATION_WORK + ENTRY
        );
        assert_eq!(b.storage(), frame);
        assert_eq!(b.failed_work(), Some(LIMIT + 1));
        assert_eq!(b.failed_storage(), None);
        assert_eq!(pipe_references(gate), 1, "refusal leaked the duplicated fd");
        result.map(|_| ())
    });
    if short_work {
        assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
        assert_eq!(b.work(), LIMIT + 1 - (Owner::OPERATION_WORK - ENTRY));
        assert_eq!(b.failed_work(), Some(LIMIT + 1));
    } else {
        result.unwrap();
        assert_eq!(b.work(), LIMIT);
        assert_eq!(b.failed_work(), None);
    }
    assert_eq!(b.failed_storage(), None);
    assert_eq!(b.storage(), entry);
    assert_eq!(b.peak_storage(), LIMIT);
    assert_eq!(pipe_references(gate), 1);
    b.release_storage(pressure).unwrap();
}
