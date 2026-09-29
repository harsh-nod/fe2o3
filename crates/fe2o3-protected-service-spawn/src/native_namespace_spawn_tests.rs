use super::*;

#[test]
fn mapping_release_sends_one_byte_and_closes_its_writer() {
    let gate = MappingGate::new().unwrap();
    let observer = rustix::io::fcntl_dupfd_cloexec(&gate.reader, 0).unwrap();
    gate.release().unwrap();
    let mut bytes = [0; 2];
    assert_eq!(rustix::io::read(&observer, &mut bytes).unwrap(), 1);
    assert_eq!(bytes[0], crate::PROTECTED_SERVICE_GATE_RELEASE_V1);
    assert_eq!(rustix::io::read(&observer, &mut bytes).unwrap(), 0);
}

#[test]
fn mapping_release_is_safe_after_the_child_read_alias_has_closed() {
    let gate = MappingGate::new().unwrap();
    let child_alias = rustix::io::fcntl_dupfd_cloexec(&gate.reader, 0).unwrap();
    drop(child_alias);
    // The gate's own reader prevents SIGPIPE without changing signal disposition.
    gate.release().unwrap();
}
