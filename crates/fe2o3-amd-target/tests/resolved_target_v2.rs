use core::{cell::Cell, convert::Infallible, fmt};
use std::{string::String, vec};

use fe2o3_amd_target::{
    AmdTargetDetectionV2, AmdTargetFeature, AmdTargetId, DecodeResolvedAmdTargetV2Error,
    DetectedTargetFeatureV2, FeatureState, MAX_AMD_TARGET_ID_BYTES_V2, MAX_DETECTED_AMD_DEVICES_V2,
    MAX_RESOLVED_AMD_TARGET_CANONICAL_BYTES_V2, ParseAmdTargetIdError, ResolveAmdTargetV2Error,
    ResolvedAmdTargetIdentityV2, ResolvedAmdTargetSourceV2, resolve_amd_target_v2,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FakeDetectionError {
    Count,
    Target(usize),
    OutOfRange(usize),
}

impl fmt::Display for FakeDetectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Fault {
    None,
    Count,
    Target(usize),
}

struct FakeDetection<'a> {
    targets: &'a [&'a str],
    fault: Fault,
}

impl<'a> FakeDetection<'a> {
    const fn new(targets: &'a [&'a str]) -> Self {
        Self {
            targets,
            fault: Fault::None,
        }
    }

    const fn with_fault(targets: &'a [&'a str], fault: Fault) -> Self {
        Self { targets, fault }
    }
}

impl AmdTargetDetectionV2 for FakeDetection<'_> {
    type Error = FakeDetectionError;

    fn device_count(&self) -> Result<usize, Self::Error> {
        if self.fault == Fault::Count {
            Err(FakeDetectionError::Count)
        } else {
            Ok(self.targets.len())
        }
    }

    fn target_id(&self, device_index: usize) -> Result<&str, Self::Error> {
        if self.fault == Fault::Target(device_index) {
            return Err(FakeDetectionError::Target(device_index));
        }
        self.targets
            .get(device_index)
            .copied()
            .ok_or(FakeDetectionError::OutOfRange(device_index))
    }
}

struct PanickingDetection;

impl AmdTargetDetectionV2 for PanickingDetection {
    type Error = Infallible;

    fn device_count(&self) -> Result<usize, Self::Error> {
        panic!("override resolution must not query device_count")
    }

    fn target_id(&self, _: usize) -> Result<&str, Self::Error> {
        panic!("override resolution must not query target_id")
    }
}

struct CountingDetection<'a> {
    target: &'a str,
    calls: Cell<usize>,
}

impl AmdTargetDetectionV2 for CountingDetection<'_> {
    type Error = Infallible;

    fn device_count(&self) -> Result<usize, Self::Error> {
        self.calls.set(self.calls.get() + 1);
        Ok(1)
    }

    fn target_id(&self, _: usize) -> Result<&str, Self::Error> {
        self.calls.set(self.calls.get() + 1);
        Ok(self.target)
    }
}

struct ReportedCount(usize);

impl AmdTargetDetectionV2 for ReportedCount {
    type Error = Infallible;

    fn device_count(&self) -> Result<usize, Self::Error> {
        Ok(self.0)
    }

    fn target_id(&self, _: usize) -> Result<&str, Self::Error> {
        panic!("an over-limit count must fail before target inspection")
    }
}

#[test]
fn existing_target_api_and_normalization_remain_compatible() {
    let target = AmdTargetId::parse("gfx942:xnack-:sramecc+").unwrap();
    assert_eq!(target.to_string(), "gfx942:sramecc+:xnack-");
    assert_eq!(target.processor(), "gfx942");
    assert_eq!(target.sramecc(), Some(FeatureState::Enabled));
    assert_eq!(target.xnack(), Some(FeatureState::Disabled));
    assert_eq!(target.capabilities().unwrap().target(), target);
    assert_eq!(target.amdhsa_elf_flags_v4_plus(), 0xe4c);
}

#[test]
fn gfx942_override_has_exact_canonical_bytes_and_digest() {
    let resolved =
        resolve_amd_target_v2(Some("gfx942:xnack-:sramecc+"), &PanickingDetection).unwrap();

    assert_eq!(resolved.architecture(), "gfx942");
    assert_eq!(resolved.sramecc(), DetectedTargetFeatureV2::Enabled);
    assert_eq!(resolved.xnack(), DetectedTargetFeatureV2::Disabled);
    assert_eq!(resolved.source(), ResolvedAmdTargetSourceV2::Override);
    assert_eq!(resolved.target_id().to_string(), "gfx942:sramecc+:xnack-");
    assert_eq!(
        resolved.encode_canonical().as_bytes(),
        b"fe2o3-amd-resolved-target-v2{architecture=gfx942;sramecc=enabled;xnack=disabled;source=override}"
    );
    assert_eq!(
        resolved.canonical_digest().to_string(),
        "3848ee3d9d067e85568ea4d70544ea9fc40634c56454f3330c58e8d85d14921a"
    );
}

#[test]
fn gfx942_detection_has_exact_canonical_bytes_and_digest() {
    let targets = ["gfx942:sramecc+:xnack-"];
    let resolved = resolve_amd_target_v2(None, &FakeDetection::new(&targets)).unwrap();

    assert_eq!(resolved.source(), ResolvedAmdTargetSourceV2::Detected);
    assert_eq!(
        resolved.encode_canonical().as_bytes(),
        b"fe2o3-amd-resolved-target-v2{architecture=gfx942;sramecc=enabled;xnack=disabled;source=detected}"
    );
    assert_eq!(
        resolved.canonical_digest().to_string(),
        "104512015d857537acbd14dd3cc88a19f03cf8a4ce5445cab3abb02acd9056f9"
    );
}

#[test]
fn override_precedes_detection_and_invalid_override_never_falls_back() {
    let detection = CountingDetection {
        target: "gfx942:sramecc+:xnack-",
        calls: Cell::new(0),
    };
    let resolved = resolve_amd_target_v2(Some("gfx942"), &detection).unwrap();
    assert_eq!(resolved.source(), ResolvedAmdTargetSourceV2::Override);
    assert_eq!(resolved.sramecc(), DetectedTargetFeatureV2::Unspecified);
    assert_eq!(resolved.xnack(), DetectedTargetFeatureV2::Unspecified);
    assert_eq!(detection.calls.get(), 0);

    assert_eq!(
        resolve_amd_target_v2(Some("gfx999"), &detection),
        Err(ResolveAmdTargetV2Error::InvalidOverride(
            ParseAmdTargetIdError::UnknownProcessor
        ))
    );
    assert_eq!(detection.calls.get(), 0);
}

#[test]
fn no_device_over_limit_and_detector_failures_are_distinct() {
    let empty: [&str; 0] = [];
    assert_eq!(
        resolve_amd_target_v2(None, &FakeDetection::new(&empty)),
        Err(ResolveAmdTargetV2Error::NoDevice)
    );
    assert_eq!(
        resolve_amd_target_v2(None, &ReportedCount(MAX_DETECTED_AMD_DEVICES_V2 + 1)),
        Err(ResolveAmdTargetV2Error::TooManyDevices {
            observed: MAX_DETECTED_AMD_DEVICES_V2 + 1,
            limit: MAX_DETECTED_AMD_DEVICES_V2,
        })
    );
    assert_eq!(
        resolve_amd_target_v2(None, &FakeDetection::with_fault(&[], Fault::Count),),
        Err(ResolveAmdTargetV2Error::Detection(
            FakeDetectionError::Count
        ))
    );

    let target = ["gfx942:sramecc+:xnack-"];
    assert_eq!(
        resolve_amd_target_v2(None, &FakeDetection::with_fault(&target, Fault::Target(0)),),
        Err(ResolveAmdTargetV2Error::Detection(
            FakeDetectionError::Target(0)
        ))
    );
}

#[test]
fn identical_multi_gpu_targets_are_accepted_at_the_bound() {
    let targets = ["gfx942:sramecc+:xnack-"; MAX_DETECTED_AMD_DEVICES_V2];
    let resolved = resolve_amd_target_v2(None, &FakeDetection::new(&targets)).unwrap();
    assert_eq!(resolved.target_id().to_string(), "gfx942:sramecc+:xnack-");

    let differently_ordered = [
        "gfx942:sramecc+:xnack-",
        "gfx942:xnack-:sramecc+",
        "gfx942:sramecc+:xnack-",
    ];
    assert!(resolve_amd_target_v2(None, &FakeDetection::new(&differently_ordered)).is_ok());
}

#[test]
fn conflicting_multi_gpu_architectures_and_features_fail_closed() {
    let architecture_conflict = ["gfx942:sramecc+:xnack-", "gfx950:sramecc+:xnack-"];
    let error = resolve_amd_target_v2(None, &FakeDetection::new(&architecture_conflict))
        .expect_err("different architectures must be ambiguous");
    assert!(matches!(
        error,
        ResolveAmdTargetV2Error::AmbiguousDevices {
            first_device_index: 0,
            conflicting_device_index: 1,
            ..
        }
    ));

    let feature_conflict = ["gfx942:sramecc+:xnack-", "gfx942:sramecc+:xnack+"];
    assert!(matches!(
        resolve_amd_target_v2(None, &FakeDetection::new(&feature_conflict)),
        Err(ResolveAmdTargetV2Error::AmbiguousDevices {
            first_device_index: 0,
            conflicting_device_index: 1,
            ..
        })
    ));
}

#[test]
fn detection_requires_exact_supported_feature_states() {
    let missing_both = ["gfx942"];
    assert_eq!(
        resolve_amd_target_v2(None, &FakeDetection::new(&missing_both)),
        Err(ResolveAmdTargetV2Error::MissingDetectedFeature {
            device_index: 0,
            feature: AmdTargetFeature::SramEcc,
        })
    );

    let missing_xnack = ["gfx942:sramecc+"];
    assert_eq!(
        resolve_amd_target_v2(None, &FakeDetection::new(&missing_xnack)),
        Err(ResolveAmdTargetV2Error::MissingDetectedFeature {
            device_index: 0,
            feature: AmdTargetFeature::Xnack,
        })
    );
}

#[test]
fn unsupported_and_malformed_features_never_reach_resolution() {
    assert_eq!(
        resolve_amd_target_v2(Some("gfx1151:xnack+"), &PanickingDetection),
        Err(ResolveAmdTargetV2Error::InvalidOverride(
            ParseAmdTargetIdError::UnsupportedFeature(AmdTargetFeature::Xnack)
        ))
    );

    let invalid = ["gfx1151:xnack+"];
    assert_eq!(
        resolve_amd_target_v2(None, &FakeDetection::new(&invalid)),
        Err(ResolveAmdTargetV2Error::InvalidDetectedTarget {
            device_index: 0,
            error: ParseAmdTargetIdError::UnsupportedFeature(AmdTargetFeature::Xnack),
        })
    );

    for target in [
        "gfx942:xnack+:xnack-",
        "gfx942:wavefrontsize64+",
        "GFX942:sramecc+:xnack-",
        " gfx942:sramecc+:xnack-",
        "gfx942:sramecc+:xnack- ",
    ] {
        assert!(matches!(
            resolve_amd_target_v2(Some(target), &PanickingDetection),
            Err(ResolveAmdTargetV2Error::InvalidOverride(_))
        ));
    }
}

#[test]
fn target_text_and_device_count_bounds_are_enforced_before_more_work() {
    let long = "x".repeat(MAX_AMD_TARGET_ID_BYTES_V2 + 1);
    assert_eq!(
        resolve_amd_target_v2(Some(&long), &PanickingDetection),
        Err(ResolveAmdTargetV2Error::OverrideTooLong)
    );

    let detected = [long.as_str()];
    assert_eq!(
        resolve_amd_target_v2(None, &FakeDetection::new(&detected)),
        Err(ResolveAmdTargetV2Error::DetectedTargetTooLong { device_index: 0 })
    );
}

#[test]
fn canonical_round_trips_cover_all_gfx942_override_feature_states() {
    let states = [None, Some(false), Some(true)];
    for sramecc in states {
        for xnack in states {
            let target = gfx942_text(sramecc, xnack);
            let resolved = resolve_amd_target_v2(Some(&target), &PanickingDetection).unwrap();
            let encoded = resolved.encode_canonical();
            assert!(encoded.len() <= MAX_RESOLVED_AMD_TARGET_CANONICAL_BYTES_V2);
            assert!(!encoded.is_empty());
            assert_eq!(
                ResolvedAmdTargetIdentityV2::decode_canonical(encoded.as_bytes()).unwrap(),
                resolved
            );
        }
    }
}

#[test]
fn canonical_round_trips_cover_all_exact_gfx942_detected_states() {
    for sramecc in [false, true] {
        for xnack in [false, true] {
            let owned = gfx942_text(Some(sramecc), Some(xnack));
            let targets = [owned.as_str()];
            let resolved = resolve_amd_target_v2(None, &FakeDetection::new(&targets)).unwrap();
            let encoded = resolved.encode_canonical();
            assert_eq!(
                ResolvedAmdTargetIdentityV2::decode_canonical(encoded.as_bytes()).unwrap(),
                resolved
            );
        }
    }
}

#[test]
fn canonical_decoder_rejects_noncanonical_or_incomplete_records() {
    let too_long = vec![b'x'; MAX_RESOLVED_AMD_TARGET_CANONICAL_BYTES_V2 + 1];
    assert_eq!(
        ResolvedAmdTargetIdentityV2::decode_canonical(&too_long),
        Err(DecodeResolvedAmdTargetV2Error::TooLong)
    );
    assert_eq!(
        ResolvedAmdTargetIdentityV2::decode_canonical(&[0xff]),
        Err(DecodeResolvedAmdTargetV2Error::NonUtf8)
    );

    for value in [
        "fe2o3-amd-resolved-target-v2{architecture=gfx942;xnack=disabled;sramecc=enabled;source=override}",
        "fe2o3-amd-resolved-target-v2{architecture=gfx942;sramecc=enabled;xnack=disabled;source=automatic}",
        "fe2o3-amd-resolved-target-v2{architecture=gfx942;sramecc=unsupported;xnack=disabled;source=override}",
        "fe2o3-amd-resolved-target-v2{architecture=gfx1151;sramecc=unsupported;xnack=enabled;source=override}",
        "fe2o3-amd-resolved-target-v2{architecture=gfx942;sramecc=unspecified;xnack=disabled;source=detected}",
        "fe2o3-amd-resolved-target-v2{architecture=gfx942:sramecc+;sramecc=enabled;xnack=disabled;source=override}",
    ] {
        assert!(
            ResolvedAmdTargetIdentityV2::decode_canonical(value.as_bytes()).is_err(),
            "accepted {value}"
        );
    }
}

#[test]
fn bounded_override_parser_campaign_is_deterministic_and_panic_free() {
    const ALPHABET: &[u8] = b"gfx942GFX-+:_ srameccxnack0123456789abcdefghijklmnopqrstuvwxyz";
    let mut state = 0x4f31_287a_d91c_b5e3_u64;
    for case in 0..100_000 {
        let mut text = String::new();
        let len = if case % 997 == 0 {
            0
        } else {
            usize::try_from(next_random(&mut state) % 96).unwrap()
        };
        for _ in 0..len {
            let index = usize::try_from(next_random(&mut state)).unwrap() % ALPHABET.len();
            text.push(char::from(ALPHABET[index]));
        }
        if case % 991 == 0 {
            text = "gfx942:xnack-:sramecc+".into();
        }

        if let Ok(identity) = resolve_amd_target_v2(Some(&text), &PanickingDetection) {
            assert!(text.len() <= MAX_AMD_TARGET_ID_BYTES_V2);
            identity.target_id().capabilities().unwrap();
            let canonical = identity.encode_canonical();
            assert_eq!(
                ResolvedAmdTargetIdentityV2::decode_canonical(canonical.as_bytes()).unwrap(),
                identity
            );
            assert_eq!(identity.canonical_digest(), identity.canonical_digest());
        }
    }
}

#[test]
fn canonical_decoder_mutation_campaign_accepts_only_round_trippable_bytes() {
    let identity =
        resolve_amd_target_v2(Some("gfx942:sramecc+:xnack-"), &PanickingDetection).unwrap();
    let golden = identity.encode_canonical().as_bytes().to_vec();
    let mut state = 0xb8a4_661d_02f9_e53c_u64;

    for case in 0..100_000 {
        let mut bytes = golden.clone();
        let mutations = usize::try_from((next_random(&mut state) % 4) + 1).unwrap();
        for _ in 0..mutations {
            let index = usize::try_from(next_random(&mut state)).unwrap() % bytes.len();
            bytes[index] = next_random(&mut state) as u8;
        }
        if case % 7 == 0 {
            let new_len = usize::try_from(next_random(&mut state)).unwrap() % (bytes.len() + 1);
            bytes.truncate(new_len);
        }
        if case % 13 == 0 && bytes.len() <= MAX_RESOLVED_AMD_TARGET_CANONICAL_BYTES_V2 {
            bytes.push(next_random(&mut state) as u8);
        }

        if let Ok(decoded) = ResolvedAmdTargetIdentityV2::decode_canonical(&bytes) {
            assert_eq!(decoded.encode_canonical().as_bytes(), bytes);
            assert_eq!(decoded.canonical_digest(), decoded.canonical_digest());
        }
    }
}

fn gfx942_text(sramecc: Option<bool>, xnack: Option<bool>) -> String {
    let mut target = String::from("gfx942");
    append_feature(&mut target, "sramecc", sramecc);
    append_feature(&mut target, "xnack", xnack);
    target
}

fn append_feature(target: &mut String, name: &str, state: Option<bool>) {
    if let Some(enabled) = state {
        target.push(':');
        target.push_str(name);
        target.push(if enabled { '+' } else { '-' });
    }
}

fn next_random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}
