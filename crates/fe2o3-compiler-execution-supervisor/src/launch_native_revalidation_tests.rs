fn refused(owner: &Prepared, supervisor: &Supervisor, budget: &mut Budget<'_>) -> Error {
    let floor = budget.storage();
    let error = owner.revalidate(supervisor, budget).unwrap_err();
    assert_eq!(budget.storage(), floor);
    error
}

fn mutations(owner: &mut Prepared, supervisor: &Supervisor, budget: &mut Budget<'_>) {
    let pipes = [
        owner.sources[STDIN_SOURCE_INDEX].as_fd(),
        owner.sources[STDOUT_SOURCE_INDEX].as_fd(),
        owner.sources[STDERR_SOURCE_INDEX].as_fd(),
        owner.sources[READINESS_SOURCE_INDEX].as_fd(),
        owner.stdout_reader.as_fd(),
        owner.stderr_reader.as_fd(),
        owner.readiness_reader.as_fd(),
    ];
    for pipe in pipes {
        let flags = rustix::io::fcntl_getfd(pipe).unwrap();
        rustix::io::fcntl_setfd(pipe, FdFlags::empty()).unwrap();
        assert!(matches!(
            refused(owner, supervisor, budget),
            Error::InvalidDescriptor { .. }
        ));
        rustix::io::fcntl_setfd(pipe, flags).unwrap();
        let status = rustix::fs::fcntl_getfl(pipe).unwrap();
        rustix::fs::fcntl_setfl(pipe, status - OFlags::NONBLOCK).unwrap();
        assert!(matches!(
            refused(owner, supervisor, budget),
            Error::InvalidDescriptor { .. }
        ));
        rustix::fs::fcntl_setfl(pipe, status).unwrap();
    }
    for (left, right) in [
        (STDOUT_SOURCE_INDEX, STDERR_SOURCE_INDEX),
        (POLICY_SOURCE_INDEX, SIGNING_KEY_SOURCE_INDEX),
        (ROOT_SOURCE_INDEX, SERVICE_PEER_SOURCE_INDEX),
        (
            CLIENT_PIDFD_SOURCE_INDEX,
            EXTERNAL_ANCHOR_PIDFD_SOURCE_INDEX,
        ),
        (SERVICE_PEER_SOURCE_INDEX, EXTERNAL_ANCHOR_PEER_SOURCE_INDEX),
    ] {
        owner.sources.swap(left, right);
        assert!(matches!(
            refused(owner, supervisor, budget),
            Error::DescriptorChanged(_) | Error::Supervisor(_) | Error::Handoff(_)
        ));
        owner.sources.swap(left, right);
    }
    std::mem::swap(&mut owner.stdout_reader, &mut owner.stderr_reader);
    assert!(matches!(
        refused(owner, supervisor, budget),
        Error::DescriptorChanged(_)
    ));
    std::mem::swap(&mut owner.stdout_reader, &mut owner.stderr_reader);
    std::mem::swap(&mut owner.launcher, &mut owner.issuer);
    assert!(matches!(
        refused(owner, supervisor, budget),
        Error::Supervisor(_)
    ));
    std::mem::swap(&mut owner.launcher, &mut owner.issuer);

    let original = owner.static_manifest.clone();
    for index in 0..SOURCE_COUNT_V1 {
        let mut entries = original.descriptors().to_vec();
        let object = entries[index].object();
        let changed = match object.class() {
            Class::Fstat => Object::new(
                object.device(),
                object.inode(),
                object.size() + 1,
                object.mode(),
            ),
            Class::ProcessPidfd => Object::new_process_pidfd(
                object.device(),
                object.inode(),
                object.size() + 1,
                object.mode(),
            ),
        };
        entries[index] = Descriptor::for_index(index, index as i32, changed).unwrap();
        owner.static_manifest = StaticManifest::from_descriptors(
            original.parent_pid(),
            original.parent_start_time(),
            *original.executable(),
            &entries,
        )
        .unwrap();
        assert!(matches!(
            refused(owner, supervisor, budget),
            Error::DescriptorChanged("static pre-exec source table")
        ));
    }
    owner.static_manifest = StaticManifest::from_descriptors(
        original.parent_pid(),
        original.parent_start_time() + 1,
        *original.executable(),
        original.descriptors(),
    )
    .unwrap();
    assert!(matches!(
        refused(owner, supervisor, budget),
        Error::ParentChanged
    ));
    owner.static_manifest = original;

    let (substitute, object) = image::create(owner.static_manifest()).unwrap();
    assert!(!checks::same_object(&object, &owner.manifest_object));
    let original = std::mem::replace(&mut owner.static_manifest_file, substitute);
    assert!(matches!(
        refused(owner, supervisor, budget),
        Error::InvalidDescriptor { .. }
    ));
    owner.static_manifest_file = original;

    for index in [
        POLICY_SOURCE_INDEX,
        SIGNING_KEY_SOURCE_INDEX,
        LAUNCH_MANIFEST_SOURCE_INDEX,
    ] {
        let original = &owner.sources[index];
        let mut bytes = vec![0; original.metadata().unwrap().len() as usize];
        assert_eq!(
            rustix::io::pread(original, &mut bytes[..], 0).unwrap(),
            bytes.len()
        );
        let substitute = raw_image(
            &bytes,
            Mode::RUSR,
            REQUIRED_MANIFEST_SEALS_V1,
            OFlags::RDONLY,
        );
        bytes.fill(0);
        assert!(!checks::same_object(
            &checks::object_identity(&substitute, "substitute").unwrap(),
            &checks::object_identity(original, "original").unwrap()
        ));
        let original = std::mem::replace(&mut owner.sources[index], substitute);
        assert!(matches!(
            refused(owner, supervisor, budget),
            Error::Supervisor(_) | Error::Capability(_)
        ));
        owner.sources[index] = original;
    }
    owner.revalidate(supervisor, budget).unwrap();
}
