use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use super::{
    ExternalDeviceLibraryContentIdentityV1, ExternalDeviceLibraryContentKindV1,
    ExternalDeviceLibraryDependencyV1, ExternalDeviceLibraryManifestIdentityV1,
    ExternalDeviceLibraryManifestV1, ExternalDeviceSymbolV1,
    MAX_EXTERNAL_DEVICE_LIBRARY_DEPENDENCIES_V1,
};

const ELF_CLASS_64: u8 = 2;
const ELF_DATA_LITTLE_ENDIAN: u8 = 1;
const ELF_HEADER_SIZE_64: usize = 64;
const ELF_OSABI_AMDGPU_HSA: u8 = 64;
const ELF_TYPE_RELOCATABLE: u16 = 1;
const ELF_MACHINE_AMDGPU: u16 = 224;
const ELF_VERSION_CURRENT: u32 = 1;

/// Maximum exact bytes in one external device-library provider blob.
pub const MAX_EXTERNAL_DEVICE_LIBRARY_PROVIDER_CONTENT_BYTES_V1: usize = 64 * 1024 * 1024;
/// Maximum aggregate bytes in all provider blobs supplied for one closure validation.
pub const MAX_EXTERNAL_DEVICE_LIBRARY_PROVIDER_CLOSURE_BYTES_V1: usize = 64 * 1024 * 1024;

/// One actual provider manifest paired with the exact bytes it describes.
///
/// Construction checks byte bounds, declared length, and the representation header without
/// hashing content. Exact digest validation is deferred until the complete provider set has passed
/// its aggregate byte preflight. This does not authenticate the producer, prove the symbol
/// declarations, or grant link authority.
#[derive(Clone, Copy)]
pub struct ExternalDeviceLibraryProviderV1<'a> {
    manifest: &'a ExternalDeviceLibraryManifestV1,
    content_bytes: &'a [u8],
}

impl fmt::Debug for ExternalDeviceLibraryProviderV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExternalDeviceLibraryProviderV1")
            .field("manifest_identity", &self.manifest.identity())
            .field("content_identity", &self.manifest.content())
            .finish_non_exhaustive()
    }
}

impl<'a> ExternalDeviceLibraryProviderV1<'a> {
    pub fn new(
        manifest: &'a ExternalDeviceLibraryManifestV1,
        content_bytes: &'a [u8],
    ) -> Result<Self, ExternalDeviceLibraryProviderSetErrorV1> {
        let value = Self {
            manifest,
            content_bytes,
        };
        value.preflight_content_size()?;
        validate_representation(value.manifest.content().kind(), value.content_bytes)?;
        Ok(value)
    }

    pub const fn manifest(&self) -> &'a ExternalDeviceLibraryManifestV1 {
        self.manifest
    }

    pub const fn content_bytes(&self) -> &'a [u8] {
        self.content_bytes
    }

    pub const fn authenticates_provider_origin(&self) -> bool {
        false
    }

    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    fn preflight_content_size(&self) -> Result<usize, ExternalDeviceLibraryProviderSetErrorV1> {
        preflight_provider_content_size(
            self.manifest.content().blob().byte_len(),
            self.content_bytes.len(),
        )
    }

    fn validate_content_digest(&self) -> Result<(), ExternalDeviceLibraryProviderSetErrorV1> {
        if !self.manifest.content().matches(self.content_bytes) {
            return Err(ExternalDeviceLibraryProviderSetErrorV1::ProviderContentDigestMismatch);
        }
        Ok(())
    }
}

/// Structural result for one exact provider closure.
///
/// This records which identities passed the current checks. It is not an attestation and grants
/// no compiler, verification, link, load, or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalDeviceLibraryProviderSetValidationV1 {
    root_manifest_identity: ExternalDeviceLibraryManifestIdentityV1,
    providers: Vec<(
        ExternalDeviceLibraryManifestIdentityV1,
        ExternalDeviceLibraryContentIdentityV1,
    )>,
}

impl ExternalDeviceLibraryProviderSetValidationV1 {
    pub const fn root_manifest_identity(&self) -> ExternalDeviceLibraryManifestIdentityV1 {
        self.root_manifest_identity
    }

    pub fn providers(
        &self,
    ) -> impl ExactSizeIterator<
        Item = (
            ExternalDeviceLibraryManifestIdentityV1,
            ExternalDeviceLibraryContentIdentityV1,
        ),
    > + '_ {
        self.providers.iter().copied()
    }

    pub const fn authenticates_provider_origin(&self) -> bool {
        false
    }

    pub const fn authenticates_verification(&self) -> bool {
        false
    }

    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl ExternalDeviceLibraryManifestV1 {
    /// Validates the exact actual provider set and every import edge in its transitive closure.
    ///
    /// Input order has no meaning. The root dependency list must name every provider exactly once,
    /// including transitive providers. Each provider's own dependency records then select which
    /// provider exports satisfy its imports.
    pub fn validate_provider_set(
        &self,
        providers: &[ExternalDeviceLibraryProviderV1<'_>],
    ) -> Result<ExternalDeviceLibraryProviderSetValidationV1, ExternalDeviceLibraryProviderSetErrorV1>
    {
        preflight_provider_set_sizes(providers)?;

        let mut providers_by_identity = BTreeMap::new();
        let mut closure_blobs = BTreeSet::from([self.content().blob()]);
        for provider in providers {
            provider.validate_content_digest()?;
            if provider.manifest.identity() == self.identity() {
                return Err(ExternalDeviceLibraryProviderSetErrorV1::RootRepeatedAsProvider);
            }
            if providers_by_identity
                .insert(provider.manifest.identity(), *provider)
                .is_some()
            {
                return Err(ExternalDeviceLibraryProviderSetErrorV1::DuplicateProviderManifest);
            }
            if !closure_blobs.insert(provider.manifest.content().blob()) {
                return Err(ExternalDeviceLibraryProviderSetErrorV1::DuplicateProviderBlob);
            }
        }

        let expected = self
            .dependencies()
            .iter()
            .map(ExternalDeviceLibraryDependencyV1::manifest_identity)
            .collect::<BTreeSet<_>>();
        for dependency in self.dependencies() {
            let provider = providers_by_identity
                .get(&dependency.manifest_identity())
                .ok_or(ExternalDeviceLibraryProviderSetErrorV1::MissingProvider)?;
            validate_dependency_identity(dependency, provider.manifest)?;
        }
        if providers_by_identity
            .keys()
            .any(|identity| !expected.contains(identity))
        {
            return Err(ExternalDeviceLibraryProviderSetErrorV1::UnexpectedProvider);
        }

        validate_unique_exports(self, providers)?;
        validate_consumer(self, &providers_by_identity)?;
        for provider in providers {
            validate_consumer(provider.manifest, &providers_by_identity)?;
        }

        let providers = providers_by_identity
            .into_iter()
            .map(|(identity, provider)| (identity, provider.manifest.content()))
            .collect();
        Ok(ExternalDeviceLibraryProviderSetValidationV1 {
            root_manifest_identity: self.identity(),
            providers,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ExternalDeviceLibraryProviderSetErrorV1 {
    TooManyProviders,
    ProviderContentByteLimitExceeded,
    ProviderClosureByteLimitExceeded,
    ProviderContentDigestMismatch,
    ProviderContentRepresentationMismatch,
    RootRepeatedAsProvider,
    DuplicateProviderManifest,
    DuplicateProviderBlob,
    MissingProvider,
    UnexpectedProvider,
    DependencyContentMismatch,
    TargetMismatch,
    CodeObjectVersionMismatch,
    LlvmIncompatible,
    DuplicateProviderExport,
    MissingProviderExport,
    ImportExportContractMismatch,
}

impl fmt::Display for ExternalDeviceLibraryProviderSetErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyProviders => {
                formatter.write_str("too many actual device-library providers")
            }
            Self::ProviderContentByteLimitExceeded => {
                formatter.write_str("device-library provider content exceeds its byte limit")
            }
            Self::ProviderClosureByteLimitExceeded => formatter
                .write_str("device-library provider closure exceeds its aggregate byte limit"),
            Self::ProviderContentDigestMismatch => {
                formatter.write_str("provider content does not match its exact digest and length")
            }
            Self::ProviderContentRepresentationMismatch => {
                formatter.write_str("provider bytes do not match their declared representation")
            }
            Self::RootRepeatedAsProvider => {
                formatter.write_str("root manifest repeated as provider")
            }
            Self::DuplicateProviderManifest => {
                formatter.write_str("duplicate actual provider manifest")
            }
            Self::DuplicateProviderBlob => {
                formatter.write_str("provider closure reuses one exact blob")
            }
            Self::MissingProvider => formatter.write_str("declared provider is missing"),
            Self::UnexpectedProvider => {
                formatter.write_str("actual provider is not in the closure")
            }
            Self::DependencyContentMismatch => {
                formatter.write_str("dependency content identity disagrees with its provider")
            }
            Self::TargetMismatch => formatter.write_str("provider target does not match consumer"),
            Self::CodeObjectVersionMismatch => {
                formatter.write_str("provider code-object version does not match consumer")
            }
            Self::LlvmIncompatible => formatter.write_str("provider LLVM identity is incompatible"),
            Self::DuplicateProviderExport => {
                formatter.write_str("provider closure contains duplicate exported symbols")
            }
            Self::MissingProviderExport => {
                formatter.write_str("assigned provider does not export the requested symbol")
            }
            Self::ImportExportContractMismatch => {
                formatter.write_str("import and provider export contracts disagree")
            }
        }
    }
}

impl Error for ExternalDeviceLibraryProviderSetErrorV1 {}

fn preflight_provider_set_sizes(
    providers: &[ExternalDeviceLibraryProviderV1<'_>],
) -> Result<(), ExternalDeviceLibraryProviderSetErrorV1> {
    if providers.len() > MAX_EXTERNAL_DEVICE_LIBRARY_DEPENDENCIES_V1 {
        return Err(ExternalDeviceLibraryProviderSetErrorV1::TooManyProviders);
    }
    let mut aggregate_byte_len = 0;
    for provider in providers {
        aggregate_byte_len = add_provider_content_to_closure(
            aggregate_byte_len,
            provider.preflight_content_size()?,
        )?;
    }
    Ok(())
}

fn preflight_provider_content_size(
    declared_byte_len: u64,
    actual_byte_len: usize,
) -> Result<usize, ExternalDeviceLibraryProviderSetErrorV1> {
    if declared_byte_len > MAX_EXTERNAL_DEVICE_LIBRARY_PROVIDER_CONTENT_BYTES_V1 as u64
        || actual_byte_len > MAX_EXTERNAL_DEVICE_LIBRARY_PROVIDER_CONTENT_BYTES_V1
    {
        return Err(ExternalDeviceLibraryProviderSetErrorV1::ProviderContentByteLimitExceeded);
    }
    if u64::try_from(actual_byte_len).ok() != Some(declared_byte_len) {
        return Err(ExternalDeviceLibraryProviderSetErrorV1::ProviderContentDigestMismatch);
    }
    Ok(actual_byte_len)
}

fn add_provider_content_to_closure(
    aggregate_byte_len: usize,
    provider_byte_len: usize,
) -> Result<usize, ExternalDeviceLibraryProviderSetErrorV1> {
    aggregate_byte_len
        .checked_add(provider_byte_len)
        .filter(|total| *total <= MAX_EXTERNAL_DEVICE_LIBRARY_PROVIDER_CLOSURE_BYTES_V1)
        .ok_or(ExternalDeviceLibraryProviderSetErrorV1::ProviderClosureByteLimitExceeded)
}

fn validate_representation(
    kind: ExternalDeviceLibraryContentKindV1,
    bytes: &[u8],
) -> Result<(), ExternalDeviceLibraryProviderSetErrorV1> {
    let valid = match kind {
        ExternalDeviceLibraryContentKindV1::LlvmBitcode => {
            bytes.starts_with(&[b'B', b'C', 0xc0, 0xde])
                || bytes.starts_with(&[0xde, 0xc0, 0x17, 0x0b])
        }
        ExternalDeviceLibraryContentKindV1::RelocatableObject => {
            bytes.len() >= ELF_HEADER_SIZE_64
                && bytes.starts_with(b"\x7fELF")
                && bytes[4] == ELF_CLASS_64
                && bytes[5] == ELF_DATA_LITTLE_ENDIAN
                && bytes[6] == ELF_VERSION_CURRENT as u8
                && bytes[7] == ELF_OSABI_AMDGPU_HSA
                && u16::from_le_bytes([bytes[16], bytes[17]]) == ELF_TYPE_RELOCATABLE
                && u16::from_le_bytes([bytes[18], bytes[19]]) == ELF_MACHINE_AMDGPU
                && u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]])
                    == ELF_VERSION_CURRENT
                && usize::from(u16::from_le_bytes([bytes[52], bytes[53]])) == ELF_HEADER_SIZE_64
        }
        ExternalDeviceLibraryContentKindV1::CodeObject
        | ExternalDeviceLibraryContentKindV1::StaticArchive => false,
    };
    if valid {
        Ok(())
    } else {
        Err(ExternalDeviceLibraryProviderSetErrorV1::ProviderContentRepresentationMismatch)
    }
}

fn validate_dependency_identity(
    dependency: &ExternalDeviceLibraryDependencyV1,
    provider: &ExternalDeviceLibraryManifestV1,
) -> Result<(), ExternalDeviceLibraryProviderSetErrorV1> {
    if dependency.manifest_identity() != provider.identity() {
        return Err(ExternalDeviceLibraryProviderSetErrorV1::MissingProvider);
    }
    if dependency.content_identity() != provider.content() {
        return Err(ExternalDeviceLibraryProviderSetErrorV1::DependencyContentMismatch);
    }
    Ok(())
}

fn validate_compatibility(
    consumer: &ExternalDeviceLibraryManifestV1,
    provider: &ExternalDeviceLibraryManifestV1,
) -> Result<(), ExternalDeviceLibraryProviderSetErrorV1> {
    if consumer.target() != provider.target() {
        return Err(ExternalDeviceLibraryProviderSetErrorV1::TargetMismatch);
    }
    if consumer.code_object_version() != provider.code_object_version() {
        return Err(ExternalDeviceLibraryProviderSetErrorV1::CodeObjectVersionMismatch);
    }
    if !consumer.llvm().is_link_compatible_with(provider.llvm()) {
        return Err(ExternalDeviceLibraryProviderSetErrorV1::LlvmIncompatible);
    }
    Ok(())
}

fn validate_consumer(
    consumer: &ExternalDeviceLibraryManifestV1,
    providers: &BTreeMap<
        ExternalDeviceLibraryManifestIdentityV1,
        ExternalDeviceLibraryProviderV1<'_>,
    >,
) -> Result<(), ExternalDeviceLibraryProviderSetErrorV1> {
    for dependency in consumer.dependencies() {
        let provider = providers
            .get(&dependency.manifest_identity())
            .ok_or(ExternalDeviceLibraryProviderSetErrorV1::MissingProvider)?;
        validate_dependency_identity(dependency, provider.manifest)?;
        validate_compatibility(consumer, provider.manifest)?;
        for import_name in dependency.resolved_imports() {
            let import = consumer
                .imports()
                .find(|symbol| symbol.symbol() == import_name)
                .ok_or(ExternalDeviceLibraryProviderSetErrorV1::ImportExportContractMismatch)?;
            let export = provider
                .manifest
                .exports()
                .find(|symbol| symbol.symbol() == import_name)
                .ok_or(ExternalDeviceLibraryProviderSetErrorV1::MissingProviderExport)?;
            if !contracts_match(import, export) {
                return Err(ExternalDeviceLibraryProviderSetErrorV1::ImportExportContractMismatch);
            }
        }
    }
    Ok(())
}

fn validate_unique_exports(
    root: &ExternalDeviceLibraryManifestV1,
    providers: &[ExternalDeviceLibraryProviderV1<'_>],
) -> Result<(), ExternalDeviceLibraryProviderSetErrorV1> {
    let mut exports = BTreeSet::new();
    for export in root.exports() {
        exports.insert(export.symbol());
    }
    for provider in providers {
        for export in provider.manifest.exports() {
            if !exports.insert(export.symbol()) {
                return Err(ExternalDeviceLibraryProviderSetErrorV1::DuplicateProviderExport);
            }
        }
    }
    Ok(())
}

fn contracts_match(import: &ExternalDeviceSymbolV1, export: &ExternalDeviceSymbolV1) -> bool {
    import.symbol() == export.symbol()
        && import.calling_convention() == export.calling_convention()
        && import.physical_abi() == export.physical_abi()
        && import.address_spaces() == export.address_spaces()
        && import.effects() == export.effects()
        && import.convergence() == export.convergence()
        && import.semantic_identity() == export.semantic_identity()
        && import.required_capabilities() == export.required_capabilities()
}

#[cfg(test)]
mod tests {
    use super::{
        ExternalDeviceLibraryProviderSetErrorV1,
        MAX_EXTERNAL_DEVICE_LIBRARY_PROVIDER_CLOSURE_BYTES_V1,
        MAX_EXTERNAL_DEVICE_LIBRARY_PROVIDER_CONTENT_BYTES_V1, add_provider_content_to_closure,
        preflight_provider_content_size,
    };

    #[test]
    fn provider_content_size_preflight_has_exact_boundaries() {
        let maximum = MAX_EXTERNAL_DEVICE_LIBRARY_PROVIDER_CONTENT_BYTES_V1;
        assert_eq!(
            preflight_provider_content_size(maximum as u64, maximum),
            Ok(maximum)
        );
        assert_eq!(
            preflight_provider_content_size(maximum as u64 + 1, 1),
            Err(ExternalDeviceLibraryProviderSetErrorV1::ProviderContentByteLimitExceeded)
        );
        assert_eq!(
            preflight_provider_content_size(1, maximum + 1),
            Err(ExternalDeviceLibraryProviderSetErrorV1::ProviderContentByteLimitExceeded)
        );
        assert_eq!(
            preflight_provider_content_size(2, 1),
            Err(ExternalDeviceLibraryProviderSetErrorV1::ProviderContentDigestMismatch)
        );
    }

    #[test]
    fn provider_closure_size_preflight_rejects_limit_and_arithmetic_overflow() {
        let maximum = MAX_EXTERNAL_DEVICE_LIBRARY_PROVIDER_CLOSURE_BYTES_V1;
        assert_eq!(add_provider_content_to_closure(maximum - 1, 1), Ok(maximum));
        assert_eq!(
            add_provider_content_to_closure(maximum, 1),
            Err(ExternalDeviceLibraryProviderSetErrorV1::ProviderClosureByteLimitExceeded)
        );
        assert_eq!(
            add_provider_content_to_closure(usize::MAX, 1),
            Err(ExternalDeviceLibraryProviderSetErrorV1::ProviderClosureByteLimitExceeded)
        );
    }
}
