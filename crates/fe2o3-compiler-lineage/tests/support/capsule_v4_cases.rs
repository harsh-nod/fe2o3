use super::*;
use std::sync::Arc;

type LayoutV4 = InertProductionSemanticCapsuleLayoutV4;
type ErrorV4<E = ()> = InertProductionSemanticCapsuleErrorV4<E>;
const LIMIT: usize = MAX_NATIVE_REFINED_FORWARDING_CARRIER_STORAGE_V1;

fn carrier(source_len: usize) -> Vec<u8> {
    let layout = NativeRefinedForwardingCarrierLayoutV1::new::<()>(19, source_len).unwrap();
    let mut bytes = vec![0x37; layout.encoded_len()];
    bytes[layout.output_range()].fill(0x53);
    seal_native_refined_forwarding_carrier_v1(layout, &mut bytes, LIMIT, |_| Ok::<_, ()>(()))
        .unwrap();
    bytes
}
fn wire(base: &[u8], pair: &[u8]) -> (LayoutV4, Vec<u8>) {
    let layout = LayoutV4::new::<()>(base.len(), pair.len()).unwrap();
    let mut bytes = vec![0; layout.encoded_len()];
    bytes[layout.base_range()].copy_from_slice(base);
    bytes[layout.carrier_range()].copy_from_slice(pair);
    seal_inert_production_semantic_capsule_v4(layout, &mut bytes, LIMIT, |_| Ok::<_, ()>(()))
        .unwrap();
    (layout, bytes)
}
fn read(bytes: &[u8]) -> Result<InertProductionSemanticCapsuleRefV4<'_>, ErrorV4> {
    read_inert_production_semantic_capsule_v4(bytes, LIMIT, |_| Ok(()))
}
fn reseal(layout: LayoutV4, bytes: &mut [u8]) {
    seal_inert_production_semantic_capsule_v4(layout, bytes, LIMIT, |_| Ok::<_, ()>(())).unwrap();
}

#[test]
fn native_v4_reuses_base_and_carrier_backing_after_input_drop() {
    let base = capsule(3);
    let pair = carrier(64);
    let (layout, bytes) = wire(base.canonical_bytes(), &pair);
    let frame = read(&bytes).unwrap();
    assert_eq!(frame.base_bytes(), base.canonical_bytes());
    assert_eq!(frame.carrier().canonical_bytes(), pair);
    assert!(!frame.grants_authority());
    let identity = frame.identity();
    let carrier_identity = frame.carrier().identity();
    assert_ne!(identity.sha256(), carrier_identity.sha256());
    assert_ne!(identity.sha256(), base.identity().sha256());

    // Exercise a nonzero offset and unrelated prefix/suffix in one allocation.
    let mut backing = vec![0x17; 13];
    backing.extend_from_slice(&bytes);
    backing.extend_from_slice(&[0x29; 23]);
    let backing = Arc::new(backing);
    let owner =
        InertProductionSemanticCapsuleV4::decode_shared_vec(backing.clone(), 13..13 + bytes.len())
            .unwrap();
    assert_eq!(owner.canonical_bytes().as_ptr(), backing[13..].as_ptr());
    assert_eq!(
        owner.base().canonical_bytes().as_ptr(),
        backing[13 + layout.base_range().start..].as_ptr()
    );
    assert_eq!(
        owner.carrier_bytes().as_ptr(),
        backing[13 + layout.carrier_range().start..].as_ptr()
    );
    let base_layout = super::layout(base.canonical_bytes());
    assert_eq!(
        owner
            .base()
            .receipts()
            .semantic_mir()
            .canonical_preimage()
            .as_ptr(),
        backing[13 + layout.base_range().start + base_layout.receipts[2].0.start..].as_ptr()
    );
    drop(backing);
    assert_eq!(owner.canonical_bytes(), bytes);
    assert_eq!(owner.base(), &base);
    assert_eq!(owner.carrier_bytes(), pair);
    assert_eq!(owner.identity(), identity);
    assert_eq!(owner.carrier_identity(), carrier_identity);
    assert!(!owner.grants_authority());

    let pointer = bytes.as_ptr();
    let owned = InertProductionSemanticCapsuleV4::decode_owned(bytes).unwrap();
    assert_eq!(owned.canonical_bytes().as_ptr(), pointer);
    assert_eq!(owned.identity(), identity);
}

#[test]
fn native_v4_keeps_complete_ceiling_and_independent_bounds() {
    let pair = carrier(64);
    let base_len = MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V4 - 80 - pair.len();
    let layout = LayoutV4::new::<()>(base_len, pair.len()).unwrap();
    assert_eq!(layout.encoded_len(), 160 * 1024 * 1024);
    assert_eq!(
        LayoutV4::new::<()>(base_len + 1, pair.len()),
        Err(ErrorV4::Length)
    );
    for (first, second, expected) in [
        (0, 1, ErrorV4::BaseLength),
        (1, 0, ErrorV4::CarrierLength),
        (
            MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3 + 1,
            1,
            ErrorV4::BaseLength,
        ),
        (
            1,
            MAX_NATIVE_REFINED_FORWARDING_CARRIER_BYTES_V1 + 1,
            ErrorV4::CarrierLength,
        ),
        (usize::MAX, 1, ErrorV4::BaseLength),
        (1, usize::MAX, ErrorV4::CarrierLength),
    ] {
        assert_eq!(LayoutV4::new::<()>(first, second), Err(expected));
    }
    // Exact ceiling acceptance is framing only; these bytes are not a V3 base.
    let mut bytes = vec![0x41; layout.encoded_len()];
    bytes[layout.carrier_range()].copy_from_slice(&pair);
    reseal(layout, &mut bytes);
    assert_eq!(read(&bytes).unwrap().base_bytes().len(), base_len);
    assert_eq!(MAX_CANONICAL_SEMANTIC_MIR_BYTES_V3, 128 * 1024 * 1024);
    assert_eq!(MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3, 4 * 1024 * 1024);
}

#[test]
fn native_v4_rejects_cross_versions_invalid_ranges_and_impossible_mir_extent() {
    let base = capsule(3);
    let pair = carrier(1);
    let (_, bytes) = wire(base.canonical_bytes(), &pair);
    assert!(InertProductionSemanticCapsuleV3::decode(base.canonical_bytes()).is_ok());
    assert!(matches!(
        InertProductionSemanticCapsuleV4::decode_owned(bytes),
        Err(ErrorV4::Base(LineageDecodeErrorV3::PreimageTooLarge {
            max: 1,
            ..
        }))
    ));
    let (_, bytes) = wire(base.canonical_bytes(), &carrier(64));
    assert!(matches!(
        InertProductionSemanticCapsuleV3::decode(&bytes),
        Err(LineageDecodeErrorV3::InvalidMagic)
    ));
    for old in [base.canonical_bytes(), pair.as_slice()] {
        assert!(matches!(read(old), Err(ErrorV4::Header)));
    }
    let backing = Arc::new(bytes);
    for range in [2..1, 0..backing.len() + 1, usize::MAX..usize::MAX, 0..0] {
        assert!(matches!(
            InertProductionSemanticCapsuleV4::decode_shared_vec(backing.clone(), range),
            Err(ErrorV4::Length)
        ));
    }
}

#[test]
fn native_v4_mir_extent_boundary_is_not_a_semantic_join() {
    let base = capsule(3);
    let mir_len = base.receipts().semantic_mir().canonical_preimage().len();
    for (source_len, succeeds) in [(mir_len, true), (mir_len - 1, false)] {
        let (_, bytes) = wire(base.canonical_bytes(), &carrier(source_len));
        assert_eq!(
            InertProductionSemanticCapsuleV4::decode_owned(bytes).is_ok(),
            succeeds
        );
    }
    // Completely unrelated but well-framed constituents are only inert content.
    let unrelated = capsule(7);
    let (_, bytes) = wire(unrelated.canonical_bytes(), &carrier(64));
    let owner = InertProductionSemanticCapsuleV4::decode_owned(bytes).unwrap();
    assert_eq!(owner.base(), &unrelated);
    assert!(!owner.grants_authority());
}

#[test]
fn native_v4_rejects_outer_and_resealed_nested_mutations_without_fallback() {
    let base = capsule(3);
    let (layout, bytes) = wire(base.canonical_bytes(), &carrier(64));
    for offset in [0, 8, 10, 12] {
        let mut bad = bytes.clone();
        bad[offset] ^= 1;
        assert!(matches!(read(&bad), Err(ErrorV4::Header)));
    }
    for offset in 40..48 {
        let mut bad = bytes.clone();
        bad[offset] = 1;
        assert!(matches!(read(&bad), Err(ErrorV4::Reserved)));
    }
    for length in 0..bytes.len() {
        assert!(read(&bytes[..length]).is_err());
    }
    let mut bad = bytes.clone();
    bad.push(0);
    assert!(matches!(read(&bad), Err(ErrorV4::Length)));
    for range in [layout.base_range(), layout.carrier_range()] {
        let mut bad = bytes.clone();
        bad[range.start] ^= 1;
        assert!(matches!(read(&bad), Err(ErrorV4::Identity)));
        reseal(layout, &mut bad);
        assert!(InertProductionSemanticCapsuleV4::decode_owned(bad).is_err());
    }
    let mut bad = bytes.clone();
    bad[layout.carrier_range().end - 1] ^= 1;
    reseal(layout, &mut bad);
    assert!(matches!(
        read(&bad),
        Err(ErrorV4::Carrier(
            NativeRefinedForwardingCarrierErrorV1::Identity
        ))
    ));
    let mut bad = bytes.clone();
    bad[24..32].copy_from_slice(&0_u64.to_le_bytes());
    assert!(matches!(read(&bad), Err(ErrorV4::BaseLength)));
    bad[24..32].copy_from_slice(&(layout.base_range().len() as u64).to_le_bytes());
    bad[32..40].copy_from_slice(&0_u64.to_le_bytes());
    assert!(matches!(read(&bad), Err(ErrorV4::CarrierLength)));
}

#[test]
fn native_v4_prepaid_visits_preserve_first_refusal_and_seal_is_atomic() {
    let (layout, bytes) = wire(&[0x41; 19], &carrier(64));
    // Independently constructed with Node crypto, without using these codecs.
    assert_eq!(
        *read(&bytes).unwrap().identity().sha256(),
        [
            0xcc, 0x55, 0xa9, 0x92, 0x57, 0x05, 0x52, 0x53, 0x59, 0xf5, 0x50, 0xeb, 0x35, 0x9c,
            0x09, 0x1b, 0x5c, 0x12, 0xf3, 0x46, 0x21, 0x39, 0xe0, 0x6f, 0xaa, 0x9a, 0xbf, 0x58,
            0x3d, 0xbe, 0x57, 0x8c,
        ]
    );
    let mut work = vec![];
    read_inert_production_semantic_capsule_v4(&bytes, LIMIT, |n| {
        work.push(n);
        Ok::<_, usize>(())
    })
    .unwrap();
    assert_eq!(work.len(), 4);
    for refused in 0..work.len() {
        let mut i = 0;
        let result = read_inert_production_semantic_capsule_v4(&bytes, LIMIT, |n| {
            assert_eq!(n, work[i]);
            let index = i;
            i += 1;
            if index == refused { Err(index) } else { Ok(()) }
        });
        match result {
            Err(ErrorV4::Charge(n)) if refused < 2 => assert_eq!(n, refused),
            Err(ErrorV4::Carrier(NativeRefinedForwardingCarrierErrorV1::Charge(n)))
                if refused >= 2 =>
            {
                assert_eq!(n, refused)
            }
            _ => panic!("wrong first refusal"),
        }
        assert_eq!(i, refused + 1);
    }
    let mut destination = vec![0x29; layout.encoded_len()];
    let before = destination.clone();
    assert_eq!(
        seal_inert_production_semantic_capsule_v4(layout, &mut destination, LIMIT, |_| Err(17)),
        Err(ErrorV4::Charge(17))
    );
    assert_eq!(destination, before);
    struct DropBomb;
    impl Drop for DropBomb {
        fn drop(&mut self) {
            panic!("callback destructor")
        }
    }
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let bomb = DropBomb;
            let _ = seal_inert_production_semantic_capsule_v4(
                layout,
                &mut destination,
                LIMIT,
                move |_| {
                    std::hint::black_box(&bomb);
                    Ok::<_, ()>(())
                },
            );
        }))
        .is_err()
    );
    assert_eq!(destination, before);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = seal_inert_production_semantic_capsule_v4(
                layout,
                &mut destination,
                LIMIT,
                |_| -> Result<(), ()> { panic!("debit") },
            );
        }))
        .is_err()
    );
    assert_eq!(destination, before);
    assert!(matches!(
        read_inert_production_semantic_capsule_v4(&bytes, LIMIT + 1, |_| -> Result<(), ()> {
            panic!("must not charge")
        }),
        Err(ErrorV4::StorageLimit)
    ));
    assert_eq!(
        seal_inert_production_semantic_capsule_v4(
            layout,
            &mut destination,
            LIMIT + 1,
            |_| -> Result<(), ()> { panic!("must not charge") }
        ),
        Err(ErrorV4::StorageLimit)
    );
    assert_eq!(destination, before);
}
