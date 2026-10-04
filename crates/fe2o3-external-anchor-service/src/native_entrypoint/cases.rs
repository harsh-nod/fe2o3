fn fixture(mode: &str) -> (tempfile::TempDir, [File; 7]) {
    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
        CompilerExecutionIssuerMeasurementV1 as M,
    };
    let dir = tempfile::tempdir().unwrap();
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    let root = dir.path().join("state");
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let lifecycle = dir.path().join(
        std::path::Path::new(
            fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1,
        )
        .file_name()
        .unwrap(),
    );
    fs::write(&lifecycle, []).unwrap();
    fs::set_permissions(&lifecycle, fs::Permissions::from_mode(0o400)).unwrap();
    let mut w = Work::new(NATIVE_EXTERNAL_ANCHOR_PROCESS_WORK_LIMIT_V2);
    let mut b = Budget::new(&mut w, NATIVE_EXTERNAL_ANCHOR_PROCESS_STORAGE_LIMIT_V2);
    let public = |n| SigningKey::from_bytes(&[n; 32]).verifying_key().to_bytes();
    let make_policy = |generation, b: &mut Budget<'_>| {
        let (p, c) = PolicyRecord::new(
            generation,
            M::new([1; 32], 1).unwrap(),
            M::new([2; 32], 2).unwrap(),
            public(3),
            public(7),
            b,
        )
        .unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        let (p, c) = Policy::create(p, b).unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        p
    };
    let p = make_policy(7, &mut b);
    let uid = rustix::process::geteuid().as_raw();
    let gid = rustix::process::getegid().as_raw();
    assert!(uid != 0 && gid != 0);
    let (s, c) = SupervisorRecord::new(
        uid + 10,
        gid + 10,
        Service::new(uid, gid).unwrap(),
        M::new([4; 32], 4).unwrap(),
        M::new([5; 32], 5).unwrap(),
        p.policy(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (s, c) = Supervisor::create(s, &mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (d, c) = DeploymentRecord::new(
        s.deployment(),
        p.policy(),
        M::new([6; 32], 6).unwrap(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (d, c) = Deployment::create(d, &mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let mut seed = [7; 32];
    b.reserve_storage(seed.len()).unwrap();
    let (key, c) = Key::create_and_zeroize(&mut seed, d.deployment(), &mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    b.release_storage(seed.len()).unwrap();
    let (key_image, c) = key.try_clone_for_transfer(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    if mode != "absent" {
        b.reserve_storage(Anchor::ROOT_STORAGE).unwrap();
        let (anchor, c) = Anchor::initialize(
            File::open(&root).unwrap().into(),
            key,
            d.deployment(),
            &mut b,
        )
        .unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        let retired = anchor.retained_storage();
        drop(anchor);
        b.release_storage(retired).unwrap();
    } else {
        let retired = key.retained_storage();
        drop(key);
        b.release_storage(retired).unwrap();
    }
    let p = if mode == "policy" {
        let charge = p.retained_storage();
        drop(p);
        b.release_storage(charge).unwrap();
        make_policy(8, &mut b)
    } else {
        p
    };
    let (policy, c) = p.try_clone_for_transfer(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (supervisor, c) = s.try_clone_for_transfer(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (deployment, c) = d.try_clone_for_transfer(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let peer = if mode == "socket" {
        use rustix::net::{AddressFamily, SocketFlags, SocketType, socketpair};
        let (peer, client) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )
        .unwrap();
        drop(client);
        File::from(peer)
    } else {
        File::open("/dev/null").unwrap()
    };
    let files = [
        peer,
        File::open(root).unwrap(),
        File::open(lifecycle).unwrap(),
        policy,
        supervisor,
        deployment,
        key_image,
    ];
    (dir, files)
}

#[test]
fn startup_refusal_unwind_and_native_custody_preserve_state_and_accounting() {
    for mode in MODES {
        let (dir, files) = fixture(mode);
        let state = dir.path().join("state").join(crate::STATE_FILE);
        let before = fs::read(&state).ok();
        let report = dir.path().join("child-report");
        let status = spawn(files, FAMILY, mode, &report);
        assert!(
            status.success(),
            "{FAMILY} {mode}: {status}; {}",
            fs::read_to_string(report).unwrap_or_default()
        );
        assert_eq!(
            fs::read(state).ok(),
            before,
            "startup must never initialize/reset: {mode}"
        );
    }
}

#[test]
fn native_startup_real_socket_eof_with_rootless_admission_fixture() {
    let (dir, files) = fixture("socket");
    let report = dir.path().join("child-report");
    let status = spawn(files, FAMILY, "socket", &report);
    assert!(
        status.success(),
        "{FAMILY}: {status}; {}",
        fs::read_to_string(report).unwrap_or_default()
    );
}
