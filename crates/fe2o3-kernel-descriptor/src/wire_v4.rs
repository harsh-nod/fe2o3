//! Strict V4 facade over shared mandatory conditional framing.
use crate::ConditionalInvocationContractV1;
use crate::conditional_v4::*;
use crate::conditional_wire_common as common;

fn error<E>(e: common::Error<E>) -> DescriptorWireErrorV4<E> {
    match e {
        common::Error::Nominal(e) => DescriptorWireErrorV4::Nominal(e),
        common::Error::Contract(e) => DescriptorWireErrorV4::Contract(e),
        common::Error::Invalid(field) => DescriptorWireErrorV4::Invalid(field),
        common::Error::OutputLength { expected, actual } => {
            DescriptorWireErrorV4::OutputLength { expected, actual }
        }
    }
}

pub fn decode_device_descriptor_table_v4<'wire, E>(
    bytes: &'wire [u8],
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV4<DeviceDescriptorTableV4<'wire>, E> {
    let (nominal, contracts) =
        common::decode::<ConditionalInvocationContractV1<'wire>, E>(bytes, c).map_err(error)?;
    Ok(DeviceDescriptorTableV4 { nominal, contracts })
}
pub fn encoded_device_descriptor_table_v4_len<E>(
    input: &DeviceDescriptorTableInputV4<'_>,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV4<usize, E> {
    common::encoded_len(&input.nominal, input.contracts, c).map_err(error)
}
/// Complete validation and work charging precede all output writes.
pub fn encode_device_descriptor_table_v4<E>(
    input: &DeviceDescriptorTableInputV4<'_>,
    output: &mut [u8],
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> ResultV4<(), E> {
    common::encode(&input.nominal, input.contracts, output, c).map_err(error)
}
