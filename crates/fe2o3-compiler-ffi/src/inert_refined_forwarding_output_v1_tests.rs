use super::*;
use std::convert::Infallible;

// Literal framing produced independently from the written field table, not by
// this encoder. Nested one-byte placeholders intentionally grant no authority.
const LITERAL: &str = concat!(
    "463252464f310000010001009000000093020000000000000e000000000000001301000000000000010000000000000001000000000000000100000000000000010000000000000000000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000e500000000000000",
    "463252464831000001000100f800000013010000000000001b00000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000000102030405060708090a0b0c0d0e0f101112131415161718191a",
    "01020304060708090a0b0c0100000007000000010101010101010101010101010101010101010101010101010101010101010100000000020202020202020202020202020202020202020202020202020202020202020200000000030000000000000005000000040404040404040404040404040404040404040404040404040404040404040401014000000001000000010000000004000001000000010000006300000000000000400000000000000001000000000000000100000000000000400000000000000001000000000000000100000000000000400000000000000001010000006b05000000656e747279"
);
fn hex(s: &str) -> Vec<u8> {
    s.as_bytes()
        .chunks_exact(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn paid(_: usize) -> Result<(), Infallible> {
    Ok(())
}
fn read(bytes: &[u8]) -> Result<InertRefinedForwardingOutputRefV1<'_>, Error<Infallible>> {
    read_inert_refined_forwarding_output_v1(bytes, MAX_INERT_REFINED_FORWARDING_STORAGE_V1, paid)
}
fn reframed(fields: [&[u8]; 14], route: u8) -> Vec<u8> {
    let len = 144 + fields.iter().map(|f| f.len()).sum::<usize>();
    let mut b = vec![0; 144];
    b[..16].copy_from_slice(b"F2RFO1\0\0\x01\0\x01\0\x90\0\0\0");
    b[16..24].copy_from_slice(&(len as u64).to_le_bytes());
    b[24] = 14;
    b[28] = route;
    for (i, field) in fields.iter().enumerate() {
        b[32 + 8 * i..40 + 8 * i].copy_from_slice(&(field.len() as u64).to_le_bytes());
        b.extend_from_slice(field);
    }
    b
}

#[test]
fn inert_output_literal_all_fields_root_and_domain_separated_identities() {
    let bytes = hex(LITERAL);
    assert_eq!(bytes.len(), 659);
    let frame = read(&bytes).unwrap();
    assert_eq!(frame.route(), InertRefinedForwardingRouteV1::Direct);
    assert_eq!(frame.root_count(), 1);
    assert!(!frame.grants_authority());
    let root = frame.root(0, 1 << 28, paid).unwrap();
    assert_eq!(
        (
            root.semantic_root,
            root.original_function,
            root.final_function
        ),
        (7, 3, 5)
    );
    assert_eq!(root.semantic_function_identity, [1; 32]);
    assert_eq!(root.descriptor_kernel_id, [2; 32]);
    assert_eq!(root.source_kernel_binding, [4; 32]);
    assert_eq!(
        (root.source_rank, root.exact_workgroup, root.source_max_grid),
        (1, Some([64, 1, 1]), [1024, 1, 1])
    );
    assert_eq!(
        (
            root.grid_identity,
            root.global_extents,
            root.workgroup_extents,
            root.subgroup_size
        ),
        (99, [64, 1, 1], [64, 1, 1], 64)
    );
    assert!(root.full_physical_workgroups);
    assert_eq!((root.logical_name, root.export_name), ("k", "entry"));
    let id = inert_refined_forwarding_output_identity_v1(&frame, 1 << 28, paid).unwrap();
    assert_eq!(
        id.sha256.as_slice(),
        hex("c95005757a65c45e6bf31ed4b945590486fbdf7d4356a703f6cccfa3af7de097")
    );
    assert_eq!(id.byte_len, 659);
    let history =
        inert_refined_forwarding_history_identity_v1(frame.fields[0], 1 << 28, paid).unwrap();
    assert_eq!(
        history.sha256.as_slice(),
        hex("0e21a611cc6964623a0a49f6cb5d4346b3746f45ec507082ec238c4769943d6f")
    );
    assert_eq!(history.byte_len, 275);
    let mut out = vec![0xa5; bytes.len()];
    encode_inert_refined_forwarding_output_into_v1(
        frame.fields,
        frame.route,
        &mut out,
        1 << 28,
        paid,
    )
    .unwrap();
    assert_eq!(out, bytes);
}

#[test]
fn inert_output_rejects_all_truncations_foreign_headers_lengths_and_reserved_bytes() {
    let bytes = hex(LITERAL);
    for len in 0..bytes.len() {
        assert!(read(&bytes[..len]).is_err(), "truncation {len}");
    }
    for at in 0..32 {
        let mut bad = bytes.clone();
        bad[at] ^= 0x80;
        assert!(read(&bad).is_err(), "header byte {at}");
    }
    for field in 0..14 {
        let mut bad = bytes.clone();
        bad[32 + 8 * field..40 + 8 * field].fill(255);
        assert!(read(&bad).is_err(), "length {field}");
    }
    let mut padded = bytes.clone();
    padded.push(0);
    assert!(read(&padded).is_err());
}

#[test]
fn inert_output_rejects_absent_fields_and_direct_erased_route_confusion() {
    let bytes = hex(LITERAL);
    let f = read(&bytes).unwrap();
    for i in (0..14).filter(|i| *i != 5) {
        let mut fields = f.fields;
        fields[i] = &[];
        assert!(read(&reframed(fields, 0)).is_err(), "missing {i}");
    }
    assert!(matches!(read(&reframed(f.fields, 1)), Err(Error::Route)));
    let mut fields = f.fields;
    fields[5] = b"erased graph claim";
    assert!(matches!(read(&reframed(fields, 0)), Err(Error::Route)));
    assert_eq!(
        read(&reframed(fields, 1)).unwrap().route(),
        InertRefinedForwardingRouteV1::Erased
    );
}

#[test]
fn inert_output_root_order_and_all_three_permutations_are_independent() {
    let bytes = hex(LITERAL);
    let f = read(&bytes).unwrap();
    let one = &f.fields[13][4..];
    let mut roots = 2u32.to_le_bytes().to_vec();
    roots.extend(one);
    roots.extend(one);
    let second = 4 + one.len();
    roots[second..second + 4].copy_from_slice(&9u32.to_le_bytes());
    for at in [36, 72, 80] {
        roots[second + at..second + at + 4].copy_from_slice(&1u32.to_le_bytes());
    }
    let mut fields = f.fields;
    fields[13] = &roots;
    assert_eq!(read(&reframed(fields, 0)).unwrap().root_count(), 2);
    for at in [36, 72, 80] {
        let mut duplicate = roots.clone();
        duplicate[second + at..second + at + 4].fill(0);
        let mut fields = f.fields;
        fields[13] = &duplicate;
        assert!(matches!(
            read(&reframed(fields, 0)),
            Err(Error::RootPermutation)
        ));
        duplicate[second + at..second + at + 4].copy_from_slice(&2u32.to_le_bytes());
        let mut fields = f.fields;
        fields[13] = &duplicate;
        assert!(matches!(
            read(&reframed(fields, 0)),
            Err(Error::RootPermutation)
        ));
    }
    roots[second..second + 4].copy_from_slice(&7u32.to_le_bytes());
    let mut fields = f.fields;
    fields[13] = &roots;
    assert!(matches!(read(&reframed(fields, 0)), Err(Error::RootOrder)));
}

#[test]
fn inert_output_root_closed_tags_count_names_and_full_consumption() {
    let bytes = hex(LITERAL);
    let f = read(&bytes).unwrap();
    for (offset, value) in [
        (0, 0),
        (0, 129),
        (124, 0),
        (124, 4),
        (125, 2),
        (214, 2),
        (219, 0xff),
        (219, 0),
    ] {
        let mut root = f.fields[13].to_vec();
        root[offset] = value;
        let mut fields = f.fields;
        fields[13] = &root;
        assert!(
            read(&reframed(fields, 0)).is_err(),
            "root offset {offset} value {value}"
        );
    }
    let mut root = f.fields[13].to_vec();
    root.push(0);
    let mut fields = f.fields;
    fields[13] = &root;
    assert!(matches!(read(&reframed(fields, 0)), Err(Error::Length)));
}

#[test]
fn inert_output_exact_root_cap_and_absent_workgroup_are_canonical() {
    let bytes = hex(LITERAL);
    let f = read(&bytes).unwrap();
    let one = &f.fields[13][4..];
    let mut roots = 128u32.to_le_bytes().to_vec();
    for i in 0u32..128 {
        let mut row = one.to_vec();
        row[..4].copy_from_slice(&(7 + i * 3).to_le_bytes());
        for (at, ordinal) in [(36, i), (72, 127 - i), (80, (i + 1) % 128)] {
            row[at..at + 4].copy_from_slice(&ordinal.to_le_bytes());
        }
        roots.extend(row);
    }
    let mut fields = f.fields;
    fields[13] = &roots;
    let encoded = reframed(fields, 0);
    let frame = read(&encoded).unwrap();
    assert_eq!(frame.root_count(), 128);
    assert_eq!(frame.root(127, 1 << 28, paid).unwrap().semantic_root, 388);
    let mut no_workgroup = f.fields[13].to_vec();
    no_workgroup[125] = 0;
    no_workgroup.drain(126..138);
    let mut fields = f.fields;
    fields[13] = &no_workgroup;
    let encoded = reframed(fields, 0);
    assert_eq!(
        read(&encoded)
            .unwrap()
            .root(0, 1 << 28, paid)
            .unwrap()
            .exact_workgroup,
        None
    );
}

#[test]
fn inert_output_callback_denials_precede_all_output_mutation() {
    let bytes = hex(LITERAL);
    let frame = read(&bytes).unwrap();
    for denied_call in 0..3 {
        let mut out = vec![0xa5; bytes.len()];
        let mut calls = 0;
        let result = encode_inert_refined_forwarding_output_into_v1(
            frame.fields,
            frame.route,
            &mut out,
            1 << 28,
            |work| {
                let at = calls;
                calls += 1;
                if at == denied_call {
                    Err((at, work))
                } else {
                    Ok(())
                }
            },
        );
        assert!(matches!(result, Err(Error::Charge((at, n))) if at == denied_call && n > 0));
        assert!(out.iter().all(|v| *v == 0xa5));
        assert_eq!(calls, denied_call + 1);
    }
    let mut out = vec![0xa5; bytes.len()];
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), Error<Infallible>> = encode_inert_refined_forwarding_output_into_v1(
            frame.fields,
            frame.route,
            &mut out,
            1 << 28,
            |_| panic!("caller work denial panic"),
        );
    }));
    assert!(panic.is_err());
    assert!(out.iter().all(|v| *v == 0xa5));
}

#[test]
fn inert_output_fixed_caps_are_checked_before_payload_or_callback() {
    let mut calls = 0;
    assert!(matches!(
        read_inert_refined_forwarding_output_v1(&[], (1 << 28) + 1, |_| {
            calls += 1;
            Ok::<_, ()>(())
        }),
        Err(Error::StorageLimit)
    ));
    assert_eq!(calls, 0);
    let too_large = vec![0; MAX_INERT_REFINED_FORWARDING_OUTPUT_BYTES_V1 + 1];
    assert!(matches!(
        read_inert_refined_forwarding_output_v1(&too_large, 1 << 28, |_| {
            calls += 1;
            Ok::<_, ()>(())
        }),
        Err(Error::Length)
    ));
    assert_eq!(calls, 0);
    let bytes = hex(LITERAL);
    let frame = read(&bytes).unwrap();
    let descriptor = vec![0; (256 << 10) + 1];
    let mut fields = frame.fields;
    fields[2] = &descriptor;
    assert!(matches!(read(&reframed(fields, 0)), Err(Error::Field(2))));
    let mut out = vec![0xa5; bytes.len() + 1];
    assert!(matches!(
        encode_inert_refined_forwarding_output_into_v1(
            frame.fields,
            frame.route,
            &mut out,
            1 << 28,
            paid
        ),
        Err(Error::Length)
    ));
    assert!(out.iter().all(|v| *v == 0xa5));
}

#[test]
fn inert_history_identity_is_framing_only_and_rejects_foreign_schema() {
    let bytes = hex(LITERAL);
    let frame = read(&bytes).unwrap();
    let history = frame.fields[0];
    for at in 0..32 {
        let mut bad = history.to_vec();
        bad[at] ^= 0x80;
        assert!(inert_refined_forwarding_history_identity_v1(&bad, 1 << 28, paid).is_err());
    }
    for field in 0..27 {
        let mut bad = history.to_vec();
        bad[32 + field * 8..40 + field * 8].fill(255);
        assert!(inert_refined_forwarding_history_identity_v1(&bad, 1 << 28, paid).is_err());
    }
    assert!(matches!(
        inert_refined_forwarding_history_identity_v1(history, 1 << 28, |_| Err("same denial")),
        Err(Error::Charge("same denial"))
    ));
}
