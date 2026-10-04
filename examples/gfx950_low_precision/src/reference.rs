//! Independent host reference for the fixed Rust kernels.

use std::fmt;

use crate::kernel::{ATTENTION_TOKENS, GEMM_K, GEMM_M, GEMM_N, VALUE_COLUMNS};

/// Input encoding accepted by one fixed kernel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LowPrecisionFormat {
    Fp4E2M1,
    Fp8E4M3,
}

impl LowPrecisionFormat {
    pub fn decode(self, bits: u8) -> f32 {
        match self {
            Self::Fp4E2M1 => decode_fp4_e2m1(bits),
            Self::Fp8E4M3 => decode_fp8_e4m3(bits),
        }
    }

    pub fn encode_test_value(self, value: f32) -> u8 {
        const VALUES: [f32; 5] = [-1.0, -0.5, 0.0, 0.5, 1.0];
        const FP4_BITS: [u8; 5] = [0xa, 0x9, 0x0, 0x1, 0x2];
        const FP8_BITS: [u8; 5] = [0xb8, 0xb0, 0x00, 0x30, 0x38];
        let index = VALUES
            .iter()
            .position(|candidate| candidate.to_bits() == value.to_bits())
            .expect("deterministic input values are exactly representable");
        match self {
            Self::Fp4E2M1 => FP4_BITS[index],
            Self::Fp8E4M3 => FP8_BITS[index],
        }
    }
}

/// Rejected fixed-shape reference input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReferenceShapeError {
    tensor: &'static str,
    expected: usize,
    actual: usize,
}

impl ReferenceShapeError {
    pub const fn tensor(self) -> &'static str {
        self.tensor
    }

    pub const fn expected(self) -> usize {
        self.expected
    }

    pub const fn actual(self) -> usize {
        self.actual
    }
}

impl fmt::Display for ReferenceShapeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} has {} elements; expected exactly {}",
            self.tensor, self.actual, self.expected
        )
    }
}

impl std::error::Error for ReferenceShapeError {}

fn require_length(
    tensor: &'static str,
    actual: usize,
    expected: usize,
) -> Result<(), ReferenceShapeError> {
    if actual == expected {
        Ok(())
    } else {
        Err(ReferenceShapeError {
            tensor,
            expected,
            actual,
        })
    }
}

/// Decodes one OCP E2M1 FP4 value from the low nibble.
pub fn decode_fp4_e2m1(bits: u8) -> f32 {
    const MAGNITUDES: [f32; 8] = [0.0, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];
    let magnitude = MAGNITUDES[usize::from(bits & 0x7)];
    if bits & 0x8 == 0 {
        magnitude
    } else {
        -magnitude
    }
}

/// Decodes one OCP E4M3 FP8 value, including subnormal and NaN encodings.
pub fn decode_fp8_e4m3(bits: u8) -> f32 {
    let sign = if bits & 0x80 == 0 { 1.0 } else { -1.0 };
    let exponent = i32::from((bits >> 3) & 0xf);
    let mantissa = u32::from(bits & 0x7);
    if exponent == 15 && mantissa == 7 {
        return f32::NAN;
    }
    if exponent == 0 {
        return sign * (mantissa as f32) * 2.0_f32.powi(-9);
    }
    sign * (1.0 + (mantissa as f32) / 8.0) * 2.0_f32.powi(exponent - 7)
}

/// Generates deterministic exact low-precision values that vary by both axes.
pub fn deterministic_matrix(
    format: LowPrecisionFormat,
    rows: usize,
    columns: usize,
    salt: usize,
) -> Vec<u8> {
    const VALUES: [f32; 5] = [-1.0, -0.5, 0.0, 0.5, 1.0];
    let mut matrix = vec![0; rows * columns];
    for row in 0..rows {
        for column in 0..columns {
            let index = (row * 3 + column * 2 + row * column + salt) % VALUES.len();
            matrix[row * columns + column] = format.encode_test_value(VALUES[index]);
        }
    }
    matrix
}

/// Generates a concatenated batch whose matrices use distinct deterministic salts.
pub fn deterministic_batched_matrix(
    format: LowPrecisionFormat,
    batches: usize,
    rows: usize,
    columns: usize,
    salt: usize,
) -> Vec<u8> {
    const VALUES: [f32; 5] = [-1.0, -0.5, 0.0, 0.5, 1.0];
    let mut values = Vec::with_capacity(batches * rows * columns);
    for batch in 0..batches {
        let mut matrix = deterministic_matrix(format, rows, columns, salt + 7 * batch);
        if matrix.len() > 1 {
            matrix[0] = format.encode_test_value(VALUES[batch % VALUES.len()]);
            matrix[1] = format.encode_test_value(VALUES[(batch / VALUES.len()) % VALUES.len()]);
        }
        values.extend(matrix);
    }
    values
}

/// Computes the fixed `16x16x128` GEMM without using a fe2o3 device operation.
pub fn gemm_reference(
    format: LowPrecisionFormat,
    lhs: &[u8],
    rhs: &[u8],
) -> Result<Vec<f32>, ReferenceShapeError> {
    require_length("lhs", lhs.len(), GEMM_M * GEMM_K)?;
    require_length("rhs", rhs.len(), GEMM_K * GEMM_N)?;
    let mut output = vec![0.0; GEMM_M * GEMM_N];
    for row in 0..GEMM_M {
        for column in 0..GEMM_N {
            let mut accumulator = 0.0;
            for depth in 0..GEMM_K {
                accumulator += format.decode(lhs[row * GEMM_K + depth])
                    * format.decode(rhs[depth * GEMM_N + column]);
            }
            output[row * GEMM_N + column] = accumulator;
        }
    }
    Ok(output)
}

/// Computes independent GEMMs for a contiguous batch of fixed-shape inputs.
pub fn batched_gemm_reference(
    format: LowPrecisionFormat,
    batches: usize,
    lhs: &[u8],
    rhs: &[u8],
) -> Result<Vec<f32>, ReferenceShapeError> {
    let lhs_elements = GEMM_M * GEMM_K;
    let rhs_elements = GEMM_K * GEMM_N;
    require_length("lhs", lhs.len(), batches * lhs_elements)?;
    require_length("rhs", rhs.len(), batches * rhs_elements)?;
    let mut output = Vec::with_capacity(batches * GEMM_M * GEMM_N);
    for batch in 0..batches {
        output.extend(gemm_reference(
            format,
            &lhs[batch * lhs_elements..(batch + 1) * lhs_elements],
            &rhs[batch * rhs_elements..(batch + 1) * rhs_elements],
        )?);
    }
    Ok(output)
}

/// Computes stable scaled dot-product attention without sharing kernel code.
pub fn attention_reference(
    format: LowPrecisionFormat,
    query: &[u8],
    key: &[u8],
    value: &[u8],
) -> Result<Vec<f32>, ReferenceShapeError> {
    require_length("query", query.len(), ATTENTION_TOKENS * GEMM_K)?;
    require_length("key", key.len(), ATTENTION_TOKENS * GEMM_K)?;
    require_length("value", value.len(), ATTENTION_TOKENS * VALUE_COLUMNS)?;

    let scale = 1.0 / (GEMM_K as f32).sqrt();
    let mut output = vec![0.0; ATTENTION_TOKENS * VALUE_COLUMNS];
    let mut scores = [0.0; ATTENTION_TOKENS];
    for query_row in 0..ATTENTION_TOKENS {
        let mut maximum = f32::NEG_INFINITY;
        for key_row in 0..ATTENTION_TOKENS {
            let mut score = 0.0;
            for depth in 0..GEMM_K {
                score += format.decode(query[query_row * GEMM_K + depth])
                    * format.decode(key[key_row * GEMM_K + depth]);
            }
            scores[key_row] = score * scale;
            maximum = maximum.max(scores[key_row]);
        }
        let denominator = scores
            .iter()
            .map(|score| (*score - maximum).exp())
            .sum::<f32>();
        for column in 0..VALUE_COLUMNS {
            let mut numerator = 0.0;
            for key_row in 0..ATTENTION_TOKENS {
                numerator += (scores[key_row] - maximum).exp()
                    * format.decode(value[key_row * VALUE_COLUMNS + column]);
            }
            output[query_row * VALUE_COLUMNS + column] = numerator / denominator;
        }
    }
    Ok(output)
}

/// Computes independent attention problems for a contiguous fixed-shape batch.
pub fn batched_attention_reference(
    format: LowPrecisionFormat,
    batches: usize,
    query: &[u8],
    key: &[u8],
    value: &[u8],
) -> Result<Vec<f32>, ReferenceShapeError> {
    let qk_elements = ATTENTION_TOKENS * GEMM_K;
    let value_elements = ATTENTION_TOKENS * VALUE_COLUMNS;
    require_length("query", query.len(), batches * qk_elements)?;
    require_length("key", key.len(), batches * qk_elements)?;
    require_length("value", value.len(), batches * value_elements)?;
    let mut output = Vec::with_capacity(batches * value_elements);
    for batch in 0..batches {
        output.extend(attention_reference(
            format,
            &query[batch * qk_elements..(batch + 1) * qk_elements],
            &key[batch * qk_elements..(batch + 1) * qk_elements],
            &value[batch * value_elements..(batch + 1) * value_elements],
        )?);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_encodings_decode_as_expected() {
        for (bits, expected) in [
            (0x0, 0.0),
            (0x1, 0.5),
            (0x2, 1.0),
            (0x3, 1.5),
            (0x7, 6.0),
            (0x9, -0.5),
            (0xf, -6.0),
        ] {
            assert_eq!(decode_fp4_e2m1(bits), expected);
        }
        for (bits, expected) in [
            (0x00, 0.0),
            (0x01, 2.0_f32.powi(-9)),
            (0x30, 0.5),
            (0x38, 1.0),
            (0xb8, -1.0),
            (0x7e, 448.0),
        ] {
            assert_eq!(decode_fp8_e4m3(bits), expected);
        }
        assert!(decode_fp8_e4m3(0x7f).is_nan());
    }

    #[test]
    fn batched_references_keep_nonuniform_problems_disjoint() {
        let lhs = deterministic_batched_matrix(LowPrecisionFormat::Fp4E2M1, 2, GEMM_M, GEMM_K, 1);
        let rhs = deterministic_batched_matrix(LowPrecisionFormat::Fp4E2M1, 2, GEMM_K, GEMM_N, 3);
        let output = batched_gemm_reference(LowPrecisionFormat::Fp4E2M1, 2, &lhs, &rhs).unwrap();
        assert_ne!(&output[..GEMM_M * GEMM_N], &output[GEMM_M * GEMM_N..]);

        let query = deterministic_batched_matrix(
            LowPrecisionFormat::Fp8E4M3,
            2,
            ATTENTION_TOKENS,
            GEMM_K,
            2,
        );
        let key = deterministic_batched_matrix(
            LowPrecisionFormat::Fp8E4M3,
            2,
            ATTENTION_TOKENS,
            GEMM_K,
            4,
        );
        let value = deterministic_batched_matrix(
            LowPrecisionFormat::Fp8E4M3,
            2,
            ATTENTION_TOKENS,
            VALUE_COLUMNS,
            6,
        );
        let output =
            batched_attention_reference(LowPrecisionFormat::Fp8E4M3, 2, &query, &key, &value)
                .unwrap();
        assert_ne!(
            &output[..ATTENTION_TOKENS * VALUE_COLUMNS],
            &output[ATTENTION_TOKENS * VALUE_COLUMNS..]
        );
    }

    #[test]
    fn deterministic_inputs_vary_across_rows_columns_and_formats() {
        let fp4 = deterministic_matrix(LowPrecisionFormat::Fp4E2M1, GEMM_M, GEMM_K, 1);
        let fp8 = deterministic_matrix(LowPrecisionFormat::Fp8E4M3, GEMM_M, GEMM_K, 1);
        assert_ne!(&fp4[0..GEMM_K], &fp4[GEMM_K..2 * GEMM_K]);
        assert!(fp4[0..GEMM_K].windows(2).any(|pair| pair[0] != pair[1]));
        assert_ne!(fp4, fp8);
    }

    #[test]
    fn gemm_reference_is_finite_nonuniform_and_shape_checked() {
        for format in [LowPrecisionFormat::Fp4E2M1, LowPrecisionFormat::Fp8E4M3] {
            let lhs = deterministic_matrix(format, GEMM_M, GEMM_K, 1);
            let rhs = deterministic_matrix(format, GEMM_K, GEMM_N, 3);
            let result = gemm_reference(format, &lhs, &rhs).unwrap();
            assert!(result.iter().all(|value| value.is_finite()));
            assert!(result.windows(2).any(|pair| pair[0] != pair[1]));
            let error = gemm_reference(format, &lhs[..lhs.len() - 1], &rhs).unwrap_err();
            assert_eq!(error.tensor(), "lhs");
            assert_eq!(error.expected(), GEMM_M * GEMM_K);
        }
    }

    #[test]
    fn attention_reference_is_finite_nonuniform_and_shape_checked() {
        for format in [LowPrecisionFormat::Fp4E2M1, LowPrecisionFormat::Fp8E4M3] {
            let query = deterministic_matrix(format, ATTENTION_TOKENS, GEMM_K, 0);
            let key = deterministic_matrix(format, ATTENTION_TOKENS, GEMM_K, 4);
            let value = deterministic_matrix(format, ATTENTION_TOKENS, VALUE_COLUMNS, 2);
            let result = attention_reference(format, &query, &key, &value).unwrap();
            assert!(result.iter().all(|element| element.is_finite()));
            assert!(result.windows(2).any(|pair| pair[0] != pair[1]));
            let error = attention_reference(format, &query, &key, &value[..255]).unwrap_err();
            assert_eq!(error.tensor(), "value");
            assert_eq!(error.actual(), 255);
        }
    }
}
