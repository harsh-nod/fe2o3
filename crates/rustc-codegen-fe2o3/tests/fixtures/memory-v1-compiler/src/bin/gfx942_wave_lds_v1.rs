use fe2o3_device::{Gfx942Collectives, kernel};

#[kernel(
    typed,
    namespace = "42190c5d1b9c267f7360f13c7265a546bc30ec9c280d3e5ec2d8c7b4f81eac5f"
)]
pub fn gfx942_wave_lds_v1(active_flag: u32, value: u32) {
    let context = Gfx942Collectives::current();
    let wave_sum = context.wave64_reduce_sum_active_u32(active_flag, value);
    let mut scratch = context.static_lds_u32x256();
    let workgroup_sum =
        context.workgroup256_reduce_sum_active_u32(&mut scratch, active_flag, value);
    let _ = wave_sum;
    let _ = workgroup_sum;
}

fn main() {}
