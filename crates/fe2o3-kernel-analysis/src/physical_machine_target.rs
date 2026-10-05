//! Exact analyzer profiles. These select decoding, never semantic authority.

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PhysicalMachineTargetV1 {
    Gfx942XnackMinusCov6,
    Gfx950XnackMinusCov6,
}

macro_rules! domain {
    ($name:ident, $suffix:literal) => {
        pub(crate) const fn $name(self) -> &'static [u8] {
            match self {
                Self::Gfx942XnackMinusCov6 => concat!("FE2O3/GFX942-", $suffix, "/V1\0").as_bytes(),
                Self::Gfx950XnackMinusCov6 => concat!("FE2O3/GFX950-", $suffix, "/V1\0").as_bytes(),
            }
        }
    };
}

impl PhysicalMachineTargetV1 {
    pub(crate) const fn wire_tag(self) -> u16 {
        match self {
            Self::Gfx942XnackMinusCov6 => 1,
            Self::Gfx950XnackMinusCov6 => 2,
        }
    }

    pub(crate) const fn analysis_argument(self) -> &'static str {
        match self {
            Self::Gfx942XnackMinusCov6 => "--machine-analysis-gfx942-v1",
            Self::Gfx950XnackMinusCov6 => "--machine-analysis-gfx950-v1",
        }
    }

    pub(crate) const fn identities_argument(self) -> &'static str {
        match self {
            Self::Gfx942XnackMinusCov6 => "--machine-effects-gfx942-identities-v1",
            Self::Gfx950XnackMinusCov6 => "--machine-effects-gfx950-identities-v1",
        }
    }

    domain!(request_domain, "PHYSICAL-MACHINE-EFFECT-REQUEST");
    domain!(
        request_identity_domain,
        "PHYSICAL-MACHINE-EFFECT-REQUEST-IDENTITY"
    );
    domain!(effect_domain, "PHYSICAL-MACHINE-EFFECT-EVIDENCE");
    domain!(
        effect_identity_domain,
        "PHYSICAL-MACHINE-EFFECT-EVIDENCE-IDENTITY"
    );
    domain!(trace_domain, "PHYSICAL-MACHINE-TRACE-EVIDENCE");
    domain!(
        trace_identity_domain,
        "PHYSICAL-MACHINE-TRACE-EVIDENCE-IDENTITY"
    );
    domain!(bundle_domain, "PHYSICAL-MACHINE-ANALYSIS-BUNDLE");
    domain!(
        bundle_identity_domain,
        "PHYSICAL-MACHINE-ANALYSIS-BUNDLE-IDENTITY"
    );
    domain!(
        identity_challenge_domain,
        "PHYSICAL-MACHINE-EFFECT-IDENTITY-CHALLENGE"
    );
    domain!(
        identity_response_domain,
        "PHYSICAL-MACHINE-EFFECT-IDENTITY-RESPONSE"
    );
    domain!(
        receipt_domain,
        "PHYSICAL-MACHINE-ANALYSIS-AUTHENTICATED-RECEIPT-RECORD"
    );
    domain!(
        receipt_identity_domain,
        "PHYSICAL-MACHINE-ANALYSIS-AUTHENTICATED-RECEIPT"
    );
}
