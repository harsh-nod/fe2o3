//! Independent logical-row specification and shared row-affine input corpus.

/// Logical view, arithmetic and complete-workgroup launch contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RowAffineConfigV1 {
    /// Element offset of the logical view inside the immutable input.
    pub offset: usize,
    /// Number of logical output rows.
    pub rows: usize,
    /// Active columns per row, in 0..=128.
    pub columns: usize,
    /// Distance in elements between consecutive row starts.
    pub row_stride: usize,
    /// Multiplier applied only to active elements.
    pub scale: u32,
    /// Addend applied only to active elements.
    pub bias: u32,
    /// Number of complete 64-lane groups, including any extra inactive groups.
    pub workgroups: usize,
}

/// Invalid contract detected before output mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowAffineErrorV1 {
    /// More than 128 logical columns were requested.
    Columns,
    /// There is no complete group or fewer groups than rows.
    Launch,
    /// The output cannot hold every logical row.
    Output,
    /// A nonempty row stride is smaller than the logical width.
    Stride,
    /// The mathematical input extent cannot be represented by usize.
    ExtentOverflow,
    /// The complete view is not contained in the input.
    Input,
}

/// Computes each mathematical logical-row sum modulo 2^32.
///
/// This oracle uses u128 arithmetic and no lane assignment, reduction tree,
/// device view, intrinsic or kernel helper. Output after the logical rows is
/// preserved. Every invalid contract is rejected before any output is changed.
pub fn row_affine_oracle_v1(
    values: &[u32],
    config: RowAffineConfigV1,
    output: &mut [u32],
) -> Result<(), RowAffineErrorV1> {
    if config.columns > 128 {
        return Err(RowAffineErrorV1::Columns);
    }
    if config.workgroups == 0
        || config.workgroups < config.rows
        || config.workgroups > u32::MAX as usize
        || (config.workgroups as u128) * 64 > usize::MAX as u128
    {
        return Err(RowAffineErrorV1::Launch);
    }
    if output.len() < config.rows {
        return Err(RowAffineErrorV1::Output);
    }
    let required = if config.rows == 0 || config.columns == 0 {
        config.offset as u128
    } else {
        if config.row_stride < config.columns {
            return Err(RowAffineErrorV1::Stride);
        }
        config.offset as u128
            + (config.rows as u128 - 1) * config.row_stride as u128
            + config.columns as u128
    };
    if required > usize::MAX as u128 {
        return Err(RowAffineErrorV1::ExtentOverflow);
    }
    if required > values.len() as u128 {
        return Err(RowAffineErrorV1::Input);
    }
    for (row, slot) in output.iter_mut().take(config.rows).enumerate() {
        let mut sum = 0_u128;
        for column in 0..config.columns {
            let index =
                config.offset as u128 + row as u128 * config.row_stride as u128 + column as u128;
            sum += u128::from(values[index as usize]) * u128::from(config.scale)
                + u128::from(config.bias);
        }
        *slot = (sum & u128::from(u32::MAX)) as u32;
    }
    Ok(())
}

/// One reusable positive input, including input padding and output canaries.
#[derive(Clone, Debug)]
pub struct RowAffineVectorV1 {
    /// Logical and launch contract.
    pub config: RowAffineConfigV1,
    /// Entire immutable input backing.
    pub input: Vec<u32>,
    /// Entire initial output, with two trailing guard elements.
    pub output: Vec<u32>,
}

/// Boundary, padded, empty, high-bit and wrapping cases shared by every variant.
pub fn row_affine_vectors_v1() -> Vec<RowAffineVectorV1> {
    let mut cases = Vec::new();
    for rows in [0, 1, 3] {
        for columns in [0, 1, 63, 64, 65, 127, 128] {
            for (scale, bias) in [(0, 7), (1, 0), (2, 3), (u32::MAX, u32::MAX)] {
                let offset = if cases.len().is_multiple_of(2) { 0 } else { 3 };
                let padding = if cases.len().is_multiple_of(3) { 0 } else { 5 };
                let row_stride = columns + padding;
                let required = if rows == 0 || columns == 0 {
                    offset
                } else {
                    offset + (rows - 1) * row_stride + columns
                };
                let mut input = vec![0xD15E_A5ED; required + 3];
                for row in 0..rows {
                    for column in 0..columns {
                        input[offset + row * row_stride + column] = match (row + column) % 4 {
                            0 => u32::MAX,
                            1 => 0,
                            2 => 0x8000_0001,
                            _ => (row * 131 + column * 17) as u32,
                        };
                    }
                }
                cases.push(RowAffineVectorV1 {
                    config: RowAffineConfigV1 {
                        offset,
                        rows,
                        columns,
                        row_stride,
                        scale,
                        bias,
                        workgroups: rows.max(1) + usize::from(cases.len().is_multiple_of(2)),
                    },
                    input,
                    output: vec![0xC0DE_CAFE; rows + 2],
                });
            }
        }
    }
    for rows in [0, 2] {
        cases.push(RowAffineVectorV1 {
            config: RowAffineConfigV1 {
                offset: 0,
                rows,
                columns: 0,
                row_stride: 0,
                scale: u32::MAX,
                bias: 7,
                workgroups: rows.max(1),
            },
            input: Vec::new(),
            output: vec![0xC0DE_CAFE; rows],
        });
    }
    cases
}
