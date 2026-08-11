/// Prefix of the canonical worker-v2 load-envelope artifact name.
pub const WORKER_V2_LOAD_ENVELOPE_NAME_PREFIX_V1: &str = ".fe2o3-worker-v2-load-envelope-v1-";

/// Suffix of the canonical worker-v2 load-envelope artifact name.
pub const WORKER_V2_LOAD_ENVELOPE_NAME_SUFFIX_V1: &str = ".envelope";

/// Derives the canonical direct-dirent name for a publication's load envelope.
pub fn worker_v2_load_envelope_name_v1(publication_identity: [u8; 32]) -> String {
    let mut encoded = String::with_capacity(
        WORKER_V2_LOAD_ENVELOPE_NAME_PREFIX_V1.len()
            + publication_identity.len() * 2
            + WORKER_V2_LOAD_ENVELOPE_NAME_SUFFIX_V1.len(),
    );
    encoded.push_str(WORKER_V2_LOAD_ENVELOPE_NAME_PREFIX_V1);
    for byte in publication_identity {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded.push_str(WORKER_V2_LOAD_ENVELOPE_NAME_SUFFIX_V1);
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_name_is_fixed_width_lowercase_hex() {
        let mut identity = [0_u8; 32];
        identity[0] = 0x0a;
        identity[31] = 0xff;
        let name = worker_v2_load_envelope_name_v1(identity);

        assert_eq!(
            name,
            ".fe2o3-worker-v2-load-envelope-v1-0a000000000000000000000000000000000000000000000000000000000000ff.envelope"
        );
    }
}
