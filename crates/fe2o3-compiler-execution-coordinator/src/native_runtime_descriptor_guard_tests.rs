use super::*;
use std::fs::OpenOptions;

fn open(flags: u64) -> Entry {
    Entry {
        number: 2,
        arguments: [0x1000, flags, 0, 0, 0, 0],
    }
}

#[test]
fn native_open_requirement_includes_readonly_creation_and_truncation_effects() {
    assert!(!open_requirement(open(0)).unwrap().write_side_effects);
    for flags in [1, 2, 0x40, 0x200, 0x41_0000] {
        assert!(open_requirement(open(flags)).unwrap().write_side_effects);
    }
    assert!(open_requirement(open(3)).is_err());
    assert!(open_requirement(open(1_u64 << 32)).is_err());
    let mut entry = open(0);
    entry.number = 257;
    entry.arguments = [(-100_i64) as u64, 0x2000, 0, 0, 0, 0];
    assert_eq!(open_requirement(entry).unwrap().directory, -100);
    entry.arguments[0] = (-100_i32) as u32 as u64;
    assert_eq!(open_requirement(entry).unwrap().directory, -100);
    entry.arguments[0] = 1_u64 << 32;
    assert!(open_requirement(entry).is_err());
}

#[test]
fn native_ioctl_query_requires_actual_supported_descriptor_kind() {
    let file = tempfile::tempfile().unwrap();
    let null = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")
        .unwrap();
    for request in [0x541b, 0x5401, 0x5413, 0x5412, u64::MAX] {
        assert!(query_ioctl(&file, request).is_err());
    }
    let (reader, _) = rustix::pipe::pipe().unwrap();
    let reader = File::from(reader);
    for request in [0x541b, 0x5401, 0x5413] {
        query_ioctl(&reader, request).unwrap();
        query_ioctl(&null, request).unwrap();
    }
    assert!(query_ioctl(&reader, 0x5412).is_err());
    assert!(query_ioctl(&null, 0x5412).is_err());
    let terminal = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/ptmx")
        .unwrap();
    for request in [0x541b, 0x5401, 0x5413] {
        query_ioctl(&terminal, request).unwrap();
    }
    // TIOCSTI and all device-changing requests remain refused.
    assert!(query_ioctl(&terminal, 0x5412).is_err());
}

#[test]
fn native_ioctl_socket_query_refuses_unreviewed_protocol_drivers() {
    let (unix, _) = net::socketpair(
        net::AddressFamily::UNIX,
        net::SocketType::STREAM,
        net::SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let unix = File::from(unix);
    query_ioctl(&unix, 0x541b).unwrap();
    assert!(query_ioctl(&unix, 0x5401).is_err());
    let inet = File::from(
        net::socket_with(
            net::AddressFamily::INET,
            net::SocketType::DGRAM,
            net::SocketFlags::CLOEXEC,
            None,
        )
        .unwrap(),
    );
    assert!(query_ioctl(&inet, 0x541b).is_err());
}

#[test]
fn native_receive_requires_actual_socket_and_credential_properties() {
    assert!(receive_socket(&tempfile::tempfile().unwrap(), false, wire::MSG_DONTWAIT).is_err());
    let (left, _) = net::socketpair(
        net::AddressFamily::UNIX,
        net::SocketType::SEQPACKET,
        net::SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    let left = File::from(left);
    assert!(receive_socket(&left, false, 0).is_err());
    let identity = receive_socket(&left, false, wire::MSG_DONTWAIT).unwrap();
    assert!(receive_socket(&left, true, wire::MSG_DONTWAIT).is_err());
    net::sockopt::set_socket_passcred(&left, true).unwrap();
    assert_eq!(
        receive_socket(&left, true, wire::MSG_DONTWAIT).unwrap(),
        identity
    );
    net::sockopt::set_socket_passcred(&left, false).unwrap();
    assert!(receive_socket(&left, true, wire::MSG_DONTWAIT).is_err());
}
