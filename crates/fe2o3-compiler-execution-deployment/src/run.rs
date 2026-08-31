use std::fmt::Write as _;
use std::path::Path;

use super::boot::boot_and_stop_systemd_machine_v1;
use super::fault::{InjectQualificationFaultV1, NoQualificationFaultV1, QualificationFaultHooksV1};
use super::install::verify_install_parent_children_v1;
use super::mount::attach_compiler_execution_qualification_mounts_with_hooks_v1;
use super::preflight::run_compiler_execution_systemd_preflight_with_hooks_v1;
use super::provision::run_compiler_execution_provisioning_with_hooks_v1;
use super::qualification::verify_empty_qualification_parent_v1;
use super::{
    CompilerExecutionInstalledRootPublicationV1, DeploymentVerificationErrorKindV1,
    DeploymentVerificationErrorV1, QualificationFaultPointV1,
    StagedCompilerExecutionQualificationV1, encode_sha256_lower_hex_v1,
    enter_private_qualification_mount_namespace_v1, install_compiler_execution_deployment_v1,
    prepare_compiler_execution_qualification_v1, stage_compiler_execution_qualification_v1,
    verify_compiler_execution_deployment_v1,
};

const QUALIFICATION_REPORT_SCHEMA_V1: &str = "fe2o3-compiler-execution-qualification-report-v1";
const QUALIFICATION_FAULT_REPORT_SCHEMA_V1: &str =
    "fe2o3-compiler-execution-qualification-fault-report-v1";
const QUALIFICATION_CAMPAIGN_REPORT_SCHEMA_V1: &str =
    "fe2o3-compiler-execution-qualification-campaign-report-v1";

/// Inert report from one fully cleaned provisioned systemd-machine transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerExecutionQualificationReportV1 {
    git_commit: String,
    target: String,
    manifest_sha256: [u8; 32],
    base_image_sha256: [u8; 32],
    installed_root_name: String,
    installed_publication: CompilerExecutionInstalledRootPublicationV1,
    staging_name: String,
    systemd_version: String,
    verified_unit_count: usize,
    compiler_uid: u32,
    compiler_gid: u32,
    anchor_uid: u32,
    anchor_gid: u32,
    policy_generation: u64,
}

impl CompilerExecutionQualificationReportV1 {
    /// Encodes the completed transaction as one stable newline-terminated key-value report.
    pub fn canonical_report(&self) -> String {
        let publication = match self.installed_publication {
            CompilerExecutionInstalledRootPublicationV1::Created => "created",
            CompilerExecutionInstalledRootPublicationV1::Reacquired => "reacquired",
        };
        let mut report = String::new();
        for (name, value) in [
            ("report_schema", QUALIFICATION_REPORT_SCHEMA_V1.to_owned()),
            ("git_commit", self.git_commit.clone()),
            ("target", self.target.clone()),
            (
                "manifest_sha256",
                encode_sha256_lower_hex_v1(self.manifest_sha256),
            ),
            (
                "base_image_sha256",
                encode_sha256_lower_hex_v1(self.base_image_sha256),
            ),
            ("installed_root_name", self.installed_root_name.clone()),
            ("installed_publication", publication.to_owned()),
            ("staging_name", self.staging_name.clone()),
            ("mount_revalidated", "true".to_owned()),
            ("systemd_version", self.systemd_version.clone()),
            ("systemd_sysusers", "complete".to_owned()),
            ("systemd_tmpfiles", "complete".to_owned()),
            (
                "systemd_unit_verify_count",
                self.verified_unit_count.to_string(),
            ),
            ("systemd_boot", "complete".to_owned()),
            ("systemd_machine_ready", "true".to_owned()),
            ("supervisor_socket_type", "unix-seqpacket".to_owned()),
            ("supervisor_socket_connected", "true".to_owned()),
            ("systemd_shutdown", "complete".to_owned()),
            ("compiler_uid", self.compiler_uid.to_string()),
            ("compiler_gid", self.compiler_gid.to_string()),
            ("anchor_uid", self.anchor_uid.to_string()),
            ("anchor_gid", self.anchor_gid.to_string()),
            ("policy_generation", self.policy_generation.to_string()),
            ("compiler_execution_provisioning", "complete".to_owned()),
            ("installed_lower_revalidated", "true".to_owned()),
            ("post_boot_provisioning_revalidated", "true".to_owned()),
            ("post_boot_lower_revalidated", "true".to_owned()),
            ("cleanup", "complete".to_owned()),
        ] {
            writeln!(report, "{name}={value}").expect("writing to a String cannot fail");
        }
        report
    }
}

/// Inert evidence that one fixed post-transition fault was observed and fully cleaned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerExecutionQualificationFaultReportV1 {
    fault_point: QualificationFaultPointV1,
    git_commit: String,
    target: String,
    manifest_sha256: [u8; 32],
    base_image_sha256: [u8; 32],
    installed_root_name: String,
    installed_publication: CompilerExecutionInstalledRootPublicationV1,
    staging_name: String,
}

impl CompilerExecutionQualificationFaultReportV1 {
    /// Encodes the successful interruption-and-cleanup result as stable key-value evidence.
    pub fn canonical_report(&self) -> String {
        let publication = match self.installed_publication {
            CompilerExecutionInstalledRootPublicationV1::Created => "created",
            CompilerExecutionInstalledRootPublicationV1::Reacquired => "reacquired",
        };
        let mut report = String::new();
        for (name, value) in [
            (
                "report_schema",
                QUALIFICATION_FAULT_REPORT_SCHEMA_V1.to_owned(),
            ),
            ("fault_point", self.fault_point.canonical_name().to_owned()),
            ("git_commit", self.git_commit.clone()),
            ("target", self.target.clone()),
            (
                "manifest_sha256",
                encode_sha256_lower_hex_v1(self.manifest_sha256),
            ),
            (
                "base_image_sha256",
                encode_sha256_lower_hex_v1(self.base_image_sha256),
            ),
            ("installed_root_name", self.installed_root_name.clone()),
            ("installed_publication", publication.to_owned()),
            ("staging_name", self.staging_name.clone()),
            ("injected_failure_observed", "true".to_owned()),
            ("installed_lower_revalidated", "true".to_owned()),
            ("cleanup", "complete".to_owned()),
        ] {
            writeln!(report, "{name}={value}").expect("writing to a String cannot fail");
        }
        report
    }
}

/// Stable aggregate evidence from two normal runs and every V1 qualification interruption.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerExecutionQualificationCampaignReportV1 {
    git_commit: String,
    target: String,
    manifest_sha256: [u8; 32],
    base_image_sha256: [u8; 32],
    installed_root_name: String,
    systemd_version: String,
    compiler_uid: u32,
    compiler_gid: u32,
    anchor_uid: u32,
    anchor_gid: u32,
    policy_generation: u64,
}

impl CompilerExecutionQualificationCampaignReportV1 {
    /// Encodes the complete campaign as one newline-terminated key-value report.
    pub fn canonical_report(&self) -> String {
        let fault_points = QualificationFaultPointV1::all()
            .iter()
            .map(|point| point.canonical_name())
            .collect::<Vec<_>>()
            .join(",");
        let mut report = String::new();
        for (name, value) in [
            (
                "report_schema",
                QUALIFICATION_CAMPAIGN_REPORT_SCHEMA_V1.to_owned(),
            ),
            ("git_commit", self.git_commit.clone()),
            ("target", self.target.clone()),
            (
                "manifest_sha256",
                encode_sha256_lower_hex_v1(self.manifest_sha256),
            ),
            (
                "base_image_sha256",
                encode_sha256_lower_hex_v1(self.base_image_sha256),
            ),
            ("installed_root_name", self.installed_root_name.clone()),
            ("fault_points", fault_points),
            ("normal_run_count", "2".to_owned()),
            ("systemd_preflight_run_count", "2".to_owned()),
            ("compiler_execution_provisioning_run_count", "2".to_owned()),
            ("systemd_boot_run_count", "2".to_owned()),
            ("supervisor_socket_connectivity_run_count", "2".to_owned()),
            ("systemd_version", self.systemd_version.clone()),
            ("compiler_uid", self.compiler_uid.to_string()),
            ("compiler_gid", self.compiler_gid.to_string()),
            ("anchor_uid", self.anchor_uid.to_string()),
            ("anchor_gid", self.anchor_gid.to_string()),
            ("policy_generation", self.policy_generation.to_string()),
            (
                "qualification_fault_count",
                QualificationFaultPointV1::all().len().to_string(),
            ),
            (
                "reacquisition_count",
                (QualificationFaultPointV1::all().len() * 2 + 1).to_string(),
            ),
            ("first_publication", "created".to_owned()),
            ("qualification_parent_empty", "true".to_owned()),
            ("cleanup", "complete".to_owned()),
        ] {
            writeln!(report, "{name}={value}").expect("writing to a String cannot fail");
        }
        report
    }
}

struct StagedQualificationTransactionV1 {
    target: String,
    installed_root_name: String,
    installed_publication: CompilerExecutionInstalledRootPublicationV1,
    staged: StagedCompilerExecutionQualificationV1,
}

/// Borrowed immutable inputs shared by normal qualification and mount-fault runs.
#[derive(Clone, Copy, Debug)]
pub struct CompilerExecutionQualificationRequestV1<'a> {
    bundle_root: &'a Path,
    expected_manifest_sha256: &'a str,
    expected_git_commit: &'a str,
    install_parent: &'a Path,
    base_image_path: &'a Path,
    expected_base_image_sha256: &'a str,
    qualification_parent: &'a Path,
}

impl<'a> CompilerExecutionQualificationRequestV1<'a> {
    /// Creates one inert request; every path and pin is still admitted by the transaction itself.
    pub const fn new(
        bundle_root: &'a Path,
        expected_manifest_sha256: &'a str,
        expected_git_commit: &'a str,
        install_parent: &'a Path,
        base_image_path: &'a Path,
        expected_base_image_sha256: &'a str,
        qualification_parent: &'a Path,
    ) -> Self {
        Self {
            bundle_root,
            expected_manifest_sha256,
            expected_git_commit,
            install_parent,
            base_image_path,
            expected_base_image_sha256,
            qualification_parent,
        }
    }
}

/// Runs and completely cleans one root-only disposable systemd boot transaction.
///
/// This is the sole high-level qualification path through verification, installation, mount
/// composition, sysusers, tmpfiles, unit verification, isolated boot, exact supervisor-socket
/// connection admission, bounded shutdown, exact postcondition admission, and cleanup. It grants
/// no persistent service or compiler-execution authority.
pub fn run_compiler_execution_qualification_v1(
    bundle_root: &Path,
    expected_manifest_sha256: &str,
    expected_git_commit: &str,
    install_parent: &Path,
    base_image_path: &Path,
    expected_base_image_sha256: &str,
    qualification_parent: &Path,
) -> Result<CompilerExecutionQualificationReportV1, DeploymentVerificationErrorV1> {
    let request = CompilerExecutionQualificationRequestV1::new(
        bundle_root,
        expected_manifest_sha256,
        expected_git_commit,
        install_parent,
        base_image_path,
        expected_base_image_sha256,
        qualification_parent,
    );
    run_compiler_execution_qualification_request_v1(request)
}

/// Runs one request through the sole root-only disposable qualification transaction.
pub fn run_compiler_execution_qualification_request_v1(
    request: CompilerExecutionQualificationRequestV1<'_>,
) -> Result<CompilerExecutionQualificationReportV1, DeploymentVerificationErrorV1> {
    let transaction = stage_qualification_transaction(&request)?;
    execute_staged_qualification_with_hooks(transaction, &mut NoQualificationFaultV1)
}

fn execute_staged_qualification_with_hooks(
    transaction: StagedQualificationTransactionV1,
    hooks: &mut impl QualificationFaultHooksV1,
) -> Result<CompilerExecutionQualificationReportV1, DeploymentVerificationErrorV1> {
    let staging_name = transaction.staged.run_name().to_owned();
    let namespace = enter_private_qualification_mount_namespace_v1()?;
    let mounted = attach_compiler_execution_qualification_mounts_with_hooks_v1(
        namespace,
        transaction.staged,
        hooks,
    )?;
    let preflight = run_compiler_execution_systemd_preflight_with_hooks_v1(mounted, hooks)?;
    let provisioned = run_compiler_execution_provisioning_with_hooks_v1(preflight, hooks)?;
    if let Err(error) = boot_and_stop_systemd_machine_v1(&provisioned, &staging_name, hooks) {
        return match provisioned.cleanup_with_hooks(hooks) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(super::invalid(
                DeploymentVerificationErrorKindV1::CleanupFailed,
                format!("systemd machine failed ({error}); cleanup also failed: {cleanup}"),
            )),
        };
    }
    let report = CompilerExecutionQualificationReportV1 {
        git_commit: provisioned.git_commit().to_owned(),
        target: transaction.target,
        manifest_sha256: provisioned.manifest_sha256(),
        base_image_sha256: provisioned.base_image_sha256(),
        installed_root_name: transaction.installed_root_name,
        installed_publication: transaction.installed_publication,
        staging_name,
        systemd_version: provisioned.systemd_version().to_owned(),
        verified_unit_count: provisioned.verified_unit_count(),
        compiler_uid: provisioned.compiler_uid(),
        compiler_gid: provisioned.compiler_gid(),
        anchor_uid: provisioned.anchor_uid(),
        anchor_gid: provisioned.anchor_gid(),
        policy_generation: provisioned.policy_generation(),
    };
    provisioned.cleanup_with_hooks(hooks)?;
    Ok(report)
}

/// Injects one fixed root-only qualification fault and admits only complete cleanup and recovery.
pub fn run_compiler_execution_qualification_fault_v1(
    fault_point: QualificationFaultPointV1,
    request: CompilerExecutionQualificationRequestV1<'_>,
) -> Result<CompilerExecutionQualificationFaultReportV1, DeploymentVerificationErrorV1> {
    let transaction = stage_qualification_transaction(&request)?;
    let git_commit = transaction.staged.git_commit().to_owned();
    let target = transaction.target.clone();
    let manifest_sha256 = transaction.staged.manifest_sha256();
    let base_image_sha256 = transaction.staged.base_image_sha256();
    let installed_root_name = transaction.installed_root_name.clone();
    let installed_publication = transaction.installed_publication;
    let staging_name = transaction.staged.run_name().to_owned();
    let mut hooks = InjectQualificationFaultV1::new(fault_point);
    let error = match execute_staged_qualification_with_hooks(transaction, &mut hooks) {
        Ok(_) => {
            return Err(super::invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationPreflight,
                format!(
                    "qualification fault point {} was not reached",
                    fault_point.canonical_name()
                ),
            ));
        }
        Err(error) => error,
    };
    if !hooks.fired() {
        return Err(error);
    }
    if error.kind() != DeploymentVerificationErrorKindV1::InjectedFailure {
        return Err(error);
    }
    verify_empty_qualification_parent_v1(request.qualification_parent)?;
    revalidate_qualification_inputs_after_fault(
        &request,
        &git_commit,
        &target,
        manifest_sha256,
        base_image_sha256,
        &installed_root_name,
    )?;
    Ok(CompilerExecutionQualificationFaultReportV1 {
        fault_point,
        git_commit,
        target,
        manifest_sha256,
        base_image_sha256,
        installed_root_name,
        installed_publication,
        staging_name,
    })
}

fn revalidate_qualification_inputs_after_fault(
    request: &CompilerExecutionQualificationRequestV1<'_>,
    expected_git_commit: &str,
    expected_target: &str,
    expected_manifest_sha256: [u8; 32],
    expected_base_image_sha256: [u8; 32],
    expected_installed_root_name: &str,
) -> Result<(), DeploymentVerificationErrorV1> {
    let verified = verify_compiler_execution_deployment_v1(
        request.bundle_root,
        request.expected_manifest_sha256,
        request.expected_git_commit,
    )?;
    let installed = install_compiler_execution_deployment_v1(verified, request.install_parent)?;
    if installed.publication() != CompilerExecutionInstalledRootPublicationV1::Reacquired
        || installed.git_commit() != expected_git_commit
        || installed.target() != expected_target
        || installed.manifest_sha256() != expected_manifest_sha256
        || installed.root_name() != expected_installed_root_name
    {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InputChanged,
            "installed lower identity changed after qualification fault cleanup",
        ));
    }
    let prepared = prepare_compiler_execution_qualification_v1(
        installed,
        request.base_image_path,
        request.expected_base_image_sha256,
        request.qualification_parent,
    )?;
    if prepared.git_commit() != expected_git_commit
        || prepared.manifest_sha256() != expected_manifest_sha256
        || prepared.installed_root_name() != expected_installed_root_name
        || prepared.base_image_sha256() != expected_base_image_sha256
    {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InputChanged,
            "qualification input identity changed after injected failure",
        ));
    }
    prepared.revalidate()?;
    drop(prepared);
    verify_empty_qualification_parent_v1(request.qualification_parent)?;
    verify_install_parent_children_v1(request.install_parent, &[expected_installed_root_name])
}

/// Runs two normal transactions and every fixed fault from one initially empty install parent.
pub fn run_compiler_execution_qualification_campaign_v1(
    request: CompilerExecutionQualificationRequestV1<'_>,
) -> Result<CompilerExecutionQualificationCampaignReportV1, DeploymentVerificationErrorV1> {
    verify_install_parent_children_v1(request.install_parent, &[])?;
    verify_empty_qualification_parent_v1(request.qualification_parent)?;

    let first = run_compiler_execution_qualification_request_v1(request)?;
    verify_empty_qualification_parent_v1(request.qualification_parent)?;
    if first.installed_publication != CompilerExecutionInstalledRootPublicationV1::Created {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InputChanged,
            "qualification campaign did not perform the first installed-root publication",
        ));
    }
    verify_install_parent_children_v1(
        request.install_parent,
        &[first.installed_root_name.as_str()],
    )?;

    for point in QualificationFaultPointV1::all() {
        let fault = run_compiler_execution_qualification_fault_v1(*point, request)?;
        require_fault_identity(&first, &fault)?;
        if fault.installed_publication != CompilerExecutionInstalledRootPublicationV1::Reacquired {
            return Err(super::invalid(
                DeploymentVerificationErrorKindV1::InputChanged,
                format!(
                    "qualification fault {} did not reacquire the exact installed root",
                    point.canonical_name()
                ),
            ));
        }
        verify_install_parent_children_v1(
            request.install_parent,
            &[first.installed_root_name.as_str()],
        )?;
    }

    let second = run_compiler_execution_qualification_request_v1(request)?;
    verify_empty_qualification_parent_v1(request.qualification_parent)?;
    require_normal_identity(&first, &second)?;
    if second.installed_publication != CompilerExecutionInstalledRootPublicationV1::Reacquired {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InputChanged,
            "second normal qualification run did not reacquire the exact installed root",
        ));
    }
    verify_install_parent_children_v1(
        request.install_parent,
        &[first.installed_root_name.as_str()],
    )?;
    Ok(CompilerExecutionQualificationCampaignReportV1 {
        git_commit: first.git_commit,
        target: first.target,
        manifest_sha256: first.manifest_sha256,
        base_image_sha256: first.base_image_sha256,
        installed_root_name: first.installed_root_name,
        systemd_version: first.systemd_version,
        compiler_uid: first.compiler_uid,
        compiler_gid: first.compiler_gid,
        anchor_uid: first.anchor_uid,
        anchor_gid: first.anchor_gid,
        policy_generation: first.policy_generation,
    })
}

fn require_fault_identity(
    expected: &CompilerExecutionQualificationReportV1,
    observed: &CompilerExecutionQualificationFaultReportV1,
) -> Result<(), DeploymentVerificationErrorV1> {
    require_identity(
        expected,
        &observed.git_commit,
        &observed.target,
        observed.manifest_sha256,
        observed.base_image_sha256,
        &observed.installed_root_name,
    )
}

fn require_normal_identity(
    expected: &CompilerExecutionQualificationReportV1,
    observed: &CompilerExecutionQualificationReportV1,
) -> Result<(), DeploymentVerificationErrorV1> {
    require_identity(
        expected,
        &observed.git_commit,
        &observed.target,
        observed.manifest_sha256,
        observed.base_image_sha256,
        &observed.installed_root_name,
    )?;
    require_systemd_identity(expected, observed)
}

fn require_identity(
    expected: &CompilerExecutionQualificationReportV1,
    git_commit: &str,
    target: &str,
    manifest_sha256: [u8; 32],
    base_image_sha256: [u8; 32],
    installed_root_name: &str,
) -> Result<(), DeploymentVerificationErrorV1> {
    if expected.git_commit != git_commit
        || expected.target != target
        || expected.manifest_sha256 != manifest_sha256
        || expected.base_image_sha256 != base_image_sha256
        || expected.installed_root_name != installed_root_name
    {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InputChanged,
            "qualification campaign identity changed between transactions",
        ));
    }
    Ok(())
}

fn require_systemd_identity(
    expected: &CompilerExecutionQualificationReportV1,
    observed: &CompilerExecutionQualificationReportV1,
) -> Result<(), DeploymentVerificationErrorV1> {
    if expected.systemd_version != observed.systemd_version
        || expected.verified_unit_count != observed.verified_unit_count
        || expected.compiler_uid != observed.compiler_uid
        || expected.compiler_gid != observed.compiler_gid
        || expected.anchor_uid != observed.anchor_uid
        || expected.anchor_gid != observed.anchor_gid
        || expected.policy_generation != observed.policy_generation
    {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InputChanged,
            "systemd or provisioning identity changed between qualification runs",
        ));
    }
    Ok(())
}

fn stage_qualification_transaction(
    request: &CompilerExecutionQualificationRequestV1<'_>,
) -> Result<StagedQualificationTransactionV1, DeploymentVerificationErrorV1> {
    let verified = verify_compiler_execution_deployment_v1(
        request.bundle_root,
        request.expected_manifest_sha256,
        request.expected_git_commit,
    )?;
    let installed = install_compiler_execution_deployment_v1(verified, request.install_parent)?;
    let target = installed.target().to_owned();
    let installed_root_name = installed.root_name().to_owned();
    let installed_publication = installed.publication();
    let prepared = prepare_compiler_execution_qualification_v1(
        installed,
        request.base_image_path,
        request.expected_base_image_sha256,
        request.qualification_parent,
    )?;
    let staged = stage_compiler_execution_qualification_v1(prepared)?;
    Ok(StagedQualificationTransactionV1 {
        target,
        installed_root_name,
        installed_publication,
        staged,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_qualification_report_is_canonical_and_explicit_about_cleanup() {
        let report = CompilerExecutionQualificationReportV1 {
            git_commit: "01".repeat(20),
            target: "x86_64-unknown-linux-musl".to_owned(),
            manifest_sha256: [0x23; 32],
            base_image_sha256: [0x45; 32],
            installed_root_name: "compiler-execution-v1-test".to_owned(),
            installed_publication: CompilerExecutionInstalledRootPublicationV1::Reacquired,
            staging_name: ".compiler-execution-qualification-v1-test".to_owned(),
            systemd_version: "255.4-1ubuntu8.17".to_owned(),
            verified_unit_count: 3,
            compiler_uid: 999,
            compiler_gid: 999,
            anchor_uid: 998,
            anchor_gid: 998,
            policy_generation: 1,
        };
        let encoded = report.canonical_report();
        assert_eq!(encoded.lines().count(), 28);
        assert!(encoded.contains("systemd_sysusers=complete\n"));
        assert!(encoded.contains("systemd_unit_verify_count=3\n"));
        assert!(encoded.contains("systemd_boot=complete\n"));
        assert!(encoded.contains("post_boot_provisioning_revalidated=true\n"));
        assert!(encoded.contains("systemd_machine_ready=true\n"));
        assert!(encoded.contains("supervisor_socket_type=unix-seqpacket\n"));
        assert!(encoded.contains("supervisor_socket_connected=true\n"));
        assert!(encoded.ends_with("post_boot_lower_revalidated=true\ncleanup=complete\n"));
        assert!(encoded.contains(&format!(
            "manifest_sha256={}\n",
            encode_sha256_lower_hex_v1([0x23; 32])
        )));
        assert!(encoded.contains("installed_publication=reacquired\n"));
    }

    #[test]
    fn qualification_fault_report_binds_the_point_lower_and_cleanup() {
        let report = CompilerExecutionQualificationFaultReportV1 {
            fault_point: QualificationFaultPointV1::SystemdTmpfilesComplete,
            git_commit: "67".repeat(20),
            target: "x86_64-unknown-linux-musl".to_owned(),
            manifest_sha256: [0x89; 32],
            base_image_sha256: [0xab; 32],
            installed_root_name: "compiler-execution-v1-fault".to_owned(),
            installed_publication: CompilerExecutionInstalledRootPublicationV1::Created,
            staging_name: ".compiler-execution-qualification-v1-fault".to_owned(),
        };
        let encoded = report.canonical_report();
        assert_eq!(encoded.lines().count(), 12);
        assert!(encoded.contains("fault_point=systemd-tmpfiles-complete\n"));
        assert!(encoded.contains("installed_publication=created\n"));
        assert!(encoded.ends_with("installed_lower_revalidated=true\ncleanup=complete\n"));
    }

    #[test]
    fn qualification_campaign_report_excludes_runtime_staging_identities() {
        let report = CompilerExecutionQualificationCampaignReportV1 {
            git_commit: "cd".repeat(20),
            target: "x86_64-unknown-linux-musl".to_owned(),
            manifest_sha256: [0xef; 32],
            base_image_sha256: [0x12; 32],
            installed_root_name: "compiler-execution-v1-campaign".to_owned(),
            systemd_version: "255.4-1ubuntu8.17".to_owned(),
            compiler_uid: 999,
            compiler_gid: 999,
            anchor_uid: 998,
            anchor_gid: 998,
            policy_generation: 1,
        };
        let encoded = report.canonical_report();
        assert_eq!(encoded.lines().count(), 23);
        assert!(encoded.contains("normal_run_count=2\n"));
        assert!(encoded.contains("systemd_preflight_run_count=2\n"));
        assert!(encoded.contains("compiler_execution_provisioning_run_count=2\n"));
        assert!(encoded.contains("systemd_boot_run_count=2\n"));
        assert!(encoded.contains("supervisor_socket_connectivity_run_count=2\n"));
        assert!(encoded.contains("qualification_fault_count=25\n"));
        assert!(encoded.contains("reacquisition_count=51\n"));
        assert!(!encoded.contains("staging_name"));
        assert!(encoded.ends_with("qualification_parent_empty=true\ncleanup=complete\n"));
    }
}
