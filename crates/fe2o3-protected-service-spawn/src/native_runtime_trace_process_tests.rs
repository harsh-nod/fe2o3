//! Original-controller mechanics using the existing gated Command fixture.
//! Closed-gate tests and an explicitly invoked typed-filter component fixture.
//! Neither fixture provides compiler admission or a runtime enforcement guard.
use super::*;
use crate::native_spawn::RootRuntimeTraceV1 as Runtime;

pub(super) fn run(mode: &str, service: &mut Service, b: &mut Budget<'_>) {
    if matches!(mode, "runtime-lifecycle" | "runtime-tree-cancel") {
        return lifecycle(service, b, mode == "runtime-tree-cancel");
    }
    let slot = service.reserve_launch(b).unwrap().into_slot();
    let (child, gate, witness) = spawn(slot);
    let retained = Owner::STORAGE + Owner::ROOT_TRACE_GROWTH;
    b.reserve_storage(retained).unwrap();
    let mut trace = child.into_root_trace(b).unwrap();
    let pid = trace.pid();
    trace.interrupt(b).unwrap();
    assert_eq!(next(&mut trace, b), Event::TrapStop);
    b.reserve_storage(Runtime::STORAGE_GROWTH).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    if mode == "runtime-expired" {
        // SAFETY: no runtime options are installed for an already expired
        // deadline. The same gate/custody is preserved through ordinary cleanup.
        let refused = unsafe { trace.into_runtime_trace(Instant::now(), b) };
        assert!(matches!(refused, Err(Error::State(_))));
    } else {
        // SAFETY: same-thread original trace and same ledger, exclusive waits,
        // no descendants and still-closed gate. This bounded fixture NEVER
        // releases the gate or claims a typed compiler/filter. The subprocess
        // is dedicated; outer fixture owns its deadline and retained pidfd.
        let mut runtime = unsafe { trace.into_runtime_trace(deadline, b) }.unwrap();
        assert_eq!(runtime.pid(), pid);
        assert_eq!(
            runtime.retained_storage(),
            retained + Runtime::STORAGE_GROWTH
        );
        assert!(runtime.root_completion().is_none());
        if mode == "runtime-account" {
            let mut foreign_work = Work::new(LIMIT);
            let mut foreign = Budget::new(&mut foreign_work, LIMIT);
            foreign.reserve_storage(runtime.retained_storage()).unwrap();
            assert!(matches!(
                runtime.poll(&mut foreign),
                Err(Error::Resource(Resource::Accounting))
            ));
            assert!(matches!(
                runtime.park_all(&mut foreign),
                Err(Error::Resource(Resource::Accounting))
            ));
        }
        if mode == "runtime-census" {
            assert!(matches!(
                runtime.with_selected_task_observation::<_, Error>(b, |_, _| Ok(())),
                Err(Error::State(_))
            ));
            runtime.park_all(b).unwrap();
            runtime
                .with_task_observation(b, |observation, _| {
                    assert_eq!(observation.pid(), pid);
                    Ok::<_, Error>(())
                })
                .unwrap();
            runtime
                .with_selected_task_observation::<_, Error>(b, |view, b| {
                    assert_eq!(view.pid(), pid);
                    let (maps, charge) = view.read_maps(b)?;
                    b.reserve_storage(charge.additional_storage())?;
                    assert!(!maps.is_empty());
                    assert!(std::str::from_utf8(&maps).is_ok());
                    assert!(maps.contains(&b'\n'));
                    drop(maps);
                    b.release_storage(charge.additional_storage())?;
                    let (personality, length) = view.read_personality(b)?;
                    crate::trace_runtime::validate_personality(
                        std::str::from_utf8(&personality[..length]).unwrap(),
                    )
                    .unwrap();
                    let (file, charge) = view.duplicate_descriptor(0, b)?;
                    b.reserve_storage(charge.additional_storage())?;
                    let actual = rustix::fs::fstat(&file).unwrap();
                    let original = rustix::fs::fstat(&gate).unwrap();
                    assert_eq!(
                        (actual.st_dev, actual.st_ino),
                        (original.st_dev, original.st_ino)
                    );
                    drop(file);
                    b.release_storage(charge.additional_storage())?;
                    Ok(())
                })
                .unwrap();
            let mut seen = 0;
            runtime
                .for_each_task_observation::<Error>(b, |view, _| {
                    assert_eq!(view.pid(), pid);
                    seen += 1;
                    Ok(())
                })
                .unwrap();
            assert_eq!(seen, 1);
        }
        loop {
            if runtime.cancel_step(b).unwrap() {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "runtime trace foreground retirement timed out"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        drop(runtime);
    }
    drop(gate);
    b.release_storage(Runtime::STORAGE_GROWTH).unwrap();
    b.release_storage(retained).unwrap();
    drain(service);
    require_reaped(&witness);
}

fn lifecycle(service: &mut Service, b: &mut Budget<'_>, cancel_at_birth: bool) {
    let slot = service.reserve_launch(b).unwrap().into_slot();
    let (child, gate, witness) = spawn_fixture(slot, true);
    let retained = Owner::STORAGE + Owner::ROOT_TRACE_GROWTH;
    b.reserve_storage(retained).unwrap();
    let mut trace = child.into_root_trace(b).unwrap();
    let root = trace.pid();
    trace.interrupt(b).unwrap();
    assert_eq!(next(&mut trace, b), Event::TrapStop);
    b.reserve_storage(Runtime::STORAGE_GROWTH).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    // SAFETY: dedicated bounded fixture with exclusive original custody. The
    // authenticated standalone fixture installs the exact checkpoint filter
    // immediately after its gate read; this test does not execute a compiler.
    let mut runtime = unsafe { trace.into_runtime_trace(deadline, b) }.unwrap();
    assert!(runtime.park_all_bounded(1, b).is_err());
    let before = b.work();
    runtime.park_all_bounded(8192, b).unwrap();
    assert!(b.work() - before <= Runtime::bounded_census_work(8192).unwrap());
    rustix::io::write(&gate, b"g").unwrap();
    runtime.resume_selected(b).unwrap();
    let (mut births, mut terminals, mut later_opens, mut events) = (0, 0, 0, 0);
    let mut observed_descendants = [None; 2];
    let mut last_generation = None;
    while !runtime.is_trace_retired() {
        assert!(
            Instant::now() < deadline,
            "runtime component deadline elapsed"
        );
        let Some(event) = runtime.poll(b).unwrap() else {
            std::thread::sleep(Duration::from_millis(1));
            continue;
        };
        events += 1;
        if let Some(previous) = last_generation {
            assert!(event.generation() > previous);
        }
        last_generation = Some(event.generation());
        assert!(events <= 256, "runtime component event bound exceeded");
        eprintln!("runtime component event {events}: {event:?}");
        if event.is_checkpoint() {
            if cancel_at_birth {
                runtime.park_all(b).unwrap();
            } else {
                runtime.park_all_bounded(8192, b).unwrap();
            }
            let entry = runtime.syscall_entry(b).unwrap();
            if matches!(entry.number, 2 | 257) && event.pid() == root && terminals > 0 {
                later_opens += 1;
            }
            runtime.step_syscall(b).unwrap();
        } else if event.is_birth() {
            let child = runtime.selected_birth_child(b).unwrap();
            assert_ne!(child, event.pid());
            while !runtime.hold_born_child(b).unwrap() {
                assert!(Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(1));
            }
            births += 1;
            assert_eq!(runtime.selected_birth_child(b).unwrap(), child);
            if cancel_at_birth {
                runtime.mark_cancellation();
                assert!(runtime.poll(b).is_err());
                while !runtime.cancel_step(b).unwrap() {
                    assert!(Instant::now() < deadline);
                    std::thread::sleep(Duration::from_millis(1));
                }
                assert!(runtime.root_completion().is_none());
                break;
            }
            runtime.resume_selected(b).unwrap();
        } else if event.is_syscall_stop() {
            runtime.syscall_result(b).unwrap();
            runtime.park_all(b).unwrap();
            runtime
                .for_each_task_observation::<Error>(b, |view, b| {
                    let (personality, length) = view.read_personality(b)?;
                    crate::trace_runtime::validate_personality(
                        std::str::from_utf8(&personality[..length]).unwrap(),
                    )
                    .unwrap();
                    if view.pid() != root && !observed_descendants.contains(&Some(view.pid())) {
                        let (file, charge) = view.duplicate_descriptor(0, b)?;
                        b.reserve_storage(charge.additional_storage())?;
                        let actual = rustix::fs::fstat(&file).unwrap();
                        let original = rustix::fs::fstat(&gate).unwrap();
                        assert_eq!(
                            (actual.st_dev, actual.st_ino),
                            (original.st_dev, original.st_ino)
                        );
                        drop(file);
                        b.release_storage(charge.additional_storage())?;
                        let (maps, charge) = view.read_maps(b)?;
                        b.reserve_storage(charge.additional_storage())?;
                        assert!(!maps.is_empty());
                        drop(maps);
                        b.release_storage(charge.additional_storage())?;
                        *observed_descendants
                            .iter_mut()
                            .find(|pid| pid.is_none())
                            .unwrap() = Some(view.pid());
                    }
                    Ok(())
                })
                .unwrap();
            runtime.resume_selected(b).unwrap();
            runtime.release_interrupts(b).unwrap();
        } else if event.is_exit_boundary() {
            runtime.resume_selected(b).unwrap();
        } else if event.is_terminal() {
            let consumed = runtime.acknowledge_terminal(b).unwrap();
            if event.pid() == root {
                assert_eq!(consumed.exit_code(), Some(9));
                assert_eq!(runtime.root_completion().unwrap().exit_code(), Some(9));
            } else {
                assert_eq!(consumed.exit_code(), Some(0));
                terminals += 1;
            }
            if !runtime.is_trace_retired() {
                runtime.release_interrupts(b).unwrap();
            }
        } else {
            panic!("unexpected runtime fixture stop: {event:?}");
        }
    }
    if cancel_at_birth {
        assert_eq!((births, terminals, later_opens), (1, 0, 0));
    } else {
        assert_eq!((births, terminals, later_opens), (2, 2, 2));
        assert!(observed_descendants.iter().all(Option::is_some));
    }
    assert_eq!(runtime.cleanup_after_retirement().unwrap(), Poll::Reaped);
    runtime.check_budget(b).unwrap();
    assert!(runtime.poll(b).is_err());
    assert!(runtime.resume_selected(b).is_err());
    let mut foreign_work = Work::new(LIMIT);
    let mut foreign = Budget::new(&mut foreign_work, LIMIT);
    foreign.reserve_storage(runtime.retained_storage()).unwrap();
    assert!(matches!(
        runtime.check_budget(&foreign),
        Err(Error::Resource(Resource::Accounting))
    ));
    drop(runtime);
    drop(gate);
    b.release_storage(Runtime::STORAGE_GROWTH).unwrap();
    b.release_storage(retained).unwrap();
    drain(service);
    require_reaped(&witness);
}
