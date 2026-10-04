//! Closed conditional framing and row adapters; no public schema selection.
use crate::nominal_v3::{Output, pay, write_nominal_prefix};
use crate::wire_common::Reader;
use crate::wire_v4_join as join;
use crate::*;

pub(crate) enum Error<E> {
    Nominal(DescriptorWireErrorV3<E>),
    Contract(ConditionalInvocationWireErrorV1<E>),
    Invalid(&'static str),
    OutputLength { expected: usize, actual: usize },
}
pub(crate) type Result<T, E> = std::result::Result<T, Error<E>>;
impl<E> From<DescriptorWireErrorV3<E>> for Error<E> {
    fn from(e: DescriptorWireErrorV3<E>) -> Self {
        Self::Nominal(e)
    }
}
impl<E> From<ConditionalInvocationWireErrorV1<E>> for Error<E> {
    fn from(e: ConditionalInvocationWireErrorV1<E>) -> Self {
        Self::Contract(e)
    }
}

pub(crate) trait Contract<'wire>: Sized {
    const VERSION: u16;
    const LIMIT: usize;
    fn decode<E>(
        bytes: &'wire [u8],
        c: &mut impl FnMut(usize) -> std::result::Result<(), E>,
    ) -> std::result::Result<Self, ConditionalInvocationWireErrorV1<E>>;
    fn canonical_bytes(&self) -> &'wire [u8];
    fn identity_bytes(&self) -> [u8; 32];
    fn require_identity<E>(
        &self,
        expected: [u8; 32],
        c: &mut impl FnMut(usize) -> std::result::Result<(), E>,
    ) -> std::result::Result<(), ConditionalInvocationWireErrorV1<E>>;
    fn kernel_id(&self) -> &[u8; 32];
    fn output(&self) -> ConditionalOutputV1;
    fn arguments(&self) -> ConditionalInvocationCursorV1<'wire, ConditionalArgumentBindingV1>;
    fn reads(&self) -> ConditionalInvocationCursorV1<'wire, ConditionalReadOccurrenceV1>;
    fn argument<E>(
        &self,
        index: usize,
        c: &mut impl FnMut(usize) -> std::result::Result<(), E>,
    ) -> std::result::Result<ConditionalArgumentBindingV1, ConditionalInvocationWireErrorV1<E>>;
}

macro_rules! contract {
    ($view:ident, $identity:ident, $decode:ident, $version:ident, $limit:ident) => {
        impl<'wire> Contract<'wire> for $view<'wire> {
            const VERSION: u16 = $version;
            const LIMIT: usize = $limit;
            fn decode<E>(
                bytes: &'wire [u8],
                c: &mut impl FnMut(usize) -> std::result::Result<(), E>,
            ) -> std::result::Result<Self, ConditionalInvocationWireErrorV1<E>> {
                $decode(bytes, c)
            }
            fn canonical_bytes(&self) -> &'wire [u8] {
                self.canonical_bytes()
            }
            fn identity_bytes(&self) -> [u8; 32] {
                *self.identity().as_bytes()
            }
            fn require_identity<E>(
                &self,
                expected: [u8; 32],
                c: &mut impl FnMut(usize) -> std::result::Result<(), E>,
            ) -> std::result::Result<(), ConditionalInvocationWireErrorV1<E>> {
                self.require_identity($identity::from_untrusted_bytes(expected), c)
            }
            fn kernel_id(&self) -> &[u8; 32] {
                &self.subjects().kernel_id
            }
            fn output(&self) -> ConditionalOutputV1 {
                self.output()
            }
            fn arguments(
                &self,
            ) -> ConditionalInvocationCursorV1<'wire, ConditionalArgumentBindingV1> {
                self.arguments()
            }
            fn reads(&self) -> ConditionalInvocationCursorV1<'wire, ConditionalReadOccurrenceV1> {
                self.reads()
            }
            fn argument<E>(
                &self,
                index: usize,
                c: &mut impl FnMut(usize) -> std::result::Result<(), E>,
            ) -> std::result::Result<
                ConditionalArgumentBindingV1,
                ConditionalInvocationWireErrorV1<E>,
            > {
                self.argument(index, c)
            }
        }
    };
}
contract!(
    ConditionalInvocationContractV1,
    ConditionalInvocationIdentityV1,
    decode_conditional_invocation_contract_v1,
    DEVICE_DESCRIPTOR_VERSION_V4,
    MAX_CONDITIONAL_INVOCATION_BYTES_V1
);
contract!(
    ConditionalInvocationContractV2,
    ConditionalInvocationIdentityV2,
    decode_conditional_invocation_contract_v2,
    DEVICE_DESCRIPTOR_VERSION_V5,
    MAX_CONDITIONAL_INVOCATION_BYTES_V2
);

pub(crate) fn decode<'wire, C: Contract<'wire>, E>(
    bytes: &'wire [u8],
    c: &mut impl FnMut(usize) -> std::result::Result<(), E>,
) -> Result<
    (
        DeviceDescriptorTableV3<'wire>,
        [(usize, usize); MAX_KERNELS],
    ),
    E,
> {
    let (nominal, start) = crate::wire_v3::decode_nominal_prefix(bytes, C::VERSION, c)?;
    let mut r = Reader::at(bytes, start);
    let count = r.count("conditional contracts", MAX_KERNELS, c)?;
    r.zero("conditional contracts", c)?;
    if count != nominal.kernel_count() {
        return Err(Error::Invalid("mandatory contract roster"));
    }
    pay(c, MAX_KERNELS * 2)?;
    let mut contracts = [(0, 0); MAX_KERNELS];
    for (i, slot) in contracts[..count].iter_mut().enumerate() {
        let length = usize::try_from(r.u32(c)?).map_err(|_| Error::Invalid("contract extent"))?;
        if length > C::LIMIT {
            return Err(Error::Invalid("contract byte limit"));
        }
        let expected = r.fixed(c)?;
        let offset = r.position;
        let contract = C::decode(r.take(length, c)?, c)?;
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
        return Err(Error::Invalid("trailing bytes"));
    }
    Ok((nominal, contracts))
}

fn preflight<'wire, C: Contract<'wire>, E>(
    nominal: &DeviceDescriptorTableInputV3<'_>,
    contracts: &[C],
    c: &mut impl FnMut(usize) -> std::result::Result<(), E>,
) -> Result<(usize, usize, crate::wire_v3::TargetText), E> {
    let (prefix, target) = crate::wire_v3::preflight(nominal, c)?;
    if contracts.len() != nominal.kernels.len() {
        return Err(Error::Invalid("mandatory contract roster"));
    }
    let mut total = prefix
        .checked_add(4)
        .ok_or(Error::Invalid("table extent"))?;
    for (contract, kernel) in contracts.iter().zip(nominal.kernels) {
        // Recheck the exact canonical contract rather than accepting a cached identity.
        let contract = C::decode(contract.canonical_bytes(), c)?;
        join::join(
            kernel.kernel_id,
            kernel.launch.rank(),
            kernel.arguments.len(),
            &contract,
            |field, c| join::input_argument(nominal, kernel, field, c),
            c,
        )?;
        total = total
            .checked_add(36)
            .and_then(|n| n.checked_add(contract.canonical_bytes().len()))
            .ok_or(Error::Invalid("table extent"))?;
        if total > MAX_DESCRIPTOR_TABLE_BYTES {
            return Err(Error::Invalid("table byte limit"));
        }
    }
    Ok((prefix, total, target))
}

pub(crate) fn encoded_len<'wire, C: Contract<'wire>, E>(
    nominal: &DeviceDescriptorTableInputV3<'_>,
    contracts: &[C],
    c: &mut impl FnMut(usize) -> std::result::Result<(), E>,
) -> Result<usize, E> {
    preflight(nominal, contracts, c).map(|(_, total, _)| total)
}

pub(crate) fn encode<'wire, C: Contract<'wire>, E>(
    nominal: &DeviceDescriptorTableInputV3<'_>,
    contracts: &[C],
    output: &mut [u8],
    c: &mut impl FnMut(usize) -> std::result::Result<(), E>,
) -> Result<(), E> {
    let (prefix, total, target) = preflight(nominal, contracts, c)?;
    if output.len() != total {
        return Err(Error::OutputLength {
            expected: total,
            actual: output.len(),
        });
    }
    pay(c, total * 2 + 1)?;
    write_nominal_prefix(nominal, &mut output[..prefix], C::VERSION, total, &target);
    let mut w = Output {
        bytes: &mut output[prefix..],
        position: 0,
    };
    w.u16(contracts.len() as u16);
    w.u16(0);
    for contract in contracts {
        w.u32(contract.canonical_bytes().len() as u32);
        w.bytes(&contract.identity_bytes());
        w.bytes(contract.canonical_bytes());
    }
    Ok(())
}
