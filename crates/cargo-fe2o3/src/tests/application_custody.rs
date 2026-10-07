use super::*;

#[test]
fn application_custodian_selection_is_explicit_run_only_and_preserves_arguments() {
    use crate::application_handoff::ApplicationCompilerServiceExposureV1 as Service;
    use std::os::unix::ffi::OsStringExt;
    let flag = OsString::from("--application-proof-custodian");
    let binary = OsString::from_vec(vec![0xff, b'a']);
    let arguments = vec![
        flag.clone(),
        "--bin".into(),
        binary.clone(),
        "--".into(),
        flag.clone(),
        flag.clone(),
    ];
    let (forwarded, service) = crate::parse_application_proof_route("run", &arguments).unwrap();
    assert_eq!(service, Service::CustodianRequired);
    assert_eq!(forwarded, arguments[1..]);
    assert!(crate::parse_application_proof_route("build", std::slice::from_ref(&flag)).is_err());
    assert!(crate::parse_application_proof_route("run", &[flag.clone(), flag.clone()]).is_err());
    assert_eq!(
        crate::parse_application_proof_route("run", &["--".into(), flag])
            .unwrap()
            .1,
        Service::Required
    );
    assert_eq!(
        crate::parse_application_proof_route("run", &[binary])
            .unwrap()
            .1,
        Service::Required
    );
    let native = OsString::from("--native-application-proof-custodian");
    let legacy = OsString::from("--application-proof-custodian");
    let (forwarded, selected) = crate::parse_application_proof_route(
        "run",
        &[native.clone(), "--".into(), legacy.clone(), native.clone()],
    )
    .unwrap();
    assert_eq!(selected, Service::NativeCustodianRequired);
    assert_eq!(
        forwarded,
        [OsString::from("--"), legacy.clone(), native.clone()]
    );
    for arguments in [
        vec![native.clone(), native.clone()],
        vec![native.clone(), legacy.clone()],
        vec![legacy, native.clone()],
    ] {
        assert!(crate::parse_application_proof_route("run", &arguments).is_err());
    }
    assert!(crate::parse_application_proof_route("build", &[native]).is_err());
    for (context, expected) in [
        (
            crate::application_handoff::RUNNER_CONTEXT_VERSION,
            Service::Required,
        ),
        (
            crate::application_handoff::RUNNER_CUSTODIAN_CONTEXT_VERSION,
            Service::CustodianRequired,
        ),
        (
            crate::application_handoff::RUNNER_NATIVE_CUSTODIAN_CONTEXT_VERSION,
            Service::NativeCustodianRequired,
        ),
    ] {
        let (timeouts, actual) = application_runner_policy(OsStr::new(context)).unwrap();
        assert_eq!(actual, expected);
        assert!(actual.is_required());
        assert_eq!(
            timeouts,
            crate::application_handoff::ApplicationTimeouts::PRODUCTION
        );
    }
    assert!(application_runner_policy(OsStr::new("4-extra")).is_err());
}

#[test]
fn internal_short_timeout_runner_context_selects_short_policy() {
    assert_eq!(
        application_runner_policy(OsStr::new(
            crate::application_handoff::RUNNER_SHORT_TIMEOUT_TEST_CONTEXT_VERSION
        ))
        .unwrap(),
        (
            crate::application_handoff::ApplicationTimeouts::TEST_SHORT,
            crate::application_handoff::ApplicationCompilerServiceExposureV1::TestDisabled,
        )
    );
}
