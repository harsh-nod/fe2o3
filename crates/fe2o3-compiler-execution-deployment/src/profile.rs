//! Closed deployment inventories. Manifest bytes never select a profile.
use super::*;

/// Canonical native V3 install-manifest name, distinct from frozen V1.
pub const COMPILER_EXECUTION_INSTALL_MANIFEST_NAME_V3: &str = "INSTALL-MANIFEST-V3";
/// V3 content count; the V1-only client checker is deliberately absent.
pub const COMPILER_EXECUTION_INSTALL_FILE_COUNT_V3: usize = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Profile {
    V1,
    V3,
}

const FILE_SPECS_V3: [FileSpecV1; COMPILER_EXECUTION_INSTALL_FILE_COUNT_V3] = [
    FILE_SPECS_V1[0],
    FILE_SPECS_V1[1],
    FILE_SPECS_V1[2],
    FILE_SPECS_V1[3],
    FILE_SPECS_V1[4],
    FILE_SPECS_V1[6],
    FileSpecV1 {
        source: "usr/libexec/fe2o3/fe2o3-compiler-execution-issuer-conditional",
        install: "/usr/libexec/fe2o3/fe2o3-compiler-execution-issuer-conditional",
        ..FILE_SPECS_V1[7]
    },
    FILE_SPECS_V1[8],
    FileSpecV1 {
        source: "usr/libexec/fe2o3/fe2o3-compiler-execution-supervisor-v3",
        install: "/usr/libexec/fe2o3/fe2o3-compiler-execution-supervisor-v3",
        ..FILE_SPECS_V1[9]
    },
    FileSpecV1 {
        source: "usr/libexec/fe2o3/fe2o3-external-anchor-provisioning-helper-v3",
        install: "/usr/libexec/fe2o3/fe2o3-external-anchor-provisioning-helper-v3",
        ..FILE_SPECS_V1[10]
    },
    FileSpecV1 {
        source: "usr/libexec/fe2o3/fe2o3-external-anchor-service-v3",
        install: "/usr/libexec/fe2o3/fe2o3-external-anchor-service-v3",
        ..FILE_SPECS_V1[11]
    },
    FILE_SPECS_V1[12],
];
const IMAGE_CHILDREN_V3: &[&str] = &[
    "fe2o3-compiler-execution-coordinator",
    "fe2o3-compiler-execution-issuer-conditional",
    "fe2o3-compiler-execution-provision",
    "fe2o3-compiler-execution-supervisor-v3",
    "fe2o3-external-anchor-provisioning-helper-v3",
    "fe2o3-external-anchor-service-v3",
    "fe2o3-static-preexec-launcher",
];
const ROOT_CHILDREN_V3: &[&str] = &[
    "BUILD-INFO",
    "INSTALL-MANIFEST-V3",
    "SHA256SUMS",
    "systemd",
    "sysusers.d",
    "tmpfiles.d",
    "usr",
];

impl Profile {
    pub(super) const fn header(self) -> &'static str {
        match self {
            Self::V1 => MANIFEST_HEADER_V1,
            Self::V3 => "fe2o3-compiler-execution-install-manifest-v3",
        }
    }
    pub(super) const fn build_schema(self) -> &'static str {
        match self {
            Self::V1 => "schema_version=1",
            Self::V3 => "schema_version=3",
        }
    }
    pub(super) const fn manifest(self) -> FileSpecV1 {
        match self {
            Self::V1 => MANIFEST_FILE_SPEC_V1,
            Self::V3 => FileSpecV1 {
                source: COMPILER_EXECUTION_INSTALL_MANIFEST_NAME_V3,
                install: "/usr/share/fe2o3/compiler-execution/INSTALL-MANIFEST-V3",
                ..MANIFEST_FILE_SPEC_V1
            },
        }
    }
    pub(super) const fn files(self) -> &'static [FileSpecV1] {
        match self {
            Self::V1 => &FILE_SPECS_V1,
            Self::V3 => &FILE_SPECS_V3,
        }
    }
    pub(super) const fn root_children(self, manifest: bool) -> &'static [&'static str] {
        match (self, manifest) {
            (_, false) => ROOT_CHILDREN_WITHOUT_MANIFEST_V1,
            (Self::V1, true) => ROOT_CHILDREN_WITH_MANIFEST_V1,
            (Self::V3, true) => ROOT_CHILDREN_V3,
        }
    }
    pub(super) fn directory_children(
        self,
        path: &str,
        legacy: &'static [&'static str],
    ) -> &'static [&'static str] {
        match (self, path) {
            (Self::V3, "usr/libexec/fe2o3") => IMAGE_CHILDREN_V3,
            (Self::V3, "usr/share/fe2o3/compiler-execution") => {
                &["BUILD-INFO", "INSTALL-MANIFEST-V3", "SHA256SUMS"]
            }
            _ => legacy,
        }
    }
    pub(super) const fn root_prefix(self) -> &'static str {
        match self {
            Self::V1 => "compiler-execution-v1-",
            Self::V3 => "compiler-execution-v3-",
        }
    }
    pub(super) const fn staging_prefix(self) -> &'static str {
        match self {
            Self::V1 => ".compiler-execution-v1-staging-",
            Self::V3 => ".compiler-execution-v3-staging-",
        }
    }
}
