use fe2o3_device_link_ffi::{cpu_oracle, evaluate_source_model_path};

fn main() {
    let input = [0, 1, 7, 31, u32::MAX];
    let expected = cpu_oracle(&input, input.len(), 0);
    let observed = evaluate_source_model_path(&input, input.len(), 0);
    assert_eq!(observed, expected);
    println!("CPU_SOURCE_MODEL bidirectional device FFI oracle: {observed:?}");
}
