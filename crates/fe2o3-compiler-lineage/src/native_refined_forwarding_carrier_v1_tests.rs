use super::*;

const LIMIT: usize = MAX_NATIVE_REFINED_FORWARDING_CARRIER_STORAGE_V1;
type Layout = NativeRefinedForwardingCarrierLayoutV1;

fn fixture() -> (Layout, Vec<u8>) {
    let layout = Layout::new::<()>(19, 7).unwrap();
    let mut bytes = vec![0x53; layout.encoded_len()];
    bytes[layout.source_range()].fill(0x37);
    seal_native_refined_forwarding_carrier_v1(layout, &mut bytes, LIMIT, |_| Ok::<_, ()>(()))
        .unwrap();
    (layout, bytes)
}
fn read(bytes: &[u8]) -> Result<NativeRefinedForwardingCarrierRefV1<'_>, Error<()>> {
    read_native_refined_forwarding_carrier_v1(bytes, LIMIT, |_| Ok(()))
}

#[test]
fn native_carrier_borrows_disjoint_payloads_and_binds_both_roles() {
    let (layout, bytes) = fixture();
    let frame = read(&bytes).unwrap();
    // Independently constructed 48-byte header + 19/7-byte payloads, hashed
    // with Node crypto SHA256. Pins the domain, length prefix and wire order.
    assert_eq!(
        *frame.identity().sha256(),
        [
            0x5d, 0x19, 0xd8, 0xcb, 0xd7, 0x9e, 0x45, 0xd6, 0x82, 0x05, 0xb0, 0x82, 0xac, 0xac,
            0x03, 0x30, 0xff, 0x15, 0x06, 0xd5, 0x9f, 0xed, 0x86, 0xbf, 0xf9, 0x72, 0x61, 0x7c,
            0xd6, 0xee, 0x48, 0x60,
        ]
    );
    assert_eq!(frame.canonical_bytes().as_ptr(), bytes.as_ptr());
    assert_eq!(frame.output(), &[0x53; 19]);
    assert_eq!(frame.source_packet(), &[0x37; 7]);
    assert_eq!(
        frame.output().as_ptr(),
        bytes[layout.output_range()].as_ptr()
    );
    assert_eq!(
        frame.source_packet().as_ptr(),
        bytes[layout.source_range()].as_ptr()
    );
    assert_eq!(frame.identity().byte_len(), bytes.len() as u64);
    assert!(!frame.grants_authority());
    for range in [layout.output_range(), layout.source_range()] {
        let mut changed = bytes.clone();
        changed[range.start] ^= 1;
        assert!(matches!(read(&changed), Err(Error::Identity)));
        let identity =
            seal_native_refined_forwarding_carrier_v1(layout, &mut changed, LIMIT, |_| {
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_ne!(identity, frame.identity());
        assert_eq!(read(&changed).unwrap().identity(), identity);
    }
    let other = Layout::new::<()>(7, 19).unwrap();
    let mut reordered = bytes.clone();
    seal_native_refined_forwarding_carrier_v1(other, &mut reordered, LIMIT, |_| Ok::<_, ()>(()))
        .unwrap();
    assert_ne!(read(&reordered).unwrap().identity(), frame.identity());
}

#[test]
fn native_carrier_accepts_both_exact_maxima_without_widening_legacy_receipts() {
    let layout = Layout::new::<()>(
        MAX_NATIVE_REFINED_FORWARDING_OUTPUT_BYTES_V1,
        MAX_NATIVE_REFINED_FORWARDING_SOURCE_BYTES_V1,
    )
    .unwrap();
    assert_eq!(
        layout.encoded_len(),
        MAX_NATIVE_REFINED_FORWARDING_CARRIER_BYTES_V1
    );
    assert_eq!(
        crate::MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
        4 * 1024 * 1024
    );
    let mut bytes = vec![0x17; layout.encoded_len()];
    seal_native_refined_forwarding_carrier_v1(layout, &mut bytes, LIMIT, |_| Ok::<_, ()>(()))
        .unwrap();
    let view = read(&bytes).unwrap();
    assert_eq!(view.output().len(), 64 * 1024 * 1024);
    assert_eq!(view.source_packet().len(), 4 * 1024 * 1024);
    for (output, source, expected) in [
        (0, 1, Error::OutputLength),
        (1, 0, Error::SourceLength),
        (
            MAX_NATIVE_REFINED_FORWARDING_OUTPUT_BYTES_V1 + 1,
            1,
            Error::OutputLength,
        ),
        (
            1,
            MAX_NATIVE_REFINED_FORWARDING_SOURCE_BYTES_V1 + 1,
            Error::SourceLength,
        ),
        (usize::MAX, 1, Error::OutputLength),
        (1, usize::MAX, Error::SourceLength),
    ] {
        assert_eq!(Layout::new::<()>(output, source), Err(expected));
    }
    assert!(matches!(
        crate::InertProofBindingReceiptV3::from_canonical_preimage(vec![1; 4 * 1024 * 1024 + 1]),
        Err(crate::LineageErrorV3::PreimageTooLarge { .. })
    ));
}

#[test]
fn native_carrier_rejects_inexact_frames_and_declared_component_overflow() {
    let (_, bytes) = fixture();
    for offset in [0, 8, 10, 12] {
        let mut bad = bytes.clone();
        bad[offset] ^= 1;
        assert!(matches!(read(&bad), Err(Error::Header)));
    }
    for offset in 40..48 {
        let mut bad = bytes.clone();
        bad[offset] = 1;
        assert!(matches!(read(&bad), Err(Error::Reserved)));
    }
    for n in 0..bytes.len() {
        assert!(read(&bytes[..n]).is_err());
    }
    let mut bad = bytes.clone();
    bad.push(0);
    assert!(matches!(read(&bad), Err(Error::Length)));
    for (offset, length, expected) in [
        (24, 0, Error::OutputLength),
        (32, 0, Error::SourceLength),
        (
            24,
            MAX_NATIVE_REFINED_FORWARDING_OUTPUT_BYTES_V1 + 1,
            Error::OutputLength,
        ),
        (
            32,
            MAX_NATIVE_REFINED_FORWARDING_SOURCE_BYTES_V1 + 1,
            Error::SourceLength,
        ),
    ] {
        let mut bad = bytes.clone();
        bad[offset..offset + 8].copy_from_slice(&(length as u64).to_le_bytes());
        let mut work = vec![];
        let error = read_native_refined_forwarding_carrier_v1(&bad, LIMIT, |n| {
            work.push(n);
            Ok::<_, ()>(())
        })
        .err()
        .unwrap();
        assert_eq!(error, expected);
        assert_eq!(work, [HEADER]);
    }
    let mut bad = bytes.clone();
    *bad.last_mut().unwrap() ^= 1;
    assert!(matches!(read(&bad), Err(Error::Identity)));
}

#[test]
fn native_carrier_denials_precede_mutation_and_preserve_typed_errors() {
    #[derive(Debug, Eq, PartialEq)]
    struct Denied(usize);
    let (layout, bytes) = fixture();
    let mut calls = vec![];
    read_native_refined_forwarding_carrier_v1(&bytes, LIMIT, |n| {
        calls.push(n);
        Ok::<_, Denied>(())
    })
    .unwrap();
    assert_eq!(calls.len(), 2);
    for refused in 0..calls.len() {
        let mut count = 0;
        let result = read_native_refined_forwarding_carrier_v1(&bytes, LIMIT, |n| {
            assert_eq!(n, calls[count]);
            let i = count;
            count += 1;
            if i == refused { Err(Denied(i)) } else { Ok(()) }
        });
        assert!(matches!(result, Err(Error::Charge(Denied(i))) if i == refused));
        assert_eq!(count, refused + 1);
    }
    let mut destination = vec![0x29; layout.encoded_len()];
    let before = destination.clone();
    assert_eq!(
        seal_native_refined_forwarding_carrier_v1(layout, &mut destination, LIMIT, |_| Err(
            Denied(3)
        )),
        Err(Error::Charge(Denied(3)))
    );
    assert_eq!(destination, before);
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = seal_native_refined_forwarding_carrier_v1(
            layout,
            &mut destination,
            LIMIT,
            |_| -> Result<(), Denied> { panic!("seal debit panic") },
        );
    }));
    assert!(panicked.is_err());
    assert_eq!(destination, before);
    let mut count = 0;
    assert_eq!(
        seal_native_refined_forwarding_carrier_v1(layout, &mut destination, LIMIT + 1, |_| {
            count += 1;
            Ok::<_, Denied>(())
        }),
        Err(Error::StorageLimit)
    );
    assert_eq!(count, 0);
    assert_eq!(destination, before);
    assert_eq!(
        seal_native_refined_forwarding_carrier_v1(
            layout,
            &mut destination[..layout.encoded_len() - 1],
            LIMIT,
            |_| {
                count += 1;
                Ok::<_, Denied>(())
            }
        ),
        Err(Error::Length)
    );
    assert_eq!(count, 0);
    assert_eq!(destination, before);
    assert!(matches!(
        read_native_refined_forwarding_carrier_v1(&bytes, LIMIT + 1, |_| {
            count += 1;
            Ok::<_, Denied>(())
        }),
        Err(Error::StorageLimit)
    ));
    assert_eq!(count, 0);
}
