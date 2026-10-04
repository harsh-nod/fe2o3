use super::*;

#[test]
fn conditional_protocol_v3_every_coherent_subject_axis_changes_the_signed_expectation() {
    let p = policy();
    let original = receipt_wire(3);
    let axes = [24, 32, 48, 88, 120]
        .into_iter()
        .chain((0..6).map(|i| 152 + i * 32))
        .chain((0..7).flat_map(|i| [378 + i * 40, 410 + i * 40]));
    for offset in axes {
        let mut subject = subject_wire(3);
        subject[offset] ^= 1;
        if (152..344).contains(&offset) {
            let mut hash = Sha256::new();
            hash.update(b"fe2o3-compiler-closure-identity-v2\0");
            hash.update(&subject[344..346]);
            hash.update(&subject[152..344]);
            subject[346..378].copy_from_slice(&hash.finalize());
        }
        seal(&mut subject, "INERT-COMPILER-EXECUTION-SUBJECT", 3);
        let mut q = request_wire(3);
        q[224..914].copy_from_slice(&subject);
        seal(&mut q, "COMPILER-EXECUTION-REQUEST", 3);
        assert_framing(request(&q).unwrap_err(), Framing::SubjectMismatch);
        // A coherent alternate request is valid content but cannot verify the
        // original signature's subject; unrelated subject axes stay unchanged.
        q[80..112].copy_from_slice(&subject[658..690]);
        seal(&mut q[24..224], "COMPILER-EXECUTION-CHALLENGE", 3);
        seal(&mut q, "COMPILER-EXECUTION-REQUEST", 3);
        let alternate = request(&q).unwrap();
        assert_ne!(alternate.identity().as_bytes(), &request_wire(3)[914..]);
        assert_framing(
            verify(&original, &p, &alternate, [0; 32]).unwrap_err(),
            Framing::SubjectMismatch,
        );
    }
}

#[test]
fn conditional_protocol_v3_pinning_is_external_to_signed_transport() {
    let p = policy();
    let q = request(&request_wire(3)).unwrap();
    let mut bytes = receipt_wire(3);
    let other_key = SigningKey::from_bytes(&[0x53; 32]);
    bytes[272..304].copy_from_slice(&other_key.verifying_key().to_bytes());
    sign(&mut bytes, 3, &other_key);
    // Strict self-signature decoding must not implicitly accept its key as policy.
    receipt(&bytes).unwrap();
    assert_framing(
        verify(&bytes, &p, &q, [0; 32]).unwrap_err(),
        Framing::PolicyMismatch,
    );
    for offset in [24, 32, 64, 72, 104] {
        let mut wire = policy_wire(3);
        wire[offset] ^= 1;
        seal(&mut wire, "COMPILER-EXECUTION-ISSUER-POLICY", 3);
        let changed = run(216, |b| Policy::decode(&wire, b)).unwrap().0;
        assert_framing(
            verify(&receipt_wire(3), &changed, &q, [0; 32]).unwrap_err(),
            Framing::PolicyMismatch,
        );
    }
    let mut wire = policy_wire(3);
    wire[144..176].copy_from_slice(&other_key.verifying_key().to_bytes());
    seal(&mut wire, "COMPILER-EXECUTION-ISSUER-POLICY", 3);
    let changed = run(216, |b| Policy::decode(&wire, b)).unwrap().0;
    assert_framing(
        verify(&receipt_wire(3), &changed, &q, [0; 32]).unwrap_err(),
        Framing::PolicyMismatch,
    );
    assert_framing(
        verify(&receipt_wire(3), &p, &q, [7; 32]).unwrap_err(),
        Framing::RollbackAnchorMismatch,
    );
}

#[test]
fn conditional_protocol_v3_domains_and_policy_subject_version_are_not_relabelable() {
    for version in [1, 2] {
        let mut bytes = receipt_wire(3);
        sign(&mut bytes, version, &SigningKey::from_bytes(&[0x51; 32]));
        seal(&mut bytes, "COMPILER-EXECUTION-RECEIPT", 3);
        assert_framing(receipt(&bytes).unwrap_err(), Framing::SignatureRejected);
        let mut bytes = receipt_wire(3);
        bytes[240..272].copy_from_slice(&anchor(&receipt_wire(3), version));
        sign(&mut bytes, 3, &SigningKey::from_bytes(&[0x51; 32]));
        assert_framing(
            receipt(&bytes).unwrap_err(),
            Framing::RollbackTransitionMismatch,
        );
        let mut bytes = receipt_wire(3);
        seal(&mut bytes, "COMPILER-EXECUTION-RECEIPT", version);
        assert_framing(
            receipt(&bytes).unwrap_err(),
            Framing::IdentityMismatch("receipt"),
        );
        let mut bytes = policy_wire(3);
        bytes[176..178].copy_from_slice(&version.to_le_bytes());
        seal(&mut bytes, "COMPILER-EXECUTION-ISSUER-POLICY", 3);
        assert_framing(
            run(216, |b| Policy::decode(&bytes, b)).unwrap_err(),
            Framing::UnsupportedSubjectVersion(version),
        );
    }
}

#[test]
fn conditional_protocol_v3_paired_failures_preserve_validation_precedence() {
    let p = policy();
    let q = request(&request_wire(3)).unwrap();
    for (first, second, expected) in [
        (64, 96, Framing::PolicyMismatch),
        (96, 200, Framing::SubjectMismatch),
        (200, 136, Framing::SequenceMismatch),
        (136, 24, Framing::ChallengeMismatch),
    ] {
        let mut bytes = receipt_wire(3);
        bytes[first] ^= 2;
        bytes[second] ^= 2;
        if bytes[200] != 1 {
            bytes[208..240].fill(0x77);
        }
        rebuild(&mut bytes, 3);
        assert_framing(verify(&bytes, &p, &q, [0; 32]).unwrap_err(), expected);
    }
    let mut bytes = receipt_wire(3);
    bytes[368..].fill(0);
    bytes[304] ^= 1;
    assert_framing(receipt(&bytes).unwrap_err(), Framing::SignatureRejected);
    let mut bytes = policy_wire(3);
    bytes[24..32].fill(0);
    bytes[178] = 1;
    assert_framing(
        run(216, |b| Policy::decode(&bytes, b)).unwrap_err(),
        Framing::NonzeroReserved,
    );
    let mut bytes = request_wire(3);
    bytes[224] ^= 1;
    bytes[914..].fill(0);
    assert!(matches!(request(&bytes), Err(Error::Subject(_))));

    let mut bytes = policy_wire(3);
    bytes[24] ^= 1;
    seal(&mut bytes, "COMPILER-EXECUTION-ISSUER-POLICY", 3);
    let changed = run(216, |b| Policy::decode(&bytes, b)).unwrap().0;
    for seed in [0x51, 0x53] {
        let key = SigningKey::from_bytes(&[seed; 32]);
        let error = run(
            changed.retained_storage() + q.retained_storage() + size_of::<SigningKey>(),
            |b| Receipt::issue(&changed, &q, &key, b),
        )
        .unwrap_err();
        assert_framing(
            error,
            if seed == 0x53 {
                Framing::SigningKeyMismatch
            } else {
                Framing::PolicyMismatch
            },
        );
    }
}
