#![allow(dead_code)]

pub const BASE_SEED: u64 = 0x6a09_e667_f3bc_c909;
pub const LENGTH_SALT: u64 = 0x9e37_79b9_7f4a_7c15;
pub const LENGTHS: [usize; 6] = [0, 1, 31, 255, 256, 257];
pub const LEFT_F32: u32 = 0x4f12_3456;
pub const RIGHT_F32: u32 = 0xcf23_4567;
pub const POISON_F32: u32 = 0x7fc0_d1ff;

pub fn case_seed(kernel: u64, length: usize) -> u64 {
    BASE_SEED ^ (kernel << 56) ^ (length as u64).wrapping_mul(LENGTH_SALT)
}

pub fn mix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

pub fn sample_i32(seed: u64, index: usize, channel: u64) -> i32 {
    let value = mix64(
        seed ^ (index as u64).wrapping_mul(LENGTH_SALT)
            ^ channel.wrapping_mul(0xd1b5_4a32_d192_ed03),
    );
    (value % 2001) as i32 - 1000
}

pub fn sample_f32(seed: u64, index: usize) -> f32 {
    match index {
        0 => f32::from_bits(0x7fc1_2345),
        1 => f32::INFINITY,
        _ => sample_i32(seed, index, 7) as f32 / 32.0,
    }
}

pub fn sample_vec_f32(seed: u64, index: usize, channel: u64) -> f32 {
    match (index, channel) {
        (0, 2) => f32::from_bits(0x7fc1_2345),
        (1, 2) => f32::INFINITY,
        _ => sample_i32(seed, index, channel) as f32 / 32.0,
    }
}

pub fn emit_bits32(kernel: &str, seed: u64, output: &[f32]) {
    print!(
        "FE2O3_DIFF_RESULT_V1\t{kernel}\tbits32\t{seed:016x}\t{}\t{:08x}\t{:08x}\t",
        output.len() - 2,
        output[0].to_bits(),
        output[output.len() - 1].to_bits()
    );
    for value in &output[1..output.len() - 1] {
        print!("{:08x}", value.to_bits());
    }
    println!();
}

pub fn emit_f32(kernel: &str, seed: u64, output: &[f32]) {
    print!(
        "FE2O3_DIFF_RESULT_V1\t{kernel}\tf32\t{seed:016x}\t{}\t{:08x}\t{:08x}\t",
        output.len() - 2,
        output[0].to_bits(),
        output[output.len() - 1].to_bits()
    );
    for value in &output[1..output.len() - 1] {
        print!("{:08x}", value.to_bits());
    }
    println!();
}
