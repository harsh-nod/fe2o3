//! Nominal V4 framing: shared nominal prefix, then one mandatory contract per
//! kernel. No embedded V3 header or optional conditional flag is produced.
use crate::conditional_invocation::{
    ConditionalInvocationIdentityV1, decode_conditional_invocation_contract_v1,
};
use crate::conditional_v4::*;
use crate::nominal_v3::{Output, pay, write_nominal_prefix};
use crate::wire_common::Reader;
use crate::wire_v4_join as join;
use crate::*;

pub fn decode_device_descriptor_table_v4<'wire, E>(
    bytes: &'wire [u8],
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV4<DeviceDescriptorTableV4<'wire>, E> {
    let (nominal, start) =
        crate::wire_v3::decode_nominal_prefix(bytes, DEVICE_DESCRIPTOR_VERSION_V4, c)?;
    let mut r = Reader::at(bytes, start);
    let count = r.count("conditional contracts", MAX_KERNELS, c)?;
    r.zero("conditional contracts", c)?;
    if count != nominal.kernel_count() {
        return Err(DescriptorWireErrorV4::Invalid("mandatory contract roster"));
    }
    pay(c, MAX_KERNELS * 2)?;
    let mut contracts = [(0, 0); MAX_KERNELS];
    for (i, slot) in contracts[..count].iter_mut().enumerate() {
        let length = usize::try_from(r.u32(c)?)
            .map_err(|_| DescriptorWireErrorV4::Invalid("contract extent"))?;
        if length > crate::conditional_invocation::MAX_CONDITIONAL_INVOCATION_BYTES_V1 {
            return Err(DescriptorWireErrorV4::Invalid("contract byte limit"));
        }
        let expected = ConditionalInvocationIdentityV1::from_untrusted_bytes(r.fixed(c)?);
        let offset = r.position;
        let contract = decode_conditional_invocation_contract_v1(r.take(length, c)?, c)?;
        contract.require_identity(expected, c)?;
        let kernel = nominal.kernel(i, c)?;
        join::join(
            kernel.kernel_id(),
            kernel.launch().rank(),
            kernel.argument_count(),
            &contract,
            |field, c| join::view_argument(&nominal, &kernel, field, c),
            c,
        )?;
        *slot = (offset, length);
    }
    if r.position != bytes.len() {
        return Err(DescriptorWireErrorV4::Invalid("trailing bytes"));
    }
    Ok(DeviceDescriptorTableV4 { nominal, contracts })
}

fn preflight<E>(
    input: &DeviceDescriptorTableInputV4<'_>,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV4<(usize, usize, crate::wire_v3::TargetText), E> {
    let (prefix, target) = crate::wire_v3::preflight(&input.nominal, c)?;
    if input.contracts.len() != input.nominal.kernels.len() {
        return Err(DescriptorWireErrorV4::Invalid("mandatory contract roster"));
    }
    let mut total = prefix
        .checked_add(4)
        .ok_or(DescriptorWireErrorV4::Invalid("table extent"))?;
    for (contract, kernel) in input.contracts.iter().zip(input.nominal.kernels) {
        // Independently replay structure instead of treating a cached identity as admission.
        let contract = decode_conditional_invocation_contract_v1(contract.canonical_bytes(), c)?;
        join::join(
            kernel.kernel_id,
            kernel.launch.rank(),
            kernel.arguments.len(),
            &contract,
            |field, c| join::input_argument(&input.nominal, kernel, field, c),
            c,
        )?;
        total = total
            .checked_add(36)
            .and_then(|n| n.checked_add(contract.canonical_bytes().len()))
            .ok_or(DescriptorWireErrorV4::Invalid("table extent"))?;
        if total > MAX_DESCRIPTOR_TABLE_BYTES {
            return Err(DescriptorWireErrorV4::Invalid("table byte limit"));
        }
    }
    Ok((prefix, total, target))
}
pub fn encoded_device_descriptor_table_v4_len<E>(
    input: &DeviceDescriptorTableInputV4<'_>,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV4<usize, E> {
    preflight(input, c).map(|(_, total, _)| total)
}
/// Complete validation and work charging precede all output writes.
pub fn encode_device_descriptor_table_v4<E>(
    input: &DeviceDescriptorTableInputV4<'_>,
    output: &mut [u8],
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV4<(), E> {
    let (prefix, total, target) = preflight(input, c)?;
    if output.len() != total {
        return Err(DescriptorWireErrorV4::OutputLength {
            expected: total,
            actual: output.len(),
        });
    }
    pay(c, total * 2 + 1)?;
    write_nominal_prefix(
        &input.nominal,
        &mut output[..prefix],
        DEVICE_DESCRIPTOR_VERSION_V4,
        total,
        &target,
    );
    let mut w = Output {
        bytes: &mut output[prefix..],
        position: 0,
    };
    w.u16(input.contracts.len() as u16);
    w.u16(0);
    for contract in input.contracts {
        w.u32(contract.canonical_bytes().len() as u32);
        w.bytes(contract.identity().as_bytes());
        w.bytes(contract.canonical_bytes());
    }
    Ok(())
}
