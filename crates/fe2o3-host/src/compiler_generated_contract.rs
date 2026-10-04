use fe2o3_device::KernelMarkerV1;
use reserved_fe2o3_symbols::{
    GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1, GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1,
    GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1, GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1,
    MAX_GENERAL_TYPED_V3_SEMANTIC_WITNESS_BYTES_V1, TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3,
};
use std::fmt;
/// Validated semantic authority for one compiler-generated kernel expectation.
///
/// This value is intentionally opaque. Implementations receive one only after
/// parsing the exact backend-issued witness bound to their kernel and generated
/// host-contract identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct ValidatedCompilerGeneratedSemanticWitnessV1 {
    profile: CompilerGeneratedKernelProfileV1,
    kernel_binding: [u8; 32],
    generated_host_contract: [u8; 32],
}

impl ValidatedCompilerGeneratedSemanticWitnessV1 {
    const fn general_v3(kernel_binding: [u8; 32], generated_host_contract: [u8; 32]) -> Self {
        Self {
            profile: CompilerGeneratedKernelProfileV1::new(generated_host_contract),
            kernel_binding,
            generated_host_contract,
        }
    }
}

/// Failure while obtaining or validating compiler-generated semantic authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
#[non_exhaustive]
pub enum CompilerGeneratedSemanticWitnessErrorV1 {
    MissingBackendWitness,
    InvalidPointer,
    InvalidLength,
    MagicMismatch,
    VersionMismatch,
    DomainMismatch,
    KernelBindingMismatch,
    GeneratedHostContractMismatch,
    ProfileTagMismatch,
    TrailingBytes,
    WitnessSubstitution,
}

impl fmt::Display for CompilerGeneratedSemanticWitnessErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingBackendWitness => {
                "the general typed kernel has no backend-issued semantic witness"
            }
            Self::InvalidPointer => "the backend semantic-witness pointer is invalid",
            Self::InvalidLength => "the backend semantic-witness length is invalid",
            Self::MagicMismatch => "the backend semantic-witness magic does not match",
            Self::VersionMismatch => "the backend semantic-witness version does not match",
            Self::DomainMismatch => "the backend semantic-witness domain does not match",
            Self::KernelBindingMismatch => {
                "the backend semantic witness names a different kernel binding"
            }
            Self::GeneratedHostContractMismatch => {
                "the backend semantic witness names a different generated host contract"
            }
            Self::ProfileTagMismatch => "the backend semantic-witness profile tag does not match",
            Self::TrailingBytes => "the backend semantic witness contains trailing bytes",
            Self::WitnessSubstitution => {
                "the backend semantic witness was substituted across expectations"
            }
        })
    }
}

impl std::error::Error for CompilerGeneratedSemanticWitnessErrorV1 {}

/// Trusted generated expectation for one compiler-generated kernel.
///
/// The associated constants are a frontend declaration of the expected host
/// ABI, effects, launch, and kernel binding. They are not by themselves proof
/// that rustc accepted those semantics. Production Worker V3 admission matches
/// the binding and generated argument layout to the independently admitted
/// compiler descriptor. The trait deliberately carries no artifact bytes.
///
/// # Safety
///
/// The profile and binding identity must describe `Self::FUNCTION` exactly,
/// including the complete physical host ABI, memory effects, launch contract,
/// and all behavior relevant to safe loading and dispatch. Implementations are
/// an explicit unsafe trust boundary. A false implementation can authorize
/// dispatch of native code under the wrong Rust signature or safety contract.
#[doc(hidden)]
pub unsafe trait CompilerGeneratedKernelExpectationV1: KernelMarkerV1 {
    /// Versioned host ABI and memory-effect profile expected by generated code.
    const PROFILE: CompilerGeneratedKernelProfileV1;

    /// Full backend-validated identity used by private host linker symbols.
    const KERNEL_BINDING_ID_V1: [u8; 32];

    /// Obtains the backend-issued witness for this exact expectation.
    fn semantic_witness_v1()
    -> Result<ValidatedCompilerGeneratedSemanticWitnessV1, CompilerGeneratedSemanticWitnessErrorV1>
    {
        Err(CompilerGeneratedSemanticWitnessErrorV1::MissingBackendWitness)
    }
}

/// Metadata for one marker in an exact compiler-generated kernel roster.
///
/// This value carries no artifact bytes and grants no verification, load, or
/// launch authority. Host admission compares the complete roster, in canonical
/// descriptor-table order, with the independently recovered compiler descriptor
/// table. V1 descriptor tables currently canonicalize kernels by `KernelId`;
/// source registration and physical ELF order do not define roster order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct CompilerGeneratedKernelExpectationRosterEntryV1 {
    logical_name: &'static str,
    export_name: &'static str,
    kernel_binding_id: [u8; 32],
    generated_host_contract_identity: [u8; 32],
}

impl CompilerGeneratedKernelExpectationRosterEntryV1 {
    pub(crate) const fn from_parts(
        logical_name: &'static str,
        export_name: &'static str,
        kernel_binding_id: [u8; 32],
        generated_host_contract_identity: [u8; 32],
    ) -> Self {
        Self {
            logical_name,
            export_name,
            kernel_binding_id,
            generated_host_contract_identity,
        }
    }

    #[doc(hidden)]
    pub const fn for_marker<K: CompilerGeneratedKernelExpectationV1>() -> Self {
        Self::from_parts(
            K::LOGICAL_NAME,
            K::EXPORT_NAME,
            K::KERNEL_BINDING_ID_V1,
            K::PROFILE.generated_host_contract_identity(),
        )
    }

    pub const fn logical_name(&self) -> &'static str {
        self.logical_name
    }

    pub const fn export_name(&self) -> &'static str {
        self.export_name
    }

    pub const fn kernel_binding_id(&self) -> [u8; 32] {
        self.kernel_binding_id
    }

    pub const fn generated_host_contract_identity(&self) -> [u8; 32] {
        self.generated_host_contract_identity
    }
}

/// Exact canonical descriptor-table-ordered set of compiler-generated kernel
/// expectations for one artifact.
///
/// Implementations are metadata only. They grant no authority and are checked
/// against the complete receipt-bound compiler descriptor table during host
/// admission. Entries must follow the descriptor table's canonical order, which
/// V1 currently defines as strictly ascending `KernelId`, rather than source
/// registration or physical ELF order. The generated host-contract identity is
/// retained for the later sealed verification transition; descriptor admission
/// itself matches only the ordered logical name, export name, and kernel binding
/// carried on both boundaries. Prefer
/// [`compiler_generated_kernel_expectation_roster_v1!`] so every entry is derived
/// directly from its generated marker.
#[doc(hidden)]
pub trait CompilerGeneratedKernelExpectationRosterV1: Send + Sync + 'static {
    const ENTRIES: &'static [CompilerGeneratedKernelExpectationRosterEntryV1];
}

/// Declares an exact roster in canonical descriptor-table order from
/// compiler-generated kernel markers.
#[macro_export]
#[doc(hidden)]
macro_rules! compiler_generated_kernel_expectation_roster_v1 {
    (
        $(#[$metadata:meta])*
        $visibility:vis struct $roster:ident = [$($marker:ty),+ $(,)?];
    ) => {
        $(#[$metadata])*
        $visibility struct $roster;

        impl $crate::CompilerGeneratedKernelExpectationRosterV1 for $roster {
            const ENTRIES: &'static [
                $crate::CompilerGeneratedKernelExpectationRosterEntryV1
            ] = &[
                $(
                    $crate::CompilerGeneratedKernelExpectationRosterEntryV1::for_marker::<
                        $marker
                    >()
                ),+
            ];
        }
    };
}

/// Obtains an opaque semantic-authority token for one exact generated
/// expectation and rejects cross-kernel token substitution.
#[doc(hidden)]
pub fn validate_compiler_generated_semantic_witness_v1<K: CompilerGeneratedKernelExpectationV1>()
-> Result<ValidatedCompilerGeneratedSemanticWitnessV1, CompilerGeneratedSemanticWitnessErrorV1> {
    let witness = K::semantic_witness_v1()?;
    if witness.profile != K::PROFILE
        || witness.kernel_binding != K::KERNEL_BINDING_ID_V1
        || witness.generated_host_contract != K::PROFILE.generated_host_contract_identity()
    {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::WitnessSubstitution);
    }
    Ok(witness)
}

/// Parses the immutable witness bytes returned by one reserved backend accessor
/// pair and binds them to an exact general typed V3 expectation.
///
/// # Safety
///
/// `pointer` must be non-null and point to one live, immutable allocation of
/// exactly `length` initialized bytes. The allocation must remain live and
/// immutable for the entire call. The range must not wrap the address space.
/// Only compiler-generated unsafe trait implementations may call this function
/// with values returned by their exact backend-owned accessor pair.
#[doc(hidden)]
pub unsafe fn semantic_witness_from_backend_v1(
    pointer: *const u8,
    length: usize,
    expected_kernel_binding: [u8; 32],
    expected_generated_host_contract: [u8; 32],
) -> Result<ValidatedCompilerGeneratedSemanticWitnessV1, CompilerGeneratedSemanticWitnessErrorV1> {
    if pointer.is_null() {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::InvalidPointer);
    }
    if !(GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1
        ..=MAX_GENERAL_TYPED_V3_SEMANTIC_WITNESS_BYTES_V1)
        .contains(&length)
        || length > isize::MAX as usize
        || pointer.addr().checked_add(length).is_none()
    {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::InvalidLength);
    }

    // SAFETY: the caller establishes the allocation, initialization,
    // immutability, range, and lifetime requirements above.
    let bytes = unsafe { core::slice::from_raw_parts(pointer, length) };
    parse_general_typed_v3_semantic_witness_v1(
        bytes,
        expected_kernel_binding,
        expected_generated_host_contract,
    )
}

fn parse_general_typed_v3_semantic_witness_v1(
    bytes: &[u8],
    expected_kernel_binding: [u8; 32],
    expected_generated_host_contract: [u8; 32],
) -> Result<ValidatedCompilerGeneratedSemanticWitnessV1, CompilerGeneratedSemanticWitnessErrorV1> {
    if !(GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1
        ..=MAX_GENERAL_TYPED_V3_SEMANTIC_WITNESS_BYTES_V1)
        .contains(&bytes.len())
    {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::InvalidLength);
    }

    let magic = u64::from_le_bytes(bytes[0..8].try_into().expect("fixed witness magic range"));
    if magic != GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1 {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::MagicMismatch);
    }
    let version = u16::from_le_bytes(
        bytes[8..10]
            .try_into()
            .expect("fixed witness version range"),
    );
    if version != GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1 {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::VersionMismatch);
    }
    let domain = u16::from_le_bytes(
        bytes[10..12]
            .try_into()
            .expect("fixed witness domain range"),
    );
    if domain != GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1 {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::DomainMismatch);
    }

    let declared_length = usize::try_from(u32::from_le_bytes(
        bytes[12..16]
            .try_into()
            .expect("fixed witness length range"),
    ))
    .map_err(|_| CompilerGeneratedSemanticWitnessErrorV1::InvalidLength)?;
    if !(GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1
        ..=MAX_GENERAL_TYPED_V3_SEMANTIC_WITNESS_BYTES_V1)
        .contains(&declared_length)
    {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::InvalidLength);
    }
    if bytes.len() > declared_length {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::TrailingBytes);
    }
    if bytes.len() != declared_length {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::InvalidLength);
    }

    if bytes[16..48] != expected_kernel_binding {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::KernelBindingMismatch);
    }
    if bytes[48..80] != expected_generated_host_contract {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::GeneratedHostContractMismatch);
    }

    let profile_length = usize::from(u16::from_le_bytes(
        bytes[80..82]
            .try_into()
            .expect("fixed witness profile-length range"),
    ));
    let profile_end = GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1
        .checked_add(profile_length)
        .ok_or(CompilerGeneratedSemanticWitnessErrorV1::InvalidLength)?;
    if profile_end < declared_length {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::TrailingBytes);
    }
    if profile_end != declared_length {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::InvalidLength);
    }
    if bytes[GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1..profile_end]
        != *TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3.as_bytes()
    {
        return Err(CompilerGeneratedSemanticWitnessErrorV1::ProfileTagMismatch);
    }

    Ok(ValidatedCompilerGeneratedSemanticWitnessV1::general_v3(
        expected_kernel_binding,
        expected_generated_host_contract,
    ))
}

/// Exact generated host contract understood by this runtime version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[doc(hidden)]
pub struct CompilerGeneratedKernelProfileV1 {
    generated_host_contract_identity: [u8; 32],
}

impl CompilerGeneratedKernelProfileV1 {
    pub const fn new(generated_host_contract_identity: [u8; 32]) -> Self {
        Self {
            generated_host_contract_identity,
        }
    }

    pub const fn generated_host_contract_identity(self) -> [u8; 32] {
        self.generated_host_contract_identity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_device::KernelMarkerV1;

    struct ExpectationWithoutBackend;

    fn marker_function() {}

    unsafe impl KernelMarkerV1 for ExpectationWithoutBackend {
        type Function = fn();
        type Registration = ();

        const LOGICAL_NAME: &'static str = "general";
        const EXPORT_NAME: &'static str = "general";
        const FUNCTION: Self::Function = marker_function;
        const REGISTRATION: &'static Self::Registration = &();
    }

    unsafe impl CompilerGeneratedKernelExpectationV1 for ExpectationWithoutBackend {
        const PROFILE: CompilerGeneratedKernelProfileV1 =
            CompilerGeneratedKernelProfileV1::new([0x42; 32]);
        const KERNEL_BINDING_ID_V1: [u8; 32] = [0x41; 32];
    }

    struct SecondExpectation;

    fn second_marker_function() {}

    unsafe impl KernelMarkerV1 for SecondExpectation {
        type Function = fn();
        type Registration = ();

        const LOGICAL_NAME: &'static str = "second";
        const EXPORT_NAME: &'static str = "second_export";
        const FUNCTION: Self::Function = second_marker_function;
        const REGISTRATION: &'static Self::Registration = &();
    }

    unsafe impl CompilerGeneratedKernelExpectationV1 for SecondExpectation {
        const PROFILE: CompilerGeneratedKernelProfileV1 =
            CompilerGeneratedKernelProfileV1::new([0x52; 32]);
        const KERNEL_BINDING_ID_V1: [u8; 32] = [0x51; 32];
    }

    crate::compiler_generated_kernel_expectation_roster_v1! {
        struct OrderedTestRoster = [ExpectationWithoutBackend, SecondExpectation];
    }

    #[test]
    fn generated_expectation_roster_preserves_marker_order_and_identity() {
        let entries = OrderedTestRoster::ENTRIES;
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].logical_name(), "general");
        assert_eq!(entries[0].export_name(), "general");
        assert_eq!(entries[0].kernel_binding_id(), [0x41; 32]);
        assert_eq!(entries[0].generated_host_contract_identity(), [0x42; 32]);
        assert_eq!(entries[1].logical_name(), "second");
        assert_eq!(entries[1].export_name(), "second_export");
        assert_eq!(entries[1].kernel_binding_id(), [0x51; 32]);
        assert_eq!(entries[1].generated_host_contract_identity(), [0x52; 32]);
    }

    fn general_v3_semantic_witness_bytes(
        kernel_binding: [u8; 32],
        generated_host_contract: [u8; 32],
    ) -> Vec<u8> {
        let profile = TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3.as_bytes();
        let length = GENERAL_TYPED_V3_SEMANTIC_WITNESS_HEADER_BYTES_V1 + profile.len();
        let mut bytes = Vec::with_capacity(length);
        bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_MAGIC_V1.to_le_bytes());
        bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&GENERAL_TYPED_V3_SEMANTIC_WITNESS_DOMAIN_V1.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(length)
                .expect("test witness length fits u32")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&kernel_binding);
        bytes.extend_from_slice(&generated_host_contract);
        bytes.extend_from_slice(
            &u16::try_from(profile.len())
                .expect("test profile length fits u16")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(profile);
        assert_eq!(bytes.len(), length);
        bytes
    }

    fn parse_test_semantic_witness(
        bytes: &[u8],
        kernel_binding: [u8; 32],
        generated_host_contract: [u8; 32],
    ) -> Result<ValidatedCompilerGeneratedSemanticWitnessV1, CompilerGeneratedSemanticWitnessErrorV1>
    {
        // SAFETY: `bytes` is one initialized immutable allocation that remains
        // live for the complete parser call.
        unsafe {
            semantic_witness_from_backend_v1(
                bytes.as_ptr(),
                bytes.len(),
                kernel_binding,
                generated_host_contract,
            )
        }
    }

    #[test]
    fn general_v3_semantic_witness_is_exact_and_identity_bound() {
        let binding = [0x51; 32];
        let contract = [0x52; 32];
        let bytes = general_v3_semantic_witness_bytes(binding, contract);
        let witness = parse_test_semantic_witness(&bytes, binding, contract).unwrap();

        assert_eq!(
            witness.profile,
            CompilerGeneratedKernelProfileV1::new(contract)
        );
        assert_eq!(witness.kernel_binding, binding);
        assert_eq!(witness.generated_host_contract, contract);
    }

    #[test]
    fn general_v3_semantic_witness_rejects_malformed_and_substituted_payloads() {
        let binding = [0x61; 32];
        let contract = [0x62; 32];
        let canonical = general_v3_semantic_witness_bytes(binding, contract);

        let mut changed = canonical.clone();
        changed[0] ^= 1;
        assert_eq!(
            parse_test_semantic_witness(&changed, binding, contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::MagicMismatch)
        );

        let mut changed = canonical.clone();
        changed[8..10].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            parse_test_semantic_witness(&changed, binding, contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::VersionMismatch)
        );

        let mut changed = canonical.clone();
        changed[10..12].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            parse_test_semantic_witness(&changed, binding, contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::DomainMismatch)
        );

        let mut changed = canonical.clone();
        let too_long = u32::try_from(changed.len() + 1).unwrap();
        changed[12..16].copy_from_slice(&too_long.to_le_bytes());
        assert_eq!(
            parse_test_semantic_witness(&changed, binding, contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::InvalidLength)
        );

        assert_eq!(
            parse_test_semantic_witness(&canonical, [0x63; 32], contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::KernelBindingMismatch)
        );
        assert_eq!(
            parse_test_semantic_witness(&canonical, binding, [0x64; 32]),
            Err(CompilerGeneratedSemanticWitnessErrorV1::GeneratedHostContractMismatch)
        );

        let mut changed = canonical.clone();
        *changed.last_mut().expect("profile tag is nonempty") ^= 1;
        assert_eq!(
            parse_test_semantic_witness(&changed, binding, contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::ProfileTagMismatch)
        );

        let mut changed = canonical.clone();
        changed.push(0);
        assert_eq!(
            parse_test_semantic_witness(&changed, binding, contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::TrailingBytes)
        );

        let mut changed = canonical.clone();
        changed[80..82].copy_from_slice(&0_u16.to_le_bytes());
        assert_eq!(
            parse_test_semantic_witness(&changed, binding, contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::TrailingBytes)
        );

        assert_eq!(
            parse_general_typed_v3_semantic_witness_v1(&[], binding, contract),
            Err(CompilerGeneratedSemanticWitnessErrorV1::InvalidLength)
        );
    }

    #[test]
    fn semantic_authority_requires_a_backend_witness() {
        assert_eq!(
            validate_compiler_generated_semantic_witness_v1::<ExpectationWithoutBackend>(),
            Err(CompilerGeneratedSemanticWitnessErrorV1::MissingBackendWitness)
        );
    }
}
