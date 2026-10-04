//! Strict V5 facade over shared mandatory conditional framing.
use crate::ConditionalInvocationContractV2;
use crate::conditional_v5::*;
use crate::conditional_wire_common as common;

fn error<E>(e: common::Error<E>) -> DescriptorWireErrorV5<E> {
    match e {
        common::Error::Nominal(e) => DescriptorWireErrorV5::Nominal(e),
        common::Error::Contract(e) => DescriptorWireErrorV5::Contract(e),
        common::Error::Invalid(field) => DescriptorWireErrorV5::Invalid(field),
        common::Error::OutputLength { expected, actual } => {
            DescriptorWireErrorV5::OutputLength { expected, actual }
        }
    }
}

/// Decode only outer V5 with mandatory invocation V2 contracts.
pub fn decode_device_descriptor_table_v5<'wire, E>(
    bytes: &'wire [u8],
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV5<DeviceDescriptorTableV5<'wire>, E> {
    let (nominal, contracts) =
        common::decode::<ConditionalInvocationContractV2<'wire>, E>(bytes, c).map_err(error)?;
    Ok(DeviceDescriptorTableV5 { nominal, contracts })
}
/// Validate the full V5 input and return its exact canonical extent.
pub fn encoded_device_descriptor_table_v5_len<E>(
    input: &DeviceDescriptorTableInputV5<'_>,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV5<usize, E> {
    common::encoded_len(&input.nominal, input.contracts, c).map_err(error)
}
/// Complete validation and work charging precede all output writes.
pub fn encode_device_descriptor_table_v5<E>(
    input: &DeviceDescriptorTableInputV5<'_>,
    output: &mut [u8],
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV5<(), E> {
    common::encode(&input.nominal, input.contracts, output, c).map_err(error)
}
