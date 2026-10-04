use fe2o3_host::__generated::semantic_witness_from_backend_v1;

fn main() {
    let bytes = [0_u8; 128];
    let _ = semantic_witness_from_backend_v1(bytes.as_ptr(), bytes.len(), [1; 32], [2; 32]);
}
