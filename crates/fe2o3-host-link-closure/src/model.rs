use crate::digest::{Sha256Digest, domain_sha256};
use crate::error::{HostLinkError, HostLinkErrorCodeV1};
use crate::{MAX_HOST_LINK_ARGUMENTS_V1, MAX_HOST_LINK_INPUT_BYTES_V1, MAX_HOST_LINK_PRODUCERS_V1};
use std::collections::BTreeSet;
use std::path::{Component, Path};

const MAX_LABEL_BYTES: usize = 128;
const MAX_TARGET_BYTES: usize = 256;
const MAX_LLVM_IDENTITY_BYTES: usize = 1024;
const MAX_PATH_BYTES: usize = 4096;
const MAX_NEEDED_DSOS: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactIdV1(Sha256Digest);

impl ArtifactIdV1 {
    pub const fn from_sha256(digest: Sha256Digest) -> Self {
        Self(digest)
    }

    pub const fn sha256(self) -> Sha256Digest {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReleaseNonceV1([u8; 32]);

impl ReleaseNonceV1 {
    pub fn new(bytes: [u8; 32]) -> Result<Self, HostLinkError> {
        if bytes == [0; 32] {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidNonce,
                "release nonce must be nonzero",
            ));
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TargetTripleV1(String);

impl TargetTripleV1 {
    pub fn new(value: impl Into<String>) -> Result<Self, HostLinkError> {
        let value = value.into();
        validate_ascii_token("target triple", &value, MAX_TARGET_BYTES)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum HostArtifactKindV1 {
    StaticWrapper = 1,
    StaticHostLld = 2,
    Crt = 3,
    Object = 4,
    RegularArchive = 5,
    Rlib = 6,
    Dso = 7,
    LinkerScript = 8,
    ResponseFile = 9,
    BuildScriptNative = 10,
    Plugin = 11,
    LtoCache = 12,
}

impl HostArtifactKindV1 {
    pub(crate) fn from_tag(tag: u8) -> Result<Self, HostLinkError> {
        match tag {
            1 => Ok(Self::StaticWrapper),
            2 => Ok(Self::StaticHostLld),
            3 => Ok(Self::Crt),
            4 => Ok(Self::Object),
            5 => Ok(Self::RegularArchive),
            6 => Ok(Self::Rlib),
            7 => Ok(Self::Dso),
            8 => Ok(Self::LinkerScript),
            9 => Ok(Self::ResponseFile),
            10 => Ok(Self::BuildScriptNative),
            11 => Ok(Self::Plugin),
            12 => Ok(Self::LtoCache),
            _ => Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidWire,
                format!("unknown artifact kind tag {tag}"),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ArtifactProvenanceV1 {
    Compiler = 1,
    CargoCatalog = 2,
    FixedRoot = 3,
    BuildScript = 4,
}

impl ArtifactProvenanceV1 {
    pub(crate) fn from_tag(tag: u8) -> Result<Self, HostLinkError> {
        match tag {
            1 => Ok(Self::Compiler),
            2 => Ok(Self::CargoCatalog),
            3 => Ok(Self::FixedRoot),
            4 => Ok(Self::BuildScript),
            _ => Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidWire,
                format!("unknown artifact provenance tag {tag}"),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ElfClassV1 {
    Elf64 = 2,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ElfEndianV1 {
    Little = 1,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ElfProfileV1 {
    pub class: ElfClassV1,
    pub endian: ElfEndianV1,
    pub elf_type: u16,
    pub machine: u16,
    pub interpreter: Option<Vec<u8>>,
    pub soname: Option<Vec<u8>>,
    pub needed: Vec<Vec<u8>>,
    pub has_writable_executable_segment: bool,
    pub has_executable_stack: bool,
}

impl ElfProfileV1 {
    pub fn validate(&self) -> Result<(), HostLinkError> {
        if self.needed.len() > MAX_NEEDED_DSOS {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "ELF dependency list exceeds its bound",
            ));
        }
        if let Some(interpreter) = &self.interpreter {
            validate_bytes("ELF interpreter", interpreter, MAX_PATH_BYTES, false)?;
        }
        if let Some(soname) = &self.soname {
            validate_bytes("ELF SONAME", soname, MAX_PATH_BYTES, false)?;
            if soname.contains(&b'/') {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "ELF SONAME must not contain a path separator",
                ));
            }
        }
        let mut previous: Option<&[u8]> = None;
        for needed in &self.needed {
            validate_bytes("ELF dependency", needed, MAX_PATH_BYTES, false)?;
            if previous.is_some_and(|value| value >= needed.as_slice()) {
                return Err(HostLinkError::new(
                    if previous == Some(needed.as_slice()) {
                        HostLinkErrorCodeV1::DuplicateRecord
                    } else {
                        HostLinkErrorCodeV1::NonCanonicalOrder
                    },
                    "ELF dependencies must be unique and byte-sorted",
                ));
            }
            previous = Some(needed);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactIdentityV1 {
    pub id: ArtifactIdV1,
    pub label: String,
    pub kind: HostArtifactKindV1,
    pub provenance: ArtifactProvenanceV1,
    pub sha256: Sha256Digest,
    pub size: u64,
    pub mode: u32,
    pub release_nonce: ReleaseNonceV1,
    pub target: TargetTripleV1,
    pub elf_profile: Option<ElfProfileV1>,
}

impl ArtifactIdentityV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        label: String,
        kind: HostArtifactKindV1,
        provenance: ArtifactProvenanceV1,
        sha256: Sha256Digest,
        size: u64,
        mode: u32,
        release_nonce: ReleaseNonceV1,
        target: TargetTripleV1,
        elf_profile: Option<ElfProfileV1>,
    ) -> Result<Self, HostLinkError> {
        validate_ascii_token("artifact label", &label, MAX_LABEL_BYTES)?;
        if size == 0 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                "published artifact must not be empty",
            ));
        }
        if size > MAX_HOST_LINK_INPUT_BYTES_V1 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactTooLarge,
                "published artifact exceeds the 256 MiB per-input bound",
            ));
        }
        if matches!(
            kind,
            HostArtifactKindV1::StaticWrapper | HostArtifactKindV1::StaticHostLld
        ) && mode & 0o111 == 0
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ElfPolicy,
                "static host-link tool has no executable mode bit",
            ));
        }
        if kind == HostArtifactKindV1::BuildScriptNative
            && provenance != ArtifactProvenanceV1::BuildScript
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::UnpublishedBuildScript,
                "build-script native artifact lacks build-script provenance",
            ));
        }
        if let Some(profile) = &elf_profile {
            profile.validate()?;
        }
        let metadata = artifact_identity_preimage(
            &label,
            kind,
            provenance,
            sha256,
            size,
            mode,
            release_nonce,
            &target,
            elf_profile.as_ref(),
        );
        let id = ArtifactIdV1(domain_sha256(
            b"fe2o3-host-artifact-identity-v1\0",
            &[&metadata],
        ));
        Ok(Self {
            id,
            label,
            kind,
            provenance,
            sha256,
            size,
            mode,
            release_nonce,
            target,
            elf_profile,
        })
    }

    pub(crate) fn validate_id(&self) -> Result<(), HostLinkError> {
        let rebuilt = Self::new(
            self.label.clone(),
            self.kind,
            self.provenance,
            self.sha256,
            self.size,
            self.mode,
            self.release_nonce,
            self.target.clone(),
            self.elf_profile.clone(),
        )?;
        if rebuilt.id != self.id {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::DigestMismatch,
                "artifact identity digest does not match its fields",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProducerArtifactSpecV1 {
    pub label: String,
    pub kind: HostArtifactKindV1,
    pub provenance: ArtifactProvenanceV1,
    pub release_nonce: ReleaseNonceV1,
    pub target: TargetTripleV1,
    pub expected_sha256: Option<Sha256Digest>,
}

impl ProducerArtifactSpecV1 {
    pub fn new(
        label: impl Into<String>,
        kind: HostArtifactKindV1,
        provenance: ArtifactProvenanceV1,
        release_nonce: ReleaseNonceV1,
        target: TargetTripleV1,
    ) -> Result<Self, HostLinkError> {
        let label = label.into();
        validate_ascii_token("artifact label", &label, MAX_LABEL_BYTES)?;
        Ok(Self {
            label,
            kind,
            provenance,
            release_nonce,
            target,
            expected_sha256: None,
        })
    }

    pub fn with_expected_sha256(mut self, expected: Sha256Digest) -> Self {
        self.expected_sha256 = Some(expected);
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableToolchainV1 {
    pub static_wrapper: ArtifactIdV1,
    pub static_host_lld: ArtifactIdV1,
    pub llvm_build_identity: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum OutputTypeV1 {
    Executable = 1,
    SharedObject = 2,
    Relocatable = 3,
}

impl OutputTypeV1 {
    pub(crate) fn from_tag(tag: u8) -> Result<Self, HostLinkError> {
        match tag {
            1 => Ok(Self::Executable),
            2 => Ok(Self::SharedObject),
            3 => Ok(Self::Relocatable),
            _ => Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidWire,
                format!("unknown output type tag {tag}"),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum RootInputKindV1 {
    Crt = 1,
    Object = 2,
    RegularArchive = 3,
    Rlib = 4,
    Dso = 5,
    LinkerScript = 6,
    ResponseFile = 7,
}

impl RootInputKindV1 {
    pub(crate) fn from_tag(tag: u8) -> Result<Self, HostLinkError> {
        match tag {
            1 => Ok(Self::Crt),
            2 => Ok(Self::Object),
            3 => Ok(Self::RegularArchive),
            4 => Ok(Self::Rlib),
            5 => Ok(Self::Dso),
            6 => Ok(Self::LinkerScript),
            7 => Ok(Self::ResponseFile),
            _ => Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidWire,
                format!("unknown root input kind tag {tag}"),
            )),
        }
    }

    pub(crate) const fn artifact_kind(self) -> HostArtifactKindV1 {
        match self {
            Self::Crt => HostArtifactKindV1::Crt,
            Self::Object => HostArtifactKindV1::Object,
            Self::RegularArchive => HostArtifactKindV1::RegularArchive,
            Self::Rlib => HostArtifactKindV1::Rlib,
            Self::Dso => HostArtifactKindV1::Dso,
            Self::LinkerScript => HostArtifactKindV1::LinkerScript,
            Self::ResponseFile => HostArtifactKindV1::ResponseFile,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum LibraryPreferenceV1 {
    StaticOnly = 1,
    DynamicOnly = 2,
}

impl LibraryPreferenceV1 {
    pub(crate) fn from_tag(tag: u8) -> Result<Self, HostLinkError> {
        match tag {
            1 => Ok(Self::StaticOnly),
            2 => Ok(Self::DynamicOnly),
            _ => Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidWire,
                format!("unknown library preference tag {tag}"),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum LinkerZPolicyV1 {
    NoExecStack = 1,
    Relro = 2,
    Now = 3,
    SeparateCode = 4,
    Defs = 5,
    MaxPageSize4096 = 6,
    CommonPageSize4096 = 7,
}

impl LinkerZPolicyV1 {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoExecStack => "noexecstack",
            Self::Relro => "relro",
            Self::Now => "now",
            Self::SeparateCode => "separate-code",
            Self::Defs => "defs",
            Self::MaxPageSize4096 => "max-page-size=4096",
            Self::CommonPageSize4096 => "common-page-size=4096",
        }
    }

    pub(crate) fn from_str(value: &str) -> Result<Self, HostLinkError> {
        match value {
            "noexecstack" => Ok(Self::NoExecStack),
            "relro" => Ok(Self::Relro),
            "now" => Ok(Self::Now),
            "separate-code" => Ok(Self::SeparateCode),
            "defs" => Ok(Self::Defs),
            "max-page-size=4096" => Ok(Self::MaxPageSize4096),
            "common-page-size=4096" => Ok(Self::CommonPageSize4096),
            _ => Err(HostLinkError::new(
                HostLinkErrorCodeV1::UnsupportedArgument,
                "unsupported -z policy",
            )),
        }
    }

    pub(crate) fn from_tag(tag: u8) -> Result<Self, HostLinkError> {
        match tag {
            1 => Ok(Self::NoExecStack),
            2 => Ok(Self::Relro),
            3 => Ok(Self::Now),
            4 => Ok(Self::SeparateCode),
            5 => Ok(Self::Defs),
            6 => Ok(Self::MaxPageSize4096),
            7 => Ok(Self::CommonPageSize4096),
            _ => Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidWire,
                format!("unknown linker -z policy tag {tag}"),
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanArgumentV1 {
    Literal(Vec<u8>),
    ZPolicy(LinkerZPolicyV1),
    UndefinedSymbol(String),
    SearchRoot(String),
    Library {
        name: String,
        preference: LibraryPreferenceV1,
    },
    FixedRootInput {
        root: String,
        relative_path: Vec<u8>,
        kind: RootInputKindV1,
    },
    ProducerArtifact(ArtifactIdV1),
    CatalogArtifact(ArtifactIdV1),
    ResponseFile {
        root: String,
        relative_path: Vec<u8>,
    },
}

impl PlanArgumentV1 {
    pub(crate) fn validate(&self) -> Result<(), HostLinkError> {
        match self {
            Self::Literal(value) => crate::control::validate_literal(value),
            Self::ZPolicy(_) => Ok(()),
            Self::UndefinedSymbol(symbol) => crate::control::validate_undefined_symbol(symbol),
            Self::SearchRoot(root) => {
                validate_ascii_token("fixed-root label", root, MAX_LABEL_BYTES)
            }
            Self::Library { name, .. } => validate_library_name(name),
            Self::FixedRootInput {
                root,
                relative_path,
                ..
            }
            | Self::ResponseFile {
                root,
                relative_path,
            } => {
                validate_ascii_token("fixed-root label", root, MAX_LABEL_BYTES)?;
                validate_relative_path(relative_path)
            }
            Self::ProducerArtifact(_) | Self::CatalogArtifact(_) => Ok(()),
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DsoBindingV1 {
    pub soname: Vec<u8>,
    pub artifact: ArtifactIdV1,
    pub needed: Vec<Vec<u8>>,
}

impl DsoBindingV1 {
    pub(crate) fn validate(&self) -> Result<(), HostLinkError> {
        validate_bytes("DSO SONAME", &self.soname, MAX_PATH_BYTES, false)?;
        let mut previous: Option<&[u8]> = None;
        for needed in &self.needed {
            validate_bytes("DSO dependency", needed, MAX_PATH_BYTES, false)?;
            if previous.is_some_and(|value| value >= needed.as_slice()) {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::NonCanonicalOrder,
                    "DSO dependencies must be unique and byte-sorted",
                ));
            }
            previous = Some(needed);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeDsoClosureV1 {
    pub interpreter_artifact: Option<ArtifactIdV1>,
    pub bindings: Vec<DsoBindingV1>,
}

impl RuntimeDsoClosureV1 {
    pub(crate) fn validate(&self) -> Result<(), HostLinkError> {
        if self.bindings.len() > MAX_NEEDED_DSOS {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "runtime DSO closure exceeds its record bound",
            ));
        }
        let mut previous: Option<&[u8]> = None;
        for binding in &self.bindings {
            binding.validate()?;
            if previous.is_some_and(|value| value >= binding.soname.as_slice()) {
                return Err(HostLinkError::new(
                    if previous == Some(binding.soname.as_slice()) {
                        HostLinkErrorCodeV1::DuplicateRecord
                    } else {
                        HostLinkErrorCodeV1::NonCanonicalOrder
                    },
                    "runtime DSO bindings must be unique and SONAME-sorted",
                ));
            }
            previous = Some(&binding.soname);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostLinkPlanSpecV1 {
    pub release_nonce: ReleaseNonceV1,
    pub target: TargetTripleV1,
    pub toolchain: ExecutableToolchainV1,
    pub output_type: OutputTypeV1,
    pub expected_output_mode: u32,
    pub expected_output_elf: ElfProfileV1,
    pub arguments: Vec<PlanArgumentV1>,
    pub runtime_dsos: RuntimeDsoClosureV1,
}

impl HostLinkPlanSpecV1 {
    pub fn validate(&self) -> Result<(), HostLinkError> {
        validate_ascii_text(
            "LLVM build identity",
            &self.toolchain.llvm_build_identity,
            MAX_LLVM_IDENTITY_BYTES,
        )?;
        if self.expected_output_mode != 0o555 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ElfPolicy,
                "V1 host-link outputs must have mode 0555",
            ));
        }
        self.expected_output_elf.validate()?;
        if self.output_type != OutputTypeV1::Executable
            || self.expected_output_elf.elf_type != object::elf::ET_EXEC
            || self.expected_output_elf.machine != object::elf::EM_X86_64
            || self.expected_output_elf.interpreter.is_some()
            || self.expected_output_elf.soname.is_some()
            || !self.expected_output_elf.needed.is_empty()
            || self.expected_output_elf.has_writable_executable_segment
            || self.expected_output_elf.has_executable_stack
            || self.runtime_dsos.interpreter_artifact.is_some()
            || !self.runtime_dsos.bindings.is_empty()
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ElfPolicy,
                "W0/W1 host-link plans require a static ET_EXEC output with no runtime loader closure",
            ));
        }
        if self.arguments.is_empty() || self.arguments.len() > MAX_HOST_LINK_ARGUMENTS_V1 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                format!(
                    "host-link plan must contain 1 through {MAX_HOST_LINK_ARGUMENTS_V1} arguments"
                ),
            ));
        }
        for argument in &self.arguments {
            argument.validate()?;
        }
        self.runtime_dsos.validate()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostLinkPlanManifestV1 {
    pub spec: HostLinkPlanSpecV1,
    pub producers: Vec<ArtifactIdentityV1>,
    pub plan_digest: Sha256Digest,
}

impl HostLinkPlanManifestV1 {
    pub(crate) fn validate(&self) -> Result<(), HostLinkError> {
        self.spec.validate()?;
        if self.producers.len() > MAX_HOST_LINK_PRODUCERS_V1 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "producer artifact count exceeds its bound",
            ));
        }
        let mut ids = BTreeSet::new();
        let mut previous = None;
        for producer in &self.producers {
            producer.validate_id()?;
            if !ids.insert(producer.id) {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::DuplicateRecord,
                    "duplicate producer artifact identity",
                ));
            }
            if previous.is_some_and(|value| value >= producer.id) {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::NonCanonicalOrder,
                    "producer artifact identities must be sorted",
                ));
            }
            previous = Some(producer.id);
            if producer.release_nonce != self.spec.release_nonce {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::WrongNonce,
                    format!("producer {} has the wrong release nonce", producer.label),
                ));
            }
            if producer.target != self.spec.target {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::WrongTarget,
                    format!("producer {} has the wrong target", producer.label),
                ));
            }
        }
        for tool in [
            (
                self.spec.toolchain.static_wrapper,
                HostArtifactKindV1::StaticWrapper,
            ),
            (
                self.spec.toolchain.static_host_lld,
                HostArtifactKindV1::StaticHostLld,
            ),
        ] {
            let Some(artifact) = self.producers.iter().find(|artifact| artifact.id == tool.0)
            else {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ReplayMismatch,
                    "toolchain executable is not present in the retained producer set",
                ));
            };
            if artifact.kind != tool.1 {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactKind,
                    "toolchain identity has the wrong artifact kind",
                ));
            }
        }
        for argument in &self.spec.arguments {
            if let PlanArgumentV1::ProducerArtifact(id) = argument
                && !ids.contains(id)
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ReplayMismatch,
                    "plan references an absent producer artifact",
                ));
            }
        }
        Ok(())
    }

    pub fn encode_canonical(&self) -> Result<Vec<u8>, HostLinkError> {
        crate::wire::encode_manifest(self)
    }

    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, HostLinkError> {
        crate::wire::decode_manifest(bytes)
    }
}

#[allow(clippy::too_many_arguments)]
fn artifact_identity_preimage(
    label: &str,
    kind: HostArtifactKindV1,
    provenance: ArtifactProvenanceV1,
    sha256: Sha256Digest,
    size: u64,
    mode: u32,
    nonce: ReleaseNonceV1,
    target: &TargetTripleV1,
    profile: Option<&ElfProfileV1>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    append(&mut bytes, label.as_bytes());
    bytes.push(kind as u8);
    bytes.push(provenance as u8);
    bytes.extend_from_slice(sha256.as_bytes());
    bytes.extend_from_slice(&size.to_le_bytes());
    bytes.extend_from_slice(&mode.to_le_bytes());
    bytes.extend_from_slice(nonce.as_bytes());
    append(&mut bytes, target.as_str().as_bytes());
    bytes.push(u8::from(profile.is_some()));
    if let Some(profile) = profile {
        bytes.push(profile.class as u8);
        bytes.push(profile.endian as u8);
        bytes.extend_from_slice(&profile.elf_type.to_le_bytes());
        bytes.extend_from_slice(&profile.machine.to_le_bytes());
        append(
            &mut bytes,
            profile.interpreter.as_deref().unwrap_or_default(),
        );
        append(&mut bytes, profile.soname.as_deref().unwrap_or_default());
        bytes.extend_from_slice(&(profile.needed.len() as u64).to_le_bytes());
        for needed in &profile.needed {
            append(&mut bytes, needed);
        }
        bytes.push(u8::from(profile.has_writable_executable_segment));
        bytes.push(u8::from(profile.has_executable_stack));
    }
    bytes
}

fn append(output: &mut Vec<u8>, field: &[u8]) {
    output.extend_from_slice(&(field.len() as u64).to_le_bytes());
    output.extend_from_slice(field);
}

pub(crate) fn validate_ascii_token(
    name: &str,
    value: &str,
    maximum: usize,
) -> Result<(), HostLinkError> {
    validate_ascii_text(name, value, maximum)?;
    if value.bytes().any(|byte| {
        !(byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'+' | b':'))
    }) {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::InvalidText,
            format!("{name} contains a non-token byte"),
        ));
    }
    Ok(())
}

fn validate_ascii_text(name: &str, value: &str, maximum: usize) -> Result<(), HostLinkError> {
    if value.is_empty() || value.len() > maximum || !value.is_ascii() || value.contains('\0') {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::InvalidText,
            format!("{name} must be 1 through {maximum} non-NUL ASCII bytes"),
        ));
    }
    Ok(())
}

pub(crate) fn validate_bytes(
    name: &str,
    value: &[u8],
    maximum: usize,
    allow_empty: bool,
) -> Result<(), HostLinkError> {
    if (!allow_empty && value.is_empty()) || value.len() > maximum || value.contains(&0) {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::FieldTooLarge,
            format!("{name} has an invalid length or contains NUL"),
        ));
    }
    Ok(())
}

pub(crate) fn validate_relative_path(value: &[u8]) -> Result<(), HostLinkError> {
    validate_bytes("fixed-root relative path", value, MAX_PATH_BYTES, false)?;
    let text = std::str::from_utf8(value).map_err(|_| {
        HostLinkError::new(
            HostLinkErrorCodeV1::InvalidPath,
            "fixed-root relative path is not UTF-8",
        )
    })?;
    let path = Path::new(text);
    if path.is_absolute()
        || path.components().any(|component| {
            !matches!(component, Component::Normal(_))
                || component.as_os_str().as_encoded_bytes().is_empty()
        })
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::InvalidPath,
            "fixed-root path must contain only nonempty normal relative components",
        ));
    }
    Ok(())
}

fn validate_library_name(value: &str) -> Result<(), HostLinkError> {
    validate_ascii_token("library name", value, MAX_LABEL_BYTES)?;
    if value.starts_with('.') || value.contains('/') {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::InvalidText,
            "library name is not canonical",
        ));
    }
    Ok(())
}
