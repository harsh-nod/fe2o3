//! Closed CLI profile selection; not a wire capability or source-authority tag.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Profile {
    EntryV20,
    GlobalCopyV21,
    LdsExchangeV22,
}
macro_rules! codes {
    ($($variant:ident => $suffix:literal),+ $(,)?) => {
        #[derive(Clone, Copy)]
        pub(super) enum Code { $($variant),+ }
        impl Profile {
            pub(super) const fn code(self, code: Code) -> &'static str {
                match (self, code) {
                    $((Self::EntryV20, Code::$variant) => concat!("kir_v20_debug_", $suffix),
                    (Self::GlobalCopyV21, Code::$variant) => concat!("kir_v21_debug_", $suffix),
                    (Self::LdsExchangeV22, Code::$variant) => concat!("kir_v22_debug_", $suffix),)+
                }
            }
        }
    }
}
codes! {
    Arguments => "arguments",
    OptionUnavailable => "option_unavailable",
    ConfigurationInvalid => "configuration_invalid",
    CursorOutOfRange => "cursor_out_of_range",
    RevisionLimit => "revision_limit",
    StaleRevision => "stale_revision",
    Terminated => "terminated",
    ForeignOrStaleCursor => "foreign_or_stale_cursor",
    ResponseEncoding => "response_encoding",
    ResponseTooLarge => "response_too_large",
    OutputFailed => "output_failed",
    NavigationUnavailable => "navigation_unavailable",
    QueryWorkLimit => "query_work_limit",
    UnsupportedOrInvalidRequest => "unsupported_or_invalid_request",
    ProtocolRefused => "protocol_refused",
    CommandLimit => "command_limit",
}
impl Profile {
    pub(super) const fn selector(self) -> &'static str {
        match self {
            Self::EntryV20 => "--diagnostic-kir-v20",
            Self::GlobalCopyV21 => "--diagnostic-kir-v21",
            Self::LdsExchangeV22 => "--diagnostic-kir-v22",
        }
    }
    pub(super) const fn configuration_domain(self) -> &'static [u8] {
        match self {
            Self::EntryV20 => b"fe2o3-debug-physical-entry-v20-cpu-config-v1\0",
            Self::GlobalCopyV21 => b"fe2o3-debug-physical-global-copy-v21-cpu-config-v1\0",
            Self::LdsExchangeV22 => b"fe2o3-debug-physical-lds-exchange-v22-cpu-config-v1\0",
        }
    }
    pub(super) const fn page_domain(self) -> &'static [u8] {
        match self {
            Self::EntryV20 => b"fe2o3-debug-physical-v20-ssa-page-v1\0",
            Self::GlobalCopyV21 => b"fe2o3-debug-physical-v21-ssa-page-v1\0",
            Self::LdsExchangeV22 => b"fe2o3-debug-physical-v22-ssa-page-v1\0",
        }
    }
    pub(super) const fn record_limit(self) -> usize {
        match self {
            Self::EntryV20 | Self::GlobalCopyV21 => 8192,
            Self::LdsExchangeV22 => 16384,
        }
    }
    pub(super) const fn logical_wave_lane(self, local_x: u32) -> (u32, u16) {
        match self {
            // Preserve the established exact V20/V21 projection.
            Self::EntryV20 | Self::GlobalCopyV21 => (0, local_x as u16),
            Self::LdsExchangeV22 => (local_x / 64, (local_x % 64) as u16),
        }
    }
    pub(super) const fn unavailable_detail(self) -> &'static str {
        match self {
            Self::EntryV20 => "diagnostic physical-entry V20 exposes bounded CPU observations only",
            Self::LdsExchangeV22 => {
                "diagnostic physical-LDS-exchange V22 exposes bounded CPU observations only"
            }
            Self::GlobalCopyV21 => {
                "diagnostic physical-global-copy V21 exposes bounded CPU observations only"
            }
        }
    }
}
