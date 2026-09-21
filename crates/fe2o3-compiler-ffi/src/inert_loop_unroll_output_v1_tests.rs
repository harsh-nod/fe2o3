use super::*;
use std::convert::Infallible;

// Independent fixed-field literal: invalid one-byte graph placeholders are
// intentional. FFI validates framing, never typed graphs or semantic history.
const HISTORY_HEX: &str = concat!(
    "46324c5548310000010001006800000014020000000000000900000000000000130100000000000001000000000000000800000000000000080000000000000008000000000000000800000000000000080000000000000008000000000000006800000000000000",
    "463252464831000001000100f800000013010000000000001b00000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000010000000000000001000000000000000100000000000000000102030405060708090a0b0c0d0e0f101112131415161718191a",
    "2a00000000000100000000000001010000000000000201000000000000030100000000000004010000000000000501000001000000000000000200000000000000030000000000000004000000000000000500000000000000060000000000000007000000000000000800000000000000080000000000000009000000000000000a0000000000000000000000000000000000000000000000"
);
const ROOT_HEX: &str = "0100000007000000010101010101010101010101010101010101010101010101010101010101010100000000020202020202020202020202020202020202020202020202020202020202020200000000030000000000000005000000040404040404040404040404040404040404040404040404040404040404040401014000000001000000010000000004000001000000010000006300000000000000400000000000000001000000000000000100000000000000400000000000000001000000000000000100000000000000400000000000000001010000006b05000000656e747279";
fn hex(value: &str) -> Vec<u8> {
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap())
        .collect()
}
fn paid(_: usize) -> Result<(), Infallible> {
    Ok(())
}
fn read(bytes: &[u8]) -> Result<InertLoopUnrollOutputRefV1<'_>, Error<Infallible>> {
    read_inert_loop_unroll_output_v1(bytes, 1 << 28, paid)
}
fn raw<const N: usize>(magic: &[u8; 8], fields: [&[u8]; N], route: u8) -> Vec<u8> {
    let header = 32 + 8 * N;
    let total = header + fields.iter().map(|v| v.len()).sum::<usize>();
    let mut out = vec![0; header];
    out[..8].copy_from_slice(magic);
    out[8..12].copy_from_slice(&[1, 0, 1, 0]);
    out[12..16].copy_from_slice(&(header as u32).to_le_bytes());
    out[16..24].copy_from_slice(&(total as u64).to_le_bytes());
    out[24..26].copy_from_slice(&(N as u16).to_le_bytes());
    out[28] = route;
    for (i, field) in fields.iter().enumerate() {
        out[32 + 8 * i..40 + 8 * i].copy_from_slice(&(field.len() as u64).to_le_bytes());
        out.extend_from_slice(field);
    }
    out
}
fn fields<'a>(history: &'a [u8], roots: &'a [u8]) -> [&'a [u8]; 14] {
    [
        history,
        &[1],
        &[2],
        &[3],
        &[4],
        &[],
        &[6],
        &[7],
        &[8],
        &[9],
        &[10],
        &[11],
        &[12],
        roots,
    ]
}
fn literal() -> Vec<u8> {
    raw(
        &INERT_LOOP_UNROLL_OUTPUT_MAGIC_V1,
        fields(&hex(HISTORY_HEX), &hex(ROOT_HEX)),
        0,
    )
}

#[test]
fn unroll_output_literal_framing_and_identity_domains_are_exact() {
    let bytes = literal();
    let view = read(&bytes).unwrap();
    assert_eq!(bytes.len(), 916);
    assert_eq!(
        &bytes[..32],
        hex("46324c554f310000010001009000000094030000000000000e00000000000000")
    );
    assert_eq!(
        view.field(InertLoopUnrollOutputFieldV1::History),
        hex(HISTORY_HEX)
    );
    assert!(!view.grants_authority());
    let output_id = inert_loop_unroll_output_identity_v1(&view, 1 << 28, paid).unwrap();
    assert_eq!(output_id.byte_len, 916);
    assert_eq!(
        output_id.sha256.as_slice(),
        hex("9c46318ea1654aa77009f241b071df864370a818d5e4eb7d778ce1ebe99d9db1")
    );
    let history_id = inert_loop_unroll_history_identity_v1(view.fields[0], 1 << 28, paid).unwrap();
    assert_eq!(history_id.byte_len, 532);
    assert_eq!(
        history_id.sha256.as_slice(),
        hex("bc81e652e638833f6b6a38f11119b1ccffc6efe0df5eb1c8fcd3dbfb4647d0bd")
    );
    let mut out = vec![0xa5; bytes.len()];
    encode_inert_loop_unroll_output_into_v1(view.fields, view.route(), &mut out, 1 << 28, paid)
        .unwrap();
    assert_eq!(out, bytes);
}
#[test]
fn unroll_output_cross_family_and_nested_header_mutations_refuse() {
    let bytes = literal();
    for len in 0..bytes.len() {
        assert!(read(&bytes[..len]).is_err(), "truncation {len}");
    }
    for start in [0, 144, 248] {
        for i in 0..32 {
            let mut bad = bytes.clone();
            bad[start + i] ^= 0x80;
            assert!(read(&bad).is_err(), "header {start}/{i}");
        }
    }
    assert!(previous::read_inert_refined_forwarding_output_v1(&bytes, 1 << 28, paid).is_err());
    let mut bad = bytes.clone();
    bad[..8].copy_from_slice(&previous::INERT_REFINED_FORWARDING_OUTPUT_MAGIC_V1);
    assert!(matches!(read(&bad), Err(Error::Header)));
    for at in [
        144 + 380 + 4,
        144 + 428 + 56,
        144 + 428 + 57,
        144 + 428 + 88,
        144 + 428 + 90,
    ] {
        let mut bad = bytes.clone();
        bad[at] = 0xff;
        assert!(matches!(read(&bad), Err(Error::Field(0))));
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(read(&extra).is_err());
}
#[test]
fn unroll_output_exact_fields_routes_and_aggregate_byte_caps() {
    let bytes = literal();
    let view = read(&bytes).unwrap();
    for i in 0..14 {
        if i == 5 {
            continue;
        }
        let mut fields = view.fields;
        fields[i] = &[];
        assert!(read(&raw(&INERT_LOOP_UNROLL_OUTPUT_MAGIC_V1, fields, 0)).is_err());
    }
    let mut fields = view.fields;
    fields[5] = &[5];
    assert!(matches!(
        read(&raw(&INERT_LOOP_UNROLL_OUTPUT_MAGIC_V1, fields, 0)),
        Err(Error::Route)
    ));
    assert_eq!(
        read(&raw(&INERT_LOOP_UNROLL_OUTPUT_MAGIC_V1, fields, 1))
            .unwrap()
            .route(),
        InertLoopUnrollRouteV1::Erased
    );
    assert!(matches!(
        read(&raw(&INERT_LOOP_UNROLL_OUTPUT_MAGIC_V1, view.fields, 1)),
        Err(Error::Route)
    ));
    assert!(matches!(
        caps::<Infallible>((64 << 20) + 1, 64 << 20, 1 << 28),
        Err(Error::Length)
    ));
    assert!(matches!(
        caps::<Infallible>(0, 64 << 20, (1 << 28) + 1),
        Err(Error::StorageLimit)
    ));
    for i in [2, 4, 13] {
        let huge = vec![1; FIELD_LIMITS[i] + 1];
        let mut f = view.fields;
        f[i] = &huge;
        assert!(
            matches!(read(&raw(&INERT_LOOP_UNROLL_OUTPUT_MAGIC_V1, f, 0)), Err(Error::Field(j)) if j == i)
        );
    }
    let prefix =
        previous::fields::<Infallible, 9>(view.fields[0], &INERT_LOOP_UNROLL_HISTORY_MAGIC_V1, 0)
            .unwrap();
    let graph = vec![1; 12 << 20];
    let mut changed = prefix;
    changed[1] = &graph;
    assert!(matches!(
        history::<Infallible>(&raw(&INERT_LOOP_UNROLL_HISTORY_MAGIC_V1, changed, 0)),
        Err(Error::Field(0))
    ));
}
#[test]
fn unroll_output_root_axes_order_names_and_permutations_are_exact() {
    let bytes = literal();
    let view = read(&bytes).unwrap();
    let original = &view.fields[13][4..];
    let mut roots = 2u32.to_le_bytes().to_vec();
    for (semantic, descriptor, native, final_u) in [(7u32, 1u32, 0u32, 1u32), (19, 0, 1, 0)] {
        let mut row = original.to_vec();
        for (at, value) in [(0, semantic), (36, descriptor), (72, native), (80, final_u)] {
            row[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        roots.extend(row);
    }
    let mut fields = view.fields;
    fields[13] = &roots;
    let wire = raw(&INERT_LOOP_UNROLL_OUTPUT_MAGIC_V1, fields, 0);
    let v = read(&wire).unwrap();
    for (i, semantic, descriptor, native, final_u) in [(0, 7, 1, 0, 1), (1, 19, 0, 1, 0)] {
        let root = v.root(i, 1 << 28, paid).unwrap();
        assert_eq!(
            (
                root.semantic_root,
                root.descriptor_ordinal,
                root.original_kernel_ordinal,
                root.final_kernel_ordinal
            ),
            (semantic, descriptor, native, final_u)
        );
    }
    for at in [36, 72, 80] {
        let mut bad = roots.clone();
        let second = 4 + original.len() + at;
        let first = bad[4 + at..8 + at].to_vec();
        bad[second..second + 4].copy_from_slice(&first);
        let mut fields = view.fields;
        fields[13] = &bad;
        assert!(matches!(
            read(&raw(&INERT_LOOP_UNROLL_OUTPUT_MAGIC_V1, fields, 0)),
            Err(Error::RootPermutation)
        ));
    }
    let mut bad = roots.clone();
    bad[4 + original.len()..8 + original.len()].copy_from_slice(&7u32.to_le_bytes());
    let mut changed = view.fields;
    changed[13] = &bad;
    assert!(matches!(
        read(&raw(&INERT_LOOP_UNROLL_OUTPUT_MAGIC_V1, changed, 0)),
        Err(Error::RootOrder)
    ));
    for value in [0, 255] {
        let mut bad = view.fields[13].to_vec();
        bad[4 + 215] = value;
        let mut changed = view.fields;
        changed[13] = &bad;
        assert!(matches!(
            read(&raw(&INERT_LOOP_UNROLL_OUTPUT_MAGIC_V1, changed, 0)),
            Err(Error::RootName)
        ));
    }
    for (at, expected) in [(4 + 121, "bool"), (4 + 214 + 4, "name")] {
        let mut bad = view.fields[13].to_vec();
        bad[at] = 0xff;
        let mut f = view.fields;
        f[13] = &bad;
        assert!(
            read(&raw(&INERT_LOOP_UNROLL_OUTPUT_MAGIC_V1, f, 0)).is_err(),
            "{expected}"
        );
    }
    assert!(matches!(v.root(2, 1 << 28, paid), Err(Error::RootCount)));
}
#[test]
fn unroll_output_callback_denials_preserve_error_and_no_writes() {
    #[derive(Debug, Eq, PartialEq)]
    struct Denied(String);
    let bytes = literal();
    let view = read(&bytes).unwrap();
    for denied in 0..3 {
        let mut out = vec![0xa5; bytes.len()];
        let mut call = 0;
        let result = encode_inert_loop_unroll_output_into_v1(
            view.fields,
            view.route,
            &mut out,
            1 << 28,
            |_| {
                let n = call;
                call += 1;
                if n == denied {
                    Err(Denied(format!("boundary-{n}")))
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(
            result,
            Err(Error::Charge(Denied(format!("boundary-{denied}"))))
        );
        assert_eq!(out, vec![0xa5; bytes.len()]);
        assert_eq!(call, denied + 1);
    }
    for denied in 0..2 {
        let mut call = 0;
        let result = read_inert_loop_unroll_output_v1(&bytes, 1 << 28, |_| {
            let n = call;
            call += 1;
            if n == denied {
                Err(Denied(format!("read-{n}")))
            } else {
                Ok(())
            }
        });
        assert!(
            matches!(result, Err(Error::Charge(Denied(message))) if message == format!("read-{denied}"))
        );
    }
    let mut calls = 0;
    assert!(matches!(
        read_inert_loop_unroll_output_v1(&bytes, (1 << 28) + 1, |_| {
            calls += 1;
            Ok::<_, Infallible>(())
        }),
        Err(Error::StorageLimit)
    ));
    assert_eq!(calls, 0);
    assert!(
        matches!(view.root(0, 1 << 28, |_| Err(Denied("root".into()))), Err(Error::Charge(Denied(message))) if message == "root")
    );
    assert!(
        matches!(inert_loop_unroll_output_identity_v1(&view, 1 << 28, |_| Err(Denied("hash".into()))), Err(Error::Charge(Denied(message))) if message == "hash")
    );
    for denied in 0..2 {
        let mut call = 0;
        let result = inert_loop_unroll_history_identity_v1(view.fields[0], 1 << 28, |_| {
            let n = call;
            call += 1;
            if n == denied {
                Err(Denied(format!("history-{n}")))
            } else {
                Ok(())
            }
        });
        assert!(
            matches!(result, Err(Error::Charge(Denied(message))) if message == format!("history-{denied}"))
        );
    }
}
#[test]
fn unroll_output_caller_buffer_size_arithmetic_and_hash_work() {
    let bytes = literal();
    let view = read(&bytes).unwrap();
    for n in [bytes.len() - 1, bytes.len() + 1] {
        let mut out = vec![0x71; n];
        assert_eq!(
            encode_inert_loop_unroll_output_into_v1(
                view.fields,
                view.route,
                &mut out,
                1 << 28,
                paid
            ),
            Err(Error::Length)
        );
        assert_eq!(out, vec![0x71; n]);
    }
    assert_eq!(add::<Infallible>(usize::MAX, 1), Err(Error::Arithmetic));
    let mut work = 0;
    let mut out = vec![0; bytes.len()];
    encode_inert_loop_unroll_output_into_v1(view.fields, view.route, &mut out, 1 << 28, |n| {
        work += n;
        paid(n)
    })
    .unwrap();
    assert_eq!(
        work,
        14 + 3 * view.fields[13].len() + 1024 + bytes.len() + 144
    );
    let mut work = 0;
    inert_loop_unroll_output_identity_v1(&view, 1 << 28, |n| {
        work += n;
        paid(n)
    })
    .unwrap();
    assert_eq!(work, bytes.len() + OUTPUT_DOMAIN.len() + 8 + 128);
    let mut work = 0;
    read_inert_loop_unroll_output_v1(&bytes, 1 << 28, |n| {
        work += n;
        paid(n)
    })
    .unwrap();
    assert_eq!(work, 144 + 3 * view.fields[13].len() + 1024);
}
#[test]
fn unroll_output_history_identity_is_framing_only() {
    let bytes = hex(HISTORY_HEX);
    let a = inert_loop_unroll_history_identity_v1(&bytes, 1 << 28, paid).unwrap();
    let mut changed = bytes.clone();
    changed[379] ^= 1;
    let b = inert_loop_unroll_history_identity_v1(&changed, 1 << 28, paid).unwrap();
    assert_ne!(a, b);
    assert_eq!(a.byte_len, b.byte_len);
    // This deliberately invalid V12 payload still has an inert content identity.
    assert_eq!(bytes[379], 42);
    let output = literal();
    assert!(!read(&output).unwrap().grants_authority());
}
#[test]
fn unroll_output_borrowed_views_and_root_storage_contract() {
    let bytes = literal();
    let frame = read(&bytes).unwrap();
    let root = frame.root(0, 1 << 28, paid).unwrap();
    assert_eq!((root.logical_name, root.export_name), ("k", "entry"));
    assert_eq!(frame.canonical_bytes().as_ptr(), bytes.as_ptr());
    assert_eq!(
        INERT_LOOP_UNROLL_READ_STORAGE_V1,
        size_of::<InertLoopUnrollOutputRefV1<'_>>()
            + size_of::<[&[u8]; 14]>()
            + size_of::<UHistoryFramingScratch<'_>>()
            + size_of::<[[bool; 128]; 3]>()
            + size_of::<previous::Cursor<'_>>()
            + size_of::<InertLoopUnrollRootRefV1<'_>>()
            + size_of::<Option<u32>>()
    );
    let caller_live = bytes.capacity()
        + INERT_LOOP_UNROLL_READ_STORAGE_V1
        + size_of::<InertLoopUnrollRootRefV1<'_>>();
    assert!(caller_live > bytes.capacity() + INERT_LOOP_UNROLL_READ_STORAGE_V1);
}
