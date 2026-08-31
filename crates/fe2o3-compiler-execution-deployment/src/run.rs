use std::fmt::Write as _;
use std::path::Path;

use super::install::verify_install_parent_children_v1;
use super::mount::{
    InjectQualificationMountFaultV1, QualificationMountFaultPointV1,
    attach_compiler_execution_qualification_mounts_with_hooks_v1,
};
use super::preflight::run_compiler_execution_systemd_preflight_v1;
use super::qualification::verify_empty_qualification_parent_v1;
use super::{
    CompilerExecutionInstalledRootPublicationV1, DeploymentVerificationErrorKindV1,
    DeploymentVerificationErrorV1, StagedCompilerExecutionQualificationV1,
    attach_compiler_execution_qualification_mounts_v1, encode_sha256_lower_hex_v1,
    enter_private_qualification_mount_namespace_v1, install_compiler_execution_deployment_v1,
    prepare_compiler_execution_qualification_v1, stage_compiler_execution_qualification_v1,
    verify_compiler_execution_deployment_v1,
};

const QUALIFICATION_REPORT_SCHEMA_V1: &str = "fe2o3-compiler-execution-qualification-report-v1";
const MOUNT_FAULT_REPORT_SCHEMA_V1: &str = "fe2o3-compiler-execution-mount-fault-report-v1";
const MOUNT_CAMPAIGN_REPORT_SCHEMA_V1: &str = "fe2o3-compiler-execution-mount-campaign-report-v1";

/// Inert report from one fully cleaned composed-root systemd preflight transaction.
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
            ("compiler_uid", self.compiler_uid.to_string()),
            ("compiler_gid", self.compiler_gid.to_string()),
            ("anchor_uid", self.anchor_uid.to_string()),
            ("anchor_gid", self.anchor_gid.to_string()),
            ("installed_lower_revalidated", "true".to_owned()),
            ("cleanup", "complete".to_owned()),
        ] {
            writeln!(report, "{name}={value}").expect("writing to a String cannot fail");
        }
        report
    }
}

/// Inert evidence that one fixed post-transition fault was observed and fully cleaned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerExecutionMountFaultReportV1 {
    fault_point: QualificationMountFaultPointV1,
    git_commit: String,
    target: String,
    manifest_sha256: [u8; 32],
    base_image_sha256: [u8; 32],
    installed_root_name: String,
    installed_publication: CompilerExecutionInstalledRootPublicationV1,
    staging_name: String,
}

impl CompilerExecutionMountFaultReportV1 {
    /// Encodes the successful interruption-and-cleanup result as stable key-value evidence.
    pub fn canonical_report(&self) -> String {
        let publication = match self.installed_publication {
            CompilerExecutionInstalledRootPublicationV1::Created => "created",
            CompilerExecutionInstalledRootPublicationV1::Reacquired => "reacquired",
        };
        let mut report = String::new();
        for (name, value) in [
            ("report_schema", MOUNT_FAULT_REPORT_SCHEMA_V1.to_owned()),
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
            ("cleanup", "complete".to_owned()),
        ] {
            writeln!(report, "{name}={value}").expect("writing to a String cannot fail");
        }
        report
    }
}

/// Stable aggregate evidence from two mount runs and every V1 lifecycle interruption.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerExecutionMountCampaignReportV1 {
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
}

impl CompilerExecutionMountCampaignReportV1 {
    /// Encodes the complete campaign as one newline-terminated key-value report.
    pub fn canonical_report(&self) -> String {
        let fault_points = QualificationMountFaultPointV1::all()
            .iter()
            .map(|point| point.canonical_name())
            .collect::<Vec<_>>()
            .join(",");
        let mut report = String::new();
        for (name, value) in [
            ("report_schema", MOUNT_CAMPAIGN_REPORT_SCHEMA_V1.to_owned()),
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
            ("systemd_version", self.systemd_version.clone()),
            ("compiler_uid", self.compiler_uid.to_string()),
            ("compiler_gid", self.compiler_gid.to_string()),
            ("anchor_uid", self.anchor_uid.to_string()),
            ("anchor_gid", self.anchor_gid.to_string()),
            (
                "mount_fault_count",
                QualificationMountFaultPointV1::all().len().to_string(),
            ),
            (
                "reacquisition_count",
                (QualificationMountFaultPointV1::all().len() + 1).to_string(),
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

struct StagedMountQualificationV1 {
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

/// Runs and completely cleans one root-only disposable systemd preflight transaction.
///
/// This is the sole high-level qualification path through verification, installation, mount
/// composition, sysusers, tmpfiles, unit verification, exact postcondition admission, and cleanup.
/// It grants no boot, service, or compiler-execution authority.
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
    let transaction = stage_mount_qualification(&request)?;
    let staging_name = transaction.staged.run_name().to_owned();
    let namespace = enter_private_qualification_mount_namespace_v1()?;
    let mounted = attach_compiler_execution_qualification_mounts_v1(namespace, transaction.staged)?;
    let preflight = run_compiler_execution_systemd_preflight_v1(mounted)?;
    let report = CompilerExecutionQualificationReportV1 {
        git_commit: preflight.git_commit().to_owned(),
        target: transaction.target,
        manifest_sha256: preflight.manifest_sha256(),
        base_image_sha256: preflight.base_image_sha256(),
        installed_root_name: transaction.installed_root_name,
        installed_publication: transaction.installed_publication,
        staging_name,
        systemd_version: preflight.systemd_version().to_owned(),
        verified_unit_count: preflight.verified_unit_count(),
        compiler_uid: preflight.compiler_uid(),
        compiler_gid: preflight.compiler_gid(),
        anchor_uid: preflight.anchor_uid(),
        anchor_gid: preflight.anchor_gid(),
    };
    preflight.cleanup()?;
    Ok(report)
}

/// Injects one fixed root-only mount fault and succeeds only after complete cleanup is proven.
pub fn run_compiler_execution_mount_fault_v1(
    fault_point: QualificationMountFaultPointV1,
    request: CompilerExecutionQualificationRequestV1<'_>,
) -> Result<CompilerExecutionMountFaultReportV1, DeploymentVerificationErrorV1> {
    let transaction = stage_mount_qualification(&request)?;
    let git_commit = transaction.staged.git_commit().to_owned();
    let manifest_sha256 = transaction.staged.manifest_sha256();
    let base_image_sha256 = transaction.staged.base_image_sha256();
    let staging_name = transaction.staged.run_name().to_owned();
    let namespace = enter_private_qualification_mount_namespace_v1()?;
    let mut hooks = InjectQualificationMountFaultV1::new(fault_point);
    let interrupted = match attach_compiler_execution_qualification_mounts_with_hooks_v1(
        namespace,
        transaction.staged,
        &mut hooks,
    ) {
        Ok(mounted) => mounted.cleanup_with_hooks(&mut hooks),
        Err(error) => Err(error),
    };
    let error = match interrupted {
        Ok(()) => {
            return Err(super::invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationMount,
                format!(
                    "qualification mount fault point {} was not reached",
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
    Ok(CompilerExecutionMountFaultReportV1 {
        fault_point,
        git_commit,
        target: transaction.target,
        manifest_sha256,
        base_image_sha256,
        installed_root_name: transaction.installed_root_name,
        installed_publication: transaction.installed_publication,
        staging_name,
    })
}

/// Runs two normal transactions and every fixed fault from one initially empty install parent.
pub fn run_compiler_execution_mount_campaign_v1(
    request: CompilerExecutionQualificationRequestV1<'_>,
) -> Result<CompilerExecutionMountCampaignReportV1, DeploymentVerificationErrorV1> {
    verify_install_parent_children_v1(request.install_parent, &[])?;
    verify_empty_qualification_parent_v1(request.qualification_parent)?;

    let first = run_compiler_execution_qualification_request_v1(request)?;
    verify_empty_qualification_parent_v1(request.qualification_parent)?;
    if first.installed_publication != CompilerExecutionInstalledRootPublicationV1::Created {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InputChanged,
            "mount campaign did not perform the first installed-root publication",
        ));
    }
    verify_install_parent_children_v1(
        request.install_parent,
        &[first.installed_root_name.as_str()],
    )?;

    for point in QualificationMountFaultPointV1::all() {
        let fault = run_compiler_execution_mount_fault_v1(*point, request)?;
        require_fault_identity(&first, &fault)?;
        if fault.installed_publication != CompilerExecutionInstalledRootPublicationV1::Reacquired {
            return Err(super::invalid(
                DeploymentVerificationErrorKindV1::InputChanged,
                format!(
                    "mount fault {} did not reacquire the exact installed root",
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
            "second normal mount run did not reacquire the exact installed root",
        ));
    }
    verify_install_parent_children_v1(
        request.install_parent,
        &[first.installed_root_name.as_str()],
    )?;
    Ok(CompilerExecutionMountCampaignReportV1 {
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
    })
}

fn require_fault_identity(
    expected: &CompilerExecutionQualificationReportV1,
    observed: &CompilerExecutionMountFaultReportV1,
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
            "mount campaign identity changed between transactions",
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
    {
        return Err(super::invalid(
            DeploymentVerificationErrorKindV1::InputChanged,
            "systemd preflight identity changed between qualification runs",
        ));
    }
    Ok(())
}

fn stage_mount_qualification(
    request: &CompilerExecutionQualificationRequestV1<'_>,
) -> Result<StagedMountQualificationV1, DeploymentVerificationErrorV1> {
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
    Ok(StagedMountQualificationV1 {
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
        };
        let encoded = report.canonical_report();
        assert_eq!(encoded.lines().count(), 19);
        assert!(encoded.contains("systemd_sysusers=complete\n"));
        assert!(encoded.contains("systemd_unit_verify_count=3\n"));
        assert!(encoded.ends_with("installed_lower_revalidated=true\ncleanup=complete\n"));
        assert!(encoded.contains(&format!(
            "manifest_sha256={}\n",
            encode_sha256_lower_hex_v1([0x23; 32])
        )));
        assert!(encoded.contains("installed_publication=reacquired\n"));
    }

    #[test]
    fn mount_fault_report_binds_the_point_and_complete_cleanup() {
        let report = CompilerExecutionMountFaultReportV1 {
            fault_point: QualificationMountFaultPointV1::OverlayUnmounted,
            git_commit: "67".repeat(20),
            target: "x86_64-unknown-linux-musl".to_owned(),
            manifest_sha256: [0x89; 32],
            base_image_sha256: [0xab; 32],
            installed_root_name: "compiler-execution-v1-fault".to_owned(),
            installed_publication: CompilerExecutionInstalledRootPublicationV1::Created,
            staging_name: ".compiler-execution-qualification-v1-fault".to_owned(),
        };
        let encoded = report.canonical_report();
        assert_eq!(encoded.lines().count(), 11);
        assert!(encoded.contains("fault_point=overlay-unmounted\n"));
        assert!(encoded.contains("installed_publication=created\n"));
        assert!(encoded.ends_with("injected_failure_observed=true\ncleanup=complete\n"));
    }

    #[test]
    fn mount_campaign_report_excludes_runtime_staging_identities() {
        let report = CompilerExecutionMountCampaignReportV1 {
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
        };
        let encoded = report.canonical_report();
        assert_eq!(encoded.lines().count(), 19);
        assert!(encoded.contains("normal_run_count=2\n"));
        assert!(encoded.contains("systemd_preflight_run_count=2\n"));
        assert!(encoded.contains("mount_fault_count=8\n"));
        assert!(encoded.contains("reacquisition_count=9\n"));
        assert!(!encoded.contains("staging_name"));
        assert!(encoded.ends_with("qualification_parent_empty=true\ncleanup=complete\n"));
    }
}
