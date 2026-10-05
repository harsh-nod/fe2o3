use super::*;

fn header() -> Header {
    Header {
        name: 0,
        name_length: 0,
        vectors: 0x4000,
        vector_count: 1,
        control: 0x2000,
        control_length: 32,
        flags: 0,
    }
}

fn vector(address: u64, length: u64) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&address.to_ne_bytes());
    bytes[8..].copy_from_slice(&length.to_ne_bytes());
    bytes
}

#[test]
fn credential_receive_rejects_all_payload_header_control_aliases() {
    let header = header();
    assert!(header.credential_layout(0x1000, &vector(0x3000, 1024)));
    for address in [0xfff, 0x1000, 0x1037, 0x1fff, 0x2000, 0x201f] {
        assert!(!header.credential_layout(0x1000, &vector(address, 2)));
    }
    for control in [0xfff, 0x1000, 0x1037] {
        assert!(!Header { control, ..header }.credential_layout(0x1000, &vector(0x3000, 1)));
    }
    assert!(header.credential_layout(0x1000, &vector(0x1038, 1)));
    assert!(header.credential_layout(0x1000, &vector(0x2020, 1)));
}

#[test]
fn credential_receive_refuses_name_writes_and_unbounded_or_wrapped_ranges() {
    let header = header();
    for bad in [
        Header {
            name: 0x1000,
            ..header
        },
        Header {
            name_length: 1,
            ..header
        },
        Header {
            control_length: 31,
            ..header
        },
        Header {
            control_length: 33,
            ..header
        },
        Header {
            control: 0,
            ..header
        },
        Header {
            control: u64::MAX - 1,
            ..header
        },
        Header {
            vector_count: 65,
            ..header
        },
    ] {
        assert!(!bad.credential_layout(0x1000, &vector(0x3000, 1)));
    }
    assert!(!header.credential_layout(u64::MAX - 1, &vector(0x3000, 1)));
    assert!(!header.credential_layout(0x1000, &vector(u64::MAX - 1, 2)));
    assert!(!header.credential_layout(0x1000, &vector(0x3000, MAX_PAYLOAD + 1)));
    assert!(!header.credential_layout(0x1000, &[0; 15]));
    assert!(header.credential_layout(0x1000, &vector(0, 0)));
}

#[test]
fn credential_receive_checks_every_vector_and_total_length() {
    let header = Header {
        vector_count: 64,
        ..header()
    };
    let mut vectors = [0; MAX_IOVECS * 16];
    for chunk in vectors.chunks_exact_mut(16) {
        chunk.copy_from_slice(&vector(0x8000, MAX_PAYLOAD / 64));
    }
    assert!(header.credential_layout(0x1000, &vectors));
    vectors[63 * 16..].copy_from_slice(&vector(0x8000, MAX_PAYLOAD / 64 + 1));
    assert!(!header.credential_layout(0x1000, &vectors));
    vectors[63 * 16..].copy_from_slice(&vector(0x2000, 1));
    assert!(!header.credential_layout(0x1000, &vectors));
}

#[test]
fn native_control_parser_accepts_only_one_complete_credential_record() {
    let header = header();
    let mut control = [0; CONTROL_BYTES];
    control[..8].copy_from_slice(&28_u64.to_ne_bytes());
    control[8..12].copy_from_slice(&1_u32.to_ne_bytes());
    control[12..16].copy_from_slice(&2_u32.to_ne_bytes());
    assert!(credential_control(header, &control));
    assert!(credential_control(
        Header {
            control_length: 28,
            ..header
        },
        &control
    ));
    for offset in [0, 8, 12] {
        let mut bad = control;
        bad[offset] ^= 1;
        assert!(!credential_control(header, &bad));
    }
    for flags in [0x08, 0x20, 0x28] {
        assert!(!credential_control(Header { flags, ..header }, &control));
    }
    for control_length in [0, 16, 27, 29, 31, 33, u64::MAX] {
        assert!(!credential_control(
            Header {
                control_length,
                ..header
            },
            &control
        ));
    }
}

#[test]
fn native_header_decode_and_stable_field_comparison_are_exact() {
    let mut bytes = [0; HEADER_BYTES];
    bytes[16..24].copy_from_slice(&0x4000_u64.to_ne_bytes());
    bytes[24..32].copy_from_slice(&1_u64.to_ne_bytes());
    bytes[32..40].copy_from_slice(&0x2000_u64.to_ne_bytes());
    bytes[40..48].copy_from_slice(&32_u64.to_ne_bytes());
    let header = Header::decode(&bytes);
    assert_eq!(header, self::header());
    assert!(header.stable_fields(Header {
        flags: 8,
        control_length: 28,
        ..header
    }));
    for offset in [0, 16, 24, 32] {
        let mut bad = bytes;
        bad[offset] ^= 1;
        assert!(!header.stable_fields(Header::decode(&bad)));
    }
}
