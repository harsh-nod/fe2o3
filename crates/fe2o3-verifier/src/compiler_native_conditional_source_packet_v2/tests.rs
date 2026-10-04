//! Signatures and leaf frames are deliberately inert, not proof fixtures.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_lower_mir_kernel::ProductionSourceLaunchInputV1 as Geometry;

mod hostile;
mod resources_tests;

const FLOOR: usize = 37;

fn fixture<R>(consume: impl FnOnce(NativeConditionalSourcePacketInputV2<'_>) -> R) -> R {
    let row = Staging {
        receipt: [1; 32],
        effect: [2; 32],
        signer: [3; 32],
        execution: [4; 32],
        toolchain: [[5; 32], [6; 32], [7; 32], [8; 32], [9; 32]],
    };
    let mut other = row;
    other.receipt = [10; 32];
    let rows = [row, other];
    let signatures = [
        Signature::from_untrusted_parts([11; SIGNATURE_BYTES], [12; 32]),
        Signature::from_untrusted_parts([13; SIGNATURE_BYTES], [14; 32]),
    ];
    let formulas = [
        Signature::from_untrusted_parts([21; SIGNATURE_BYTES], [22; 32]),
        Signature::from_untrusted_parts([23; SIGNATURE_BYTES], [24; 32]),
    ];
    let roots = [
        NativeConditionalSourceRootV2 {
            semantic_root: 9,
            launch_rank: 1,
            launch: Launch::new(
                " logical/name ",
                [7; 32],
                Geometry::new(2, None, [0, u32::MAX, 3]),
            ),
            induction_bytes: b"induction9",
            recipe_bytes: b"recipe9",
            source_rows_bytes: b"rows9",
            ranked_ir: "diagnostic\ntext",
            cpu_input_bytes: b"cpu9",
            staging_commitments: &rows,
            effect_receipts: &signatures,
            formula_receipt: &formulas[0],
        },
        NativeConditionalSourceRootV2 {
            semantic_root: 4,
            launch_rank: 2,
            launch: Launch::new(
                "other",
                [6; 32],
                Geometry::new(3, Some([1, 2, 3]), [9, 0, 1]),
            ),
            induction_bytes: b"induction4",
            recipe_bytes: b"recipe4",
            source_rows_bytes: b"rows4",
            ranked_ir: "other text",
            cpu_input_bytes: b"cpu4",
            staging_commitments: &rows[1..],
            effect_receipts: &signatures[1..],
            formula_receipt: &formulas[1],
        },
    ];
    consume(NativeConditionalSourcePacketInputV2 {
        semantic_mir: b"mir\0",
        native_module: b"native/N",
        canonical_kernel_order: &[1, 0],
        roots: &roots,
    })
}

fn encoded() -> Vec<u8> {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    fixture(|input| {
        encode_native_conditional_source_packet_v2(input, &mut budget)
            .unwrap()
            .0
    })
}

fn decode_only(bytes: &[u8]) -> Result<(), E> {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_decoded_native_conditional_source_packet_v2(bytes, &mut budget, |_, _| ());
    assert_eq!(budget.storage(), FLOOR);
    result
}

fn word(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn blob(out: &mut Vec<u8>, bytes: &[u8]) {
    word(out, bytes.len() as u32);
    out.extend_from_slice(bytes);
}
fn signature(out: &mut Vec<u8>, byte: u8, key: u8) {
    out.extend_from_slice(&[byte; SIGNATURE_BYTES]);
    out.extend_from_slice(&[key; 32]);
}

// Independent explicit fixture transcript, without the production writer/view.
fn golden() -> Vec<u8> {
    let mut out = b"F2NSRC2\0\x02\0\0\0\0\0\0\0\x03".to_vec();
    blob(&mut out, b"mir\0");
    blob(&mut out, b"native/N");
    for n in [2, 1, 0, 2] {
        word(&mut out, n);
    }
    for second in [false, true] {
        word(&mut out, if second { 4 } else { 9 });
        out.push(if second { 2 } else { 1 });
        blob(&mut out, if second { b"other" } else { b" logical/name " });
        out.extend_from_slice(&[if second { 6 } else { 7 }; 32]);
        out.extend_from_slice(if second { &[3, 1] } else { &[2, 0] });
        if second {
            for n in [1, 2, 3] {
                word(&mut out, n);
            }
        }
        for n in if second { [9, 0, 1] } else { [0, u32::MAX, 3] } {
            word(&mut out, n);
        }
        let frames: [&[u8]; 5] = if second {
            [b"induction4", b"recipe4", b"rows4", b"other text", b"cpu4"]
        } else {
            [
                b"induction9",
                b"recipe9",
                b"rows9",
                b"diagnostic\ntext",
                b"cpu9",
            ]
        };
        for frame in frames {
            blob(&mut out, frame);
        }
        word(&mut out, if second { 1 } else { 2 });
        for entry in if second { 1..2 } else { 0..2 } {
            out.extend_from_slice(&[if entry == 0 { 1 } else { 10 }; 32]);
            for byte in 2..=9 {
                out.extend_from_slice(&[byte; 32]);
            }
            signature(
                &mut out,
                if entry == 0 { 11 } else { 13 },
                if entry == 0 { 12 } else { 14 },
            );
        }
        word(&mut out, (4 + SIGNATURE_BYTES + 32) as u32);
        out.extend_from_slice(&[2, 0, 0, 0]);
        signature(
            &mut out,
            if second { 23 } else { 21 },
            if second { 24 } else { 22 },
        );
    }
    let length = out.len() as u32;
    out[12..16].copy_from_slice(&length.to_le_bytes());
    out
}

#[test]
fn golden_multi_root_roundtrip_preserves_all_independent_fields() {
    let bytes = encoded();
    assert_eq!(bytes, golden());
    assert_eq!(bytes, encoded());
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(FLOOR).unwrap();
    with_decoded_native_conditional_source_packet_v2(&bytes, &mut budget, |view, budget| {
        assert_eq!(view.semantic_mir, b"mir\0");
        assert_eq!(view.native_module, b"native/N");
        assert_eq!(view.canonical_kernel_order, [1, 0]);
        assert_eq!(view.roots.len(), 2);
        for (i, root) in view.roots.iter().enumerate() {
            assert_eq!(root.semantic_root, [9, 4][i]);
            assert_eq!(root.launch_rank, [1, 2][i]);
            assert_eq!(root.launch.launch().rank(), [2, 3][i]);
            assert_eq!(root.launch.kernel_binding(), [[7; 32], [6; 32]][i]);
            assert_eq!(
                root.formula_receipt.wire(),
                &[[21; SIGNATURE_BYTES], [23; SIGNATURE_BYTES]][i]
            );
            assert_eq!(
                root.formula_receipt.verifying_key(),
                &[[22; 32], [24; 32]][i]
            );
        }
        assert_eq!(view.roots[0].launch.logical_name(), " logical/name ");
        assert_eq!(
            view.roots[1].launch.launch(),
            Geometry::new(3, Some([1, 2, 3]), [9, 0, 1])
        );
        assert_eq!(
            view.roots[0].staging_commitments[1],
            view.roots[1].staging_commitments[0]
        );
        assert_eq!(
            view.roots[0].effect_receipts[1],
            view.roots[1].effect_receipts[0]
        );
        let floor = budget.storage();
        let (again, receipt) = encode_native_conditional_source_packet_v2(view, budget).unwrap();
        assert_eq!(again, bytes);
        assert_eq!(receipt.retained_storage(), again.capacity());
        assert_eq!(budget.storage(), floor);
    })
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn opaque_leaf_bytes_and_diagnostic_unicode_are_not_admitted_or_normalized() {
    fixture(|input| {
        let mut roots = input.roots.to_vec();
        roots[0].source_rows_bytes = &[0, 255, 0, 1];
        roots[0].cpu_input_bytes = b"not a B1 frame";
        roots[0].ranked_ir = "e\u{301}\n\u{00e9}";
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let (bytes, _) = encode_native_conditional_source_packet_v2(
            NativeConditionalSourcePacketInputV2 {
                roots: &roots,
                ..input
            },
            &mut budget,
        )
        .unwrap();
        with_decoded_native_conditional_source_packet_v2(&bytes, &mut budget, |view, _| {
            assert_eq!(view.roots[0].source_rows_bytes, [0, 255, 0, 1]);
            assert_eq!(view.roots[0].cpu_input_bytes, b"not a B1 frame");
            assert_eq!(
                view.roots[0].ranked_ir.as_bytes(),
                roots[0].ranked_ir.as_bytes()
            );
        })
        .unwrap();
    });
}

#[test]
fn minimum_single_root_frame_matches_count_preflight_without_leaf_admission() {
    fixture(|input| {
        let roots = [NativeConditionalSourceRootV2 {
            semantic_root: 0,
            launch_rank: 0,
            launch: Launch::new("", [0; 32], Geometry::new(0, None, [0; 3])),
            induction_bytes: &[],
            recipe_bytes: &[],
            source_rows_bytes: &[],
            ranked_ir: "",
            cpu_input_bytes: &[],
            staging_commitments: &input.roots[1].staging_commitments[..1],
            effect_receipts: &input.roots[1].effect_receipts[..1],
            formula_receipt: input.roots[1].formula_receipt,
        }];
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let (bytes, _) = encode_native_conditional_source_packet_v2(
            NativeConditionalSourcePacketInputV2 {
                semantic_mir: &[],
                native_module: &[],
                canonical_kernel_order: &[0],
                roots: &roots,
            },
            &mut budget,
        )
        .unwrap();
        // 17 header + two blob lengths + order count/index + root count.
        assert_eq!(bytes.len(), 37 + MIN_ROOT_BYTES);
        decode_only(&bytes).unwrap();
    });
}
