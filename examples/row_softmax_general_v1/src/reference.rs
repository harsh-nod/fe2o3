#![forbid(unsafe_code)]

//! Sequential safe-Rust reference for a dynamic strided row transform.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReferenceLayoutV1 {
    pub rows: u32,
    pub columns: u32,
    pub input_stride: u32,
    pub output_stride: u32,
}

pub fn evaluate_reference_v1(
    input: &[f32],
    initial_output: &[f32],
    layout: ReferenceLayoutV1,
) -> Result<Vec<f32>, &'static str> {
    if layout.columns == 0 {
        return Err("the reference requires at least one logical column");
    }
    if layout.input_stride < layout.columns || layout.output_stride < layout.columns {
        return Err("a physical row stride is smaller than the logical row width");
    }
    let input_len = usize::try_from(layout.rows)
        .ok()
        .and_then(|rows| rows.checked_mul(layout.input_stride as usize))
        .ok_or("input extent overflow")?;
    let output_len = usize::try_from(layout.rows)
        .ok()
        .and_then(|rows| rows.checked_mul(layout.output_stride as usize))
        .ok_or("output extent overflow")?;
    if input.len() < input_len || initial_output.len() < output_len {
        return Err("reference input is shorter than its declared strided extent");
    }

    let mut output = initial_output.to_vec();
    for row in 0..layout.rows as usize {
        let values = &input[row * layout.input_stride as usize
            ..row * layout.input_stride as usize + layout.columns as usize];
        let maximum = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let denominator = values
            .iter()
            .map(|value| (*value - maximum).exp())
            .sum::<f32>();
        for (column, value) in values.iter().copied().enumerate() {
            output[row * layout.output_stride as usize + column] =
                (value - maximum).exp() / denominator;
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_normalized_and_padding_is_preserved() {
        let input = [0.0, 0.0, -9.0, 1.0, 1.0, -8.0];
        let initial = [-7.0; 8];
        let output = evaluate_reference_v1(
            &input,
            &initial,
            ReferenceLayoutV1 {
                rows: 2,
                columns: 2,
                input_stride: 3,
                output_stride: 4,
            },
        )
        .unwrap();
        assert_eq!(&output[..4], &[0.5, 0.5, -7.0, -7.0]);
        assert_eq!(&output[4..], &[0.5, 0.5, -7.0, -7.0]);
    }
}
