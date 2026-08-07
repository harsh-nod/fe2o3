//! Backend-issued semantic authority for general typed V3 kernels.

use crate::compiler_descriptor::TypedDescriptorRootV1;
use reserved_fe2o3_symbols::{
    GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1, GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1,
    GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1, GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1,
    GeneratedHostContractIdV3, KernelBindingIdV1, MAX_GENERAL_TYPED_V3_SEMANTIC_WITNESS_BYTES_V1,
    TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3,
};
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SemanticWitnessPlanV1 {
    kernel_binding: KernelBindingIdV1,
    payload: Box<[u8]>,
}

impl SemanticWitnessPlanV1 {
    pub(crate) fn kernel_binding(&self) -> KernelBindingIdV1 {
        self.kernel_binding
    }

    pub(crate) fn payload(&self) -> &[u8] {
        &self.payload
    }
}

pub(crate) fn plans_from_descriptor_roots(
    roots: &[TypedDescriptorRootV1],
) -> Result<Vec<SemanticWitnessPlanV1>, SemanticWitnessError> {
    plans_from_identities(
        roots
            .iter()
            .filter_map(TypedDescriptorRootV1::general_v3_semantic_identity),
    )
}

fn plans_from_identities(
    identities: impl IntoIterator<Item = (KernelBindingIdV1, GeneratedHostContractIdV3)>,
) -> Result<Vec<SemanticWitnessPlanV1>, SemanticWitnessError> {
    let mut identities = identities.into_iter().collect::<Vec<_>>();
    identities.sort_unstable_by_key(|(binding, _)| *binding);
    for duplicate in identities.windows(2) {
        if duplicate[0].0 == duplicate[1].0 {
            return Err(SemanticWitnessError::DuplicateKernelBinding(duplicate[0].0));
        }
    }
    identities
        .into_iter()
        .map(|(kernel_binding, generated_host_contract)| {
            Ok(SemanticWitnessPlanV1 {
                kernel_binding,
                payload: encode_general_typed_v3_semantic_witness(
                    kernel_binding,
                    generated_host_contract,
                )?,
            })
        })
        .collect()
}

fn encode_general_typed_v3_semantic_witness(
    kernel_binding: KernelBindingIdV1,
    generated_host_contract: GeneratedHostContractIdV3,
) -> Result<Box<[u8]>, SemanticWitnessError> {
    let profile = TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3.as_bytes();
    let declared_length = GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1
        .checked_add(profile.len())
        .ok_or(SemanticWitnessError::PayloadLengthOverflow)?;
    if declared_length > MAX_GENERAL_TYPED_V3_SEMANTIC_WITNESS_BYTES_V1 {
        return Err(SemanticWitnessError::PayloadTooLarge(declared_length));
    }
    let declared_length =
        u32::try_from(declared_length).map_err(|_| SemanticWitnessError::PayloadLengthOverflow)?;
    let profile_length =
        u16::try_from(profile.len()).map_err(|_| SemanticWitnessError::PayloadLengthOverflow)?;

    let mut payload = Vec::with_capacity(declared_length as usize);
    payload.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1.to_le_bytes());
    payload.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1.to_le_bytes());
    payload.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1.to_le_bytes());
    payload.extend_from_slice(&declared_length.to_le_bytes());
    payload.extend_from_slice(&kernel_binding.as_bytes());
    payload.extend_from_slice(&generated_host_contract.as_bytes());
    payload.extend_from_slice(&profile_length.to_le_bytes());
    payload.extend_from_slice(profile);
    debug_assert_eq!(payload.len(), declared_length as usize);
    Ok(payload.into_boxed_slice())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SemanticWitnessError {
    DuplicateKernelBinding(KernelBindingIdV1),
    PayloadLengthOverflow,
    PayloadTooLarge(usize),
}

impl fmt::Display for SemanticWitnessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateKernelBinding(binding) => write!(
                formatter,
                "general typed V3 semantic-witness binding is duplicated: {}",
                binding.to_hex()
            ),
            Self::PayloadLengthOverflow => formatter
                .write_str("general typed V3 semantic-witness length overflows its wire field"),
            Self::PayloadTooLarge(length) => write!(
                formatter,
                "general typed V3 semantic witness is {length} bytes; maximum is {MAX_GENERAL_TYPED_V3_SEMANTIC_WITNESS_BYTES_V1}"
            ),
        }
    }
}

impl std::error::Error for SemanticWitnessError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(binding: u8, contract: u8) -> (KernelBindingIdV1, GeneratedHostContractIdV3) {
        (
            KernelBindingIdV1::from_bytes([binding; 32]),
            GeneratedHostContractIdV3::from_bytes([contract; 32]),
        )
    }

    #[test]
    fn exact_bytes_match_the_host_wire_schema() {
        let (binding, contract) = identity(0x31, 0x52);
        let payload = encode_general_typed_v3_semantic_witness(binding, contract).unwrap();
        let profile = TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3.as_bytes();
        let expected_length = GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1 + profile.len();

        assert_eq!(payload.len(), expected_length);
        assert_eq!(&payload[0..8], b"FE2O3SMW");
        assert_eq!(&payload[8..10], &1_u16.to_le_bytes());
        assert_eq!(&payload[10..12], &1_u16.to_le_bytes());
        assert_eq!(
            &payload[12..16],
            &u32::try_from(expected_length).unwrap().to_le_bytes()
        );
        assert_eq!(&payload[16..48], &[0x31; 32]);
        assert_eq!(&payload[48..80], &[0x52; 32]);
        assert_eq!(
            &payload[80..82],
            &u16::try_from(profile.len()).unwrap().to_le_bytes()
        );
        assert_eq!(&payload[82..], profile);
    }

    #[test]
    fn payload_is_bound_to_both_semantic_identities() {
        let first =
            encode_general_typed_v3_semantic_witness(identity(1, 2).0, identity(1, 2).1).unwrap();
        let changed_binding =
            encode_general_typed_v3_semantic_witness(identity(3, 2).0, identity(3, 2).1).unwrap();
        let changed_contract =
            encode_general_typed_v3_semantic_witness(identity(1, 4).0, identity(1, 4).1).unwrap();
        assert_ne!(first, changed_binding);
        assert_ne!(first, changed_contract);
    }

    #[test]
    fn plans_are_stably_ordered_and_ordinary_roots_emit_nothing() {
        assert!(plans_from_identities([]).unwrap().is_empty());
        let plans = plans_from_identities([identity(0x7a, 2), identity(0x61, 1)]).unwrap();
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].kernel_binding().as_bytes(), [0x61; 32]);
        assert_eq!(plans[1].kernel_binding().as_bytes(), [0x7a; 32]);
    }

    #[test]
    fn duplicate_accessor_identity_is_rejected() {
        let error = plans_from_identities([identity(0x61, 1), identity(0x61, 2)]).unwrap_err();
        assert_eq!(
            error,
            SemanticWitnessError::DuplicateKernelBinding(KernelBindingIdV1::from_bytes([0x61; 32]))
        );
    }
}
