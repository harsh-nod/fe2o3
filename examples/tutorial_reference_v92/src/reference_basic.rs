use crate::protocol::{
    Argument, Output, Request, buffer, float_scalar, floats, integers, scalar, unsigned,
};
use crate::references::{READ, WRITE, input, output};

pub const KERNELS: [&str; 17] = [
    "fill",
    "vecadd",
    "scalar_gemm_v1",
    "moe_top2_route_f32_t8_e4_k2_c4_v1",
    "wave64_collectives_v1",
    "lds_publish_read_reduce_i32_v1",
    "row_affine_sum_u32_v1",
    "barrier_before_access",
    "aggregate_pair_struct",
    "aggregate_pair_tuple",
    "aggregate_pair_array",
    "aggregate_zst",
    "aggregate_nested",
    "wave_reduce_f32",
    "workgroup_reduce_u32",
    "workgroup_reduce_i32",
    "workgroup_reduce_f32",
];
pub fn contains(k: &str) -> bool {
    KERNELS.contains(&k)
}
fn scalar64(ty: &str, n: u64) -> Argument {
    Argument::Scalar {
        ty: ty.to_owned(),
        bits: format!("0x{n:016x}"),
    }
}
fn wide_output() -> Argument {
    buffer("u64", WRITE, &[0x5a; 64 * 8])
}
pub fn generate(k: &str) -> Result<Request, String> {
    let mut groups = 1;
    let args = match k {
        "fill" => vec![output(65)],
        "vecadd" => vec![input(64), input(64), output(64)],
        "scalar_gemm_v1" => vec![
            input(15),
            input(20),
            output(12),
            scalar(3),
            scalar(4),
            scalar(5),
        ],
        "moe_top2_route_f32_t8_e4_k2_c4_v1" => {
            let mut v = vec![input(32)];
            for n in [16, 4, 4, 5, 16, 16, 16] {
                v.push(unsigned(&vec![0xdeadbeef; n], WRITE));
            }
            v
        }
        "wave64_collectives_v1" => vec![
            floats(
                &(0..64).map(|i| (i % 11) as f32 - 5.0).collect::<Vec<_>>(),
                READ,
            ),
            scalar64("u64", 0x5a5a123489abcdef),
            output(64),
            output(64),
            output(64),
        ],
        "lds_publish_read_reduce_i32_v1" => vec![
            integers(&(0..64).map(|i| i - 32).collect::<Vec<_>>(), READ),
            integers(&[-999], WRITE),
        ],
        "row_affine_sum_u32_v1" => {
            groups = 3;
            vec![
                unsigned(
                    &(0..3 * 133 + 3)
                        .map(|i| (i as u32).wrapping_mul(0x1234567))
                        .collect::<Vec<_>>(),
                    READ,
                ),
                scalar64("index", 3),
                scalar64("index", 3),
                scalar64("index", 128),
                scalar64("index", 133),
                scalar(u32::MAX),
                scalar(17),
                unsigned(&[0xdeadbeef; 5], WRITE),
            ]
        }
        "barrier_before_access" => vec![output(128)],
        "aggregate_pair_struct" | "aggregate_pair_tuple" => vec![
            scalar(7),
            scalar64("u64", 0x123456789abc),
            wide_output(),
            scalar64("u64", 91),
        ],
        "aggregate_pair_array" => vec![
            scalar64("u64", 7),
            scalar64("u64", 0x123456789abc),
            wide_output(),
            scalar64("u64", 91),
        ],
        "aggregate_zst" => vec![wide_output(), scalar64("u64", 91)],
        "aggregate_nested" => vec![
            scalar(7),
            scalar64("u64", 0x123456789abc),
            Argument::Scalar {
                ty: "u16".to_owned(),
                bits: "0x0123".to_owned(),
            },
            Argument::Scalar {
                ty: "u16".to_owned(),
                bits: "0x0456".to_owned(),
            },
            wide_output(),
            scalar64("u64", 91),
        ],
        "wave_reduce_f32" | "workgroup_reduce_f32" => vec![float_scalar(0.5), output(64)],
        "workgroup_reduce_u32" => vec![scalar(0x08000001), unsigned(&[0xdeadbeef; 64], WRITE)],
        "workgroup_reduce_i32" => vec![
            Argument::Scalar {
                ty: "i32".to_owned(),
                bits: "0xfffffff9".to_owned(),
            },
            integers(&[-999; 64], WRITE),
        ],
        _ => return Err("unknown basic corpus".to_owned()),
    };
    Ok(Request::new(k, 64, groups, args))
}
pub fn evaluate(r: &Request) -> Result<Vec<Output>, String> {
    r.corpus_shape(&generate(&r.kernel)?)?;
    match r.kernel.as_str() {
        "fill" => {
            let mut v = r.f32s(0)?;
            for (i, value) in v.iter_mut().take(r.grid[0] as usize).enumerate() {
                fe2o3_fill::fill_reference(i, value);
            }
            Ok(vec![Output::f32s(0, &v, 0.0)])
        }
        "vecadd" => Ok(vec![Output::f32s(
            2,
            &r.f32s(0)?
                .iter()
                .zip(r.f32s(1)?)
                .map(|(a, b)| a + b)
                .collect::<Vec<_>>(),
            0.0,
        )]),
        "scalar_gemm_v1" => {
            use fe2o3_scalar_gemm_v1::harness::{Shape, scalar_gemm_oracle};
            let shape = Shape::checked(
                r.scalar(3, "u32")?,
                r.scalar(4, "u32")?,
                r.scalar(5, "u32")?,
            )
            .map_err(|e| format!("{e:?}"))?;
            let [m, n, k] = shape.dimensions();
            let a = r.f32s(0)?;
            let b = r.f32s(1)?;
            if u64::from(m) * u64::from(k) != a.len() as u64
                || u64::from(k) * u64::from(n) != b.len() as u64
                || u64::from(m) * u64::from(n) != 12
            {
                return Err("GEMM request extent".to_owned());
            }
            Ok(vec![Output::f32s(
                2,
                &scalar_gemm_oracle(shape, &a, &b),
                0.0,
            )])
        }
        "moe_top2_route_f32_t8_e4_k2_c4_v1" => {
            use fe2o3_moe_top2_v1::oracle::{RoutingOutputsV1, moe_top2_oracle_v1};
            let mut v = RoutingOutputsV1::filled(0);
            moe_top2_oracle_v1(&r.f32s(0)?, &mut v).map_err(|e| format!("{e:?}"))?;
            Ok(vec![
                Output::u32s(1, &v.top2_experts),
                Output::u32s(2, &v.requested_counts),
                Output::u32s(3, &v.admitted_counts),
                Output::u32s(4, &v.expert_offsets),
                Output::u32s(5, &v.route_slots),
                Output::u32s(6, &v.permutation),
                Output::u32s(7, &v.inverse),
            ])
        }
        "wave64_collectives_v1" => {
            let (mut reduce, mut inclusive, mut exclusive) = ([0.0; 64], [0.0; 64], [0.0; 64]);
            fe2o3_wave64_collectives_v1::oracle::wave64_collectives_oracle_v1(
                &r.f32s(0)?,
                r.scalar64(1, "u64")?,
                &mut reduce,
                &mut inclusive,
                &mut exclusive,
            )
            .map_err(|e| format!("{e:?}"))?;
            Ok(vec![
                Output::f32s(2, &reduce, 0.0),
                Output::f32s(3, &inclusive, 0.0),
                Output::f32s(4, &exclusive, 0.0),
            ])
        }
        "lds_publish_read_reduce_i32_v1" => {
            use fe2o3_workgroup_sync_v1::contract::{
                canonical_reduction_trace_v1, lds_reduction_oracle_v1,
            };
            let mut out = [0];
            lds_reduction_oracle_v1(&r.i32s(0)?, 0, &canonical_reduction_trace_v1(0), &mut out)
                .map_err(|e| format!("{e:?}"))?;
            Ok(vec![Output::i32s(1, &out)])
        }
        "row_affine_sum_u32_v1" => {
            use fe2o3_workgroup_sync_v1::row_affine_oracle::{
                RowAffineConfigV1, row_affine_oracle_v1,
            };
            let bounded = |i| {
                r.scalar64(i, "index").and_then(|v| {
                    if v <= 4096 {
                        Ok(v as usize)
                    } else {
                        Err("row extent bound".to_owned())
                    }
                })
            };
            let mut output = r.u32s(7)?;
            row_affine_oracle_v1(
                &r.u32s(0)?,
                RowAffineConfigV1 {
                    offset: bounded(1)?,
                    rows: bounded(2)?,
                    columns: bounded(3)?,
                    row_stride: bounded(4)?,
                    scale: r.scalar(5, "u32")?,
                    bias: r.scalar(6, "u32")?,
                    workgroups: 3,
                },
                &mut output,
            )
            .map_err(|e| format!("{e:?}"))?;
            Ok(vec![Output::u32s(7, &output)])
        }
        "barrier_before_access" => {
            let mut v = r.f32s(0)?;
            for i in 0..64 {
                v[i * 2 + 1] = 1.0;
            }
            Ok(vec![Output::f32s(0, &v, 0.0)])
        }
        "aggregate_pair_struct" | "aggregate_pair_tuple" | "aggregate_pair_array" => {
            Ok(vec![Output::u64s(2, &[r.scalar64(1, "u64")?; 64])])
        }
        "aggregate_nested" => Ok(vec![Output::u64s(4, &[r.scalar64(1, "u64")?; 64])]),
        "aggregate_zst" => Ok(vec![Output::u64s(0, &[r.scalar64(1, "u64")?; 64])]),
        "wave_reduce_f32" | "workgroup_reduce_f32" => {
            let value = f32::from_bits(r.scalar(0, "f32")?);
            if !value.is_finite() {
                return Err("nonfinite reduction".to_owned());
            }
            Ok(vec![Output::f32s(1, &[value * 64.0; 64], 0.0)])
        }
        "workgroup_reduce_u32" => Ok(vec![Output::u32s(
            1,
            &[r.scalar(0, "u32")?.wrapping_mul(64); 64],
        )]),
        "workgroup_reduce_i32" => Ok(vec![Output::i32s(
            1,
            &[(r.scalar(0, "i32")? as i32).wrapping_mul(64); 64],
        )]),
        _ => Err("unknown basic reference".to_owned()),
    }
}
