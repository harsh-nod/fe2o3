use fe2o3_scalar_gemm_v1::harness::{Shape, scalar_gemm_inputs, scalar_gemm_oracle};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let shape = Shape::checked(2, 3, 4)?;
    let (a, b) = scalar_gemm_inputs(shape);
    let expected = scalar_gemm_oracle(shape, &a, &b);
    println!("scalar GEMM V1 CPU oracle: {expected:?}");
    println!("GPU execution requires an authenticated Scalar GEMM V1 Worker V2 HSA capability");
    Ok(())
}
