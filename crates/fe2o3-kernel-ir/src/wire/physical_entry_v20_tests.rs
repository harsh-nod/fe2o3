//! Fixed-width payload controls only; no executable/source authority.
use super::*;
use crate::gfx942_physical_entry_fixture_v20_tests as fixture;
fn declaration() -> Declaration {
    let module = fixture::module(false);
    let crate::OperationKind::Gfx942PhysicalEntryDeclaration(value) =
        module.functions[0].body.as_ref().unwrap().blocks[0].operations[0].kind
    else {
        panic!("declaration")
    };
    value
}
fn step() -> Step {
    let module = fixture::module(false);
    let crate::OperationKind::Gfx942PhysicalEntryStep(value) =
        module.functions[0].body.as_ref().unwrap().blocks[0].operations[1].kind
    else {
        panic!("step")
    };
    value
}
fn declaration_bytes() -> Vec<u8> {
    let mut writer = Writer::new(KERNEL_IR_VERSION_V20, None);
    encode_declaration(&mut writer, &declaration()).unwrap();
    writer.bytes
}
fn step_bytes() -> Vec<u8> {
    let mut writer = Writer::new(KERNEL_IR_VERSION_V20, None);
    encode_step(&mut writer, &step()).unwrap();
    writer.bytes
}
fn decode_d(bytes: &[u8]) -> Result<Declaration, KernelIrDecodeError> {
    let mut reader = Reader::new(bytes, None);
    reader.version = KERNEL_IR_VERSION_V20;
    let result = decode_declaration(&mut reader)?;
    if !reader.is_finished() {
        return Err(KernelIrDecodeError::TrailingBytes);
    }
    Ok(result)
}
fn decode_s(bytes: &[u8]) -> Result<Step, KernelIrDecodeError> {
    let mut reader = Reader::new(bytes, None);
    reader.version = KERNEL_IR_VERSION_V20;
    let result = decode_step(&mut reader)?;
    if !reader.is_finished() {
        return Err(KernelIrDecodeError::TrailingBytes);
    }
    Ok(result)
}
#[test]
fn exact_756_and_85_byte_payloads_retain_every_source_axis() {
    let d = declaration_bytes();
    let s = step_bytes();
    assert_eq!(d.len(), 756);
    assert_eq!(s.len(), 85);
    assert_eq!(decode_d(&d), Ok(declaration()));
    assert_eq!(decode_s(&s), Ok(step()));
    assert_eq!(d[0], 0);
    assert_eq!(&d[1..161], &[1; 160]);
    assert_eq!(&d[161..193], &[2; 32]);
    assert_eq!(&d[193..225], &[3; 32]);
    assert_eq!(&d[225..257], &[4; 32]);
    assert_eq!(&d[257..289], &[5; 32]);
    assert_eq!(&d[334..338], &0_u32.to_le_bytes());
    assert_eq!(&d[350..354], &4_u32.to_le_bytes());
    assert_eq!(&d[354..356], &[1, 21]);
    assert_eq!(&d[356..368], &[64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]);
    assert_eq!(&d[368..380], &[2, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]);
    assert_eq!(&d[474..], &[0; 282]); // inactive block rows are all zero
    assert_eq!(s[46], 0);
    assert_eq!(&s[47..55], &step().instruction.descriptor());
    assert_eq!(&s[65..85], &[0; 20]); // four absent operands, exact zero padding
}
#[test]
fn every_truncation_extra_byte_revision_and_presence_tag_refuses() {
    let d = declaration_bytes();
    let s = step_bytes();
    for end in 0..d.len() {
        assert!(decode_d(&d[..end]).is_err());
    }
    for end in 0..s.len() {
        assert!(decode_s(&s[..end]).is_err());
    }
    assert!(decode_d(&[d.clone(), vec![0]].concat()).is_err());
    assert!(decode_s(&[s.clone(), vec![0]].concat()).is_err());
    for tag in 1..=u8::MAX {
        let mut changed = d.clone();
        changed[0] = tag;
        assert!(decode_d(&changed).is_err());
        let mut changed = s.clone();
        changed[0] = tag;
        assert!(decode_s(&changed).is_err());
    }
    for (at, value) in [(55, 2), (65, 1), (69, 1), (54, 1), (47, 255)] {
        let mut changed = s.clone();
        changed[at] = value;
        assert!(decode_s(&changed).is_err());
    }
}
#[test]
fn declaration_reserved_padding_sites_counts_and_liveins_reject_malformed_bytes() {
    let d = declaration_bytes();
    for (at, value) in [
        (354, 0),
        (354, 2),
        (355, 0),
        (355, 65),
        (356, 32),
        (368, 3),
        (380 + 46, 255),
        (472, 2),
        (474, 1),
        (475, 1),
        (289, 1),
    ] {
        let mut changed = d.clone();
        changed[at] = value;
        assert!(decode_d(&changed).is_err(), "{at}");
    }
    for axis in 0..9 {
        let mut changed = d.clone();
        changed[1 + axis * 32..1 + (axis + 1) * 32].fill(0);
        assert!(decode_d(&changed).is_err());
    }
    let mut changed = d;
    changed[338..342].copy_from_slice(&0_u32.to_le_bytes());
    assert!(decode_d(&changed).is_err());
}
#[test]
fn every_closed_instruction_descriptor_roundtrips_and_reserved_fields_refuse() {
    for select in [false, true] {
        let module = fixture::module(select);
        for operation in module.functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|b| &b.operations)
        {
            if let crate::OperationKind::Gfx942PhysicalEntryStep(step) = &operation.kind {
                let descriptor = step.instruction.descriptor();
                assert_eq!(
                    Instruction::from_descriptor(descriptor),
                    Ok(step.instruction)
                );
            }
        }
    }
    for tag in 19..=u8::MAX {
        let mut value = [0; 8];
        value[0] = tag;
        assert!(Instruction::from_descriptor(value).is_err());
    }
}
