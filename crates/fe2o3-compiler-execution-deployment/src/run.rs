use std::fmt::Write as _;
use std::path::Path;

use super::{
    CompilerExecutionInstalledRootPublicationV1, DeploymentVerificationErrorKindV1,
    DeploymentVerificationErrorV1, attach_compiler_execution_qualification_mounts_v1,
    encode_sha256_lower_hex_v1, enter_private_qualification_mount_namespace_v1,
    install_compiler_execution_deployment_v1, prepare_compiler_execution_qualification_v1,
    stage_compiler_execution_qualification_v1, verify_compiler_execution_deployment_v1,
};

const MOUNT_QUALIFICATION_REPORT_SCHEMA_V1: &str =
    "fe2o3-compiler-execution-mount-qualification-report-v1";

/// Inert report from one fully cleaned mount-only qualification transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerExecutionMountQualificationReportV1 {
    git_commit: String,
    target: String,
    manifest_sha256: [u8; 32],
    base_image_sha256: [u8; 32],
    installed_root_name: String,
    installed_publication: CompilerExecutionInstalledRootPublicationV1,
    staging_name: String,
}

impl CompilerExecutionMountQualificationReportV1 {
    /// Encodes the completed transaction as one stable newline-terminated key-value report.
    pub fn canonical_report(&self) -> String {
        let publication = match self.installed_publication {
            CompilerExecutionInstalledRootPublicationV1::Created => "created",
            CompilerExecutionInstalledRootPublicationV1::Reacquired => "reacquired",
        };
        let mut report = String::new();
        for (name, value) in [
            (
                "report_schema",
                MOUNT_QUALIFICATION_REPORT_SCHEMA_V1.to_owned(),
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
            ("installed_publication", publication.to_owned()),
            ("staging_name", self.staging_name.clone()),
            ("mount_revalidated", "true".to_owned()),
            ("cleanup", "complete".to_owned()),
        ] {
            writeln!(report, "{name}={value}").expect("writing to a String cannot fail");
        }
        report
    }
}

/// Runs and completely cleans one root-only disposable mount qualification transaction.
///
/// This is the sole high-level mount path: verify the pinned bundle, atomically install or
/// reacquire it, seal the pinned base, stage an empty disposable tree, enter a private mount
/// namespace, attach and revalidate both filesystems, and explicitly clean all mounts and staging.
/// It grants no boot, service, or compiler-execution authority.
pub fn run_compiler_execution_mount_qualification_v1(
    bundle_root: &Path,
    expected_manifest_sha256: &str,
    expected_git_commit: &str,
    install_parent: &Path,
    base_image_path: &Path,
    expected_base_image_sha256: &str,
    qualification_parent: &Path,
) -> Result<CompilerExecutionMountQualificationReportV1, DeploymentVerificationErrorV1> {
    let verified = verify_compiler_execution_deployment_v1(
        bundle_root,
        expected_manifest_sha256,
        expected_git_commit,
    )?;
    let installed = install_compiler_execution_deployment_v1(verified, install_parent)?;
    let target = installed.target().to_owned();
    let installed_root_name = installed.root_name().to_owned();
    let installed_publication = installed.publication();
    let prepared = prepare_compiler_execution_qualification_v1(
        installed,
        base_image_path,
        expected_base_image_sha256,
        qualification_parent,
    )?;
    let staged = stage_compiler_execution_qualification_v1(prepared)?;
    let staging_name = staged.run_name().to_owned();
    let namespace = enter_private_qualification_mount_namespace_v1()?;
    let mounted = attach_compiler_execution_qualification_mounts_v1(namespace, staged)?;
    if let Err(error) = mounted.revalidate() {
        return match mounted.cleanup() {
            Ok(()) => Err(error),
            Err(cleanup) => Err(super::invalid(
                DeploymentVerificationErrorKindV1::CleanupFailed,
                format!("qualification revalidation failed and cleanup also failed: {cleanup}"),
            )),
        };
    }
    let report = CompilerExecutionMountQualificationReportV1 {
        git_commit: mounted.git_commit().to_owned(),
        target,
        manifest_sha256: mounted.manifest_sha256(),
        base_image_sha256: mounted.base_image_sha256(),
        installed_root_name,
        installed_publication,
        staging_name,
    };
    mounted.cleanup()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_mount_report_is_canonical_and_explicit_about_cleanup() {
        let report = CompilerExecutionMountQualificationReportV1 {
            git_commit: "01".repeat(20),
            target: "x86_64-unknown-linux-musl".to_owned(),
            manifest_sha256: [0x23; 32],
            base_image_sha256: [0x45; 32],
            installed_root_name: "compiler-execution-v1-test".to_owned(),
            installed_publication: CompilerExecutionInstalledRootPublicationV1::Reacquired,
            staging_name: ".compiler-execution-qualification-v1-test".to_owned(),
        };
        let encoded = report.canonical_report();
        assert_eq!(encoded.lines().count(), 10);
        assert!(encoded.ends_with("mount_revalidated=true\ncleanup=complete\n"));
        assert!(encoded.contains(&format!(
            "manifest_sha256={}\n",
            encode_sha256_lower_hex_v1([0x23; 32])
        )));
        assert!(encoded.contains("installed_publication=reacquired\n"));
    }
}
