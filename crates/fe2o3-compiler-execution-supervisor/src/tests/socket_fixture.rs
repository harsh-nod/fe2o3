use super::*;

#[test]
fn named_socket_fixture_supports_ci_length_parent_and_long_test_name() {
    let base = tempfile::tempdir_in("/tmp").unwrap();
    let parent = base
        .path()
        .join("x".repeat(55 - base.path().as_os_str().len()));
    fs::create_dir(&parent).unwrap();
    let fixture = Fixture::with_code_in(&"fixture-name-".repeat(32), &[0xc3], &parent);
    let path = fixture.root.join("supervisor.sock");
    let socket = bound_named_seqpacket_socket(&path);
    assert_eq!(fixture.root.parent(), Some(parent.as_path()));
    assert_eq!(fs::metadata(&fixture.root).unwrap().mode() & 0o777, 0o700);
    drop(socket);
    let root = fixture.root.clone();
    drop(fixture);
    assert!(!root.exists());
}

#[test]
fn same_named_fixtures_have_independent_owned_directories() {
    let first = Fixture::new("same-name");
    let second = Fixture::new("same-name");
    assert_ne!(first.root, second.root);
    drop(first);
    assert!(second.image.is_file());
}
