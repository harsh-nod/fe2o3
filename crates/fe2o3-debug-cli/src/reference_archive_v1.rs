//! Canonical, authority-free evidence archive for the reference debugger workflow.

use std::error::Error;
use std::fmt;

use fe2o3_semantic_query::MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1;
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const REFERENCE_EVIDENCE_ARCHIVE_SCHEMA_V1: &str = "fe2o3-reference-evidence-archive-v1";
pub const REFERENCE_EVIDENCE_ARCHIVE_REPORT_SCHEMA_V1: &str =
    "fe2o3-agent-reference-archive-report-v1";
pub const MAX_REFERENCE_EVIDENCE_ARCHIVE_BYTES_V1: u64 = 192 * 1024 * 1024;
pub const MAX_REFERENCE_EVIDENCE_ARCHIVE_MEMBERS_V1: usize = 22;
pub const MAX_REFERENCE_DEBUG_INPUT_BYTES_V1: u64 = 64 * 1024 * 1024;

const MAGIC_V1: &[u8] = b"FE2O3-REFERENCE-ARCHIVE-V1\0";
const MEMBER_HEADER_FIXED_BYTES_V1: usize = 2 + 8 + 32;

#[derive(Clone, Copy, Debug)]
pub struct ReferenceSimulatorCaseInputV1<'a> {
    pub kernel: &'a [u8],
    pub request: &'a [u8],
}

#[derive(Clone, Copy, Debug)]
pub struct ReferenceTreatmentInputV1<'a> {
    pub manifest: &'a [u8],
    pub semantic_workload: &'a [u8],
    pub raw_profiler_source: &'a [u8],
    pub bundle: &'a [u8],
    pub schedule: &'a [u8],
    pub artifact: &'a [u8],
    pub isa_projection: Option<&'a [u8]>,
    pub counters: Option<&'a [u8]>,
    pub pc_samples: Option<&'a [u8]>,
}

#[derive(Clone, Copy, Debug)]
pub struct ReferenceEvidenceArchiveInputV1<'a> {
    pub out_of_bounds: ReferenceSimulatorCaseInputV1<'a>,
    pub barrier_divergence: ReferenceSimulatorCaseInputV1<'a>,
    pub baseline: ReferenceTreatmentInputV1<'a>,
    pub candidate: ReferenceTreatmentInputV1<'a>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceSimulatorCaseV1 {
    pub kernel: Vec<u8>,
    pub request: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceTreatmentV1 {
    pub manifest: Vec<u8>,
    pub semantic_workload: Vec<u8>,
    pub raw_profiler_source: Vec<u8>,
    pub bundle: Vec<u8>,
    pub schedule: Vec<u8>,
    pub artifact: Vec<u8>,
    pub isa_projection: Option<Vec<u8>>,
    pub counters: Option<Vec<u8>>,
    pub pc_samples: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReferenceArchiveContentIdentityV1 {
    pub scheme: &'static str,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReferenceArchiveMemberIdentityV1 {
    pub role: &'static str,
    pub sha256: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceEvidenceArchiveV1 {
    pub identity: ReferenceArchiveContentIdentityV1,
    pub members: Vec<ReferenceArchiveMemberIdentityV1>,
    pub out_of_bounds: ReferenceSimulatorCaseV1,
    pub barrier_divergence: ReferenceSimulatorCaseV1,
    pub baseline: ReferenceTreatmentV1,
    pub candidate: ReferenceTreatmentV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReferenceArchiveErrorV1 {
    ArchiveTooLarge,
    AllocationFailed,
    IdentityMismatch,
    InvalidHeader,
    InvalidMemberCount,
    InvalidMemberRole,
    NonCanonicalMemberOrder,
    DuplicateMember,
    MissingRequiredMember,
    MemberTooLarge,
    TreatmentTooLarge,
    MemberIdentityMismatch,
    Truncated,
    TrailingBytes,
}

impl fmt::Display for ReferenceArchiveErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ArchiveTooLarge => "reference evidence archive exceeds its byte bound",
            Self::AllocationFailed => "reference evidence archive allocation failed",
            Self::IdentityMismatch => "reference evidence archive identity mismatch",
            Self::InvalidHeader => "invalid reference evidence archive header",
            Self::InvalidMemberCount => "invalid reference evidence archive member count",
            Self::InvalidMemberRole => "invalid reference evidence archive member role",
            Self::NonCanonicalMemberOrder => {
                "reference evidence archive member order is not canonical"
            }
            Self::DuplicateMember => "reference evidence archive contains a duplicate member",
            Self::MissingRequiredMember => {
                "reference evidence archive is missing a required member"
            }
            Self::MemberTooLarge => "reference evidence archive member exceeds its byte bound",
            Self::TreatmentTooLarge => {
                "reference evidence archive treatment exceeds its aggregate byte bound"
            }
            Self::MemberIdentityMismatch => "reference evidence archive member identity mismatch",
            Self::Truncated => "truncated reference evidence archive",
            Self::TrailingBytes => "reference evidence archive contains trailing bytes",
        })
    }
}

impl Error for ReferenceArchiveErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
enum MemberRoleV1 {
    BarrierKernel,
    BarrierRequest,
    BaselineArtifact,
    BaselineBundle,
    BaselineCounters,
    BaselineIsa,
    BaselineManifest,
    BaselinePcSamples,
    BaselineRawProfiler,
    BaselineSchedule,
    BaselineSemanticWorkload,
    CandidateArtifact,
    CandidateBundle,
    CandidateCounters,
    CandidateIsa,
    CandidateManifest,
    CandidatePcSamples,
    CandidateRawProfiler,
    CandidateSchedule,
    CandidateSemanticWorkload,
    OutOfBoundsKernel,
    OutOfBoundsRequest,
}

impl MemberRoleV1 {
    const ALL: [Self; MAX_REFERENCE_EVIDENCE_ARCHIVE_MEMBERS_V1] = [
        Self::BarrierKernel,
        Self::BarrierRequest,
        Self::BaselineArtifact,
        Self::BaselineBundle,
        Self::BaselineCounters,
        Self::BaselineIsa,
        Self::BaselineManifest,
        Self::BaselinePcSamples,
        Self::BaselineRawProfiler,
        Self::BaselineSchedule,
        Self::BaselineSemanticWorkload,
        Self::CandidateArtifact,
        Self::CandidateBundle,
        Self::CandidateCounters,
        Self::CandidateIsa,
        Self::CandidateManifest,
        Self::CandidatePcSamples,
        Self::CandidateRawProfiler,
        Self::CandidateSchedule,
        Self::CandidateSemanticWorkload,
        Self::OutOfBoundsKernel,
        Self::OutOfBoundsRequest,
    ];

    const fn wire(self) -> &'static str {
        match self {
            Self::BarrierKernel => "barrier/kernel.kir-v7",
            Self::BarrierRequest => "barrier/request.json",
            Self::BaselineArtifact => "baseline/artifact.hsaco",
            Self::BaselineBundle => "baseline/bundle.v4",
            Self::BaselineCounters => "baseline/counters.v2",
            Self::BaselineIsa => "baseline/isa",
            Self::BaselineManifest => "baseline/manifest.v1",
            Self::BaselinePcSamples => "baseline/pc-samples.v3",
            Self::BaselineRawProfiler => "baseline/raw-profiler.json",
            Self::BaselineSchedule => "baseline/schedule",
            Self::BaselineSemanticWorkload => "baseline/semantic-workload",
            Self::CandidateArtifact => "candidate/artifact.hsaco",
            Self::CandidateBundle => "candidate/bundle.v4",
            Self::CandidateCounters => "candidate/counters.v2",
            Self::CandidateIsa => "candidate/isa",
            Self::CandidateManifest => "candidate/manifest.v1",
            Self::CandidatePcSamples => "candidate/pc-samples.v3",
            Self::CandidateRawProfiler => "candidate/raw-profiler.json",
            Self::CandidateSchedule => "candidate/schedule",
            Self::CandidateSemanticWorkload => "candidate/semantic-workload",
            Self::OutOfBoundsKernel => "out-of-bounds/kernel.kir-v7",
            Self::OutOfBoundsRequest => "out-of-bounds/request.json",
        }
    }

    fn parse(value: &[u8]) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|role| value == role.wire().as_bytes())
    }

    const fn required(self) -> bool {
        !matches!(
            self,
            Self::BaselineCounters
                | Self::BaselineIsa
                | Self::BaselinePcSamples
                | Self::CandidateCounters
                | Self::CandidateIsa
                | Self::CandidatePcSamples
        )
    }

    const fn debug_member(self) -> bool {
        matches!(
            self,
            Self::BarrierKernel
                | Self::BarrierRequest
                | Self::OutOfBoundsKernel
                | Self::OutOfBoundsRequest
        )
    }
}

pub fn reference_evidence_archive_sha256_v1(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub fn encode_reference_evidence_archive_v1(
    input: ReferenceEvidenceArchiveInputV1<'_>,
) -> Result<Vec<u8>, ReferenceArchiveErrorV1> {
    let members = input_members(input);
    validate_input_members_v1(&members)?;
    let mut encoded_len = MAGIC_V1
        .len()
        .checked_add(2)
        .ok_or(ReferenceArchiveErrorV1::ArchiveTooLarge)?;
    for (role, bytes) in &members {
        encoded_len = encoded_len
            .checked_add(MEMBER_HEADER_FIXED_BYTES_V1)
            .and_then(|value| value.checked_add(role.wire().len()))
            .and_then(|value| value.checked_add(bytes.len()))
            .ok_or(ReferenceArchiveErrorV1::ArchiveTooLarge)?;
    }
    if encoded_len as u64 > MAX_REFERENCE_EVIDENCE_ARCHIVE_BYTES_V1 {
        return Err(ReferenceArchiveErrorV1::ArchiveTooLarge);
    }
    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(encoded_len)
        .map_err(|_| ReferenceArchiveErrorV1::AllocationFailed)?;
    encoded.extend_from_slice(MAGIC_V1);
    encoded.extend_from_slice(&(members.len() as u16).to_le_bytes());
    for (role, bytes) in members {
        let role_bytes = role.wire().as_bytes();
        encoded.extend_from_slice(&(role_bytes.len() as u16).to_le_bytes());
        encoded.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        encoded.extend_from_slice(&Sha256::digest(bytes));
        encoded.extend_from_slice(role_bytes);
        encoded.extend_from_slice(bytes);
    }
    debug_assert_eq!(encoded.len(), encoded_len);
    Ok(encoded)
}

pub fn decode_reference_evidence_archive_v1(
    bytes: &[u8],
    expected_sha256: [u8; 32],
) -> Result<ReferenceEvidenceArchiveV1, ReferenceArchiveErrorV1> {
    if bytes.len() as u64 > MAX_REFERENCE_EVIDENCE_ARCHIVE_BYTES_V1 {
        return Err(ReferenceArchiveErrorV1::ArchiveTooLarge);
    }
    if reference_evidence_archive_sha256_v1(bytes) != expected_sha256 {
        return Err(ReferenceArchiveErrorV1::IdentityMismatch);
    }
    let mut reader = ReaderV1::new(bytes);
    if reader.take(MAGIC_V1.len())? != MAGIC_V1 {
        return Err(ReferenceArchiveErrorV1::InvalidHeader);
    }
    let member_count = usize::from(reader.u16()?);
    if member_count == 0 || member_count > MAX_REFERENCE_EVIDENCE_ARCHIVE_MEMBERS_V1 {
        return Err(ReferenceArchiveErrorV1::InvalidMemberCount);
    }
    let mut parsed = Vec::new();
    parsed
        .try_reserve_exact(member_count)
        .map_err(|_| ReferenceArchiveErrorV1::AllocationFailed)?;
    let mut previous = None;
    let mut present = [false; MAX_REFERENCE_EVIDENCE_ARCHIVE_MEMBERS_V1];
    let mut baseline_bytes = 0_u64;
    let mut candidate_bytes = 0_u64;
    for _ in 0..member_count {
        let role_len = usize::from(reader.u16()?);
        let content_len =
            usize::try_from(reader.u64()?).map_err(|_| ReferenceArchiveErrorV1::MemberTooLarge)?;
        let claimed_digest = reader.array_32()?;
        let role = MemberRoleV1::parse(reader.take(role_len)?)
            .ok_or(ReferenceArchiveErrorV1::InvalidMemberRole)?;
        if previous.is_some_and(|value| value >= role) {
            return Err(if previous == Some(role) {
                ReferenceArchiveErrorV1::DuplicateMember
            } else {
                ReferenceArchiveErrorV1::NonCanonicalMemberOrder
            });
        }
        previous = Some(role);
        if role.debug_member() && content_len as u64 > MAX_REFERENCE_DEBUG_INPUT_BYTES_V1 {
            return Err(ReferenceArchiveErrorV1::MemberTooLarge);
        }
        if content_len as u64 > MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1 {
            return Err(ReferenceArchiveErrorV1::MemberTooLarge);
        }
        accumulate_treatment_bytes_v1(
            role,
            content_len as u64,
            &mut baseline_bytes,
            &mut candidate_bytes,
        )?;
        let content = reader.take(content_len)?;
        let actual_digest: [u8; 32] = Sha256::digest(content).into();
        if actual_digest != claimed_digest {
            return Err(ReferenceArchiveErrorV1::MemberIdentityMismatch);
        }
        present[role as usize] = true;
        parsed.push(ParsedMemberV1 {
            role,
            content,
            digest: actual_digest,
        });
    }
    if !reader.is_empty() {
        return Err(ReferenceArchiveErrorV1::TrailingBytes);
    }
    if MemberRoleV1::ALL
        .into_iter()
        .any(|role| role.required() && !present[role as usize])
    {
        return Err(ReferenceArchiveErrorV1::MissingRequiredMember);
    }
    let mut decoded = DecodedMembersV1::default();
    let mut identities = Vec::new();
    identities
        .try_reserve_exact(member_count)
        .map_err(|_| ReferenceArchiveErrorV1::AllocationFailed)?;
    for member in parsed {
        decoded.insert_prevalidated(member.role, member.content)?;
        identities.push(ReferenceArchiveMemberIdentityV1 {
            role: member.role.wire(),
            sha256: encode_hex(&member.digest)?,
            bytes: member.content.len() as u64,
        });
    }
    decoded.finish(bytes, identities)
}

struct ParsedMemberV1<'a> {
    role: MemberRoleV1,
    content: &'a [u8],
    digest: [u8; 32],
}

fn input_members<'a>(input: ReferenceEvidenceArchiveInputV1<'a>) -> Vec<(MemberRoleV1, &'a [u8])> {
    let mut members = vec![
        (MemberRoleV1::BarrierKernel, input.barrier_divergence.kernel),
        (
            MemberRoleV1::BarrierRequest,
            input.barrier_divergence.request,
        ),
        (MemberRoleV1::BaselineArtifact, input.baseline.artifact),
        (MemberRoleV1::BaselineBundle, input.baseline.bundle),
    ];
    push_optional(
        &mut members,
        MemberRoleV1::BaselineCounters,
        input.baseline.counters,
    );
    push_optional(
        &mut members,
        MemberRoleV1::BaselineIsa,
        input.baseline.isa_projection,
    );
    members.push((MemberRoleV1::BaselineManifest, input.baseline.manifest));
    push_optional(
        &mut members,
        MemberRoleV1::BaselinePcSamples,
        input.baseline.pc_samples,
    );
    members.extend([
        (
            MemberRoleV1::BaselineRawProfiler,
            input.baseline.raw_profiler_source,
        ),
        (MemberRoleV1::BaselineSchedule, input.baseline.schedule),
        (
            MemberRoleV1::BaselineSemanticWorkload,
            input.baseline.semantic_workload,
        ),
        (MemberRoleV1::CandidateArtifact, input.candidate.artifact),
        (MemberRoleV1::CandidateBundle, input.candidate.bundle),
    ]);
    push_optional(
        &mut members,
        MemberRoleV1::CandidateCounters,
        input.candidate.counters,
    );
    push_optional(
        &mut members,
        MemberRoleV1::CandidateIsa,
        input.candidate.isa_projection,
    );
    members.push((MemberRoleV1::CandidateManifest, input.candidate.manifest));
    push_optional(
        &mut members,
        MemberRoleV1::CandidatePcSamples,
        input.candidate.pc_samples,
    );
    members.extend([
        (
            MemberRoleV1::CandidateRawProfiler,
            input.candidate.raw_profiler_source,
        ),
        (MemberRoleV1::CandidateSchedule, input.candidate.schedule),
        (
            MemberRoleV1::CandidateSemanticWorkload,
            input.candidate.semantic_workload,
        ),
        (MemberRoleV1::OutOfBoundsKernel, input.out_of_bounds.kernel),
        (
            MemberRoleV1::OutOfBoundsRequest,
            input.out_of_bounds.request,
        ),
    ]);
    members
}

fn push_optional<'a>(
    members: &mut Vec<(MemberRoleV1, &'a [u8])>,
    role: MemberRoleV1,
    content: Option<&'a [u8]>,
) {
    if let Some(content) = content {
        members.push((role, content));
    }
}

fn validate_input_members_v1(
    members: &[(MemberRoleV1, &[u8])],
) -> Result<(), ReferenceArchiveErrorV1> {
    let mut baseline = 0_u64;
    let mut candidate = 0_u64;
    for (role, bytes) in members {
        if role.debug_member() && bytes.len() as u64 > MAX_REFERENCE_DEBUG_INPUT_BYTES_V1 {
            return Err(ReferenceArchiveErrorV1::MemberTooLarge);
        }
        if bytes.len() as u64 > MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1 {
            return Err(ReferenceArchiveErrorV1::MemberTooLarge);
        }
        accumulate_treatment_bytes_v1(*role, bytes.len() as u64, &mut baseline, &mut candidate)?;
    }
    Ok(())
}

fn accumulate_treatment_bytes_v1(
    role: MemberRoleV1,
    bytes: u64,
    baseline: &mut u64,
    candidate: &mut u64,
) -> Result<(), ReferenceArchiveErrorV1> {
    let total = match role {
        MemberRoleV1::BaselineArtifact
        | MemberRoleV1::BaselineBundle
        | MemberRoleV1::BaselineCounters
        | MemberRoleV1::BaselineIsa
        | MemberRoleV1::BaselineManifest
        | MemberRoleV1::BaselinePcSamples
        | MemberRoleV1::BaselineRawProfiler
        | MemberRoleV1::BaselineSchedule
        | MemberRoleV1::BaselineSemanticWorkload => baseline,
        MemberRoleV1::CandidateArtifact
        | MemberRoleV1::CandidateBundle
        | MemberRoleV1::CandidateCounters
        | MemberRoleV1::CandidateIsa
        | MemberRoleV1::CandidateManifest
        | MemberRoleV1::CandidatePcSamples
        | MemberRoleV1::CandidateRawProfiler
        | MemberRoleV1::CandidateSchedule
        | MemberRoleV1::CandidateSemanticWorkload => candidate,
        _ => return Ok(()),
    };
    *total = total
        .checked_add(bytes)
        .ok_or(ReferenceArchiveErrorV1::TreatmentTooLarge)?;
    if *total > MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1 {
        return Err(ReferenceArchiveErrorV1::TreatmentTooLarge);
    }
    Ok(())
}

#[derive(Default)]
struct DecodedMembersV1 {
    members: [Option<Vec<u8>>; MAX_REFERENCE_EVIDENCE_ARCHIVE_MEMBERS_V1],
}

impl DecodedMembersV1 {
    fn insert_prevalidated(
        &mut self,
        role: MemberRoleV1,
        bytes: &[u8],
    ) -> Result<(), ReferenceArchiveErrorV1> {
        let mut value = Vec::new();
        value
            .try_reserve_exact(bytes.len())
            .map_err(|_| ReferenceArchiveErrorV1::AllocationFailed)?;
        value.extend_from_slice(bytes);
        self.members[role as usize] = Some(value);
        Ok(())
    }

    fn take_required(&mut self, role: MemberRoleV1) -> Result<Vec<u8>, ReferenceArchiveErrorV1> {
        self.members[role as usize]
            .take()
            .ok_or(ReferenceArchiveErrorV1::MissingRequiredMember)
    }

    fn take_optional(&mut self, role: MemberRoleV1) -> Option<Vec<u8>> {
        self.members[role as usize].take()
    }

    fn treatment(
        &mut self,
        baseline: bool,
    ) -> Result<ReferenceTreatmentV1, ReferenceArchiveErrorV1> {
        let role = |baseline_role, candidate_role| {
            if baseline {
                baseline_role
            } else {
                candidate_role
            }
        };
        Ok(ReferenceTreatmentV1 {
            manifest: self.take_required(role(
                MemberRoleV1::BaselineManifest,
                MemberRoleV1::CandidateManifest,
            ))?,
            semantic_workload: self.take_required(role(
                MemberRoleV1::BaselineSemanticWorkload,
                MemberRoleV1::CandidateSemanticWorkload,
            ))?,
            raw_profiler_source: self.take_required(role(
                MemberRoleV1::BaselineRawProfiler,
                MemberRoleV1::CandidateRawProfiler,
            ))?,
            bundle: self.take_required(role(
                MemberRoleV1::BaselineBundle,
                MemberRoleV1::CandidateBundle,
            ))?,
            schedule: self.take_required(role(
                MemberRoleV1::BaselineSchedule,
                MemberRoleV1::CandidateSchedule,
            ))?,
            artifact: self.take_required(role(
                MemberRoleV1::BaselineArtifact,
                MemberRoleV1::CandidateArtifact,
            ))?,
            isa_projection: self
                .take_optional(role(MemberRoleV1::BaselineIsa, MemberRoleV1::CandidateIsa)),
            counters: self.take_optional(role(
                MemberRoleV1::BaselineCounters,
                MemberRoleV1::CandidateCounters,
            )),
            pc_samples: self.take_optional(role(
                MemberRoleV1::BaselinePcSamples,
                MemberRoleV1::CandidatePcSamples,
            )),
        })
    }

    fn finish(
        mut self,
        archive: &[u8],
        identities: Vec<ReferenceArchiveMemberIdentityV1>,
    ) -> Result<ReferenceEvidenceArchiveV1, ReferenceArchiveErrorV1> {
        for role in MemberRoleV1::ALL {
            if role.required() && self.members[role as usize].is_none() {
                return Err(ReferenceArchiveErrorV1::MissingRequiredMember);
            }
        }
        let out_of_bounds = ReferenceSimulatorCaseV1 {
            kernel: self.take_required(MemberRoleV1::OutOfBoundsKernel)?,
            request: self.take_required(MemberRoleV1::OutOfBoundsRequest)?,
        };
        let barrier_divergence = ReferenceSimulatorCaseV1 {
            kernel: self.take_required(MemberRoleV1::BarrierKernel)?,
            request: self.take_required(MemberRoleV1::BarrierRequest)?,
        };
        let baseline = self.treatment(true)?;
        let candidate = self.treatment(false)?;
        Ok(ReferenceEvidenceArchiveV1 {
            identity: ReferenceArchiveContentIdentityV1 {
                scheme: "sha256_of_exact_archive_bytes",
                sha256: encode_hex(&reference_evidence_archive_sha256_v1(archive))?,
                bytes: archive.len() as u64,
            },
            members: identities,
            out_of_bounds,
            barrier_divergence,
            baseline,
            candidate,
        })
    }
}

struct ReaderV1<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ReaderV1<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ReferenceArchiveErrorV1> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(ReferenceArchiveErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ReferenceArchiveErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn u16(&mut self) -> Result<u16, ReferenceArchiveErrorV1> {
        let bytes: [u8; 2] = self
            .take(2)?
            .try_into()
            .map_err(|_| ReferenceArchiveErrorV1::Truncated)?;
        Ok(u16::from_le_bytes(bytes))
    }

    fn u64(&mut self) -> Result<u64, ReferenceArchiveErrorV1> {
        let bytes: [u8; 8] = self
            .take(8)?
            .try_into()
            .map_err(|_| ReferenceArchiveErrorV1::Truncated)?;
        Ok(u64::from_le_bytes(bytes))
    }

    fn array_32(&mut self) -> Result<[u8; 32], ReferenceArchiveErrorV1> {
        self.take(32)?
            .try_into()
            .map_err(|_| ReferenceArchiveErrorV1::Truncated)
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn encode_hex(bytes: &[u8]) -> Result<String, ReferenceArchiveErrorV1> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let capacity = bytes
        .len()
        .checked_mul(2)
        .ok_or(ReferenceArchiveErrorV1::AllocationFailed)?;
    let mut encoded = String::new();
    encoded
        .try_reserve_exact(capacity)
        .map_err(|_| ReferenceArchiveErrorV1::AllocationFailed)?;
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn treatment<'a>(seed: &'a [u8]) -> ReferenceTreatmentInputV1<'a> {
        ReferenceTreatmentInputV1 {
            manifest: seed,
            semantic_workload: seed,
            raw_profiler_source: seed,
            bundle: seed,
            schedule: seed,
            artifact: seed,
            isa_projection: Some(seed),
            counters: None,
            pc_samples: None,
        }
    }

    fn complete_treatment<'a>(seed: &'a [u8]) -> ReferenceTreatmentInputV1<'a> {
        ReferenceTreatmentInputV1 {
            counters: Some(seed),
            pc_samples: Some(seed),
            ..treatment(seed)
        }
    }

    fn archive() -> Vec<u8> {
        let input = ReferenceEvidenceArchiveInputV1 {
            out_of_bounds: ReferenceSimulatorCaseInputV1 {
                kernel: b"ok",
                request: b"or",
            },
            barrier_divergence: ReferenceSimulatorCaseInputV1 {
                kernel: b"bk",
                request: b"br",
            },
            baseline: treatment(b"baseline"),
            candidate: treatment(b"candidate"),
        };
        encode_reference_evidence_archive_v1(input).unwrap()
    }

    #[test]
    fn canonical_round_trip_and_external_identity_pin() {
        let bytes = archive();
        let decoded = decode_reference_evidence_archive_v1(
            &bytes,
            reference_evidence_archive_sha256_v1(&bytes),
        )
        .unwrap();
        assert_eq!(decoded.out_of_bounds.kernel, b"ok");
        assert_eq!(decoded.baseline.manifest, b"baseline");
        assert!(decoded.baseline.counters.is_none());
        assert_eq!(decoded.members.len(), 18);
        assert_eq!(decoded.identity.bytes, bytes.len() as u64);
        assert_eq!(
            decode_reference_evidence_archive_v1(&bytes, [7; 32]),
            Err(ReferenceArchiveErrorV1::IdentityMismatch)
        );
    }

    #[test]
    fn complete_optional_layout_has_all_twenty_two_canonical_members() {
        let bytes = encode_reference_evidence_archive_v1(ReferenceEvidenceArchiveInputV1 {
            out_of_bounds: ReferenceSimulatorCaseInputV1 {
                kernel: b"ok",
                request: b"or",
            },
            barrier_divergence: ReferenceSimulatorCaseInputV1 {
                kernel: b"bk",
                request: b"br",
            },
            baseline: complete_treatment(b"baseline"),
            candidate: complete_treatment(b"candidate"),
        })
        .unwrap();
        let decoded = decode_reference_evidence_archive_v1(
            &bytes,
            reference_evidence_archive_sha256_v1(&bytes),
        )
        .unwrap();
        assert_eq!(
            decoded.members.len(),
            MAX_REFERENCE_EVIDENCE_ARCHIVE_MEMBERS_V1
        );
        assert_eq!(decoded.baseline.counters.as_deref(), Some(&b"baseline"[..]));
        assert_eq!(
            decoded.baseline.pc_samples.as_deref(),
            Some(&b"baseline"[..])
        );
        assert_eq!(
            decoded.candidate.counters.as_deref(),
            Some(&b"candidate"[..])
        );
        assert_eq!(
            decoded.candidate.pc_samples.as_deref(),
            Some(&b"candidate"[..])
        );
    }

    #[test]
    fn truncation_at_header_role_and_content_boundaries_is_typed() {
        let bytes = archive();
        let header = MAGIC_V1.len() + 2;
        let first_role_len =
            u16::from_le_bytes(bytes[header..header + 2].try_into().unwrap()) as usize;
        let first_content_len =
            u64::from_le_bytes(bytes[header + 2..header + 10].try_into().unwrap()) as usize;
        let first_role_start = header + MEMBER_HEADER_FIXED_BYTES_V1;
        let first_content_start = first_role_start + first_role_len;
        let first_end = first_content_start + first_content_len;
        for length in [
            0,
            MAGIC_V1.len() - 1,
            MAGIC_V1.len() + 1,
            header + 1,
            first_role_start - 1,
            first_role_start + first_role_len - 1,
            first_content_start,
            first_end - 1,
        ] {
            let truncated = &bytes[..length];
            assert_eq!(
                decode_reference_evidence_archive_v1(
                    truncated,
                    reference_evidence_archive_sha256_v1(truncated),
                ),
                Err(ReferenceArchiveErrorV1::Truncated),
                "unexpected result at truncated length {length}"
            );
        }
    }

    #[test]
    fn zero_and_one_over_member_counts_are_rejected_before_records() {
        for count in [0_u16, MAX_REFERENCE_EVIDENCE_ARCHIVE_MEMBERS_V1 as u16 + 1] {
            let mut bytes = archive();
            bytes[MAGIC_V1.len()..MAGIC_V1.len() + 2].copy_from_slice(&count.to_le_bytes());
            let digest = reference_evidence_archive_sha256_v1(&bytes);
            assert_eq!(
                decode_reference_evidence_archive_v1(&bytes, digest),
                Err(ReferenceArchiveErrorV1::InvalidMemberCount)
            );
        }
    }

    #[test]
    fn treatment_aggregate_one_over_is_rejected_during_descriptor_preflight() {
        let limit = usize::try_from(MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1).unwrap();
        let first = vec![0x11; limit / 2 + 1];
        let second = vec![0x22; limit - limit / 2];
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC_V1);
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        for (role, content) in [
            (MemberRoleV1::BaselineArtifact, first.as_slice()),
            (MemberRoleV1::BaselineBundle, second.as_slice()),
        ] {
            bytes.extend_from_slice(&(role.wire().len() as u16).to_le_bytes());
            bytes.extend_from_slice(&(content.len() as u64).to_le_bytes());
            bytes.extend_from_slice(&Sha256::digest(content));
            bytes.extend_from_slice(role.wire().as_bytes());
            bytes.extend_from_slice(content);
        }
        assert!(bytes.len() as u64 <= MAX_REFERENCE_EVIDENCE_ARCHIVE_BYTES_V1);
        let digest = reference_evidence_archive_sha256_v1(&bytes);
        assert_eq!(
            decode_reference_evidence_archive_v1(&bytes, digest),
            Err(ReferenceArchiveErrorV1::TreatmentTooLarge)
        );
    }

    #[test]
    fn member_substitution_traversal_duplicate_missing_and_trailing_bytes_fail_closed() {
        let bytes = archive();
        let header = MAGIC_V1.len() + 2;

        let mut substituted = bytes.clone();
        *substituted.last_mut().unwrap() ^= 1;
        let digest = reference_evidence_archive_sha256_v1(&substituted);
        assert_eq!(
            decode_reference_evidence_archive_v1(&substituted, digest),
            Err(ReferenceArchiveErrorV1::MemberIdentityMismatch)
        );

        let first_role_len =
            u16::from_le_bytes(bytes[header..header + 2].try_into().unwrap()) as usize;
        let first_role_start = header + MEMBER_HEADER_FIXED_BYTES_V1;
        let mut traversal = bytes.clone();
        let replacement = b"../escape/member.json";
        assert_eq!(replacement.len(), first_role_len);
        traversal[first_role_start..first_role_start + first_role_len].copy_from_slice(replacement);
        let digest = reference_evidence_archive_sha256_v1(&traversal);
        assert_eq!(
            decode_reference_evidence_archive_v1(&traversal, digest),
            Err(ReferenceArchiveErrorV1::InvalidMemberRole)
        );

        let first_content_len =
            u64::from_le_bytes(bytes[header + 2..header + 10].try_into().unwrap()) as usize;
        let first_end = first_role_start + first_role_len + first_content_len;
        let first_record = &bytes[header..first_end];

        let second_role_len =
            u16::from_le_bytes(bytes[first_end..first_end + 2].try_into().unwrap()) as usize;
        let second_content_len =
            u64::from_le_bytes(bytes[first_end + 2..first_end + 10].try_into().unwrap()) as usize;
        let second_end =
            first_end + MEMBER_HEADER_FIXED_BYTES_V1 + second_role_len + second_content_len;
        let mut reordered = Vec::from(&bytes[..header]);
        reordered.extend_from_slice(&bytes[first_end..second_end]);
        reordered.extend_from_slice(first_record);
        reordered.extend_from_slice(&bytes[second_end..]);
        let digest = reference_evidence_archive_sha256_v1(&reordered);
        assert_eq!(
            decode_reference_evidence_archive_v1(&reordered, digest),
            Err(ReferenceArchiveErrorV1::NonCanonicalMemberOrder)
        );

        let mut duplicate = Vec::from(&bytes[..first_end]);
        duplicate.extend_from_slice(first_record);
        duplicate.extend_from_slice(&bytes[first_end..]);
        duplicate[MAGIC_V1.len()..header].copy_from_slice(&19_u16.to_le_bytes());
        let digest = reference_evidence_archive_sha256_v1(&duplicate);
        assert_eq!(
            decode_reference_evidence_archive_v1(&duplicate, digest),
            Err(ReferenceArchiveErrorV1::DuplicateMember)
        );

        let mut missing = Vec::from(&bytes[..header]);
        missing.extend_from_slice(&bytes[first_end..]);
        missing[MAGIC_V1.len()..header].copy_from_slice(&17_u16.to_le_bytes());
        let digest = reference_evidence_archive_sha256_v1(&missing);
        assert_eq!(
            decode_reference_evidence_archive_v1(&missing, digest),
            Err(ReferenceArchiveErrorV1::MissingRequiredMember)
        );

        let mut trailing = bytes.clone();
        trailing.push(0);
        let digest = reference_evidence_archive_sha256_v1(&trailing);
        assert_eq!(
            decode_reference_evidence_archive_v1(&trailing, digest),
            Err(ReferenceArchiveErrorV1::TrailingBytes)
        );
    }

    #[test]
    fn declared_member_oversize_fails_before_content_access() {
        let mut bytes = archive();
        let content_len = MAGIC_V1.len() + 2 + 2;
        bytes[content_len..content_len + 8]
            .copy_from_slice(&(MAX_REFERENCE_DEBUG_INPUT_BYTES_V1 + 1).to_le_bytes());
        let digest = reference_evidence_archive_sha256_v1(&bytes);
        assert_eq!(
            decode_reference_evidence_archive_v1(&bytes, digest),
            Err(ReferenceArchiveErrorV1::MemberTooLarge)
        );
    }
}
