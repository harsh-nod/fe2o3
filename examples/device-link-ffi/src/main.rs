use fe2o3_device_link_ffi::{cpu_oracle, emulate_linked_device_path};

fn main() {
    let input = [0, 1, 7, 31, u32::MAX];
    let expected = cpu_oracle(&input);
    let observed = emulate_linked_device_path(&input);
    assert_eq!(observed, expected);
    println!("CPU_ONLY bidirectional device FFI oracle: {observed:?}");
}
