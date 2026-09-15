#[test]
fn exact_original_borrow_coordinates_are_not_blockwide_use_coordinates() {
    let observation = Observation {
        body: [0xab; 32], root: 17, block: 16, statement: Some(0), local: 45,
        projections: 0, source_block: Some((5, 24, 0)), source_local: Some((5, 24, 0)),
        role: Role::Subgroup, owner_type: Some(12), promoted: false,
    };
    let mut output = Vec::new();
    write_observation(&mut output, &observation).unwrap();
    assert_eq!(String::from_utf8(output).unwrap(), format!(
        "capability-ssa-absent-borrow body={} root=17 block=16 statement=Some(0) local=45 projections=0 source_block=Some((5, 24, 0)) source_local=Some((5, 24, 0)) role=Subgroup owner_type=Some(12) promoted=false query=original-borrow-place error=NoPromotedUse diagnostic_only=true\n",
        "ab".repeat(32)));
}

#[test]
fn bounded_metadata_output_and_writer_failure_cannot_supply_a_value() {
    let observation = Observation {
        body: [255; 32], root: u32::MAX, block: u32::MAX, statement: Some(u32::MAX),
        local: u32::MAX, projections: usize::MAX,
        role: Role::Partition, owner_type: Some(u32::MAX), promoted: true,
        source_block: Some((u32::MAX, u32::MAX, u32::MAX)),
        source_local: Some((u32::MAX, u32::MAX, u32::MAX)),
    };
    let mut output = Vec::new();
    write_observation(&mut output, &observation).unwrap();
    assert!(output.len() < 512);
    let mut full = &mut [0u8; 0][..];
    assert_eq!(write_observation(&mut full, &observation).unwrap_err().kind(), io::ErrorKind::WriteZero);
}
