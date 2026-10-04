#[test]
fn release_contract_sealing_preserves_bytes_and_refuses_mutation() {
    let file = create_contract_file().unwrap();
    let bytes = vec![0x5a_u8; 32 * 1024 + 7];
    write_and_seal_contract(&file, &bytes).unwrap();
    assert_eq!(rustix::fs::fcntl_get_seals(&file).unwrap(), REQUIRED_SEALS);
    assert_eq!(file.metadata().unwrap().len(), bytes.len() as u64);
    let mut observed = vec![0_u8; bytes.len()];
    file.read_exact_at(&mut observed, 0).unwrap();
    assert_eq!(observed, bytes);
    assert!(file.write_at(b"x", 0).is_err());
    assert!(file.set_len(0).is_err());
    assert!(file.set_len(bytes.len() as u64 + 1).is_err());
    assert!(write_and_seal_contract(&file, b"replacement").is_err());
}
