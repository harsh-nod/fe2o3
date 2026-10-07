use super::*;

#[test]
fn registration_codec_is_canonical_and_distinct_from_occurrence_protocol() {
    for kind in [
        RegistryKind::Register,
        RegistryKind::Registered,
        RegistryKind::Bind,
        RegistryKind::Bound,
        RegistryKind::RegisterApplication,
        RegistryKind::RegisteredApplication,
        RegistryKind::AttachApplication,
        RegistryKind::ApplicationInstalled,
        RegistryKind::RegisterCustodianApplication,
        RegistryKind::RegisteredCustodianApplication,
    ] {
        let expected = packet(kind);
        let bytes = expected.encode().unwrap();
        assert_eq!(RegistryPacket::decode(&bytes).unwrap(), expected);
        assert!(Packet::decode(&bytes).is_err());
        assert!(
            fe2o3_runtime_protocol::WorkerV3ApplicationSessionMessageV1::decode(&bytes).is_err()
        );
        for length in 0..bytes.len() {
            assert!(RegistryPacket::decode(&bytes[..length]).is_err());
        }
        for offset in [0, 8, 9, 15] {
            let mut changed = bytes.clone();
            changed[offset] = 255;
            assert!(RegistryPacket::decode(&changed).is_err());
        }
        for (a, b) in [(16, 24), (24, 56)] {
            let mut changed = bytes.clone();
            changed[a..b].fill(0);
            assert!(RegistryPacket::decode(&changed).is_err());
        }
        let mut changed = bytes.clone();
        changed[56..88].fill(
            if matches!(
                kind,
                RegistryKind::Register
                    | RegistryKind::RegisterApplication
                    | RegistryKind::RegisterCustodianApplication
            ) {
                1
            } else {
                0
            },
        );
        assert!(RegistryPacket::decode(&changed).is_err());
        let mut extra = bytes;
        extra.push(0);
        assert!(RegistryPacket::decode(&extra).is_err());
    }
}

#[test]
fn registration_enforces_exact_rights_and_sender_not_socket_creator() {
    let (a, b) = observer_pair().unwrap();
    let sender = Endpoint::admit(a).unwrap();
    let receiver = Endpoint::admit(b).unwrap();
    let actual = current_identity().unwrap();
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    let child = ChildOwner(crate::test_process_execution::spawn(&mut command).unwrap());
    let other = child_identity(&child.0);
    for kind in [
        RegistryKind::Register,
        RegistryKind::Registered,
        RegistryKind::Bind,
        RegistryKind::Bound,
        RegistryKind::RegisterApplication,
        RegistryKind::RegisteredApplication,
        RegistryKind::AttachApplication,
        RegistryKind::ApplicationInstalled,
        RegistryKind::RegisterCustodianApplication,
        RegistryKind::RegisteredCustodianApplication,
    ] {
        let packet = packet(kind);
        for count in 0..=2 {
            let rights = vec![actual.pidfd.as_fd(); count];
            assert!(
                sender
                    .send_bytes(&packet.encode().unwrap(), &rights)
                    .unwrap()
            );
            assert_eq!(
                RegistryPacket::receive(&receiver, &actual).is_ok(),
                count == packet.rights()
            );
        }
        assert!(
            packet
                .send(&sender, &vec![actual.pidfd.as_fd(); packet.rights()])
                .unwrap()
        );
        assert!(RegistryPacket::receive(&receiver, &other).is_err());
    }
}
