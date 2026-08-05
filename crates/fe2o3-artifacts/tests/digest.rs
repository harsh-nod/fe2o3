use fe2o3_artifacts::{DigestAlgorithm, DigestBytes, PayloadDigest};

fn hex_32(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64);
    let mut bytes = [0; 32];
    for (index, output) in bytes.iter_mut().enumerate() {
        *output = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap();
    }
    bytes
}

#[test]
fn sha256_matches_published_test_vectors() {
    let cases = [
        (
            b"".as_slice(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            b"abc".as_slice(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
    ];

    for (payload, expected) in cases {
        let digest = DigestAlgorithm::Sha256.calculate(payload);
        assert_eq!(digest.algorithm(), DigestAlgorithm::Sha256);
        assert_eq!(digest.bytes(), DigestBytes::from_bytes(hex_32(expected)));
        assert_eq!(digest.verify(payload), Ok(()));
    }
}

#[test]
fn verification_rejects_modified_payloads() {
    let expected = DigestAlgorithm::Sha256.calculate(b"code object");
    let error = expected.verify(b"code object!").unwrap_err();
    assert_eq!(error.algorithm(), DigestAlgorithm::Sha256);
}

#[test]
fn explicit_digest_values_can_be_verified() {
    let expected = PayloadDigest::new(
        DigestAlgorithm::Sha256,
        DigestBytes::from_bytes(hex_32(
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        )),
    );
    assert_eq!(expected.verify(b"abc"), Ok(()));
}
