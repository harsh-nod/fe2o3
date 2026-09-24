//! Fixed payload tests share the one inert canonical fixture, not a second owner.
use super::*;
use crate::canonical_kir_v21::tests::fixture;
fn declaration() -> Declaration {
    let module = fixture::module();
    let crate::OperationKind::Gfx942PhysicalGlobalCopyDeclaration(value) =
        module.functions[0].body.as_ref().unwrap().blocks[0].operations[0].kind
    else {
        panic!("declaration")
    };
    value
}
fn step() -> Step {
    let module = fixture::module();
    let crate::OperationKind::Gfx942PhysicalGlobalCopyStep(value) =
        module.functions[0].body.as_ref().unwrap().blocks[0].operations[1].kind
    else {
        panic!("step")
    };
    value
}
fn declaration_bytes() -> Vec<u8> {
    let mut writer = Writer::new(KERNEL_IR_VERSION_V21, None);
    encode_declaration(&mut writer, &declaration()).unwrap();
    writer.bytes
}
fn step_bytes() -> Vec<u8> {
    let mut writer = Writer::new(KERNEL_IR_VERSION_V21, None);
    encode_step(&mut writer, &step()).unwrap();
    writer.bytes
}
fn decode_d(bytes: &[u8]) -> Result<Declaration, KernelIrDecodeError> {
    let mut reader = Reader::new(bytes, None);
    reader.version = KERNEL_IR_VERSION_V21;
    let value = decode_declaration(&mut reader)?;
    if !reader.is_finished() {
        return Err(KernelIrDecodeError::TrailingBytes);
    }
    Ok(value)
}
fn decode_s(bytes: &[u8]) -> Result<Step, KernelIrDecodeError> {
    let mut reader = Reader::new(bytes, None);
    reader.version = KERNEL_IR_VERSION_V21;
    let value = decode_step(&mut reader)?;
    if !reader.is_finished() {
        return Err(KernelIrDecodeError::TrailingBytes);
    }
    Ok(value)
}
#[test]
fn exact_461_and_80_payloads_retain_origins_two_roots_and_instruction() {
    let d = declaration_bytes();
    let s = step_bytes();
    assert_eq!(
        d.len(),
        crate::GFX942_PHYSICAL_GLOBAL_COPY_DECLARATION_BYTES_V21
    );
    assert_eq!(s.len(), crate::GFX942_PHYSICAL_GLOBAL_COPY_STEP_BYTES_V21);
    assert_eq!(decode_d(&d), Ok(declaration()));
    assert_eq!(decode_s(&s), Ok(step()));
    assert_eq!(d[0], 0);
    assert_eq!(&d[1..161], &[1; 160]);
    for (axis, value) in [(5, 2), (6, 3), (7, 4), (8, 5)] {
        assert_eq!(&d[1 + axis * 32..1 + (axis + 1) * 32], &[value; 32]);
    }
    assert_eq!(&d[334..338], &0u32.to_le_bytes());
    assert_eq!(&d[338..342], &1u32.to_le_bytes());
    assert_eq!(d[342], 24);
    assert_eq!(&d[343..355], &[64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]);
    assert_eq!(&d[355..367], &[2, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0]);
    assert_eq!(d[367], 0);
    assert_eq!(d[413], 4);
    assert_eq!(&d[459..461], &[1, 23]);
    assert_eq!(s[46], 0);
    assert_eq!(&s[47..55], &step().instruction.descriptor());
    assert_eq!(&s[65..80], &[0; 15]);
}
#[test]
fn every_truncation_trailing_byte_revision_and_optional_padding_refuses() {
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
        assert!(decode_s(&changed).is_err(), "{at}");
    }
}
#[test]
fn exact_abi_counts_sites_terminal_presence_and_source_origins_refuse_mutation() {
    let d = declaration_bytes();
    for (at, value) in [
        (342, 0),
        (342, 33),
        (343, 32),
        (355, 3),
        (367, 1),
        (368, 34),
        (413, 1),
        (459, 2),
        (460, 0),
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
    changed[338..342].copy_from_slice(&0u32.to_le_bytes());
    assert!(decode_d(&changed).is_err());
}
#[test]
fn all_fifteen_descriptors_are_closed_and_global_load_requires_exact_shape() {
    for (opcode, d, a, b, immediate) in fixture::ROWS {
        let value = Instruction {
            opcode,
            destination: d,
            source0: a,
            source1: b,
            immediate,
        };
        assert_eq!(Instruction::from_descriptor(value.descriptor()), Ok(value));
    }
    let end = Instruction {
        opcode: crate::Gfx942PhysicalGlobalCopyOpcodeV1::Endpgm0,
        destination: 0,
        source0: 0,
        source1: 0,
        immediate: 0,
    };
    assert_eq!(Instruction::from_descriptor(end.descriptor()), Ok(end));
    for tag in 15..=u8::MAX {
        let mut changed = [0; 8];
        changed[0] = tag;
        assert!(Instruction::from_descriptor(changed).is_err());
    }
    let load = Instruction {
        opcode: crate::Gfx942PhysicalGlobalCopyOpcodeV1::GlobalLoadDword,
        destination: 8,
        source0: 6,
        source1: 0,
        immediate: 0,
    };
    for (at, value) in [(1, 0), (1, 64), (2, 63), (2, 5), (3, 1), (4, 1)] {
        let mut changed = load.descriptor();
        changed[at] = value;
        assert!(Instruction::from_descriptor(changed).is_err());
    }
}
