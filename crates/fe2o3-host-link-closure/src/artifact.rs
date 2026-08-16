use crate::digest::Sha256Digest;
use crate::error::{HostLinkError, HostLinkErrorCodeV1, ResultContext};
use crate::model::{
    ArtifactIdV1, ArtifactIdentityV1, ArtifactProvenanceV1, HostArtifactKindV1,
    HostLinkPlanManifestV1, HostLinkPlanSpecV1, ProducerArtifactSpecV1, ReleaseNonceV1,
    TargetTripleV1,
};
use crate::platform;
use crate::{
    MAX_HOST_LINK_INPUT_BYTES_V1, MAX_HOST_LINK_PLAN_BYTES_V1, MAX_HOST_LINK_PRODUCERS_V1,
    MAX_HOST_LINK_RETAINED_BYTES_V1,
};
use std::collections::BTreeMap;
use std::fs::File;

pub(crate) fn checked_retained_bytes(
    current: u64,
    additional: u64,
    context: &str,
) -> Result<u64, HostLinkError> {
    if additional == 0 || additional > MAX_HOST_LINK_INPUT_BYTES_V1 {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::ArtifactTooLarge,
            format!("{context} contains an input outside the 256 MiB per-input bound"),
        ));
    }
    current
        .checked_add(additional)
        .filter(|total| *total <= MAX_HOST_LINK_RETAINED_BYTES_V1)
        .ok_or_else(|| {
            HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactTooLarge,
                format!("{context} exceeds the 2 GiB aggregate retained-input bound"),
            )
        })
}

fn validate_retained_set(
    count: usize,
    sizes: impl IntoIterator<Item = u64>,
    context: &str,
) -> Result<u64, HostLinkError> {
    if count > MAX_HOST_LINK_PRODUCERS_V1 {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::FieldTooLarge,
            format!("{context} count exceeds the canonical argv bound"),
        ));
    }
    let mut retained_bytes = 0_u64;
    for size in sizes {
        retained_bytes = checked_retained_bytes(retained_bytes, size, context)?;
    }
    Ok(retained_bytes)
}

pub struct PublishedHostArtifactV1 {
    identity: ArtifactIdentityV1,
    file: File,
    archive_members: u64,
}

impl PublishedHostArtifactV1 {
    pub fn from_producer_fd(
        file: File,
        spec: ProducerArtifactSpecV1,
    ) -> Result<Self, HostLinkError> {
        if matches!(spec.kind, HostArtifactKindV1::BuildScriptNative)
            && spec.provenance != ArtifactProvenanceV1::BuildScript
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::UnpublishedBuildScript,
                "build-script native artifact has no build-script provenance",
            ));
        }
        let captured =
            platform::capture_to_sealed_memfd(file, &spec.label, MAX_HOST_LINK_INPUT_BYTES_V1)?;
        if spec
            .expected_sha256
            .is_some_and(|expected| expected != captured.sha256)
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::DigestMismatch,
                format!("producer {} does not match its expected digest", spec.label),
            ));
        }
        let inspection = platform::inspect_artifact(spec.kind, &captured.bytes)?;
        let identity = ArtifactIdentityV1::new(
            spec.label,
            spec.kind,
            spec.provenance,
            captured.sha256,
            captured.size,
            captured.mode,
            spec.release_nonce,
            spec.target,
            inspection.elf_profile,
        )?;
        Ok(Self {
            identity,
            file: captured.file,
            archive_members: inspection.archive_members,
        })
    }

    pub fn identity(&self) -> &ArtifactIdentityV1 {
        &self.identity
    }

    pub fn id(&self) -> ArtifactIdV1 {
        self.identity.id
    }

    pub fn try_clone_file(&self) -> Result<File, HostLinkError> {
        self.file.try_clone().context(HostLinkErrorCodeV1::Io, || {
            format!("clone published artifact {}", self.identity.label)
        })
    }

    pub fn revalidate(&self) -> Result<(), HostLinkError> {
        platform::verify_sealed_artifact(
            &self.file,
            self.identity.sha256,
            self.identity.size,
            self.identity.mode,
            &self.identity.label,
        )
    }

    pub(crate) fn sealed_bytes(&self) -> Result<Vec<u8>, HostLinkError> {
        Ok(platform::read_sealed_file(
            self.try_clone_file()?,
            &self.identity.label,
            MAX_HOST_LINK_INPUT_BYTES_V1,
        )?
        .bytes)
    }

    pub(crate) fn from_sealed_fd(
        file: File,
        identity: ArtifactIdentityV1,
    ) -> Result<Self, HostLinkError> {
        identity.validate_id()?;
        let captured =
            platform::read_sealed_file(file, &identity.label, MAX_HOST_LINK_INPUT_BYTES_V1)?;
        if captured.sha256 != identity.sha256
            || captured.size != identity.size
            || captured.mode != identity.mode
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::DigestMismatch,
                format!("sealed {} does not match its plan identity", identity.label),
            ));
        }
        let inspection = platform::inspect_artifact(identity.kind, &captured.bytes)?;
        if inspection.elf_profile != identity.elf_profile {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                format!("sealed {} profile does not match its plan", identity.label),
            ));
        }
        Ok(Self {
            identity,
            file: captured.file,
            archive_members: inspection.archive_members,
        })
    }

    pub(crate) const fn archive_members(&self) -> u64 {
        self.archive_members
    }

    fn into_file(self) -> File {
        self.file
    }
}

pub struct HostLinkHandoffV1 {
    plan_file: File,
    producer_files: Vec<File>,
    manifest: HostLinkPlanManifestV1,
}

impl HostLinkHandoffV1 {
    pub fn new(
        spec: HostLinkPlanSpecV1,
        mut producers: Vec<PublishedHostArtifactV1>,
    ) -> Result<Self, HostLinkError> {
        validate_retained_set(
            producers.len(),
            producers.iter().map(|producer| producer.identity.size),
            "host-link handoff producer",
        )?;
        producers.sort_by_key(PublishedHostArtifactV1::id);
        let manifest = HostLinkPlanManifestV1 {
            spec,
            producers: producers
                .iter()
                .map(|artifact| artifact.identity.clone())
                .collect(),
            plan_digest: Sha256Digest::ZERO,
        };
        let bytes = manifest.encode_canonical()?;
        let manifest = HostLinkPlanManifestV1::decode_canonical(&bytes)?;
        let plan_file = platform::sealed_file_from_bytes(&bytes, "host-link plan")?;
        Ok(Self {
            plan_file,
            producer_files: producers
                .into_iter()
                .map(PublishedHostArtifactV1::into_file)
                .collect(),
            manifest,
        })
    }

    pub fn manifest(&self) -> &HostLinkPlanManifestV1 {
        &self.manifest
    }

    pub fn into_parts(self) -> (File, Vec<File>) {
        (self.plan_file, self.producer_files)
    }
}

pub struct HostLinkPlanV1 {
    manifest: HostLinkPlanManifestV1,
    plan_file: File,
    producers: BTreeMap<ArtifactIdV1, PublishedHostArtifactV1>,
}

impl HostLinkPlanV1 {
    pub fn from_sealed_fd(plan_file: File, producer_fds: Vec<File>) -> Result<Self, HostLinkError> {
        let captured = platform::read_sealed_file(
            plan_file,
            "host-link plan",
            MAX_HOST_LINK_PLAN_BYTES_V1 as u64,
        )?;
        let manifest = HostLinkPlanManifestV1::decode_canonical(&captured.bytes)?;
        validate_retained_set(
            manifest.producers.len(),
            manifest.producers.iter().map(|identity| identity.size),
            "sealed host-link plan producer",
        )?;
        if producer_fds.len() != manifest.producers.len() {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ReplayMismatch,
                format!(
                    "plan binds {} producer descriptors but {} were transferred",
                    manifest.producers.len(),
                    producer_fds.len()
                ),
            ));
        }
        let producers = manifest
            .producers
            .iter()
            .cloned()
            .zip(producer_fds)
            .map(|(identity, file)| {
                PublishedHostArtifactV1::from_sealed_fd(file, identity)
                    .map(|artifact| (artifact.id(), artifact))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(Self {
            manifest,
            plan_file: captured.file,
            producers,
        })
    }

    pub fn manifest(&self) -> &HostLinkPlanManifestV1 {
        &self.manifest
    }

    pub fn plan_digest(&self) -> Sha256Digest {
        self.manifest.plan_digest
    }

    pub fn release_nonce(&self) -> ReleaseNonceV1 {
        self.manifest.spec.release_nonce
    }

    pub fn target(&self) -> &TargetTripleV1 {
        &self.manifest.spec.target
    }

    pub(crate) fn producer(&self, id: ArtifactIdV1) -> Option<&PublishedHostArtifactV1> {
        self.producers.get(&id)
    }

    pub fn revalidate(&self) -> Result<(), HostLinkError> {
        platform::read_sealed_file(
            self.plan_file
                .try_clone()
                .context(HostLinkErrorCodeV1::Io, || {
                    "clone sealed host-link plan".to_owned()
                })?,
            "host-link plan",
            MAX_HOST_LINK_PLAN_BYTES_V1 as u64,
        )?;
        for artifact in self.producers.values() {
            artifact.revalidate()?;
        }
        Ok(())
    }
}

#[derive(Default)]
pub struct HostArtifactCatalogV1 {
    release_nonce: Option<ReleaseNonceV1>,
    target: Option<TargetTripleV1>,
    artifacts: BTreeMap<ArtifactIdV1, PublishedHostArtifactV1>,
    retained_bytes: u64,
}

impl HostArtifactCatalogV1 {
    pub fn new(release_nonce: ReleaseNonceV1, target: TargetTripleV1) -> Self {
        Self {
            release_nonce: Some(release_nonce),
            target: Some(target),
            artifacts: BTreeMap::new(),
            retained_bytes: 0,
        }
    }

    pub fn insert(&mut self, artifact: PublishedHostArtifactV1) -> Result<(), HostLinkError> {
        if Some(artifact.identity.release_nonce) != self.release_nonce {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::WrongNonce,
                "catalog artifact release nonce does not match the catalog",
            ));
        }
        if self.target.as_ref() != Some(&artifact.identity.target) {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::WrongTarget,
                "catalog artifact target does not match the catalog",
            ));
        }
        if self.artifacts.contains_key(&artifact.id()) {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::DuplicateRecord,
                "catalog already contains this artifact identity",
            ));
        }
        if self.artifacts.len() >= MAX_HOST_LINK_PRODUCERS_V1 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "artifact catalog count exceeds the canonical argv bound",
            ));
        }
        let retained_bytes = checked_retained_bytes(
            self.retained_bytes,
            artifact.identity.size,
            "artifact catalog",
        )?;
        self.artifacts.insert(artifact.id(), artifact);
        self.retained_bytes = retained_bytes;
        Ok(())
    }

    pub fn get(&self, id: ArtifactIdV1) -> Option<&PublishedHostArtifactV1> {
        self.artifacts.get(&id)
    }

    pub(crate) fn validate_binding(
        &self,
        release_nonce: ReleaseNonceV1,
        target: &TargetTripleV1,
    ) -> Result<(), HostLinkError> {
        if self.release_nonce != Some(release_nonce) {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::WrongNonce,
                "artifact catalog is bound to a different release nonce",
            ));
        }
        if self.target.as_ref() != Some(target) {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::WrongTarget,
                "artifact catalog is bound to a different target",
            ));
        }
        Ok(())
    }

    pub fn revalidate(&self) -> Result<(), HostLinkError> {
        if self.artifacts.len() > MAX_HOST_LINK_PRODUCERS_V1 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "artifact catalog count exceeds the canonical argv bound",
            ));
        }
        let mut retained_bytes = 0_u64;
        for artifact in self.artifacts.values() {
            retained_bytes =
                checked_retained_bytes(retained_bytes, artifact.identity.size, "artifact catalog")?;
            artifact.revalidate()?;
        }
        if retained_bytes != self.retained_bytes {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::DescriptorChanged,
                "artifact catalog aggregate accounting changed",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_resource_boundaries_and_overflow_fail_before_file_reads() {
        assert_eq!(
            checked_retained_bytes(0, MAX_HOST_LINK_INPUT_BYTES_V1, "test").unwrap(),
            MAX_HOST_LINK_INPUT_BYTES_V1
        );
        assert_eq!(
            checked_retained_bytes(
                MAX_HOST_LINK_RETAINED_BYTES_V1 - MAX_HOST_LINK_INPUT_BYTES_V1,
                MAX_HOST_LINK_INPUT_BYTES_V1,
                "test",
            )
            .unwrap(),
            MAX_HOST_LINK_RETAINED_BYTES_V1
        );
        for (current, additional) in [
            (0, MAX_HOST_LINK_INPUT_BYTES_V1 + 1),
            (MAX_HOST_LINK_RETAINED_BYTES_V1, 1),
            (u64::MAX, 1),
        ] {
            assert_eq!(
                checked_retained_bytes(current, additional, "test")
                    .unwrap_err()
                    .code(),
                HostLinkErrorCodeV1::ArtifactTooLarge
            );
        }
    }

    #[test]
    fn retained_set_count_and_aggregate_are_bounded_without_allocating_artifacts() {
        assert_eq!(
            validate_retained_set(
                8,
                std::iter::repeat_n(MAX_HOST_LINK_INPUT_BYTES_V1, 8),
                "test set",
            )
            .unwrap(),
            MAX_HOST_LINK_RETAINED_BYTES_V1
        );
        assert_eq!(
            validate_retained_set(
                9,
                std::iter::repeat_n(MAX_HOST_LINK_INPUT_BYTES_V1, 9),
                "test set",
            )
            .unwrap_err()
            .code(),
            HostLinkErrorCodeV1::ArtifactTooLarge
        );
        assert!(
            validate_retained_set(
                MAX_HOST_LINK_PRODUCERS_V1,
                std::iter::repeat_n(1, MAX_HOST_LINK_PRODUCERS_V1),
                "test set",
            )
            .is_ok()
        );
        assert_eq!(
            validate_retained_set(
                MAX_HOST_LINK_PRODUCERS_V1 + 1,
                std::iter::empty(),
                "test set",
            )
            .unwrap_err()
            .code(),
            HostLinkErrorCodeV1::FieldTooLarge
        );
    }
}
