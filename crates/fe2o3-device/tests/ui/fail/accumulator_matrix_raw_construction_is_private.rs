use core::marker::PhantomData;
use fe2o3_device::F32AccumulatorMatrix;

fn main() {
    let _: F32AccumulatorMatrix<'_> = F32AccumulatorMatrix {
        values: &[],
        offset: 0,
        rows: 0,
        columns: 0,
        stride: 0,
        _contract: PhantomData,
    };
}
