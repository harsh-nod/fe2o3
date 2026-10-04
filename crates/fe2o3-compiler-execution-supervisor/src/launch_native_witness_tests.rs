struct HandoffWitness {
    control: Witness,
    service: Witness,
}
impl HandoffWitness {
    fn assert_released(&self) {
        self.control.assert_released();
        self.service.assert_released();
    }
}

fn accept_with_witness(
    supervisor: &Supervisor,
    submitter: &OwnedFd,
    budget: &mut Budget<'_>,
) -> (Accepted, HandoffWitness, u32, usize) {
    send_packet(submitter, &frame(FAMILY.handoff_tag(), 0), &[]).unwrap();
    let (payload, [control]) = receive_packet::<1>(submitter, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(&payload[..4], FAMILY.handoff_tag());
    let pid = u32::from_le_bytes(payload[4..].try_into().unwrap());
    let pidfds = pidfd_references(pid);
    let witness = Witness::new(&control, 1);
    budget.reserve_storage(Accepted::CONTROL_STORAGE).unwrap();
    let (accepted, delta) = supervisor
        .accept_handoff(control, IO_TIMEOUT, budget)
        .unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(accepted.manifest().client().pid(), pid);
    assert_ne!(
        accepted.submitter().uid(),
        rustix::process::geteuid().as_raw()
    );
    let (service, client, retained) = accepted.clone_launch_peers(budget).unwrap();
    budget.reserve_storage(retained).unwrap();
    let service_witness = Witness::new(&service, 2);
    drop((service, client));
    budget.release_storage(retained).unwrap();
    (
        accepted,
        HandoffWitness {
            control: witness,
            service: service_witness,
        },
        pid,
        pidfds,
    )
}

fn prepared_witnesses_with_readiness(owner: &Prepared, pin_readiness_writer: bool) -> Vec<Witness> {
    let mut witnesses = vec![
        Witness::new(&owner.launcher, 1),
        Witness::new(&owner.issuer, 1),
        Witness::new(&owner.static_manifest_file, 1),
    ];
    for (index, source) in owner.sources.iter().enumerate() {
        // Pidfds use target identities, not Linux's shared anonymous-inode key.
        if matches!(
            index,
            CLIENT_PIDFD_SOURCE_INDEX | EXTERNAL_ANCHOR_PIDFD_SOURCE_INDEX
        ) || (index == READINESS_SOURCE_INDEX && !pin_readiness_writer)
        {
            continue;
        }
        let owned = match index {
            STDOUT_SOURCE_INDEX | STDERR_SOURCE_INDEX | READINESS_SOURCE_INDEX => 2,
            // Accepted custody and the capability each retain their original FD.
            SERVICE_PEER_SOURCE_INDEX | LAUNCH_MANIFEST_SOURCE_INDEX => 2,
            _ => 1,
        };
        witnesses.push(Witness::new(source, owned));
    }
    witnesses
}
