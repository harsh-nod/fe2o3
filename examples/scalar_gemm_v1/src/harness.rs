use std::error::Error;
use std::fmt;
use std::hint::black_box;

pub const SCALAR_GEMM_WORKGROUP_X: u32 = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Shape {
    pub m: usize,
    pub n: usize,
    pub k: usize,
    pub a_len: usize,
    pub b_len: usize,
    pub c_len: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShapeError {
    ElementCountOverflow(&'static str),
    ByteCountOverflow(&'static str),
    HostLengthOverflow(&'static str),
}

impl fmt::Display for ShapeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ElementCountOverflow(field) => {
                write!(formatter, "{field} element count overflows u64")
            }
            Self::ByteCountOverflow(field) => write!(formatter, "{field} byte count overflows u64"),
            Self::HostLengthOverflow(field) => {
                write!(formatter, "{field} element count does not fit usize")
            }
        }
    }
}

impl Error for ShapeError {}

impl Shape {
    pub fn checked(m: u32, n: u32, k: u32) -> Result<Self, ShapeError> {
        fn extent(field: &'static str, rows: u32, columns: u32) -> Result<usize, ShapeError> {
            let elements = u64::from(rows)
                .checked_mul(u64::from(columns))
                .ok_or(ShapeError::ElementCountOverflow(field))?;
            elements
                .checked_mul(size_of::<f32>() as u64)
                .ok_or(ShapeError::ByteCountOverflow(field))?;
            usize::try_from(elements).map_err(|_| ShapeError::HostLengthOverflow(field))
        }

        Ok(Self {
            m: m as usize,
            n: n as usize,
            k: k as usize,
            a_len: extent("A", m, k)?,
            b_len: extent("B", k, n)?,
            c_len: extent("C", m, n)?,
        })
    }

    pub fn dimensions(self) -> [u32; 3] {
        [self.m as u32, self.n as u32, self.k as u32]
    }

    pub fn expected_groups(self) -> Result<Option<u32>, ShapeError> {
        if self.c_len == 0 {
            return Ok(None);
        }
        let groups = self.c_len.div_ceil(SCALAR_GEMM_WORKGROUP_X as usize);
        let groups = u32::try_from(groups).map_err(|_| ShapeError::HostLengthOverflow("grid"))?;
        groups
            .checked_mul(SCALAR_GEMM_WORKGROUP_X)
            .ok_or(ShapeError::ElementCountOverflow("rounded grid"))?;
        Ok(Some(groups))
    }
}

fn ordered_product(left: f32, right: f32) -> f32 {
    black_box(black_box(left) * black_box(right))
}

fn ordered_sum(left: f32, right: f32) -> f32 {
    black_box(black_box(left) + black_box(right))
}

pub fn scalar_gemm_oracle(shape: Shape, a: &[f32], b: &[f32]) -> Vec<f32> {
    assert_eq!(a.len(), shape.a_len);
    assert_eq!(b.len(), shape.b_len);
    let mut c = vec![f32::from_bits(0); shape.c_len];
    for row in 0..shape.m {
        for col in 0..shape.n {
            let mut accumulator = f32::from_bits(0);
            for t in 0..shape.k {
                let product = ordered_product(a[row * shape.k + t], b[t * shape.n + col]);
                accumulator = ordered_sum(accumulator, product);
            }
            c[row * shape.n + col] = accumulator;
        }
    }
    c
}

pub fn scalar_gemm_inputs(shape: Shape) -> (Vec<f32>, Vec<f32>) {
    let a = (0..shape.a_len)
        .map(|index| ((index.wrapping_mul(17) % 29) as f32 - 14.0) * 0.125)
        .collect();
    let b = (0..shape.b_len)
        .map(|index| ((index.wrapping_mul(11) % 31) as f32 - 15.0) * 0.0625)
        .collect();
    (a, b)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputProfile {
    Deterministic,
    FmaDistinguishing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HardwareCase {
    pub name: &'static str,
    pub m: u32,
    pub n: u32,
    pub k: u32,
    pub input_profile: InputProfile,
}

impl HardwareCase {
    pub fn shape(self) -> Result<Shape, ShapeError> {
        Shape::checked(self.m, self.n, self.k)
    }

    pub fn inputs(self, shape: Shape) -> (Vec<f32>, Vec<f32>) {
        match self.input_profile {
            InputProfile::Deterministic => scalar_gemm_inputs(shape),
            InputProfile::FmaDistinguishing => {
                assert_eq!(shape.dimensions(), [1, 1, 2]);
                (
                    vec![-1.0, 1.0 + f32::EPSILON],
                    vec![1.0, 1.0 - f32::EPSILON],
                )
            }
        }
    }
}

pub const HARDWARE_CASES: &[HardwareCase] = &[
    HardwareCase {
        name: "zero-m",
        m: 0,
        n: 257,
        k: 3,
        input_profile: InputProfile::Deterministic,
    },
    HardwareCase {
        name: "zero-n",
        m: 7,
        n: 0,
        k: 3,
        input_profile: InputProfile::Deterministic,
    },
    HardwareCase {
        name: "zero-k",
        m: 3,
        n: 5,
        k: 0,
        input_profile: InputProfile::Deterministic,
    },
    HardwareCase {
        name: "one-output",
        m: 1,
        n: 1,
        k: 1,
        input_profile: InputProfile::Deterministic,
    },
    HardwareCase {
        name: "wg-minus-one",
        m: 1,
        n: 255,
        k: 3,
        input_profile: InputProfile::Deterministic,
    },
    HardwareCase {
        name: "one-wg",
        m: 1,
        n: 256,
        k: 3,
        input_profile: InputProfile::Deterministic,
    },
    HardwareCase {
        name: "wg-plus-one",
        m: 1,
        n: 257,
        k: 3,
        input_profile: InputProfile::Deterministic,
    },
    HardwareCase {
        name: "rectangular",
        m: 7,
        n: 5,
        k: 9,
        input_profile: InputProfile::Deterministic,
    },
    HardwareCase {
        name: "fma-distinguishing",
        m: 1,
        n: 1,
        k: 2,
        input_profile: InputProfile::FmaDistinguishing,
    },
];

#[cfg(test)]
mod tests {
    use super::{
        HARDWARE_CASES, HardwareCase, InputProfile, SCALAR_GEMM_WORKGROUP_X, Shape,
        scalar_gemm_oracle,
    };

    const KERNEL_SOURCE: &str = include_str!("kernel.rs");

    #[test]
    fn canonical_source_retains_the_exact_entry_and_ordered_recurrence() {
        for required in [
            "pub fn scalar_gemm_v1(",
            "let n_index = n as usize",
            "if n_index != 0",
            "if p < output_extent",
            "let row = (p / n_index) as u32",
            "let col = (p % n_index) as u32",
            "while t < k",
            "let product = a[a_index] * b[b_index]",
            "accumulator = accumulator + product",
            "c.get_mut(index)",
        ] {
            assert!(KERNEL_SOURCE.contains(required), "missing `{required}`");
        }
        for forbidden in ["mul_add", "unsafe", "FE2O3_HSACO_DIR", "load_module"] {
            assert!(!KERNEL_SOURCE.contains(forbidden), "found `{forbidden}`");
        }
    }

    #[test]
    fn boundary_matrix_covers_no_dispatch_zero_k_and_wg_edges() {
        let groups = HARDWARE_CASES
            .iter()
            .map(|case| (case.name, case.shape().unwrap().expected_groups().unwrap()))
            .collect::<Vec<_>>();
        assert!(groups.contains(&("zero-m", None)));
        assert!(groups.contains(&("zero-n", None)));
        assert!(groups.contains(&("zero-k", Some(1))));
        assert!(groups.contains(&("wg-minus-one", Some(1))));
        assert!(groups.contains(&("one-wg", Some(1))));
        assert!(groups.contains(&("wg-plus-one", Some(2))));
        assert_eq!(SCALAR_GEMM_WORKGROUP_X, 256);
    }

    #[test]
    fn oracle_covers_rectangular_zero_k_and_positive_zero() {
        let shape = Shape::checked(2, 3, 2).unwrap();
        let actual = scalar_gemm_oracle(
            shape,
            &[1.0, 2.0, 3.0, 4.0],
            &[5.0, 6.0, 7.0, 8.0, 9.0, 10.0],
        );
        assert_eq!(actual, [21.0, 24.0, 27.0, 47.0, 54.0, 61.0]);

        let zero_k = Shape::checked(2, 3, 0).unwrap();
        let actual = scalar_gemm_oracle(zero_k, &[], &[]);
        assert_eq!(actual.len(), 6);
        assert!(actual.into_iter().all(|value| value.to_bits() == 0));
    }

    #[test]
    fn fma_distinguishing_case_requires_separate_rounding() {
        let case = HardwareCase {
            name: "fma",
            m: 1,
            n: 1,
            k: 2,
            input_profile: InputProfile::FmaDistinguishing,
        };
        let shape = case.shape().unwrap();
        let (a, b) = case.inputs(shape);
        let sequential = scalar_gemm_oracle(shape, &a, &b)[0];
        let contracted = (1.0 + f32::EPSILON).mul_add(1.0 - f32::EPSILON, -1.0);
        assert_eq!(sequential.to_bits(), 0.0_f32.to_bits());
        assert_ne!(sequential.to_bits(), contracted.to_bits());
    }

    #[test]
    fn shape_uses_checked_extents() {
        let shape = Shape::checked(3, 5, 7).unwrap();
        assert_eq!((shape.a_len, shape.b_len, shape.c_len), (21, 35, 15));
        assert!(Shape::checked(u32::MAX, u32::MAX, u32::MAX).is_err());
    }
}
