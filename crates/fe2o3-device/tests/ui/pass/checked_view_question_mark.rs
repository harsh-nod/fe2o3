use fe2o3_device::{Bf16MfmaAMatrix, KernelResult, StridedReadView2D};

fn checked_matrix(bits: &[u16]) -> KernelResult<Bf16MfmaAMatrix<'_>> {
    Ok(Bf16MfmaAMatrix::row_major(bits, 0, 2, 2, 2)?)
}

fn checked_read_view(values: &[f32]) -> KernelResult<StridedReadView2D<'_, f32>> {
    Ok(StridedReadView2D::from_shared_slice(values, 0, 2, 2, 2)?)
}

fn main() {
    assert!(checked_matrix(&[0; 4]).is_ok());
    assert!(checked_read_view(&[0.0; 4]).is_ok());
}
