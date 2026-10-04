//! Test-only refusal transport; a discovered refusal is never qualification.
use super::*;

const CHILD_DISCOVERY_REPORT: &str = "FE2O3_TEST_MASKED_SHIFT_DISCOVERY_REPORT_V1";

#[derive(Debug, Deserialize, Serialize)]
struct RefusalReceipt {
    version: u8,
    batch: Batch,
    arguments_digest: [u8; 32],
    response_digest: [u8; 32],
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn arguments_digest(args: &[String]) -> [u8; 32] {
    digest(&serde_json::to_vec(args).expect("serialize exact child invocation"))
}

pub(super) fn ordinary_refusal(error: &SourceFailure) -> bool {
    matches!(
        error.stage,
        SourceStage::SourceCollection
            | SourceStage::RankedChecks
            | SourceStage::Policy4
            | SourceStage::NativeSourceProof
            | SourceStage::NativeHandoff
            | SourceStage::Simulation
            | SourceStage::Observation
    )
}

pub(in super::super) fn record_child_refusal(
    args: &[String],
    result: &Result<Observation, SourceFailure>,
) {
    let Some(path) = env::var_os(CHILD_DISCOVERY_REPORT) else {
        return;
    };
    let Err(error) = result else {
        return;
    };
    if !ordinary_refusal(error) {
        return;
    }
    let Some(simulation::Case::MaskedShift(batch)) =
        simulation::requested().expect("discovery case")
    else {
        panic!("masked-shift discovery receipt requires exact requested batch");
    };
    let receipt = RefusalReceipt {
        version: 1,
        batch,
        arguments_digest: arguments_digest(args),
        response_digest: digest(&serde_json::to_vec(result).expect("serialize exact response")),
    };
    assert!(
        !Path::new(&path).exists(),
        "discovery receipt must be fresh"
    );
    std::fs::write(path, serde_json::to_vec(&receipt).unwrap()).expect("write discovery receipt");
    // The caller's next statement is its existing intentional result assertion.
}

fn validate_completion(
    batch: Batch,
    args: &[String],
    exit: Option<i32>,
    response: &[u8],
    receipt: Option<&[u8]>,
) -> Result<Result<Observation, SourceFailure>, String> {
    let result: Result<Observation, SourceFailure> = serde_json::from_slice(response)
        .map_err(|error| format!("invalid current-case response: {error}"))?;
    match &result {
        Ok(observed)
            if exit == Some(0)
                && receipt.is_none()
                && observed
                    .masked_shift
                    .as_ref()
                    .is_some_and(|report| report.batch == batch) =>
        {
            Ok(result)
        }
        Err(error) if exit == Some(101) && ordinary_refusal(error) => {
            let receipt: RefusalReceipt =
                serde_json::from_slice(receipt.ok_or("missing exact-case refusal receipt")?)
                    .map_err(|error| format!("invalid exact-case refusal receipt: {error}"))?;
            if receipt.version != 1
                || receipt.batch != batch
                || receipt.arguments_digest != arguments_digest(args)
                || receipt.response_digest != digest(response)
            {
                return Err(
                    "refusal receipt changed case, invocation, schema or exact response".into(),
                );
            }
            Ok(result)
        }
        _ => Err(
            "unexpected child exit, panic, setup error, or inconsistent structured response".into(),
        ),
    }
}

pub(super) fn invoke(
    config: &Config,
    command: &mut Command,
    args: &[String],
    response: &Path,
) -> (std::process::Output, Result<Observation, SourceFailure>) {
    assert!(
        config.discovery,
        "strict qualification cannot consume a refused child"
    );
    let receipt = response.with_extension("masked-discovery.json");
    assert!(
        !response.exists() && !receipt.exists(),
        "discovery case reports must be fresh"
    );
    let child = command
        .env(CHILD_DISCOVERY_REPORT, &receipt)
        .output()
        .expect("execute exact masked-shift discovery child");
    let bytes = std::fs::read(response).expect("current discovery child must write its response");
    let receipt_bytes = match std::fs::read(&receipt) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => panic!("read exact-case discovery receipt: {error}"),
    };
    let result = validate_completion(
        config.batch,
        args,
        child.status.code(),
        &bytes,
        receipt_bytes.as_deref(),
    )
    .unwrap_or_else(|error| {
        panic!(
            "{error}\n{command:?}\n{}\n{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        )
    });
    (child, result)
}

#[test]
fn discovery_transport_requires_exact_current_case_and_intentional_assert_exit() {
    let batch = Batch::all()[0];
    let args = vec!["rustc".to_owned(), "-Ctarget-cpu=gfx942".to_owned()];
    let result: Result<Observation, SourceFailure> = Err(SourceFailure::new(
        SourceStage::Policy4,
        "exact source refusal",
    ));
    let response = serde_json::to_vec(&result).unwrap();
    let receipt = RefusalReceipt {
        version: 1,
        batch,
        arguments_digest: arguments_digest(&args),
        response_digest: digest(&response),
    };
    let bytes = serde_json::to_vec(&receipt).unwrap();
    let transported = validate_completion(batch, &args, Some(101), &response, Some(&bytes))
        .unwrap()
        .unwrap_err();
    assert_eq!(transported.stage, SourceStage::Policy4);
    assert_eq!(transported.detail, "exact source refusal");
    for status in [Some(0), Some(1), Some(2), Some(102), None] {
        assert!(validate_completion(batch, &args, status, &response, Some(&bytes)).is_err());
    }
    assert!(
        validate_completion(Batch::all()[1], &args, Some(101), &response, Some(&bytes)).is_err()
    );
    assert!(
        validate_completion(
            batch,
            &["different invocation".into()],
            Some(101),
            &response,
            Some(&bytes)
        )
        .is_err()
    );
    assert!(validate_completion(batch, &args, Some(101), &response, None).is_err());
    assert!(validate_completion(batch, &args, Some(101), &response, Some(b"malformed")).is_err());
    assert!(validate_completion(batch, &args, Some(101), b"malformed", Some(&bytes)).is_err());
    assert!(validate_completion(batch, &args, Some(0), b"", None).is_err());
    let changed: Result<Observation, SourceFailure> = Err(SourceFailure::new(
        SourceStage::Policy4,
        "different exact refusal",
    ));
    assert!(
        validate_completion(
            batch,
            &args,
            Some(101),
            &serde_json::to_vec(&changed).unwrap(),
            Some(&bytes)
        )
        .is_err()
    );
    let wrong_version = RefusalReceipt {
        version: 2,
        ..receipt
    };
    assert!(
        validate_completion(
            batch,
            &args,
            Some(101),
            &response,
            Some(&serde_json::to_vec(&wrong_version).unwrap())
        )
        .is_err()
    );
}

#[test]
fn discovery_transport_never_consumes_rustc_panic_or_setup_failure() {
    let batch = Batch::all()[0];
    let args = vec!["rustc".into()];
    for stage in [
        SourceStage::Rustc,
        SourceStage::Manifest,
        SourceStage::CargoMetadata,
        SourceStage::CargoDependencies,
        SourceStage::Invocation,
        SourceStage::Policy5,
    ] {
        let result: Result<Observation, SourceFailure> =
            Err(SourceFailure::new(stage, "unexpected failure"));
        let response = serde_json::to_vec(&result).unwrap();
        let receipt = RefusalReceipt {
            version: 1,
            batch,
            arguments_digest: arguments_digest(&args),
            response_digest: digest(&response),
        };
        assert!(
            validate_completion(
                batch,
                &args,
                Some(101),
                &response,
                Some(&serde_json::to_vec(&receipt).unwrap())
            )
            .is_err()
        );
    }
}

#[test]
fn discovery_success_transport_requires_exact_batch_and_no_refusal_receipt() {
    let batch = Batch::all()[0];
    let args = vec!["rustc".into()];
    // This is only a protocol payload. The parent still independently requires
    // the real source/O/native/simulation observations before qualification.
    let mut observed = Observation {
        roots: ROOTS.into_iter().map(str::to_owned).collect(),
        source_route: None,
        transparent_result_wrappers: None,
        internal_helpers: 0,
        helper_calls: 0,
        reads: 0,
        writes: 0,
        global_reads: 0,
        global_writes: 0,
        private_reads: 0,
        private_writes: 0,
        other_reads: 0,
        other_writes: 0,
        formal_accesses: 0,
        runtime_domains: None,
        simulation: None,
        constant_shift: None,
        masked_shift: Some(ShiftObservation {
            batch,
            source_digest: [1; 32],
            original_digest: [2; 32],
            output_digest: [3; 32],
            roots: vec![],
        }),
        policy: 4,
        output_digest: [3; 32],
        llvm_bytes: 1,
        descriptor_roots: 2,
        missing_proof_refused: false,
    };
    let encode =
        |observed: &Observation| serde_json::to_vec(&Ok::<_, SourceFailure>(observed)).unwrap();
    let response = encode(&observed);
    assert!(
        validate_completion(batch, &args, Some(0), &response, None)
            .unwrap()
            .is_ok()
    );
    for status in [None, Some(1), Some(101), Some(137)] {
        assert!(validate_completion(batch, &args, status, &response, None).is_err());
    }
    assert!(
        validate_completion(
            batch,
            &args,
            Some(0),
            &response,
            Some(b"unexpected receipt")
        )
        .is_err()
    );
    assert!(validate_completion(Batch::all()[1], &args, Some(0), &response, None).is_err());
    observed.masked_shift.as_mut().unwrap().batch = Batch::all()[1];
    assert!(validate_completion(batch, &args, Some(0), &encode(&observed), None).is_err());
    observed.masked_shift = None;
    assert!(validate_completion(batch, &args, Some(0), &encode(&observed), None).is_err());
    assert!(validate_completion(batch, &args, Some(0), br#"{"Ok":{}}"#, None).is_err());
}
