use fe2o3_device::Gfx942TiledGemmWave64V1;

fn main() {
    let wave = Gfx942TiledGemmWave64V1::from_compiler(16);
    let staged = wave.stage([0; 4], [0; 4]);
    let _ = staged.multiply_accumulate();
}
