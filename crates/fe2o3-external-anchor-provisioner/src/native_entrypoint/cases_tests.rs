fn fixture(mode: &str) -> (tempfile::TempDir, [File; 9]) {
    fixture_in(mode, None)
}
fn fixture_in(mode: &str, existing: Option<tempfile::TempDir>) -> (tempfile::TempDir, [File; 9]) {
    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
        CompilerExecutionIssuerMeasurementV1 as M,
    };
    use sha2::{Digest, Sha256};
    let dir = existing.unwrap_or_else(|| tempfile::tempdir().unwrap());
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    let root = dir.path().join("state");
    if !root.exists() {
        fs::create_dir(&root).unwrap();
    }
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    let life = dir.path().join(
        std::path::Path::new(
            fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1,
        )
        .file_name()
        .unwrap(),
    );
    if !life.exists() {
        fs::write(&life, []).unwrap();
    }
    fs::set_permissions(&life, fs::Permissions::from_mode(0o400)).unwrap();
    let mut w = Work::new(NATIVE_EXTERNAL_ANCHOR_HELPER_PROCESS_WORK_V2);
    let mut b = Budget::new(&mut w, NATIVE_EXTERNAL_ANCHOR_HELPER_PROCESS_STORAGE_V2);
    let bytes = crate::entrypoint::tests::static_pause_elf();
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    let m = M::new(digest, bytes.len() as u64).unwrap();
    let source = dir.path().join("probe");
    if source.exists() {
        fs::remove_file(&source).unwrap();
    }
    fs::write(&source, bytes).unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o555)).unwrap();
    b.reserve_storage(Executable::file_storage(measurement(m).unwrap()).unwrap())
        .unwrap();
    let (image, c) = Executable::seal_source_for_owner(
        File::open(source).unwrap(),
        measurement(m).unwrap(),
        Owner::current(),
        "native helper test probe",
        &mut b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (image_file, c) = image.try_clone_for_exec(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
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
    let service = Service::new(uid + u32::from(mode == "credentials"), gid).unwrap();
    let (s, c) = SupervisorRecord::new(
        uid + 10,
        gid + 10,
        service,
        M::new([4; 32], 4).unwrap(),
        M::new([5; 32], 5).unwrap(),
        p.policy(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (s, c) = Supervisor::create(s, &mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (d, c) = DeploymentRecord::new(s.deployment(), p.policy(), m, &mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (d, c) = Deployment::create(d, &mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let mismatch = if mode == "provisioning" {
        let (other, c) = DeploymentRecord::new(
            s.deployment(),
            p.policy(),
            M::new([9; 32], 9).unwrap(),
            &mut b,
        )
        .unwrap();
        b.reserve_storage(c.additional_storage()).unwrap();
        Some(other)
    } else {
        None
    };
    let (q, c) = ProvisioningRecord::new(
        mismatch.as_ref().unwrap_or(d.deployment()),
        M::new([6; 32], 6).unwrap(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (q, c) = Provisioning::create(q, &mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let mut seed = [7; 32];
    b.reserve_storage(32).unwrap();
    let (key, c) = Key::create_and_zeroize(&mut seed, d.deployment(), &mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    b.release_storage(32).unwrap();
    let (key, c) = key.try_clone_for_transfer(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let replacement = if mode == "policy" {
        Some(make_policy(8, &mut b))
    } else {
        None
    };
    let (policy, c) = replacement
        .as_ref()
        .unwrap_or(&p)
        .try_clone_for_transfer(&mut b)
        .unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (supervisor, c) = s.try_clone_for_transfer(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (deployment, c) = d.try_clone_for_transfer(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    let (provisioning, c) = q.try_clone_for_transfer(&mut b).unwrap();
    b.reserve_storage(c.additional_storage()).unwrap();
    if mode == "corrupt" {
        fs::write(root.join("anchor-state-v1"), b"malformed").unwrap();
    }
    (
        dir,
        [
            File::open("/dev/null").unwrap(),
            File::open(root).unwrap(),
            image_file,
            File::open(life).unwrap(),
            policy,
            supervisor,
            deployment,
            key,
            provisioning,
        ],
    )
}

#[test]
fn native_helper_refusals_and_late_failure_preserve_custody_and_state() {
    for mode in MODES {
        let (dir, files) = fixture(mode);
        let state = dir.path().join("state/anchor-state-v1");
        let before = fs::read(&state).ok();
        let report = dir.path().join("report");
        let status = wait(&mut spawn(files, FAMILY, mode, &report));
        assert!(
            status.success(),
            "{FAMILY} {mode}: {status}; {}",
            fs::read_to_string(&report).unwrap_or_default()
        );
        let after = fs::read(&state).ok();
        if [
            "after-state",
            "unwind",
            "ready",
            "pre-exec",
            "post-ready-work",
            "post-ready-storage",
        ]
        .contains(mode)
        {
            assert!(
                before.is_none() && after.is_some(),
                "late failure must retain genesis"
            );
        } else {
            assert_eq!(
                after, before,
                "refusal must not initialize/reset state: {mode}"
            );
        }
    }
}

#[test]
fn native_helper_exec_retains_only_the_exact_daemon_table_and_lifecycle_lock() {
    let (dir, files) = fixture("exec");
    let m = files[DAEMON].metadata().unwrap();
    let expected = (m.dev(), m.ino());
    let report = dir.path().join("report");
    let mut child = spawn(files, FAMILY, "exec", &report);
    let deadline = Instant::now() + Duration::from_secs(20);
    let result = catch_unwind(AssertUnwindSafe(|| {
        let fds = loop {
            assert!(
                child.try_wait().unwrap().is_none(),
                "helper terminated before exec: {}",
                fs::read_to_string(&report).unwrap_or_default()
            );
            let fds = fs::read_dir(format!("/proc/{}/fd", child.id())).and_then(|entries| {
                let mut fds = entries
                    .map(|entry| Ok(entry?.file_name().to_str().unwrap().parse::<i32>().unwrap()))
                    .collect::<std::io::Result<Vec<_>>>()?;
                fds.sort_unstable();
                Ok(fds)
            });
            if let Err(error) = &fds {
                assert!(
                    matches!(
                        error.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied
                    ),
                    "unexpected descriptor observation failure: {error}"
                );
            }
            // Exec transiently changes procfs access and publishes exe before CLOEXEC cleanup.
            if fds
                .as_ref()
                .is_ok_and(|fds| fds == &[3, 4, 5, 202, 220, 221, 222])
                && fs::metadata(format!("/proc/{}/exe", child.id()))
                    .is_ok_and(|m| (m.dev(), m.ino()) == expected)
            {
                break fds.unwrap();
            }
            assert!(
                Instant::now() < deadline,
                "native terminal exec exceeded deadline; descriptors: {fds:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(fds, [3, 4, 5, 202, 220, 221, 222]);
        let lock = dir.path().join(
            std::path::Path::new(
                fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1,
            )
            .file_name()
            .unwrap(),
        );
        let probe = File::open(lock).unwrap();
        assert_eq!(
            rustix::fs::flock(&probe, rustix::fs::FlockOperation::NonBlockingLockExclusive),
            Err(rustix::io::Errno::WOULDBLOCK)
        );
        assert!(dir.path().join("state/anchor-state-v1").is_file());
    }));
    let _ = child.kill();
    let _ = child.wait();
    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
    let lock = dir.path().join(
        std::path::Path::new(
            fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_LIFECYCLE_LOCK_PATH_V1,
        )
        .file_name()
        .unwrap(),
    );
    rustix::fs::flock(
        File::open(lock).unwrap(),
        rustix::fs::FlockOperation::NonBlockingLockExclusive,
    )
    .unwrap();
}

#[test]
fn native_helper_reopens_exact_genesis_after_every_late_failure() {
    for failure in [
        "after-state",
        "unwind",
        "ready",
        "pre-exec",
        "post-ready-work",
        "post-ready-storage",
    ] {
        let (dir, files) = fixture(failure);
        let report = dir.path().join("report");
        assert!(
            wait(&mut spawn(files, FAMILY, failure, &report)).success(),
            "{}",
            fs::read_to_string(&report).unwrap_or_default()
        );
        let state = dir.path().join("state/anchor-state-v1");
        let expected = fs::read(&state).unwrap();
        let (dir, files) = fixture_in("reopen", Some(dir));
        assert!(
            wait(&mut spawn(files, FAMILY, "reopen", &report)).success(),
            "{}",
            fs::read_to_string(&report).unwrap_or_default()
        );
        assert_eq!(
            fs::read(dir.path().join("state/anchor-state-v1")).unwrap(),
            expected
        );
    }
}

#[test]
fn native_helper_real_bootstrap_ready_with_rootless_profiles() {
    use rustix::net::{
        AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
        SocketFlags, SocketType, recvmsg, socketpair,
    };
    let (dir, mut files) = fixture("socket");
    let (parent, child) = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
        None,
    )
    .unwrap();
    files[BOOTSTRAP] = child.into();
    let report = dir.path().join("report");
    assert!(
        wait(&mut spawn(files, FAMILY, "socket", &report)).success(),
        "{}",
        fs::read_to_string(&report).unwrap_or_default()
    );
    let mut payload = [0_u8; crate::EXTERNAL_ANCHOR_PROVISIONING_READY_BYTES_V1];
    let mut space = [std::mem::MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut ancillary = RecvAncillaryBuffer::new(&mut space);
    let result = recvmsg(
        &parent,
        &mut [std::io::IoSliceMut::new(&mut payload)],
        &mut ancillary,
        RecvFlags::CMSG_CLOEXEC,
    )
    .unwrap();
    assert_eq!(result.bytes, payload.len());
    assert!(
        !result
            .flags
            .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
    );
    assert_eq!(
        Ready::decode(&payload).unwrap().disposition(),
        ReadyDisposition::Initialized
    );
    let mut endpoints = Vec::new();
    for message in ancillary.drain() {
        match message {
            RecvAncillaryMessage::ScmRights(rights) => endpoints.extend(rights),
            _ => panic!("unexpected ancillary"),
        }
    }
    assert_eq!(endpoints.len(), 1);
}
