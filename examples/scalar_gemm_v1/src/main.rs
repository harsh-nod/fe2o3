mod kernel;

use std::error::Error;
use std::fmt;
use std::hint::black_box;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Shape {
    m: usize,
    n: usize,
    k: usize,
    a_len: usize,
    b_len: usize,
    c_len: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShapeError {
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
    fn checked(m: u32, n: u32, k: u32) -> Result<Self, ShapeError> {
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
}

fn ordered_product(left: f32, right: f32) -> f32 {
    black_box(black_box(left) * black_box(right))
}

fn ordered_sum(left: f32, right: f32) -> f32 {
    black_box(black_box(left) + black_box(right))
}

fn scalar_gemm_oracle(shape: Shape, a: &[f32], b: &[f32]) -> Vec<f32> {
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

fn main() -> Result<(), Box<dyn Error>> {
    let shape = Shape::checked(2, 3, 4)?;
    let a: Vec<f32> = (0..shape.a_len).map(|value| value as f32 - 2.0).collect();
    let b: Vec<f32> = (0..shape.b_len)
        .map(|value| value as f32 * 0.25 - 1.0)
        .collect();
    let expected = scalar_gemm_oracle(shape, &a, &b);
    println!("scalar GEMM V1 CPU oracle: {expected:?}");
    println!(
        "GPU execution remains fail-closed until an authenticated gfx942 COV6 artifact exists"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Shape, scalar_gemm_oracle};

    const KERNEL_SOURCE: &str = include_str!("kernel.rs");

    #[test]
    fn canonical_source_retains_the_exact_entry_and_ordered_recurrence() {
        for required in [
            "pub fn scalar_gemm_v1(",
            "if p < output_extent",
            "let row = p / (n as usize)",
            "let col = p % (n as usize)",
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
    fn oracle_covers_rectangular_and_zero_k_shapes() {
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
    fn oracle_preserves_separate_rounding_instead_of_fma() {
        let shape = Shape::checked(1, 1, 2).unwrap();
        let a = [1.0_f32, 1.0000001192092896_f32];
        let b = [1.0_f32, -1.0_f32];
        let actual = scalar_gemm_oracle(shape, &a, &b);
        assert_eq!(
            actual[0].to_bits(),
            (1.0_f32 + -1.0000001192092896_f32).to_bits()
        );
    }

    #[test]
    fn shape_uses_checked_extents() {
        let shape = Shape::checked(3, 5, 7).unwrap();
        assert_eq!((shape.a_len, shape.b_len, shape.c_len), (21, 35, 15));
        assert!(Shape::checked(u32::MAX, u32::MAX, u32::MAX).is_err());
    }
}
