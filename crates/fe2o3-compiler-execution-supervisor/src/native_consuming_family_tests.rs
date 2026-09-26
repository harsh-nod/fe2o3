use super::Case;

// Private fixture routing only. Each selected role constructs its own typed
// policy owners; these command tags do not supply production authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Family {
    V2,
    V3,
}

impl Family {
    pub(crate) fn opt_in(self) -> &'static str {
        match self {
            Self::V2 => super::OPT_IN,
            Self::V3 => "FE2O3_RUN_NATIVE_CONSUMING_SUPERVISOR_V3_TEST",
        }
    }

    pub(crate) fn image_env(self, case: Case) -> &'static str {
        match (self, case) {
            (Self::V2, Case::Ready) => "FE2O3_NATIVE_READY_FIXTURE_READY",
            (Self::V2, Case::MissingEof) => "FE2O3_NATIVE_READY_FIXTURE_NO_EOF",
            (Self::V2, Case::Trailing) => "FE2O3_NATIVE_READY_FIXTURE_TRAILING",
            (Self::V2, Case::DropBeforeReady) => "FE2O3_NATIVE_READY_FIXTURE_SILENT",
            (Self::V3, Case::Ready) => "FE2O3_NATIVE_READY_FIXTURE_V3_READY",
            (Self::V3, Case::MissingEof) => "FE2O3_NATIVE_READY_FIXTURE_V3_NO_EOF",
            (Self::V3, Case::Trailing) => "FE2O3_NATIVE_READY_FIXTURE_V3_TRAILING",
            (Self::V3, Case::DropBeforeReady) => "FE2O3_NATIVE_READY_FIXTURE_V3_SILENT",
        }
    }

    pub(crate) fn handoff_tag(self) -> &'static [u8; 4] {
        match self {
            Self::V2 => b"HOF2",
            Self::V3 => b"HOF3",
        }
    }

    pub(crate) fn case_tag(self) -> &'static [u8; 4] {
        match self {
            Self::V2 => b"NCF2",
            Self::V3 => b"NCF3",
        }
    }

    pub(crate) fn submitter_helper(self) -> &'static str {
        match self {
            Self::V2 => "handoff_v2_test_process::native_consuming_submitter_process_helper",
            Self::V3 => "native_consuming_test_process::v3::submitter::submitter_process_helper",
        }
    }

    pub(crate) fn supervisor_helper(self) -> &'static str {
        match self {
            Self::V2 => "native_consuming_test_process::locked_supervisor_process_helper",
            Self::V3 => "native_consuming_test_process::v3::locked_supervisor_process_helper",
        }
    }

    pub(crate) fn submitter_role(self) -> &'static str {
        match self {
            Self::V2 => "native-consuming-submitter",
            Self::V3 => "native-consuming-submitter-v3",
        }
    }

    pub(crate) fn supervisor_role(self) -> &'static str {
        match self {
            Self::V2 => "native-consuming-supervisor",
            Self::V3 => "native-consuming-supervisor-v3",
        }
    }
}

#[test]
fn fixture_families_select_distinct_roles_and_explicit_images() {
    let (old, new) = (Family::V2, Family::V3);
    assert_ne!(old.opt_in(), new.opt_in());
    assert_ne!(old.case_tag(), new.case_tag());
    assert_ne!(old.handoff_tag(), new.handoff_tag());
    assert_ne!(old.submitter_helper(), new.submitter_helper());
    assert_ne!(old.supervisor_helper(), new.supervisor_helper());
    assert_ne!(old.submitter_role(), new.submitter_role());
    assert_ne!(old.supervisor_role(), new.supervisor_role());
    let cases = [
        Case::Ready,
        Case::MissingEof,
        Case::Trailing,
        Case::DropBeforeReady,
    ];
    for case in cases {
        assert_eq!(Case::from_id(case.id()), case);
        assert_eq!(case.image_env(), old.image_env(case));
        assert_ne!(old.image_env(case), new.image_env(case));
        for other in cases {
            if other != case {
                assert_ne!(old.image_env(case), old.image_env(other));
                assert_ne!(new.image_env(case), new.image_env(other));
            }
        }
    }
}
