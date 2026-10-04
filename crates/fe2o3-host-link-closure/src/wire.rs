use crate::digest::{Sha256Digest, domain_sha256};
use crate::error::{HostLinkError, HostLinkErrorCodeV1};
use crate::model::*;
use crate::{MAX_HOST_LINK_ARGUMENTS_V1, MAX_HOST_LINK_PLAN_BYTES_V1, MAX_HOST_LINK_PRODUCERS_V1};

const MAGIC: &[u8; 8] = b"FE2OHLP\0";
const VERSION: u16 = 1;
const HEADER_BYTES: usize = 16;
const DIGEST_BYTES: usize = 32;
const MAX_FIELD_BYTES: usize = 64 * 1024;
const MAX_DSO_RECORDS: usize = 4096;

pub(crate) fn encode_manifest(manifest: &HostLinkPlanManifestV1) -> Result<Vec<u8>, HostLinkError> {
    manifest.validate()?;
    let mut payload = Writer::default();
    encode_spec(&mut payload, &manifest.spec)?;
    payload.count(manifest.producers.len())?;
    for producer in &manifest.producers {
        encode_artifact(&mut payload, producer)?;
    }
    let payload = payload.finish();
    let digest = domain_sha256(b"fe2o3-host-link-plan-v1\0", &[&payload]);
    if manifest.plan_digest != Sha256Digest::ZERO && manifest.plan_digest != digest {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::DigestMismatch,
            "manifest plan digest does not match its canonical fields",
        ));
    }
    let exact_size = HEADER_BYTES
        .checked_add(payload.len())
        .and_then(|size| size.checked_add(DIGEST_BYTES))
        .ok_or_else(|| {
            HostLinkError::new(HostLinkErrorCodeV1::PlanTooLarge, "plan size overflow")
        })?;
    if exact_size > MAX_HOST_LINK_PLAN_BYTES_V1 {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::PlanTooLarge,
            "canonical host-link plan exceeds its byte bound",
        ));
    }
    let payload_len = u32::try_from(payload.len()).map_err(|_| {
        HostLinkError::new(
            HostLinkErrorCodeV1::PlanTooLarge,
            "canonical plan payload does not fit its wire length",
        )
    })?;
    let mut output = Vec::with_capacity(exact_size);
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&VERSION.to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&payload_len.to_le_bytes());
    output.extend_from_slice(&payload);
    output.extend_from_slice(digest.as_bytes());
    Ok(output)
}

pub(crate) fn decode_manifest(bytes: &[u8]) -> Result<HostLinkPlanManifestV1, HostLinkError> {
    if bytes.len() > MAX_HOST_LINK_PLAN_BYTES_V1 {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::PlanTooLarge,
            "host-link plan exceeds its byte bound",
        ));
    }
    if bytes.len() < HEADER_BYTES + DIGEST_BYTES || bytes.get(..8) != Some(MAGIC) {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::InvalidWire,
            "host-link plan has no complete V1 header",
        ));
    }
    let version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if version != VERSION {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::InvalidVersion,
            format!("host-link plan version {version} is not V1"),
        ));
    }
    if bytes[10..12] != [0, 0] {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::NonCanonicalWire,
            "host-link plan reserved flags are nonzero",
        ));
    }
    let payload_len = u32::from_le_bytes(bytes[12..16].try_into().expect("fixed slice")) as usize;
    let expected_len = HEADER_BYTES
        .checked_add(payload_len)
        .and_then(|size| size.checked_add(DIGEST_BYTES))
        .ok_or_else(|| {
            HostLinkError::new(HostLinkErrorCodeV1::PlanTooLarge, "plan size overflow")
        })?;
    if expected_len != bytes.len() {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::InvalidWire,
            "host-link plan declared length does not match its exact bytes",
        ));
    }
    let payload = &bytes[HEADER_BYTES..HEADER_BYTES + payload_len];
    let observed_digest = Sha256Digest::from_bytes(
        bytes[HEADER_BYTES + payload_len..]
            .try_into()
            .expect("digest length checked"),
    );
    let expected_digest = domain_sha256(b"fe2o3-host-link-plan-v1\0", &[payload]);
    if observed_digest != expected_digest {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::DigestMismatch,
            "host-link plan digest does not match its payload",
        ));
    }
    let mut reader = Reader::new(payload);
    let spec = decode_spec(&mut reader)?;
    let producer_count = reader.count(MAX_HOST_LINK_PRODUCERS_V1)?;
    let mut producers = Vec::with_capacity(producer_count);
    for _ in 0..producer_count {
        producers.push(decode_artifact(&mut reader)?);
    }
    reader.finish()?;
    let manifest = HostLinkPlanManifestV1 {
        spec,
        producers,
        plan_digest: observed_digest,
    };
    manifest.validate()?;
    if encode_manifest(&manifest)? != bytes {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::NonCanonicalWire,
            "accepted host-link plan does not round-trip canonically",
        ));
    }
    Ok(manifest)
}

fn encode_spec(writer: &mut Writer, spec: &HostLinkPlanSpecV1) -> Result<(), HostLinkError> {
    spec.validate()?;
    writer.raw(spec.release_nonce.as_bytes());
    writer.text(spec.target.as_str())?;
    writer.raw(spec.toolchain.static_wrapper.sha256().as_bytes());
    writer.raw(spec.toolchain.static_host_lld.sha256().as_bytes());
    writer.text(&spec.toolchain.llvm_build_identity)?;
    writer.u8(spec.output_type as u8);
    writer.u32(spec.expected_output_mode);
    encode_profile(writer, &spec.expected_output_elf)?;
    writer.count(spec.arguments.len())?;
    for argument in &spec.arguments {
        encode_argument(writer, argument)?;
    }
    match spec.runtime_dsos.interpreter_artifact {
        Some(artifact) => {
            writer.u8(1);
            writer.raw(artifact.sha256().as_bytes());
        }
        None => writer.u8(0),
    }
    writer.count(spec.runtime_dsos.bindings.len())?;
    for binding in &spec.runtime_dsos.bindings {
        writer.bytes(&binding.soname)?;
        writer.raw(binding.artifact.sha256().as_bytes());
        writer.count(binding.needed.len())?;
        for needed in &binding.needed {
            writer.bytes(needed)?;
        }
    }
    Ok(())
}

fn decode_spec(reader: &mut Reader<'_>) -> Result<HostLinkPlanSpecV1, HostLinkError> {
    let release_nonce = ReleaseNonceV1::new(reader.array()?)?;
    let target = TargetTripleV1::new(reader.text()?)?;
    let static_wrapper = ArtifactIdV1::from_sha256(Sha256Digest::from_bytes(reader.array()?));
    let static_host_lld = ArtifactIdV1::from_sha256(Sha256Digest::from_bytes(reader.array()?));
    let llvm_build_identity = reader.text()?;
    let output_type = OutputTypeV1::from_tag(reader.u8()?)?;
    let expected_output_mode = reader.u32()?;
    let expected_output_elf = decode_profile(reader)?;
    let argument_count = reader.count(MAX_HOST_LINK_ARGUMENTS_V1)?;
    let mut arguments = Vec::with_capacity(argument_count);
    for _ in 0..argument_count {
        arguments.push(decode_argument(reader)?);
    }
    let interpreter_artifact = match reader.u8()? {
        0 => None,
        1 => Some(ArtifactIdV1::from_sha256(Sha256Digest::from_bytes(
            reader.array()?,
        ))),
        value => {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::NonCanonicalWire,
                format!("runtime interpreter-presence boolean is {value}"),
            ));
        }
    };
    let dso_count = reader.count(MAX_DSO_RECORDS)?;
    let mut bindings = Vec::with_capacity(dso_count);
    for _ in 0..dso_count {
        let soname = reader.bytes()?;
        let artifact = ArtifactIdV1::from_sha256(Sha256Digest::from_bytes(reader.array()?));
        let needed_count = reader.count(MAX_DSO_RECORDS)?;
        let mut needed = Vec::with_capacity(needed_count);
        for _ in 0..needed_count {
            needed.push(reader.bytes()?);
        }
        bindings.push(DsoBindingV1 {
            soname,
            artifact,
            needed,
        });
    }
    let spec = HostLinkPlanSpecV1 {
        release_nonce,
        target,
        toolchain: ExecutableToolchainV1 {
            static_wrapper,
            static_host_lld,
            llvm_build_identity,
        },
        output_type,
        expected_output_mode,
        expected_output_elf,
        arguments,
        runtime_dsos: RuntimeDsoClosureV1 {
            interpreter_artifact,
            bindings,
        },
    };
    spec.validate()?;
    Ok(spec)
}

fn encode_argument(writer: &mut Writer, argument: &PlanArgumentV1) -> Result<(), HostLinkError> {
    argument.validate()?;
    match argument {
        PlanArgumentV1::Literal(value) => {
            writer.u8(1);
            writer.bytes(value)?;
        }
        PlanArgumentV1::SearchRoot(root) => {
            writer.u8(2);
            writer.text(root)?;
        }
        PlanArgumentV1::Library { name, preference } => {
            writer.u8(3);
            writer.text(name)?;
            writer.u8(*preference as u8);
        }
        PlanArgumentV1::FixedRootInput {
            root,
            relative_path,
            kind,
        } => {
            writer.u8(4);
            writer.text(root)?;
            writer.bytes(relative_path)?;
            writer.u8(*kind as u8);
        }
        PlanArgumentV1::ProducerArtifact(id) => {
            writer.u8(5);
            writer.raw(id.sha256().as_bytes());
        }
        PlanArgumentV1::CatalogArtifact(id) => {
            writer.u8(6);
            writer.raw(id.sha256().as_bytes());
        }
        PlanArgumentV1::ResponseFile {
            root,
            relative_path,
        } => {
            writer.u8(7);
            writer.text(root)?;
            writer.bytes(relative_path)?;
        }
        PlanArgumentV1::ZPolicy(policy) => {
            writer.u8(8);
            writer.u8(*policy as u8);
        }
        PlanArgumentV1::UndefinedSymbol(symbol) => {
            writer.u8(9);
            writer.text(symbol)?;
        }
    }
    Ok(())
}

fn decode_argument(reader: &mut Reader<'_>) -> Result<PlanArgumentV1, HostLinkError> {
    let argument = match reader.u8()? {
        1 => PlanArgumentV1::Literal(reader.bytes()?),
        2 => PlanArgumentV1::SearchRoot(reader.text()?),
        3 => PlanArgumentV1::Library {
            name: reader.text()?,
            preference: LibraryPreferenceV1::from_tag(reader.u8()?)?,
        },
        4 => PlanArgumentV1::FixedRootInput {
            root: reader.text()?,
            relative_path: reader.bytes()?,
            kind: RootInputKindV1::from_tag(reader.u8()?)?,
        },
        5 => PlanArgumentV1::ProducerArtifact(ArtifactIdV1::from_sha256(Sha256Digest::from_bytes(
            reader.array()?,
        ))),
        6 => PlanArgumentV1::CatalogArtifact(ArtifactIdV1::from_sha256(Sha256Digest::from_bytes(
            reader.array()?,
        ))),
        7 => PlanArgumentV1::ResponseFile {
            root: reader.text()?,
            relative_path: reader.bytes()?,
        },
        8 => PlanArgumentV1::ZPolicy(LinkerZPolicyV1::from_tag(reader.u8()?)?),
        9 => PlanArgumentV1::UndefinedSymbol(reader.text()?),
        tag => {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidWire,
                format!("unknown plan argument tag {tag}"),
            ));
        }
    };
    argument.validate()?;
    Ok(argument)
}

fn encode_artifact(
    writer: &mut Writer,
    artifact: &ArtifactIdentityV1,
) -> Result<(), HostLinkError> {
    artifact.validate_id()?;
    writer.raw(artifact.id.sha256().as_bytes());
    writer.text(&artifact.label)?;
    writer.u8(artifact.kind as u8);
    writer.u8(artifact.provenance as u8);
    writer.raw(artifact.sha256.as_bytes());
    writer.u64(artifact.size);
    writer.u32(artifact.mode);
    writer.raw(artifact.release_nonce.as_bytes());
    writer.text(artifact.target.as_str())?;
    match &artifact.elf_profile {
        Some(profile) => {
            writer.u8(1);
            encode_profile(writer, profile)?;
        }
        None => writer.u8(0),
    }
    Ok(())
}

fn decode_artifact(reader: &mut Reader<'_>) -> Result<ArtifactIdentityV1, HostLinkError> {
    let id = ArtifactIdV1::from_sha256(Sha256Digest::from_bytes(reader.array()?));
    let label = reader.text()?;
    let kind = HostArtifactKindV1::from_tag(reader.u8()?)?;
    let provenance = ArtifactProvenanceV1::from_tag(reader.u8()?)?;
    let sha256 = Sha256Digest::from_bytes(reader.array()?);
    let size = reader.u64()?;
    let mode = reader.u32()?;
    let release_nonce = ReleaseNonceV1::new(reader.array()?)?;
    let target = TargetTripleV1::new(reader.text()?)?;
    let elf_profile = match reader.u8()? {
        0 => None,
        1 => Some(decode_profile(reader)?),
        value => {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::NonCanonicalWire,
                format!("artifact ELF-presence boolean is {value}"),
            ));
        }
    };
    let artifact = ArtifactIdentityV1 {
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
    };
    artifact.validate_id()?;
    Ok(artifact)
}

fn encode_profile(writer: &mut Writer, profile: &ElfProfileV1) -> Result<(), HostLinkError> {
    profile.validate()?;
    writer.u8(profile.class as u8);
    writer.u8(profile.endian as u8);
    writer.u16(profile.elf_type);
    writer.u16(profile.machine);
    match &profile.interpreter {
        Some(interpreter) => {
            writer.u8(1);
            writer.bytes(interpreter)?;
        }
        None => writer.u8(0),
    }
    match &profile.soname {
        Some(soname) => {
            writer.u8(1);
            writer.bytes(soname)?;
        }
        None => writer.u8(0),
    }
    writer.count(profile.needed.len())?;
    for needed in &profile.needed {
        writer.bytes(needed)?;
    }
    writer.u8(u8::from(profile.has_writable_executable_segment));
    writer.u8(u8::from(profile.has_executable_stack));
    Ok(())
}

fn decode_profile(reader: &mut Reader<'_>) -> Result<ElfProfileV1, HostLinkError> {
    if reader.u8()? != ElfClassV1::Elf64 as u8 {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::InvalidWire,
            "only ELF64 is admitted",
        ));
    }
    if reader.u8()? != ElfEndianV1::Little as u8 {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::InvalidWire,
            "only little-endian ELF is admitted",
        ));
    }
    let elf_type = reader.u16()?;
    let machine = reader.u16()?;
    let interpreter = match reader.u8()? {
        0 => None,
        1 => Some(reader.bytes()?),
        value => {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::NonCanonicalWire,
                format!("ELF interpreter-presence boolean is {value}"),
            ));
        }
    };
    let soname = match reader.u8()? {
        0 => None,
        1 => Some(reader.bytes()?),
        value => {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::NonCanonicalWire,
                format!("ELF SONAME-presence boolean is {value}"),
            ));
        }
    };
    let needed_count = reader.count(MAX_DSO_RECORDS)?;
    let mut needed = Vec::with_capacity(needed_count);
    for _ in 0..needed_count {
        needed.push(reader.bytes()?);
    }
    let has_writable_executable_segment = reader.boolean()?;
    let has_executable_stack = reader.boolean()?;
    let profile = ElfProfileV1 {
        class: ElfClassV1::Elf64,
        endian: ElfEndianV1::Little,
        elf_type,
        machine,
        interpreter,
        soname,
        needed,
        has_writable_executable_segment,
        has_executable_stack,
    };
    profile.validate()?;
    Ok(profile)
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }
    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
    fn raw(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }
    fn count(&mut self, value: usize) -> Result<(), HostLinkError> {
        self.u32(u32::try_from(value).map_err(|_| {
            HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "record count does not fit the canonical wire",
            )
        })?);
        Ok(())
    }
    fn text(&mut self, value: &str) -> Result<(), HostLinkError> {
        self.bytes(value.as_bytes())
    }
    fn bytes(&mut self, value: &[u8]) -> Result<(), HostLinkError> {
        if value.len() > MAX_FIELD_BYTES {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "wire field exceeds its byte bound",
            ));
        }
        self.u32(u32::try_from(value.len()).expect("bounded field fits u32"));
        self.raw(value);
        Ok(())
    }
    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], HostLinkError> {
        let end = self.offset.checked_add(count).ok_or_else(|| {
            HostLinkError::new(HostLinkErrorCodeV1::InvalidWire, "wire offset overflow")
        })?;
        let value = self.bytes.get(self.offset..end).ok_or_else(|| {
            HostLinkError::new(HostLinkErrorCodeV1::InvalidWire, "truncated wire field")
        })?;
        self.offset = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], HostLinkError> {
        Ok(self.take(N)?.try_into().expect("slice length checked"))
    }
    fn u8(&mut self) -> Result<u8, HostLinkError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, HostLinkError> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, HostLinkError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, HostLinkError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn count(&mut self, maximum: usize) -> Result<usize, HostLinkError> {
        let value = self.u32()? as usize;
        if value > maximum {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                format!("wire record count {value} exceeds {maximum}"),
            ));
        }
        Ok(value)
    }
    fn bytes(&mut self) -> Result<Vec<u8>, HostLinkError> {
        let count = self.count(MAX_FIELD_BYTES)?;
        Ok(self.take(count)?.to_vec())
    }
    fn text(&mut self) -> Result<String, HostLinkError> {
        String::from_utf8(self.bytes()?).map_err(|_| {
            HostLinkError::new(
                HostLinkErrorCodeV1::InvalidText,
                "wire text field is not UTF-8",
            )
        })
    }
    fn boolean(&mut self) -> Result<bool, HostLinkError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(HostLinkError::new(
                HostLinkErrorCodeV1::NonCanonicalWire,
                format!("wire boolean is {value}"),
            )),
        }
    }
    fn finish(self) -> Result<(), HostLinkError> {
        if self.offset != self.bytes.len() {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidWire,
                "host-link plan contains trailing payload bytes",
            ));
        }
        Ok(())
    }
}
