use fe2o3_device::{F32AccumulatorMatrix, Wave32, WaveLane};

fn load(matrix: &F32AccumulatorMatrix<'_>, lane: &WaveLane<Wave32>) {
    let _ = matrix.load_m16n16(lane, 0, 0);
}

fn main() {}
