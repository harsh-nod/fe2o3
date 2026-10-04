use std::error::Error;
use std::fmt;

macro_rules! error_codes {
    ($($variant:ident => $text:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub enum HostLinkErrorCodeV1 {
            $($variant),+
        }

        impl HostLinkErrorCodeV1 {
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }
        }

        pub const HOST_LINK_REJECTION_CODES_V1: &[&str] = &[
            $($text),+
        ];
    };
}

error_codes! {
    UnsupportedPlatform => "unsupported-platform",
    Io => "io",
    InvalidVersion => "invalid-version",
    InvalidWire => "invalid-wire",
    NonCanonicalWire => "noncanonical-wire",
    PlanTooLarge => "plan-too-large",
    FieldTooLarge => "field-too-large",
    InvalidText => "invalid-text",
    UnsupportedArgument => "unsupported-argument",
    InvalidPath => "invalid-path",
    InvalidNonce => "invalid-nonce",
    DuplicateRecord => "duplicate-record",
    NonCanonicalOrder => "noncanonical-order",
    DigestMismatch => "digest-mismatch",
    ReplayMismatch => "replay-mismatch",
    WrongTarget => "wrong-target",
    WrongNonce => "wrong-nonce",
    NotRegular => "not-regular",
    DescriptorChanged => "descriptor-changed",
    DescriptorUnsealed => "descriptor-unsealed",
    ArtifactTooLarge => "artifact-too-large",
    ArtifactKind => "artifact-kind",
    ThinArchive => "thin-archive",
    Symlink => "symlink",
    RootChanged => "root-changed",
    RootMutation => "root-mutation",
    UnresolvedSearch => "unresolved-search",
    UnresolvedLibrary => "unresolved-library",
    ResponseFile => "response-file",
    NestedResponseFile => "nested-response-file",
    LinkerScript => "linker-script",
    ScriptSearchDir => "script-search-dir",
    ScriptInclude => "script-include",
    AbsoluteNestedPath => "absolute-nested-path",
    Plugin => "plugin",
    Lto => "lto",
    UnpublishedBuildScript => "unpublished-build-script",
    ElfPolicy => "elf-policy",
    OutputChanged => "output-changed",
    OutputEmpty => "output-empty",
    OutputTruncated => "output-truncated",
    ResultPending => "result-pending",
    WorkerLaunch => "worker-launch",
    WorkerIdentity => "worker-identity",
    WorkerExit => "worker-exit",
    WorkerTimeout => "worker-timeout",
    WorkerCapacity => "worker-capacity",
    ToolApproval => "tool-approval",
    RuntimeDsoClosure => "runtime-dso-closure",
    InvalidState => "invalid-state",
}

impl fmt::Display for HostLinkErrorCodeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A stable rejection code plus a bounded human-readable diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostLinkError {
    code: HostLinkErrorCodeV1,
    detail: String,
}

impl HostLinkError {
    pub(crate) fn new(code: HostLinkErrorCodeV1, detail: impl Into<String>) -> Self {
        let mut detail = detail.into();
        if detail.len() > 4096 {
            detail.truncate(4096);
        }
        Self { code, detail }
    }

    pub const fn code(&self) -> HostLinkErrorCodeV1 {
        self.code
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl fmt::Display for HostLinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "host-link-closure-v1[{}]: {}",
            self.code, self.detail
        )
    }
}

impl Error for HostLinkError {}

pub(crate) trait ResultContext<T> {
    fn context(
        self,
        code: HostLinkErrorCodeV1,
        message: impl FnOnce() -> String,
    ) -> Result<T, HostLinkError>;
}

impl<T, E: fmt::Display> ResultContext<T> for Result<T, E> {
    fn context(
        self,
        code: HostLinkErrorCodeV1,
        message: impl FnOnce() -> String,
    ) -> Result<T, HostLinkError> {
        self.map_err(|error| HostLinkError::new(code, format!("{}: {error}", message())))
    }
}
