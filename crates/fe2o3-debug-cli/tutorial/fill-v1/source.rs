fn fill(global_id: u64, out: &mut [u32]) {
    let index = global_id;
    let value = 7_u32;
    out[index as usize] = value;
}
