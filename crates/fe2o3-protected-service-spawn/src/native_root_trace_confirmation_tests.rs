use super::*;

pub(super) fn run(mode: &str, service: &mut Service, b: &mut Budget<'_>) {
    if mode == "confirm-budget-address" {
        moved_budget(service);
        return;
    }
    let slot = service.reserve_launch(b).unwrap().into_slot();
    let (child, gate, witness) = spawn(slot);
    let retained = Owner::STORAGE + Owner::ROOT_TRACE_GROWTH;
    b.reserve_storage(retained).unwrap();
    let mut trace = child.into_root_trace(b).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let custody = service.report().unwrap();

    // SAFETY: Command::spawn completed the fixture's initial exec/CLOEXEC. Its
    // builtin read and explicit exec create no descendants or surviving aliases.
    assert!(matches!(
        unsafe { trace.confirm_exec(b) },
        Err(Error::State(_))
    ));
    assert!(trace.child.record().unwrap().retains_spawn_lease());
    assert_eq!(trace.held, Event::Pending);
    rustix::io::write(&gate, b"go\n").unwrap();
    assert_eq!(next(&mut trace, b), Event::Exec);
    assert!(trace.child.record().unwrap().retains_spawn_lease());

    match mode {
        "confirm-exact" => {
            b.release_storage(1).unwrap();
            // SAFETY: this fixture's actual exec is held; its aliases closed on exec.
            assert!(matches!(
                unsafe { trace.confirm_exec(b) },
                Err(Error::Resource(Resource::Accounting))
            ));
            assert!(trace.child.record().unwrap().retains_spawn_lease());
            assert_eq!(trace.held, Event::Exec);
            b.reserve_storage(1).unwrap();

            let pressure = LIMIT - retained - RootTaskTraceV2::CONFIRM_EXEC_SCRATCH;
            b.reserve_storage(pressure).unwrap();
            b.charge_work(LIMIT - b.work() - RootTaskTraceV2::CONFIRM_EXEC_WORK)
                .unwrap();
            let entry = b.storage();
            // SAFETY: the owned wait observed this fixture's exec and all aliases closed.
            unsafe { trace.confirm_exec(b) }.unwrap();
            assert_eq!(b.work(), LIMIT);
            assert_eq!(b.storage(), entry);
            assert_eq!(b.peak_storage(), LIMIT);
            assert!(!trace.child.record().unwrap().retains_spawn_lease());
            assert_eq!(trace.held, Event::Exec);
            assert!(
                rustix::process::waitid(
                    WaitId::PidFd(witness.as_fd()),
                    WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
                )
                .unwrap()
                .is_none()
            );
            b.release_storage(pressure).unwrap();
        }
        "confirm-resumed" => {
            trace.resume(b).unwrap();
            assert_eq!(trace.held, Event::Pending);
            // SAFETY: the fixture's authenticated exec/alias closure survives resume.
            assert!(matches!(
                unsafe { trace.confirm_exec(b) },
                Err(Error::State(_))
            ));
            assert!(trace.child.record().unwrap().retains_spawn_lease());
            assert_eq!(trace.held, Event::Pending);
        }
        "confirm-short-work" => {
            b.charge_work(LIMIT - b.work() - (RootTaskTraceV2::CONFIRM_EXEC_WORK - 1))
                .unwrap();
            let before = b.work();
            // SAFETY: this fixture's actual exec is held and its aliases closed.
            assert!(matches!(
                unsafe { trace.confirm_exec(b) },
                Err(Error::Resource(Resource::Work(_)))
            ));
            assert_eq!(b.work() - before, RootTaskTraceV2::OPERATION_WORK + ENTRY);
            assert_eq!(b.failed_work(), Some(LIMIT + 1));
            assert!(trace.child.record().unwrap().retains_spawn_lease());
            assert_eq!(trace.held, Event::Exec);
        }
        "confirm-short-storage" => {
            let pressure = LIMIT - retained - RootTaskTraceV2::CONFIRM_EXEC_SCRATCH + 1;
            b.reserve_storage(pressure).unwrap();
            let entry = b.storage();
            let before = b.work();
            // SAFETY: this fixture's actual exec is held and its aliases closed.
            assert!(matches!(
                unsafe { trace.confirm_exec(b) },
                Err(Error::Resource(Resource::Storage(_)))
            ));
            assert_eq!(b.work() - before, RootTaskTraceV2::CONFIRM_EXEC_WORK);
            assert_eq!(b.storage(), entry);
            assert_eq!(b.failed_storage(), Some(LIMIT + 1));
            assert!(trace.child.record().unwrap().retains_spawn_lease());
            assert_eq!(trace.held, Event::Exec);

            b.release_storage(1).unwrap();
            let before = b.work();
            // SAFETY: refusal left the same owned exec stop and closed aliases intact.
            unsafe { trace.confirm_exec(b) }.unwrap();
            assert_eq!(b.work() - before, RootTaskTraceV2::CONFIRM_EXEC_WORK);
            assert_eq!(b.storage(), entry - 1);
            assert_eq!(b.peak_storage(), LIMIT);
            assert_eq!(b.failed_storage(), Some(LIMIT + 1));
            assert!(!trace.child.record().unwrap().retains_spawn_lease());
            assert_eq!(trace.held, Event::Exec);
            b.release_storage(pressure - 1).unwrap();
        }
        _ => panic!("unknown confirmation fixture: {mode}"),
    }
    assert!(ledger == b.work_ledger_identity_v1());
    assert_eq!(b.storage(), retained);
    assert_eq!(trace.retained_storage(), retained);
    assert_eq!(service.report().unwrap(), custody);
    assert_ne!(trace.cancel(), Poll::Quarantined);
    drop(trace);
    drain(service);
    require_reaped(&witness);
}

fn moved_budget(service: &mut Service) {
    let mut work = Work::new(LIMIT);
    let mut other_work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let mut other = Budget::new(&mut other_work, LIMIT);
    let (reservation, resources, charge) = service.reserve_retaining((), 0, &mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (child, gate, witness) = spawn(reservation.into_slot());
    let original = Retained::<()>::storage_for(0).unwrap();
    b.reserve_storage(original - b.storage()).unwrap();
    let retained = Retained::new(child, resources, original);
    let complete = retained.root_trace_storage().unwrap();
    b.reserve_storage(complete - original).unwrap();
    other.reserve_storage(complete).unwrap();
    let address = &b as *const Budget<'_> as usize;
    let ledger = b.work_ledger_identity_v1();
    let mut trace = retained.into_root_trace(&mut b).unwrap();
    rustix::io::write(&gate, b"go\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let event = trace.poll(&mut b).unwrap();
        if event.is_exec() {
            break;
        }
        assert!(event.is_pending());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    let custody = service.report().unwrap();
    std::mem::swap(&mut b, &mut other);
    assert_eq!(&b as *const Budget<'_> as usize, address);
    assert!(b.work_ledger_identity_v1() != ledger);
    assert_ne!(&other as *const Budget<'_> as usize, address);
    assert!(other.work_ledger_identity_v1() == ledger);
    for candidate in [&mut b, &mut other] {
        assert!(matches!(
            trace.poll(candidate),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert!(matches!(
            trace.resume(candidate),
            Err(Error::Resource(Resource::Accounting))
        ));
        // SAFETY: this exact fixture exec and alias closure were observed before the swap.
        assert!(matches!(
            unsafe { trace.confirm_exec(candidate) },
            Err(Error::Resource(Resource::Accounting))
        ));
        assert!(matches!(
            trace.with_resources::<(), Error>(candidate, |_, _| panic!(
                "foreign Budget reached retained backing"
            )),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(candidate.storage(), complete);
    }
    std::mem::swap(&mut b, &mut other);
    assert!(ledger == b.work_ledger_identity_v1());
    assert_eq!(service.report().unwrap(), custody);
    assert!(trace.poll(&mut b).unwrap().is_exec());
    trace
        .with_resources::<(), Error>(&mut b, |_, _| Ok(()))
        .unwrap();
    let before = b.work();
    // SAFETY: restoring the account preserves this fixture's held exec and alias closure.
    unsafe { trace.confirm_exec(&mut b) }.unwrap();
    assert_eq!(b.work() - before, RootTaskTraceV2::CONFIRM_EXEC_WORK);
    assert_eq!(b.storage(), complete);
    assert_eq!(service.report().unwrap(), custody);
    assert!(trace.poll(&mut b).unwrap().is_exec());
    trace.resume(&mut b).unwrap();
    assert_ne!(trace.cancel(), Poll::Quarantined);
    drop(trace);
    drain(service);
    require_reaped(&witness);
}
