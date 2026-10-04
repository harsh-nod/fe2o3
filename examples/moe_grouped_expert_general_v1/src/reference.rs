#![forbid(unsafe_code)]

//! Sequential safe-Rust reference for one dynamic routed expert launch.

use fe2o3_device::Bf16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReferenceLayoutV1 {
    pub rows_padded: u32,
    pub output_columns: u32,
    pub reduction: u32,
    pub token_stride: u32,
    pub weight_stride: u32,
    pub expert_weight_stride: u32,
    pub bias_stride: u32,
    pub output_stride: u32,
    pub expert: u32,
    pub experts: u32,
}

pub fn evaluate_reference_v1(
    tokens: &[u16],
    weights: &[u16],
    gates: &[f32],
    bias: &[f32],
    initial_output: &[f32],
    layout: ReferenceLayoutV1,
) -> Result<Vec<f32>, &'static str> {
    if layout.expert >= layout.experts {
        return Err("expert identity is outside the declared expert domain");
    }
    if layout.token_stride < layout.reduction
        || layout.weight_stride < layout.output_columns
        || layout.bias_stride < layout.output_columns
        || layout.output_stride < layout.output_columns
    {
        return Err("a physical stride is smaller than its logical extent");
    }
    let checked_len = |major: u32, stride: u32| {
        usize::try_from(major)
            .ok()
            .and_then(|major| major.checked_mul(stride as usize))
    };
    let token_len =
        checked_len(layout.rows_padded, layout.token_stride).ok_or("token extent overflow")?;
    let weight_len =
        checked_len(layout.experts, layout.expert_weight_stride).ok_or("weight extent overflow")?;
    let bias_len = checked_len(layout.experts, layout.bias_stride).ok_or("bias extent overflow")?;
    let output_len =
        checked_len(layout.rows_padded, layout.output_stride).ok_or("output extent overflow")?;
    if tokens.len() < token_len
        || weights.len() < weight_len
        || gates.len() < layout.rows_padded as usize
        || bias.len() < bias_len
        || initial_output.len() < output_len
    {
        return Err("reference input is shorter than its declared physical extent");
    }

    let mut output = initial_output.to_vec();
    for row in 0..layout.rows_padded as usize {
        for column in 0..layout.output_columns as usize {
            let mut projection = 0.0_f32;
            for depth in 0..layout.reduction as usize {
                let activation =
                    Bf16::from_bits(tokens[row * layout.token_stride as usize + depth]).to_f32();
                let weight = Bf16::from_bits(
                    weights[layout.expert as usize * layout.expert_weight_stride as usize
                        + depth * layout.weight_stride as usize
                        + column],
                )
                .to_f32();
                projection += activation * weight;
            }
            projection += bias[layout.expert as usize * layout.bias_stride as usize + column];
            output[row * layout.output_stride as usize + column] = gates[row] * projection;
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_gate_zeroes_a_padded_route_and_preserves_columns() {
        let bits = |value| Bf16::from_f32(value).to_bits();
        let tokens = [
            bits(2.0),
            bits(3.0),
            bits(9.0),
            bits(4.0),
            bits(5.0),
            bits(8.0),
        ];
        let weights = [bits(7.0), bits(11.0), bits(13.0), bits(17.0)];
        let output = evaluate_reference_v1(
            &tokens,
            &weights,
            &[0.5, 0.0],
            &[1.0, 2.0],
            &[-7.0; 6],
            ReferenceLayoutV1 {
                rows_padded: 2,
                output_columns: 2,
                reduction: 2,
                token_stride: 3,
                weight_stride: 2,
                expert_weight_stride: 4,
                bias_stride: 2,
                output_stride: 3,
                expert: 0,
                experts: 1,
            },
        )
        .unwrap();
        assert_eq!(output, [27.0, 37.5, -7.0, 0.0, 0.0, -7.0]);
    }
}
