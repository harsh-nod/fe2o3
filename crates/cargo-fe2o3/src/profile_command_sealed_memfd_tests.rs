#[test]
fn sealed_image_verification_rejects_wrong_digest_and_length() {
    let bytes = b"profile-image";
    let digest = Sha256::digest(bytes).into();
    let length = bytes.len() as u64;
    let mut image = SealedImage::from_bytes(bytes, false, "test").unwrap();
    assert_eq!(
        image.seals,
        fe2o3_process_identity::EXACT_IMMUTABLE_MEMFD_SEALS_V1
    );
    SealedImage::verify_contents(&mut image.file, digest, length, "test").unwrap();
    for (expected_digest, expected_length) in [
        ([0_u8; 32], length),
        (digest, length - 1),
        (digest, length + 1),
    ] {
        let error =
            SealedImage::verify_contents(&mut image.file, expected_digest, expected_length, "test")
                .unwrap_err();
        assert!(
            error.contains("content does not match its source"),
            "{error}"
        );
    }
    image.validate("test").unwrap();
}

#[test]
fn image_finish_rejects_incorrect_expected_content_before_sealing() {
    for (digest, length) in [([0_u8; 32], 3), (Sha256::digest(b"abc").into(), 4)] {
        let mut file = SealedImage::writable("test").unwrap();
        file.write_all(b"abc").unwrap();
        let retained = file.try_clone().unwrap();
        let error = SealedImage::finish(file, false, digest, length, "test")
            .err()
            .unwrap();
        assert!(
            error.contains("content does not match its source"),
            "{error}"
        );
        assert!(rustix::fs::fcntl_get_seals(&retained).unwrap().is_empty());
    }
}
