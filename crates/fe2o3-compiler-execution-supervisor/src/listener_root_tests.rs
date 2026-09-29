use super::*;
use crate::{
    IssuerServiceCredentialProfileV1 as Credentials,
    tests::{Fixture, bound_named_seqpacket_socket},
};

fn admitted(f: &Fixture) -> ProvisionedProtectedIssuerSocketV1 {
    let path = f.root.join("s.sock");
    let credentials = Credentials::new(
        rustix::process::geteuid().as_raw(),
        rustix::process::getegid().as_raw(),
    )
    .unwrap();
    ProvisionedProtectedIssuerSocketV1::admit_checked(
        bound_named_seqpacket_socket(&path),
        &path,
        ListenerFilesystemPolicyV1::fixture(credentials),
    )
    .unwrap()
}

#[test]
fn original_listener_rejects_changed_passcred_and_path() {
    let f = Fixture::new("lr-credentials");
    let mut owner = admitted(&f);
    owner.activate_original_root().unwrap();
    rustix::net::sockopt::set_socket_passcred(&owner.socket.descriptor, false).unwrap();
    assert!(matches!(
        owner.try_accept_original_root(),
        Err(SocketError::InvalidListener(
            "original-root listener lost credentials"
        ))
    ));
    rustix::net::sockopt::set_socket_passcred(&owner.socket.descriptor, true).unwrap();
    assert!(owner.try_accept_original_root().unwrap().is_none());
    std::fs::remove_file(f.root.join("s.sock")).unwrap();
    assert!(owner.try_accept_original_root().is_err());
}

#[test]
fn post_accept_revalidation_closes_the_actual_new_endpoint() {
    let f = Fixture::new("lr-post-accept");
    let mut owner = admitted(&f);
    owner.activate_original_root().unwrap();
    let client = rustix::net::socket_with(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    rustix::net::connect(
        &client,
        &SocketAddrUnix::new(&f.root.join("s.sock")).unwrap(),
    )
    .unwrap();
    let accepted = accept_with(
        &owner.socket.descriptor,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
    )
    .unwrap();
    std::fs::remove_file(f.root.join("s.sock")).unwrap();
    // Exercise the same post-syscall validation with the real newly owned FD.
    // This seam cannot construct invocation or compiler authority.
    assert!(owner.finish_root_accept(Some(accepted)).is_err());
    let mut bytes = [0; 1];
    let result = rustix::net::recv(&client, &mut bytes, rustix::net::RecvFlags::DONTWAIT).unwrap();
    assert_eq!(result, 0);
}

#[test]
fn listener_export_is_monotone_across_alias_drop_and_repeated_bound_clones() {
    let f = Fixture::new("lr-export-fail");
    let mut owner = admitted(&f);
    // Repeated old bound transfers remain valid, but dropping their aliases
    // cannot make the original-root path available again.
    drop(owner.clone_checked().unwrap());
    drop(owner.clone_checked().unwrap());
    assert!(matches!(
        owner.activate_original_root(),
        Err(SocketError::InvalidListener(
            "listener was exported for indirect activation"
        ))
    ));
    assert!(!rustix::net::sockopt::socket_acceptconn(&owner.socket.descriptor).unwrap());
}
