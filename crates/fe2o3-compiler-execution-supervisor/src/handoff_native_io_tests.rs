#[test]
fn finite_receive_preserves_order_and_cloexec() {
    let (send, receive) = pair();
    let (first, second) = pair();
    send_packet(&send, &[7; BYTES], &[first.as_fd(), second.as_fd()]).unwrap();
    let (payload, rights) = transport::receive(&receive, Instant::now() + IO_TIMEOUT).unwrap();
    assert_eq!(payload, [7; BYTES]);
    for (received, original) in rights.iter().zip([&first, &second]) {
        assert_eq!(
            checks::snapshot(received).unwrap(),
            checks::snapshot(original).unwrap()
        );
        assert!(
            rustix::io::fcntl_getfd(received)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
    }
}

#[test]
fn finite_receive_drains_bad_packets_and_obeys_deadline() {
    let (source, _held) = pair();
    let object = checks::snapshot(&source).unwrap();
    let before = references(object);
    for (length, count) in [
        (BYTES - 1, 2),
        (BYTES + 1, 2),
        (BYTES, 0),
        (BYTES, 1),
        (BYTES, 3),
    ] {
        let (send, receive) = pair();
        send_packet(&send, &vec![0; length], &vec![source.as_fd(); count]).unwrap();
        assert!(matches!(
            transport::receive(&receive, Instant::now() + IO_TIMEOUT),
            Err(HandoffError::MalformedTransfer)
        ));
        assert_eq!(references(object), before);
    }
    let (send, receive) = pair();
    crate::handoff_v2_test_process::send_excess_rights(&send, &[0; BYTES], &source);
    assert!(matches!(
        transport::receive(&receive, Instant::now() + IO_TIMEOUT),
        Err(HandoffError::MalformedTransfer)
    ));
    assert_eq!(references(object), before);
    let (send, receive) = pair();
    assert!(matches!(
        transport::receive(&receive, Instant::now()),
        Err(HandoffError::Timeout)
    ));
    drop(send);
    assert!(transport::receive(&receive, Instant::now() + IO_TIMEOUT).is_err());
}

#[test]
fn shared_socket_failures_preserve_legacy_and_native_categories() {
    use crate::ProtectedIssuerHandoffErrorV1 as Legacy;
    let errors = [
        checks::Failure::InvalidControl("test"),
        checks::Failure::SubmitterCredentialsMismatch,
        checks::Failure::InvalidServicePeer,
        checks::Failure::ServicePeerCredentialsMismatch,
        checks::Failure::DescriptorChanged,
        checks::Failure::DescriptorAlias,
        checks::Failure::Io(rustix::io::Errno::BADF),
    ];
    for error in errors {
        let legacy = Legacy::from(error);
        let native = HandoffError::from(error);
        // Both adapters retain the same original diagnostics, including errno text.
        assert_eq!(legacy.to_string(), native.to_string());
        assert_eq!(legacy.source().is_some(), native.source().is_some());
    }
    let (control, _held) = pair();
    checks::control_shape(&control).unwrap();
    let client = checks::control_peer(&control).unwrap();
    checks::service_peer(&control, client).unwrap();
    let snapshot = checks::snapshot(&control).unwrap();
    assert!(matches!(
        checks::distinct(snapshot, snapshot, Snapshot(1, 2, 3)),
        Err(checks::Failure::DescriptorAlias)
    ));
    rustix::io::fcntl_setfd(&control, rustix::io::FdFlags::empty()).unwrap();
    assert!(matches!(
        checks::control_shape(&control),
        Err(checks::Failure::InvalidControl(
            "control descriptor is inheritable"
        ))
    ));
    assert!(matches!(
        checks::service_peer(&control, client),
        Err(checks::Failure::InvalidServicePeer)
    ));
}
