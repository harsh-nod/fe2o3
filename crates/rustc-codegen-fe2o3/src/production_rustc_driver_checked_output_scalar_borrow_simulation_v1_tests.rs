//! Fixed independent byte oracle for the ordinary safe-borrow source fixture.
use super::*;

pub(super) const NUMERICAL_POLICY: &str = "initialized-private-u32-equal-reads-seven-v1";

fn backing(
    value: u32,
    len: usize,
) -> Result<(SimulationArgumentV1, SharedBufferV1), SourceFailure> {
    let mut bytes = vec![0xa5; (len + 2 * GUARD_ELEMENTS) * 4];
    bytes[(len + GUARD_ELEMENTS) * 4..].fill(0x5a);
    for cell in bytes[GUARD_ELEMENTS * 4..(GUARD_ELEMENTS + len) * 4].chunks_exact_mut(4) {
        cell.copy_from_slice(&value.to_le_bytes());
    }
    let buffer = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        bytes.clone(),
        vec![true; bytes.len()],
        TARGET,
    )
    .map_err(failure)?;
    let view = BufferViewArgumentV1::new(
        BufferBackingIdV1(0),
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        GUARD_ELEMENTS * 4,
        len,
        TARGET,
    )
    .map_err(failure)?;
    Ok((
        SimulationArgumentV1::BufferView(view),
        SharedBufferV1 {
            id: BufferBackingIdV1(0),
            buffer,
        },
    ))
}

pub(super) fn scenarios() -> Result<Vec<Scenario>, SourceFailure> {
    [
        (0, 0),
        (1, 0),
        (63, 1),
        (64, u32::MAX),
        (65, 0x8000_0000),
        (129, 0x5555_aaaa),
    ]
    .into_iter()
    .enumerate()
    .map(|(ordinal, (len, input))| {
        let (output, initial) = backing(0x37, len)?;
        Ok(Scenario {
            label: format!("private-u32-{ordinal}-{input:08x}-len-{len}"),
            active: len,
            arguments: vec![
                output,
                SimulationArgumentV1::Scalar(
                    ScalarBitsV1::new(ScalarType::U32, u128::from(input), TARGET)
                        .map_err(failure)?,
                ),
            ],
            backings: vec![initial],
            expected: vec![backing(7, len)?.1],
            output_elements: len,
            written_elements: len,
        })
    })
    .collect()
}

pub(super) fn require_abi(module: &AdmittedSimulationModuleV1) -> Result<&Kernel, SourceFailure> {
    let [kernel] = module.module().kernels.as_slice() else {
        return Err(failure("scalar borrow requires one actual O root"));
    };
    let function = module
        .module()
        .function(&kernel.entry)
        .ok_or_else(|| failure("scalar borrow actual O entry"))?;
    let [Type::Slice(output), input] = function.signature.parameters.as_slice() else {
        return Err(failure("scalar borrow output/scalar ABI"));
    };
    if output.address_space != AddressSpace::Global
        || output.element.as_ref() != &Type::Scalar(ScalarType::U32)
        || output.access != AccessMode::ReadWrite
        || input != &Type::Scalar(ScalarType::U32)
        || !function.signature.results.is_empty()
        || kernel.domain.rank() != 1
    {
        return Err(failure("scalar borrow exact u32 ABI"));
    }
    Ok(kernel)
}

#[test]
fn independent_scalar_borrow_vectors_cover_zero_and_wave_boundaries() {
    let rows = scenarios().unwrap();
    assert_eq!(rows.len(), 6);
    assert_eq!(
        rows.iter()
            .map(|row| row.output_elements)
            .collect::<Vec<_>>(),
        [0, 1, 63, 64, 65, 129]
    );
    for row in rows {
        let bytes = row.expected[0].buffer.bytes();
        assert!(bytes[..GUARD_ELEMENTS * 4].iter().all(|byte| *byte == 0xa5));
        assert!(
            bytes[(GUARD_ELEMENTS + row.output_elements) * 4..]
                .iter()
                .all(|byte| *byte == 0x5a)
        );
        for cell in
            bytes[GUARD_ELEMENTS * 4..(GUARD_ELEMENTS + row.output_elements) * 4].chunks_exact(4)
        {
            assert_eq!(cell, 7_u32.to_le_bytes());
        }
    }
}
