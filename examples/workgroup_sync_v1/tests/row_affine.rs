use fe2o3_workgroup_sync_v1::row_affine_oracle::{
    RowAffineConfigV1, RowAffineErrorV1, row_affine_oracle_v1, row_affine_vectors_v1,
};

fn config(columns: usize) -> RowAffineConfigV1 {
    RowAffineConfigV1 {
        offset: 0,
        rows: 1,
        columns,
        row_stride: columns,
        scale: 1,
        bias: 0,
        workgroups: 1,
    }
}

#[test]
fn oracle_has_independent_wrapping_and_mask_anchors() {
    for (input, scale, bias, expected) in [
        (vec![0; 65], 0, 7, 455),
        (vec![u32::MAX; 65], 2, 3, 65),
        (vec![u32::MAX; 128], 1, 0, 4_294_967_168),
        (vec![0x8000_0001], u32::MAX, u32::MAX, 0x7fff_fffe),
    ] {
        let mut output = [0xCAFE_BABE, 0xDEAD_BEEF];
        row_affine_oracle_v1(
            &input,
            RowAffineConfigV1 {
                scale,
                bias,
                ..config(input.len())
            },
            &mut output,
        )
        .unwrap();
        assert_eq!(output, [expected, 0xDEAD_BEEF]);
    }
}

#[test]
fn empty_views_preserve_inactive_contributions_and_outputs() {
    let mut output = [91, 92, 93];
    row_affine_oracle_v1(
        &[],
        RowAffineConfigV1 {
            rows: 2,
            workgroups: 3,
            scale: u32::MAX,
            bias: 7,
            ..config(0)
        },
        &mut output,
    )
    .unwrap();
    assert_eq!(output, [0, 0, 93]);
    row_affine_oracle_v1(
        &[],
        RowAffineConfigV1 {
            rows: 0,
            ..config(128)
        },
        &mut output,
    )
    .unwrap();
    assert_eq!(output, [0, 0, 93]);
}

#[test]
fn full_corpus_preserves_input_padding_and_output_canaries() {
    let cases = row_affine_vectors_v1();
    assert_eq!(cases.len(), 86);
    for case in cases {
        let before = case.input.clone();
        let mut output = case.output.clone();
        row_affine_oracle_v1(&case.input, case.config, &mut output).unwrap();
        assert_eq!(
            &output[case.config.rows..],
            &case.output[case.config.rows..]
        );
        assert_eq!(case.input, before);
        let first = output.clone();
        row_affine_oracle_v1(&case.input, case.config, &mut output).unwrap();
        assert_eq!(output, first);
        if case.config.scale == 0 {
            assert!(
                output[..case.config.rows]
                    .iter()
                    .all(|&value| value == case.config.columns as u32 * case.config.bias)
            );
        }
    }
}

#[test]
fn invalid_contracts_reject_before_any_mutation() {
    let base = config(1);
    for (config, input, expected) in [
        (
            RowAffineConfigV1 {
                columns: 129,
                ..base
            },
            vec![0; 129],
            RowAffineErrorV1::Columns,
        ),
        (
            RowAffineConfigV1 {
                workgroups: 0,
                ..base
            },
            vec![1],
            RowAffineErrorV1::Launch,
        ),
        (
            RowAffineConfigV1 { rows: 2, ..base },
            vec![1; 2],
            RowAffineErrorV1::Launch,
        ),
        (
            RowAffineConfigV1 {
                rows: 3,
                workgroups: 3,
                ..base
            },
            vec![1; 3],
            RowAffineErrorV1::Output,
        ),
        (
            RowAffineConfigV1 {
                row_stride: 0,
                ..base
            },
            vec![1],
            RowAffineErrorV1::Stride,
        ),
        (
            RowAffineConfigV1 {
                offset: usize::MAX,
                ..base
            },
            vec![1],
            RowAffineErrorV1::ExtentOverflow,
        ),
        (
            RowAffineConfigV1 {
                rows: 2,
                workgroups: 2,
                row_stride: usize::MAX,
                ..base
            },
            vec![1],
            RowAffineErrorV1::ExtentOverflow,
        ),
        (base, vec![], RowAffineErrorV1::Input),
        (
            RowAffineConfigV1 {
                offset: 1,
                ..config(0)
            },
            vec![],
            RowAffineErrorV1::Input,
        ),
    ] {
        let mut output = [0x1234_5678; 2];
        assert_eq!(
            row_affine_oracle_v1(&input, config, &mut output),
            Err(expected)
        );
        assert_eq!(output, [0x1234_5678; 2]);
    }
}

#[test]
fn final_row_needs_no_trailing_padding_and_empty_view_accepts_end_offset() {
    let mut output = [0; 2];
    row_affine_oracle_v1(
        &[9, 2, 3, 99, 99, 5, 7],
        RowAffineConfigV1 {
            offset: 1,
            rows: 2,
            columns: 2,
            row_stride: 4,
            workgroups: 2,
            ..config(2)
        },
        &mut output,
    )
    .unwrap();
    assert_eq!(output, [5, 12]);
    row_affine_oracle_v1(
        &[9],
        RowAffineConfigV1 {
            offset: 1,
            ..config(0)
        },
        &mut output,
    )
    .unwrap();
    assert_eq!(output, [0, 12]);
}
