use std::fs::File;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt as _;

use fe2o3_compiler_execution_protocol::CompilerExecutionClientProfileV1;
use rustix::fs::{FileType, Mode, OFlags, ResolveFlags, fstat, openat2};
use sha2::{Digest as _, Sha256};

use super::{
    DeploymentVerificationErrorKindV1, DeploymentVerificationErrorV1, ObjectSnapshotV1, changed,
    invalid, io_error, require_no_xattrs, snapshot, std_io_error,
};

pub(super) const CLIENT_TRANSACTION_REPORT_PATH_V1: &str =
    "run/fe2o3/compiler-execution-client-check.report";
const CLIENT_TRANSACTION_REPORT_SCHEMA_V1: &str = "fe2o3-compiler-execution-client-check-report-v1";
const CLIENT_TRANSACTION_REPORT_MAX_BYTES_V1: u64 = 4096;
const CLIENT_TRANSACTION_REPORT_MODE_V1: u32 = 0o600;
const REPORT_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/COMPILER-EXECUTION-CLIENT-CHECK-REPORT/V1\0";

#[derive(Debug)]
pub(super) struct CompilerExecutionClientTransactionEvidenceV1 {
    report: File,
    snapshot: ObjectSnapshotV1,
    canonical_bytes: Vec<u8>,
    identity: [u8; 32],
    launch_manifest_identity: [u8; 32],
    service_ready_identity: [u8; 32],
    submitter_pid: u32,
    client_pid: u32,
    client_uid: u32,
    client_gid: u32,
    issuer_pid: u32,
    subject_identity: [u8; 32],
    sequence: u64,
    rollback_anchor: [u8; 32],
}

impl CompilerExecutionClientTransactionEvidenceV1 {
    pub(super) const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    pub(super) const fn launch_manifest_identity(&self) -> [u8; 32] {
        self.launch_manifest_identity
    }

    pub(super) const fn service_ready_identity(&self) -> [u8; 32] {
        self.service_ready_identity
    }

    pub(super) const fn submitter_pid(&self) -> u32 {
        self.submitter_pid
    }

    pub(super) const fn client_pid(&self) -> u32 {
        self.client_pid
    }

    pub(super) const fn client_uid(&self) -> u32 {
        self.client_uid
    }

    pub(super) const fn client_gid(&self) -> u32 {
        self.client_gid
    }

    pub(super) const fn issuer_pid(&self) -> u32 {
        self.issuer_pid
    }

    pub(super) const fn subject_identity(&self) -> [u8; 32] {
        self.subject_identity
    }

    pub(super) const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(super) const fn rollback_anchor(&self) -> [u8; 32] {
        self.rollback_anchor
    }

    pub(super) fn revalidate(&self, root: &OwnedFd) -> Result<(), DeploymentVerificationErrorV1> {
        let retained =
            snapshot(&fstat(&self.report).map_err(|source| {
                io_error("reinspect retained client transaction report", source)
            })?);
        if retained != self.snapshot {
            return Err(changed("client transaction report changed after admission"));
        }
        let reopened = open_report(root).map_err(|source| {
            io_error("reopen admitted client transaction report pathname", source)
        })?;
        let reopened_snapshot =
            snapshot(&fstat(&reopened).map_err(|source| {
                io_error("reinspect client transaction report pathname", source)
            })?);
        if reopened_snapshot != self.snapshot {
            return Err(changed(
                "client transaction report pathname changed after admission",
            ));
        }
        let bytes = read_exact_report(&File::from(reopened), self.snapshot.byte_len)?;
        if bytes != self.canonical_bytes || report_identity(&bytes) != self.identity {
            return Err(changed(
                "client transaction report bytes changed after admission",
            ));
        }
        Ok(())
    }
}

pub(super) fn try_admit_client_transaction_report_v1(
    root: &OwnedFd,
    profile: &CompilerExecutionClientProfileV1,
    expected_client: (u32, u32),
) -> Result<Option<CompilerExecutionClientTransactionEvidenceV1>, DeploymentVerificationErrorV1> {
    let descriptor = match open_report(root) {
        Ok(descriptor) => descriptor,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(source) => return Err(io_error("open client transaction report", source)),
    };
    let before = snapshot(
        &fstat(&descriptor)
            .map_err(|source| io_error("inspect client transaction report", source))?,
    );
    if FileType::from_raw_mode(before.mode) != FileType::RegularFile
        || before.mode & 0o7777 != CLIENT_TRANSACTION_REPORT_MODE_V1
        || (before.uid, before.gid) != (0, 0)
        || before.links != 1
        || before.byte_len > CLIENT_TRANSACTION_REPORT_MAX_BYTES_V1
    {
        return Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
            "client transaction report metadata is not canonical",
        ));
    }
    require_no_xattrs(&descriptor, "client transaction report")?;
    let report = File::from(descriptor);
    let bytes = read_exact_report(&report, before.byte_len)?;
    let after = snapshot(
        &fstat(&report)
            .map_err(|source| io_error("reinspect client transaction report", source))?,
    );
    if before != after || !bytes.ends_with(b"complete=true\n") {
        return Ok(None);
    }
    let parsed = parse_client_transaction_report(&bytes, profile, expected_client)?;
    Ok(Some(CompilerExecutionClientTransactionEvidenceV1 {
        report,
        snapshot: before,
        canonical_bytes: bytes.clone(),
        identity: report_identity(&bytes),
        launch_manifest_identity: parsed.launch_manifest_identity,
        service_ready_identity: parsed.service_ready_identity,
        submitter_pid: parsed.submitter_pid,
        client_pid: parsed.client_pid,
        client_uid: parsed.client_uid,
        client_gid: parsed.client_gid,
        issuer_pid: parsed.issuer_pid,
        subject_identity: parsed.subject_identity,
        sequence: parsed.sequence,
        rollback_anchor: parsed.rollback_anchor,
    }))
}

fn open_report(root: &OwnedFd) -> Result<OwnedFd, rustix::io::Errno> {
    openat2(
        root,
        CLIENT_TRANSACTION_REPORT_PATH_V1,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
        ResolveFlags::BENEATH
            | ResolveFlags::NO_SYMLINKS
            | ResolveFlags::NO_MAGICLINKS
            | ResolveFlags::NO_XDEV,
    )
}

pub(super) fn require_client_transaction_report_absent_v1(
    root: &OwnedFd,
) -> Result<(), DeploymentVerificationErrorV1> {
    match open_report(root) {
        Err(rustix::io::Errno::NOENT) => Ok(()),
        Ok(_) => Err(invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
            "client transaction report remained after shutdown",
        )),
        Err(source) => Err(io_error("verify client transaction report removal", source)),
    }
}

fn read_exact_report(
    report: &File,
    byte_len: u64,
) -> Result<Vec<u8>, DeploymentVerificationErrorV1> {
    let byte_len = usize::try_from(byte_len).map_err(|_| {
        invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
            "client transaction report length does not fit memory",
        )
    })?;
    let mut bytes = vec![0; byte_len];
    report
        .read_exact_at(&mut bytes, 0)
        .map_err(|source| std_io_error("read client transaction report", source))?;
    Ok(bytes)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ParsedClientTransactionReportV1 {
    profile_identity: [u8; 32],
    policy_identity: [u8; 32],
    launch_manifest_identity: [u8; 32],
    service_ready_identity: [u8; 32],
    submitter_pid: u32,
    client_pid: u32,
    client_uid: u32,
    client_gid: u32,
    issuer_pid: u32,
    subject_identity: [u8; 32],
    sequence: u64,
    rollback_anchor: [u8; 32],
}

fn parse_client_transaction_report(
    bytes: &[u8],
    profile: &CompilerExecutionClientProfileV1,
    expected_client: (u32, u32),
) -> Result<ParsedClientTransactionReportV1, DeploymentVerificationErrorV1> {
    if !bytes.ends_with(b"\n") || bytes.contains(&b'\r') || bytes.contains(&0) {
        return Err(report_error());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| report_error())?;
    let mut lines = text.split_terminator('\n');
    require_field(
        &mut lines,
        "report_schema",
        CLIENT_TRANSACTION_REPORT_SCHEMA_V1,
    )?;
    let profile_identity = parse_identity_field(&mut lines, "profile_identity")?;
    let policy_identity = parse_identity_field(&mut lines, "policy_identity")?;
    let launch_manifest_identity = parse_identity_field(&mut lines, "launch_manifest_identity")?;
    let service_ready_identity = parse_identity_field(&mut lines, "service_ready_identity")?;
    let submitter_pid = parse_u32_field(&mut lines, "submitter_pid")?;
    let client_pid = parse_u32_field(&mut lines, "client_pid")?;
    let client_uid = parse_u32_field(&mut lines, "client_uid")?;
    let client_gid = parse_u32_field(&mut lines, "client_gid")?;
    let issuer_pid = parse_u32_field(&mut lines, "issuer_pid")?;
    let subject_identity = parse_identity_field(&mut lines, "subject_identity")?;
    let sequence = parse_u64_field(&mut lines, "sequence")?;
    let rollback_anchor = parse_identity_field(&mut lines, "rollback_anchor")?;
    require_field(&mut lines, "recover", "complete")?;
    require_field(&mut lines, "cancel", "complete")?;
    require_field(&mut lines, "complete", "true")?;
    if lines.next().is_some()
        || profile_identity != *profile.identity().as_bytes()
        || policy_identity != *profile.policy().identity().as_bytes()
        || (client_uid, client_gid) != expected_client
        || client_uid == 0
        || client_uid == profile.supervisor_uid()
        || submitter_pid == 0
        || client_pid == 0
        || issuer_pid == 0
        || submitter_pid == client_pid
        || submitter_pid == issuer_pid
        || client_pid == issuer_pid
    {
        return Err(report_error());
    }
    Ok(ParsedClientTransactionReportV1 {
        profile_identity,
        policy_identity,
        launch_manifest_identity,
        service_ready_identity,
        submitter_pid,
        client_pid,
        client_uid,
        client_gid,
        issuer_pid,
        subject_identity,
        sequence,
        rollback_anchor,
    })
}

fn require_field<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    name: &str,
    expected: &str,
) -> Result<(), DeploymentVerificationErrorV1> {
    let value = parse_field(lines, name)?;
    (value == expected).then_some(()).ok_or_else(report_error)
}

fn parse_identity_field<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    name: &str,
) -> Result<[u8; 32], DeploymentVerificationErrorV1> {
    let value = parse_field(lines, name)?;
    let bytes = super::parse_lower_hex_exact(value, 32, "client transaction report identity")
        .map_err(|_| report_error())?;
    bytes.try_into().map_err(|_| report_error())
}

fn parse_u32_field<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    name: &str,
) -> Result<u32, DeploymentVerificationErrorV1> {
    let value = parse_decimal_field(lines, name)?;
    u32::try_from(value).map_err(|_| report_error())
}

fn parse_u64_field<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    name: &str,
) -> Result<u64, DeploymentVerificationErrorV1> {
    parse_decimal_field(lines, name)
}

fn parse_decimal_field<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    name: &str,
) -> Result<u64, DeploymentVerificationErrorV1> {
    let value = parse_field(lines, name)?;
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(report_error());
    }
    value.parse().map_err(|_| report_error())
}

fn parse_field<'a>(
    lines: &mut impl Iterator<Item = &'a str>,
    name: &str,
) -> Result<&'a str, DeploymentVerificationErrorV1> {
    lines
        .next()
        .and_then(|line| line.strip_prefix(name))
        .and_then(|value| value.strip_prefix('='))
        .ok_or_else(report_error)
}

fn report_identity(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(REPORT_IDENTITY_DOMAIN_V1);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn report_error() -> DeploymentVerificationErrorV1 {
    invalid(
        DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
        "client transaction report is not canonical or does not match deployment policy",
    )
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionClientProfileV1, CompilerExecutionExternalAnchorServiceIdentityV1,
        CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1,
    };

    use super::*;

    #[test]
    fn exact_report_is_admitted_and_every_runtime_field_is_retained() {
        let profile = profile();
        let bytes = report(&profile);
        let parsed = parse_client_transaction_report(&bytes, &profile, (997, 997)).unwrap();
        assert_eq!(parsed.submitter_pid, 41);
        assert_eq!(parsed.client_pid, 42);
        assert_eq!(parsed.issuer_pid, 43);
        assert_eq!(parsed.sequence, 0);
        assert_eq!(parsed.rollback_anchor, [0x66; 32]);
    }

    #[test]
    fn report_rejects_reordering_aliases_mutation_and_identity_mismatch() {
        let profile = profile();
        let valid = report(&profile);
        let text = std::str::from_utf8(&valid).unwrap();
        for invalid_report in [
            text.replace(
                "submitter_pid=41\nclient_pid=42",
                "client_pid=42\nsubmitter_pid=41",
            ),
            text.replace("sequence=0", "sequence=00"),
            text.replace("recover=complete", "recover=true"),
            text.replace("complete=true\n", "complete=true\nextra=true\n"),
            text.trim_end().to_owned(),
            text.replace("profile_identity=", "profile-identity="),
            text.replacen('a', "A", 1),
        ] {
            assert_eq!(
                parse_client_transaction_report(invalid_report.as_bytes(), &profile, (997, 997))
                    .unwrap_err()
                    .kind(),
                DeploymentVerificationErrorKindV1::InvalidQualificationBoot
            );
        }

        let other = profile_with_seed(8);
        assert!(parse_client_transaction_report(&valid, &other, (997, 997)).is_err());
        assert!(parse_client_transaction_report(&valid, &profile, (996, 997)).is_err());
    }

    #[test]
    fn report_rejects_root_aliasing_and_zero_process_identities() {
        let profile = profile();
        let valid = String::from_utf8(report(&profile)).unwrap();
        for invalid_report in [
            valid.replace("client_uid=997", "client_uid=0"),
            valid.replace("client_uid=997", "client_uid=999"),
            valid.replace("submitter_pid=41", "submitter_pid=0"),
            valid.replace("client_pid=42", "client_pid=41"),
            valid.replace("issuer_pid=43", "issuer_pid=42"),
        ] {
            assert!(
                parse_client_transaction_report(invalid_report.as_bytes(), &profile, (997, 997))
                    .is_err()
            );
        }
    }

    fn report(profile: &CompilerExecutionClientProfileV1) -> Vec<u8> {
        format!(
            "report_schema={CLIENT_TRANSACTION_REPORT_SCHEMA_V1}\n\
             profile_identity={}\n\
             policy_identity={}\n\
             launch_manifest_identity={}\n\
             service_ready_identity={}\n\
             submitter_pid=41\n\
             client_pid=42\n\
             client_uid=997\n\
             client_gid=997\n\
             issuer_pid=43\n\
             subject_identity={}\n\
             sequence=0\n\
             rollback_anchor={}\n\
             recover=complete\n\
             cancel=complete\n\
             complete=true\n",
            super::super::encode_sha256_lower_hex_v1(*profile.identity().as_bytes()),
            super::super::encode_sha256_lower_hex_v1(*profile.policy().identity().as_bytes()),
            "33".repeat(32),
            "44".repeat(32),
            "55".repeat(32),
            "66".repeat(32),
        )
        .into_bytes()
    }

    fn profile() -> CompilerExecutionClientProfileV1 {
        profile_with_seed(7)
    }

    fn profile_with_seed(seed: u8) -> CompilerExecutionClientProfileV1 {
        let issuer = SigningKey::from_bytes(&[seed; 32]);
        let anchor = SigningKey::from_bytes(&[seed.wrapping_add(1); 32]);
        let policy = CompilerExecutionIssuerPolicyV1::new(
            1,
            CompilerExecutionIssuerMeasurementV1::new([seed.wrapping_add(2); 32], 100).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed.wrapping_add(3); 32], 101).unwrap(),
            issuer.verifying_key().to_bytes(),
            anchor.verifying_key().to_bytes(),
        )
        .unwrap();
        CompilerExecutionClientProfileV1::new(
            999,
            999,
            CompilerExecutionExternalAnchorServiceIdentityV1::new(998, 998).unwrap(),
            policy,
        )
        .unwrap()
    }
}
