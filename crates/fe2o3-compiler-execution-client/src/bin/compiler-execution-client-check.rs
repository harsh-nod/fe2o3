use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::io::{self, Read as _, Write as _};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use fe2o3_artifact_transaction::{
    INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
    INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1, InertCompilerExecutionSubjectV1,
};
use fe2o3_compiler_closure_capability::{
    COMPILER_EXECUTION_POLICY_CHILD_FD_V1, CompilerExecutionClientProfileCapabilityV1,
    CompilerExecutionPolicyCapabilityV1,
};
use fe2o3_compiler_execution_client::{
    COMPILER_EXECUTION_SERVICE_CHILD_FD_V1, CompilerExecutionClientV1,
    CompilerExecutionReceiptRecoveryV1, PendingCompilerExecutionChildChannelV1,
};
use sha2::{Digest, Sha256};

const USAGE: &str = "usage: fe2o3-compiler-execution-client-check";
const CHILD_ARGUMENT: &str = "__fe2o3-compiler-execution-client-check-child-v1";
const REPORT_SCHEMA: &str = "fe2o3-compiler-execution-client-check-report-v1";
const CHILD_REPORT_SCHEMA: &str = "fe2o3-compiler-execution-client-check-child-report-v1";
const MAX_REPORT_BYTES: usize = 2_048;
const MAX_CHILD_REPORT_BYTES: usize = 1_024;
const MAX_SUPPLEMENTARY_GROUPS: usize = 65_536;
const CLIENT_CHECK_TIMEOUT: Duration = Duration::from_secs(20);
const WAIT_INTERVAL: Duration = Duration::from_millis(5);
const SUBJECT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V1\0";
const COMPILER_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-compiler-closure-identity-v2\0";
const QUALIFICATION_SUBJECT_FIELD_DOMAIN: &[u8] =
    b"FE2O3/COMPILER-EXECUTION-CLIENT-CHECK/QUALIFICATION-SUBJECT/FIELD/V1\0";
const QUALIFICATION_SUBJECT_NONCE: [u8; 32] = [0x71; 32];

fn main() {
    let arguments: Vec<OsString> = std::env::args_os().skip(1).collect();
    match mode(&arguments) {
        Ok(Mode::Parent) => match run_parent() {
            Ok(report) => {
                let encoded = report.encode();
                if ParentReportV1::decode(encoded.as_bytes()).as_ref() != Ok(&report) {
                    eprintln!(
                        "compiler-execution client check failed: final report is not canonical"
                    );
                    std::process::exit(1);
                }
                if let Err(error) = io::stdout().lock().write_all(encoded.as_bytes()) {
                    eprintln!(
                        "compiler-execution client check failed: cannot publish final report: {error}"
                    );
                    std::process::exit(1);
                }
            }
            Err(error) => {
                eprintln!("compiler-execution client check failed: {error}");
                std::process::exit(1);
            }
        },
        Ok(Mode::Child) => match run_child() {
            Ok(report) => {
                let encoded = report.encode();
                if ChildReportV1::decode(encoded.as_bytes()).as_ref() != Ok(&report) {
                    eprintln!(
                        "compiler-execution client check child failed: report is not canonical"
                    );
                    std::process::exit(1);
                }
                if let Err(error) = io::stdout().lock().write_all(encoded.as_bytes()) {
                    eprintln!(
                        "compiler-execution client check child failed: cannot publish report: {error}"
                    );
                    std::process::exit(1);
                }
            }
            Err(error) => {
                eprintln!("compiler-execution client check child failed: {error}");
                std::process::exit(1);
            }
        },
        Err(()) => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Parent,
    Child,
}

fn mode(arguments: &[OsString]) -> Result<Mode, ()> {
    match arguments {
        [] => Ok(Mode::Parent),
        [argument] if argument == OsStr::new(CHILD_ARGUMENT) => Ok(Mode::Child),
        _ => Err(()),
    }
}

fn run_parent() -> Result<ParentReportV1, String> {
    let identity = CurrentIdentityV1::capture()?;
    identity.require_non_root()?;

    let profile = CompilerExecutionClientProfileCapabilityV1::from_production_profile()
        .map_err(|error| format!("production client-profile admission failed: {error}"))?;
    identity.require_profile_supervisor_group(
        profile.profile().supervisor_uid(),
        profile.profile().supervisor_gid(),
    )?;
    profile
        .revalidate()
        .map_err(|error| format!("production client-profile revalidation failed: {error}"))?;

    let deadline = Instant::now()
        .checked_add(CLIENT_CHECK_TIMEOUT)
        .ok_or_else(|| "client-check deadline overflowed".to_owned())?;
    let policy = CompilerExecutionPolicyCapabilityV1::create(profile.profile().policy().clone())
        .map_err(|error| format!("policy capability creation failed: {error}"))?;
    let mut command = Command::new("/proc/self/exe");
    command
        .arg(CHILD_ARGUMENT)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    policy
        .inherit_for_child(&mut command)
        .map_err(|error| format!("policy inheritance preparation failed: {error}"))?;
    let child_channel = PendingCompilerExecutionChildChannelV1::prepare(&mut command)
        .map_err(|error| format!("child-channel preparation failed: {error}"))?;
    let child = command
        .spawn()
        .map_err(|error| format!("client-check child spawn failed: {error}"))?;
    let mut child = ChildCustodyV1::new(child);

    let launch = child_channel
        .finish_until(child.id(), deadline)
        .map_err(|error| format!("child-channel admission failed: {error}"))?;
    let submitter = launch.submitter();
    let client = launch.client();
    let pending = launch
        .transfer_to_supervisor_until(profile.profile(), deadline)
        .map_err(|error| format!("supervisor transfer failed: {error}"))?;
    let launch_manifest_identity = *pending.manifest().identity().as_bytes();
    let readiness = pending
        .await_readiness_until(profile.profile(), deadline)
        .map_err(|error| format!("supervisor readiness failed: {error}"))?;

    let status = child.wait_until(deadline)?;
    if !status.success() {
        return Err(format!(
            "client-check child exited unsuccessfully: {status}"
        ));
    }
    let child_bytes = child.read_stdout_bounded()?;
    let child_report = ChildReportV1::decode(&child_bytes)
        .map_err(|error| format!("child report admission failed: {error}"))?;
    let expected_policy_identity = *profile.profile().policy().identity().as_bytes();
    if child_report.policy_identity != expected_policy_identity {
        return Err("child admitted a policy other than the production profile policy".to_owned());
    }

    profile
        .revalidate()
        .map_err(|error| format!("final client-profile revalidation failed: {error}"))?;
    policy
        .revalidate()
        .map_err(|error| format!("final policy revalidation failed: {error}"))?;

    let report = ParentReportV1 {
        profile_identity: *profile.profile().identity().as_bytes(),
        policy_identity: expected_policy_identity,
        launch_manifest_identity,
        service_ready_identity: *readiness.identity().as_bytes(),
        submitter_pid: submitter.pid(),
        client_pid: client.pid(),
        client_uid: client.uid(),
        client_gid: client.gid(),
        issuer_pid: readiness.issuer_pid(),
        subject_identity: child_report.subject_identity,
        sequence: child_report.sequence,
        rollback_anchor: child_report.rollback_anchor,
    };
    report.validate()?;
    Ok(report)
}

fn run_child() -> Result<ChildReportV1, String> {
    let policy = CompilerExecutionPolicyCapabilityV1::from_inherited_child().map_err(|error| {
        format!("FD {COMPILER_EXECUTION_POLICY_CHILD_FD_V1} admission failed: {error}")
    })?;
    let client = CompilerExecutionClientV1::admit_inherited_child(CLIENT_CHECK_TIMEOUT).map_err(
        |error| format!("FD {COMPILER_EXECUTION_SERVICE_CHILD_FD_V1} admission failed: {error}"),
    )?;
    policy
        .revalidate()
        .map_err(|error| format!("inherited policy revalidation failed: {error}"))?;
    let subject = qualification_subject()?;
    let subject_identity = *subject.identity().sha256();
    let recovery = client
        .recover_only(policy.policy(), subject)
        .map_err(|error| format!("recovery-only session failed: {error}"))?;
    let CompilerExecutionReceiptRecoveryV1::Absent {
        sequence,
        rollback_anchor,
    } = recovery
    else {
        return Err("recovery-only session found an existing compiler receipt".to_owned());
    };
    policy
        .revalidate()
        .map_err(|error| format!("final inherited-policy revalidation failed: {error}"))?;
    Ok(ChildReportV1 {
        policy_identity: *policy.policy().identity().as_bytes(),
        subject_identity,
        sequence,
        rollback_anchor,
    })
}

struct ChildCustodyV1 {
    child: Child,
    reaped: bool,
}

impl ChildCustodyV1 {
    fn new(child: Child) -> Self {
        Self {
            child,
            reaped: false,
        }
    }

    fn id(&self) -> u32 {
        self.child.id()
    }

    fn wait_until(&mut self, deadline: Instant) -> Result<ExitStatus, String> {
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    self.reaped = true;
                    return Ok(status);
                }
                Ok(None) => {}
                Err(error) => return Err(format!("cannot wait for client-check child: {error}")),
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("client-check child completion timed out".to_owned());
            }
            thread::sleep(remaining.min(WAIT_INTERVAL));
        }
    }

    fn read_stdout_bounded(&mut self) -> Result<Vec<u8>, String> {
        let mut stdout = self
            .child
            .stdout
            .take()
            .ok_or_else(|| "client-check child stdout is missing".to_owned())?;
        let mut bounded = stdout.by_ref().take((MAX_CHILD_REPORT_BYTES + 1) as u64);
        let mut bytes = Vec::with_capacity(MAX_CHILD_REPORT_BYTES);
        bounded
            .read_to_end(&mut bytes)
            .map_err(|error| format!("cannot read client-check child report: {error}"))?;
        if bytes.len() > MAX_CHILD_REPORT_BYTES {
            return Err("client-check child report exceeds its fixed bound".to_owned());
        }
        Ok(bytes)
    }
}

impl Drop for ChildCustodyV1 {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
            self.reaped = true;
        }
    }
}

struct CurrentIdentityV1 {
    real_uid: u32,
    effective_uid: u32,
    saved_uid: u32,
    effective_gid: u32,
    supplementary_groups: Vec<u32>,
}

impl CurrentIdentityV1 {
    fn capture() -> Result<Self, String> {
        let mut real_uid = 0;
        let mut effective_uid = 0;
        let mut saved_uid = 0;
        // SAFETY: all pointers name initialized writable uid_t values for the complete syscall.
        if unsafe { libc::getresuid(&mut real_uid, &mut effective_uid, &mut saved_uid) } != 0 {
            return Err(format!(
                "cannot inspect client UIDs: {}",
                io::Error::last_os_error()
            ));
        }
        let groups = rustix::process::getgroups()
            .map_err(|error| format!("cannot inspect client supplementary groups: {error}"))?;
        if groups.len() > MAX_SUPPLEMENTARY_GROUPS {
            return Err("client supplementary group set exceeds the Linux bound".to_owned());
        }
        Ok(Self {
            real_uid,
            effective_uid,
            saved_uid,
            effective_gid: rustix::process::getegid().as_raw(),
            supplementary_groups: groups.into_iter().map(|group| group.as_raw()).collect(),
        })
    }

    fn require_non_root(&self) -> Result<(), String> {
        require_identity_gate(
            self.real_uid,
            self.effective_uid,
            self.saved_uid,
            self.effective_gid,
            &self.supplementary_groups,
            None,
        )
    }

    fn require_profile_supervisor_group(
        &self,
        supervisor_uid: u32,
        supervisor_gid: u32,
    ) -> Result<(), String> {
        require_identity_gate(
            self.real_uid,
            self.effective_uid,
            self.saved_uid,
            self.effective_gid,
            &self.supplementary_groups,
            Some((supervisor_uid, supervisor_gid)),
        )
    }
}

fn require_identity_gate(
    real_uid: u32,
    effective_uid: u32,
    saved_uid: u32,
    effective_gid: u32,
    supplementary_groups: &[u32],
    profile: Option<(u32, u32)>,
) -> Result<(), String> {
    if [real_uid, effective_uid, saved_uid].contains(&0) {
        return Err("root credentials are forbidden".to_owned());
    }
    if let Some((supervisor_uid, supervisor_gid)) = profile {
        if effective_uid == supervisor_uid {
            return Err("client UID must differ from the protected supervisor UID".to_owned());
        }
        if effective_gid != supervisor_gid && !supplementary_groups.contains(&supervisor_gid) {
            return Err(format!(
                "client is not a member of profile supervisor group {supervisor_gid}"
            ));
        }
    }
    Ok(())
}

fn qualification_subject() -> Result<InertCompilerExecutionSubjectV1, String> {
    qualification_subject_from_nonce(QUALIFICATION_SUBJECT_NONCE)
}

fn qualification_subject_from_nonce(
    nonce: [u8; 32],
) -> Result<InertCompilerExecutionSubjectV1, String> {
    if nonce == [0; 32] {
        return Err("qualification-subject nonce is all zero".to_owned());
    }
    let fields: [[u8; 32]; 17] = std::array::from_fn(|index| {
        let mut digest = Sha256::new();
        digest.update(QUALIFICATION_SUBJECT_FIELD_DOMAIN);
        digest.update(nonce);
        digest.update((index as u64).to_le_bytes());
        digest.finalize().into()
    });
    if fields.contains(&[0; 32]) {
        return Err("derived qualification-subject field is all zero".to_owned());
    }

    let closure_pins: [[u8; 32]; 6] = fields[4..10]
        .try_into()
        .expect("fixed field slice has six entries");
    let mut closure_digest = Sha256::new();
    closure_digest.update(COMPILER_CLOSURE_IDENTITY_DOMAIN);
    closure_digest.update(1_u16.to_le_bytes());
    for pin in closure_pins {
        closure_digest.update(pin);
    }
    let closure_identity: [u8; 32] = closure_digest.finalize().into();

    let mut bytes = [0_u8; INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1];
    let mut output_offset = 0;
    put(
        &mut bytes,
        &mut output_offset,
        &INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
    );
    put(
        &mut bytes,
        &mut output_offset,
        &INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1.to_le_bytes(),
    );
    put(&mut bytes, &mut output_offset, &0_u16.to_le_bytes());
    put(
        &mut bytes,
        &mut output_offset,
        &(INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1 as u64).to_le_bytes(),
    );
    put(&mut bytes, &mut output_offset, &0_u32.to_le_bytes());
    put(&mut bytes, &mut output_offset, &1_u64.to_le_bytes());
    put(&mut bytes, &mut output_offset, &fields[0][..16]);
    put(&mut bytes, &mut output_offset, &fields[1]);
    bytes[output_offset] = 0;
    output_offset += 8;
    put(&mut bytes, &mut output_offset, &fields[2]);
    put(&mut bytes, &mut output_offset, &fields[3]);
    for pin in closure_pins {
        put(&mut bytes, &mut output_offset, &pin);
    }
    put(&mut bytes, &mut output_offset, &1_u16.to_le_bytes());
    put(&mut bytes, &mut output_offset, &closure_identity);
    for (axis, binding) in fields[10..17].iter().enumerate() {
        put(&mut bytes, &mut output_offset, binding);
        put(
            &mut bytes,
            &mut output_offset,
            &(u64::try_from(axis).expect("bounded axis") + 1).to_le_bytes(),
        );
    }
    let identity = digest(SUBJECT_IDENTITY_DOMAIN, &bytes[..output_offset]);
    put(&mut bytes, &mut output_offset, &identity);
    if output_offset != bytes.len() {
        return Err("qualification-subject encoder length mismatch".to_owned());
    }
    InertCompilerExecutionSubjectV1::decode(&bytes)
        .map_err(|error| format!("qualification subject is not canonical: {error}"))
}

fn digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn put(output: &mut [u8], offset: &mut usize, value: &[u8]) {
    let end = offset
        .checked_add(value.len())
        .expect("bounded subject offset");
    output[*offset..end].copy_from_slice(value);
    *offset = end;
}

#[derive(Debug, Eq, PartialEq)]
struct ParentReportV1 {
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

impl ParentReportV1 {
    fn validate(&self) -> Result<(), String> {
        if self.submitter_pid == 0
            || self.client_pid == 0
            || self.issuer_pid == 0
            || self.submitter_pid == self.client_pid
            || self.submitter_pid == self.issuer_pid
            || self.client_pid == self.issuer_pid
        {
            return Err("final report contains zero or aliased process IDs".to_owned());
        }
        if self.client_uid == 0 {
            return Err("final report contains root client credentials".to_owned());
        }
        if self.profile_identity == [0; 32]
            || self.policy_identity == [0; 32]
            || self.launch_manifest_identity == [0; 32]
            || self.service_ready_identity == [0; 32]
            || self.subject_identity == [0; 32]
        {
            return Err("final report contains a zero identity".to_owned());
        }
        let encoded = self.encode();
        if encoded.len() > MAX_REPORT_BYTES || !encoded.ends_with("complete=true\n") {
            return Err("final report violates its fixed canonical bound".to_owned());
        }
        Ok(())
    }

    fn encode(&self) -> String {
        let mut output = String::with_capacity(1_024);
        writeln!(output, "report_schema={REPORT_SCHEMA}").expect("String writes do not fail");
        write_hex_line(&mut output, "profile_identity", self.profile_identity);
        write_hex_line(&mut output, "policy_identity", self.policy_identity);
        write_hex_line(
            &mut output,
            "launch_manifest_identity",
            self.launch_manifest_identity,
        );
        write_hex_line(
            &mut output,
            "service_ready_identity",
            self.service_ready_identity,
        );
        writeln!(output, "submitter_pid={}", self.submitter_pid)
            .expect("String writes do not fail");
        writeln!(output, "client_pid={}", self.client_pid).expect("String writes do not fail");
        writeln!(output, "client_uid={}", self.client_uid).expect("String writes do not fail");
        writeln!(output, "client_gid={}", self.client_gid).expect("String writes do not fail");
        writeln!(output, "issuer_pid={}", self.issuer_pid).expect("String writes do not fail");
        write_hex_line(&mut output, "subject_identity", self.subject_identity);
        writeln!(output, "sequence={}", self.sequence).expect("String writes do not fail");
        write_hex_line(&mut output, "rollback_anchor", self.rollback_anchor);
        output.push_str("recover=complete\ncancel=complete\ncomplete=true\n");
        output
    }

    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut lines = report_lines(bytes, MAX_REPORT_BYTES)?;
        require_literal(next_line(&mut lines)?, "report_schema", REPORT_SCHEMA)?;
        let report = Self {
            profile_identity: parse_hex_line(next_line(&mut lines)?, "profile_identity")?,
            policy_identity: parse_hex_line(next_line(&mut lines)?, "policy_identity")?,
            launch_manifest_identity: parse_hex_line(
                next_line(&mut lines)?,
                "launch_manifest_identity",
            )?,
            service_ready_identity: parse_hex_line(
                next_line(&mut lines)?,
                "service_ready_identity",
            )?,
            submitter_pid: parse_u32_line(next_line(&mut lines)?, "submitter_pid")?,
            client_pid: parse_u32_line(next_line(&mut lines)?, "client_pid")?,
            client_uid: parse_u32_line(next_line(&mut lines)?, "client_uid")?,
            client_gid: parse_u32_line(next_line(&mut lines)?, "client_gid")?,
            issuer_pid: parse_u32_line(next_line(&mut lines)?, "issuer_pid")?,
            subject_identity: parse_hex_line(next_line(&mut lines)?, "subject_identity")?,
            sequence: parse_u64_line(next_line(&mut lines)?, "sequence")?,
            rollback_anchor: parse_hex_line(next_line(&mut lines)?, "rollback_anchor")?,
        };
        require_literal(next_line(&mut lines)?, "recover", "complete")?;
        require_literal(next_line(&mut lines)?, "cancel", "complete")?;
        require_literal(next_line(&mut lines)?, "complete", "true")?;
        require_end(lines)?;
        report.validate()?;
        if report.encode().as_bytes() != bytes {
            return Err("report is not byte-canonical".to_owned());
        }
        Ok(report)
    }
}

#[derive(Debug, Eq, PartialEq)]
struct ChildReportV1 {
    policy_identity: [u8; 32],
    subject_identity: [u8; 32],
    sequence: u64,
    rollback_anchor: [u8; 32],
}

impl ChildReportV1 {
    fn encode(&self) -> String {
        let mut output = String::with_capacity(512);
        writeln!(output, "child_report_schema={CHILD_REPORT_SCHEMA}")
            .expect("String writes do not fail");
        write_hex_line(&mut output, "policy_identity", self.policy_identity);
        write_hex_line(&mut output, "subject_identity", self.subject_identity);
        output.push_str("service_fd_195=admitted\npolicy_fd_202=admitted\n");
        writeln!(output, "sequence={}", self.sequence).expect("String writes do not fail");
        write_hex_line(&mut output, "rollback_anchor", self.rollback_anchor);
        output.push_str("recover=absent\ncancel=complete\ncomplete=true\n");
        output
    }

    fn decode(bytes: &[u8]) -> Result<Self, String> {
        let mut lines = report_lines(bytes, MAX_CHILD_REPORT_BYTES)?;
        require_literal(
            next_line(&mut lines)?,
            "child_report_schema",
            CHILD_REPORT_SCHEMA,
        )?;
        let policy_identity = parse_hex_line(next_line(&mut lines)?, "policy_identity")?;
        let subject_identity = parse_hex_line(next_line(&mut lines)?, "subject_identity")?;
        require_literal(next_line(&mut lines)?, "service_fd_195", "admitted")?;
        require_literal(next_line(&mut lines)?, "policy_fd_202", "admitted")?;
        let sequence = parse_u64_line(next_line(&mut lines)?, "sequence")?;
        let rollback_anchor = parse_hex_line(next_line(&mut lines)?, "rollback_anchor")?;
        require_literal(next_line(&mut lines)?, "recover", "absent")?;
        require_literal(next_line(&mut lines)?, "cancel", "complete")?;
        require_literal(next_line(&mut lines)?, "complete", "true")?;
        require_end(lines)?;
        let report = Self {
            policy_identity,
            subject_identity,
            sequence,
            rollback_anchor,
        };
        if policy_identity == [0; 32]
            || subject_identity == [0; 32]
            || report.encode().as_bytes() != bytes
        {
            return Err("child report is not canonical".to_owned());
        }
        Ok(report)
    }
}

fn report_lines(bytes: &[u8], limit: usize) -> Result<std::str::Lines<'_>, String> {
    if bytes.len() > limit {
        return Err("report exceeds its fixed bound".to_owned());
    }
    if !bytes.ends_with(b"\n") {
        return Err("report lacks its terminal newline".to_owned());
    }
    std::str::from_utf8(bytes)
        .map(str::lines)
        .map_err(|_| "report is not UTF-8".to_owned())
}

fn next_line<'a>(lines: &mut std::str::Lines<'a>) -> Result<&'a str, String> {
    lines.next().ok_or_else(|| "report is truncated".to_owned())
}

fn require_end(mut lines: std::str::Lines<'_>) -> Result<(), String> {
    if lines.next().is_some() {
        return Err("report has trailing fields".to_owned());
    }
    Ok(())
}

fn require_literal(line: &str, key: &str, expected: &str) -> Result<(), String> {
    let value = line_value(line, key)?;
    if value != expected {
        return Err(format!("report field {key:?} has a noncanonical value"));
    }
    Ok(())
}

fn parse_hex_line(line: &str, key: &str) -> Result<[u8; 32], String> {
    let value = line_value(line, key)?;
    if value.len() != 64 {
        return Err(format!("report field {key:?} is not lowercase 64-hex"));
    }
    let mut decoded = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        decoded[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Ok(decoded)
}

fn hex_nibble(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err("report contains non-lowercase-hex data".to_owned()),
    }
}

fn parse_u32_line(line: &str, key: &str) -> Result<u32, String> {
    u32::try_from(parse_u64_line(line, key)?)
        .map_err(|_| format!("report field {key:?} exceeds u32"))
}

fn parse_u64_line(line: &str, key: &str) -> Result<u64, String> {
    let value = line_value(line, key)?;
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(format!("report field {key:?} is not strict decimal"));
    }
    value
        .parse()
        .map_err(|_| format!("report field {key:?} exceeds u64"))
}

fn line_value<'a>(line: &'a str, key: &str) -> Result<&'a str, String> {
    let (actual, value) = line
        .split_once('=')
        .ok_or_else(|| format!("report field {key:?} has no separator"))?;
    if actual != key || value.contains('=') {
        return Err(format!("expected report field {key:?}"));
    }
    Ok(value)
}

fn write_hex_line(output: &mut String, key: &str, bytes: [u8; 32]) {
    output.push_str(key);
    output.push('=');
    for byte in bytes {
        write!(output, "{byte:02x}").expect("String writes do not fail");
    }
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parent_report() -> ParentReportV1 {
        ParentReportV1 {
            profile_identity: [0x11; 32],
            policy_identity: [0x22; 32],
            launch_manifest_identity: [0x33; 32],
            service_ready_identity: [0x44; 32],
            submitter_pid: 10,
            client_pid: 11,
            client_uid: 1_000,
            client_gid: 1_001,
            issuer_pid: 12,
            subject_identity: [0x55; 32],
            sequence: 7,
            rollback_anchor: [0x66; 32],
        }
    }

    fn child_report() -> ChildReportV1 {
        ChildReportV1 {
            policy_identity: [0x22; 32],
            subject_identity: [0x55; 32],
            sequence: 7,
            rollback_anchor: [0x66; 32],
        }
    }

    #[test]
    fn parent_report_has_the_exact_ordered_canonical_schema() {
        let report = parent_report();
        let encoded = report.encode();
        let expected = concat!(
            "report_schema=fe2o3-compiler-execution-client-check-report-v1\n",
            "profile_identity=1111111111111111111111111111111111111111111111111111111111111111\n",
            "policy_identity=2222222222222222222222222222222222222222222222222222222222222222\n",
            "launch_manifest_identity=3333333333333333333333333333333333333333333333333333333333333333\n",
            "service_ready_identity=4444444444444444444444444444444444444444444444444444444444444444\n",
            "submitter_pid=10\n",
            "client_pid=11\n",
            "client_uid=1000\n",
            "client_gid=1001\n",
            "issuer_pid=12\n",
            "subject_identity=5555555555555555555555555555555555555555555555555555555555555555\n",
            "sequence=7\n",
            "rollback_anchor=6666666666666666666666666666666666666666666666666666666666666666\n",
            "recover=complete\n",
            "cancel=complete\n",
            "complete=true\n",
        );
        assert_eq!(encoded, expected);
        assert!(encoded.len() <= MAX_REPORT_BYTES);
        assert_eq!(ParentReportV1::decode(encoded.as_bytes()).unwrap(), report);
    }

    #[test]
    fn parent_report_rejects_noncanonical_structure_and_values() {
        let canonical = parent_report().encode();
        for hostile in [
            canonical.trim_end().to_owned(),
            canonical.replace("client_pid=11", "client_pid=011"),
            canonical.replace("profile_identity=11", "profile_identity=AA"),
            canonical.replace("recover=complete", "recover=absent"),
            canonical.replace("issuer_pid=12", "issuer_pid=11"),
            canonical.replace("cancel=complete\n", ""),
            canonical.replace("complete=true\n", "complete=true\nextra=true\n"),
        ] {
            assert!(ParentReportV1::decode(hostile.as_bytes()).is_err());
        }
        assert!(ParentReportV1::decode(&vec![b'x'; MAX_REPORT_BYTES + 1]).is_err());
    }

    #[test]
    fn child_private_report_is_separate_bounded_and_strict() {
        let report = child_report();
        let encoded = report.encode();
        assert!(encoded.starts_with("child_report_schema="));
        assert!(!encoded.starts_with("report_schema="));
        assert!(encoded.len() <= MAX_CHILD_REPORT_BYTES);
        assert_eq!(ChildReportV1::decode(encoded.as_bytes()).unwrap(), report);
        for hostile in [
            encoded.replace("service_fd_195=admitted", "service_fd_195=195"),
            encoded.replace("policy_fd_202=admitted", "policy_fd_202=202"),
            encoded.replace("recover=absent", "recover=complete"),
            encoded.replace("complete=true\n", "complete=false\n"),
        ] {
            assert!(ChildReportV1::decode(hostile.as_bytes()).is_err());
        }
    }

    #[test]
    fn identity_gate_rejects_every_root_slot_and_requires_profile_group() {
        for root_slot in 0..3 {
            let mut uids = [1_000, 1_000, 1_000];
            uids[root_slot] = 0;
            assert!(
                require_identity_gate(uids[0], uids[1], uids[2], 1_000, &[1_001], None).is_err()
            );
        }
        assert!(
            require_identity_gate(1_000, 1_000, 1_000, 1_000, &[], Some((2_000, 2_001))).is_err()
        );
        assert!(
            require_identity_gate(1_000, 2_000, 1_000, 1_000, &[2_001], Some((2_000, 2_001)),)
                .is_err()
        );
        require_identity_gate(1_000, 1_000, 1_000, 2_001, &[], Some((2_000, 2_001))).unwrap();
        require_identity_gate(1_000, 1_000, 1_000, 1_000, &[2_001], Some((2_000, 2_001))).unwrap();
    }

    #[test]
    fn qualification_subject_is_repeatable_bound_and_strictly_canonical() {
        let first = qualification_subject().unwrap();
        let repeated = qualification_subject().unwrap();
        let different = qualification_subject_from_nonce([0x72; 32]).unwrap();
        assert_eq!(first, repeated);
        assert_ne!(first.identity(), different.identity());
        assert_eq!(
            InertCompilerExecutionSubjectV1::decode(first.canonical_bytes()).unwrap(),
            first
        );
        assert!(qualification_subject_from_nonce([0; 32]).is_err());
    }

    #[test]
    fn only_the_public_parent_and_private_child_invocations_are_admitted() {
        assert_eq!(mode(&[]), Ok(Mode::Parent));
        assert_eq!(mode(&[OsString::from(CHILD_ARGUMENT)]), Ok(Mode::Child));
        assert_eq!(mode(&[OsString::from("forbidden")]), Err(()));
        assert_eq!(
            mode(&[OsString::from(CHILD_ARGUMENT), OsString::from("extra")]),
            Err(())
        );
    }
}
