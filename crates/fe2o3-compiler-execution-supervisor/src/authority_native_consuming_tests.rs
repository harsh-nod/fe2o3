/// Separate real images; the synthetic preparation fixture remains inert.
pub(crate) fn bound_consuming_fixture(
    launcher: crate::native_consuming_test_process::MeasuredImage,
    issuer: crate::native_consuming_test_process::MeasuredImage,
    root: &std::path::Path,
    peer: &OwnedFd,
    pidfd: &OwnedFd,
    budget: &mut Budget<'_>,
) -> Supervisor {
    let policy = measured_policy(issuer.issuer_measurement(), 7, budget);
    let (cap, delta) = Cap::create(policy, budget).unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    for image in [&launcher, &issuer] {
        budget
            .reserve_storage(Image::file_storage(image.measurement).unwrap())
            .unwrap();
    }
    let provisioned = crate::ProvisionedStaticExecutableMeasurementV1::new(
        launcher.measurement.sha256(),
        launcher.measurement.byte_len(),
    )
    .unwrap();
    let (program, delta) =
        Program::provision(launcher.file, provisioned, issuer.file, cap, budget).unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    budget.reserve_storage(SEED.len()).unwrap();
    let mut seed = SEED;
    let (key, delta) = Key::create_and_zeroize(&mut seed, program.policy(), budget).unwrap();
    assert_eq!(seed, [0; 32]);
    budget.reserve_storage(delta.additional_storage()).unwrap();
    budget.release_storage(SEED.len()).unwrap();
    budget.reserve_storage(Anchor::PAIR_STORAGE).unwrap();
    let (anchor, delta) = Anchor::admit(
        rustix::io::fcntl_dupfd_cloexec(peer, 3).unwrap(),
        rustix::io::fcntl_dupfd_cloexec(pidfd, 3).unwrap(),
        AnchorIdentity::new(65_534, 65_534).unwrap(),
        budget,
    )
    .unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    budget
        .reserve_storage(Supervisor::ROOT_FILE_STORAGE)
        .unwrap();
    let (supervisor, delta) = Supervisor::bind(
        program,
        credentials(),
        File::open(root).unwrap(),
        key,
        anchor,
        budget,
    )
    .unwrap();
    budget.reserve_storage(delta.additional_storage()).unwrap();
    supervisor
}
