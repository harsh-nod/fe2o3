use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use fe2o3_pliron::{
    AuthorityEffect, ContextBuildError, DiagnosticCode, DialectRegistration, PLIRON_REVISION,
    PassPipeline, PassPlanError, PlironSession, RegistrationHookError, ShellLimits, StageStatus,
};
use pliron::{
    builtin::ops::ModuleOp,
    context::Context,
    dialect::DialectName,
    irbuild::IRStatus,
    op::Op,
    operation::Operation,
    pass::{AnalysisManager, Pass, PassResult, Passes},
    result::Result as PlironResult,
};

fn empty_registration(
    _context: &mut Context,
    _name: &DialectName,
) -> Result<(), RegistrationHookError> {
    Ok(())
}

#[derive(Clone)]
struct RecordingPass {
    name: &'static str,
    log: Arc<Mutex<Vec<&'static str>>>,
}

impl Pass for RecordingPass {
    fn name(&self) -> &str {
        self.name
    }

    fn run(
        &mut self,
        _operation: pliron::context::Ptr<Operation>,
        _context: &mut Context,
        _analyses: &mut AnalysisManager,
    ) -> PlironResult<PassResult> {
        self.log
            .lock()
            .expect("recording log poisoned")
            .push(self.name);
        let mut result = PassResult::default();
        result.ir_changed = IRStatus::Unchanged;
        Ok(result)
    }
}

fn session_with_module(limits: ShellLimits) -> (PlironSession, pliron::context::Ptr<Operation>) {
    let registrations = [
        DialectRegistration::new("kernel", empty_registration).expect("valid dialect"),
        DialectRegistration::new("gpu", empty_registration).expect("valid dialect"),
    ];
    let mut session = PlironSession::new(limits, registrations).expect("fresh context");
    let root = session
        .with_context_mut(|context| {
            ModuleOp::new(context, "root".try_into().expect("valid module name")).get_operation()
        })
        .expect("healthy session");
    (session, root)
}

#[test]
fn fresh_contexts_and_passes_have_deterministic_metadata() {
    let limits = ShellLimits::default();
    let (mut first, first_root) = session_with_module(limits);
    let (mut second, second_root) = session_with_module(limits);
    assert_eq!(first.manifest(), second.manifest());
    assert_eq!(first.manifest().pliron_revision(), PLIRON_REVISION);
    assert_eq!(
        first.manifest().registration_order(),
        &["kernel".to_owned(), "gpu".to_owned()]
    );

    let first_log = Arc::new(Mutex::new(Vec::new()));
    let second_log = Arc::new(Mutex::new(Vec::new()));
    let mut first_pipeline = PassPipeline::new(limits);
    let mut second_pipeline = PassPipeline::new(limits);
    for (pipeline, log) in [
        (&mut first_pipeline, Arc::clone(&first_log)),
        (&mut second_pipeline, Arc::clone(&second_log)),
    ] {
        pipeline
            .add_pass(RecordingPass {
                name: "admit",
                log: Arc::clone(&log),
            })
            .expect("first pass");
        pipeline
            .add_pass(RecordingPass {
                name: "normalize",
                log,
            })
            .expect("second pass");
    }

    let first_report = first_pipeline
        .run(&mut first, first_root)
        .expect("first pipeline");
    let second_report = second_pipeline
        .run(&mut second, second_root)
        .expect("second pipeline");
    assert_eq!(first_report, second_report);
    assert_eq!(
        *first_log.lock().expect("first log"),
        vec!["admit", "normalize"]
    );
    assert_eq!(
        *second_log.lock().expect("second log"),
        vec!["admit", "normalize"]
    );
    assert!(first_report.receipts().iter().all(|receipt| {
        receipt.verify_before() == StageStatus::Passed
            && receipt.pass_status() == StageStatus::Passed
            && receipt.verify_after() == StageStatus::Passed
            && receipt.authority_effect() == AuthorityEffect::None
    }));
}

static REGISTRATION_CALLS: AtomicUsize = AtomicUsize::new(0);

fn counted_registration(
    _context: &mut Context,
    _name: &DialectName,
) -> Result<(), RegistrationHookError> {
    REGISTRATION_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

#[test]
fn duplicate_registration_fails_before_any_hook_runs() {
    REGISTRATION_CALLS.store(0, Ordering::SeqCst);
    let duplicate = DialectRegistration::new("kernel", counted_registration).expect("valid");
    let result = PlironSession::new(ShellLimits::default(), [duplicate.clone(), duplicate]);
    assert!(matches!(
        result,
        Err(ContextBuildError::DuplicateDialect(name)) if name == "kernel"
    ));
    assert_eq!(REGISTRATION_CALLS.load(Ordering::SeqCst), 0);
}

fn oversized_failure(
    _context: &mut Context,
    _name: &DialectName,
) -> Result<(), RegistrationHookError> {
    Err(RegistrationHookError::new(
        "failure:".to_owned() + &"x".repeat(4_096),
    ))
}

#[test]
fn registration_diagnostics_are_utf8_safe_and_bounded() {
    let limits = ShellLimits::new(4, 4, 17).expect("valid limits");
    let registration =
        DialectRegistration::new("kernel", oversized_failure).expect("valid dialect");
    let error = match PlironSession::new(limits, [registration]) {
        Err(ContextBuildError::RegistrationFailed(diagnostic)) => diagnostic,
        _ => panic!("registration failure expected"),
    };
    assert_eq!(error.code(), DiagnosticCode::DialectHookFailed);
    assert_eq!(error.stage(), Some("kernel"));
    assert!(error.was_truncated());
    assert!(error.message().len() <= limits.max_diagnostic_bytes());
    assert!(error.message().is_char_boundary(error.message().len()));
}

struct CorruptModulePass;

impl Pass for CorruptModulePass {
    fn name(&self) -> &str {
        "corrupt_module"
    }

    fn run(
        &mut self,
        operation: pliron::context::Ptr<Operation>,
        context: &mut Context,
        _analyses: &mut AnalysisManager,
    ) -> PlironResult<PassResult> {
        Operation::erase_region(operation, context, 0);
        let mut result = PassResult::default();
        result.ir_changed = IRStatus::Changed;
        Ok(result)
    }
}

#[test]
fn corruption_from_a_pass_fails_post_verification() {
    let limits = ShellLimits::default();
    let (mut session, root) = session_with_module(limits);
    let mut pipeline = PassPipeline::new(limits);
    pipeline
        .add_pass(CorruptModulePass)
        .expect("valid corrupting pass");

    let failure = pipeline
        .run(&mut session, root)
        .expect_err("post-verification must reject corruption");
    let failed = failure.failed().expect("failed stage receipt");
    assert_eq!(failed.verify_before(), StageStatus::Passed);
    assert_eq!(failed.pass_status(), StageStatus::Passed);
    assert_eq!(failed.verify_after(), StageStatus::Failed);
    assert_eq!(failed.ir_changed(), Some(true));
    assert_eq!(failed.authority_effect(), AuthorityEffect::None);
    assert_eq!(
        failure.diagnostic().expect("diagnostic").code(),
        DiagnosticCode::VerifyAfterFailed
    );
    assert!(session.is_poisoned());
}

#[test]
fn malformed_root_fails_pre_verification_without_running_the_pass() {
    let limits = ShellLimits::default();
    let mut session = PlironSession::new(limits, []).expect("fresh context");
    let malformed = session
        .with_context_mut(|context| {
            Operation::new(
                context,
                ModuleOp::get_concrete_op_info(),
                vec![],
                vec![],
                vec![],
                0,
            )
        })
        .expect("healthy session");
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut pipeline = PassPipeline::new(limits);
    pipeline
        .add_pass(RecordingPass {
            name: "must_not_run",
            log: Arc::clone(&log),
        })
        .expect("valid pass");

    let failure = pipeline
        .run(&mut session, malformed)
        .expect_err("verification must reject malformed root");
    assert!(failure.completed().is_empty());
    let failed = failure.failed().expect("failed stage receipt");
    assert_eq!(failed.verify_before(), StageStatus::Failed);
    assert_eq!(failed.pass_status(), StageStatus::NotRun);
    assert_eq!(failed.verify_after(), StageStatus::NotRun);
    assert_eq!(failed.authority_effect(), AuthorityEffect::None);
    assert_eq!(
        failure.diagnostic().expect("diagnostic").code(),
        DiagnosticCode::VerifyBeforeFailed
    );
    assert!(log.lock().expect("recording log").is_empty());
    assert!(session.is_poisoned());
    assert!(session.with_context_mut(|_| ()).is_err());
}

#[test]
fn pass_plan_rejects_duplicates_and_honors_the_count_bound() {
    let limits = ShellLimits::new(1, 1, 64).expect("valid limits");
    let log = Arc::new(Mutex::new(Vec::new()));
    let pass = || RecordingPass {
        name: "only",
        log: Arc::clone(&log),
    };

    let mut duplicate_pipeline = PassPipeline::new(ShellLimits::default());
    duplicate_pipeline.add_pass(pass()).expect("first pass");
    assert_eq!(
        duplicate_pipeline.add_pass(pass()),
        Err(PassPlanError::DuplicatePass("only".to_owned()))
    );

    let mut bounded_pipeline = PassPipeline::new(limits);
    bounded_pipeline.add_pass(pass()).expect("within bound");
    assert_eq!(
        bounded_pipeline.add_pass(RecordingPass {
            name: "second",
            log,
        }),
        Err(PassPlanError::TooManyPasses)
    );
}

#[test]
fn pass_plan_rejects_hidden_nested_passes() {
    let mut pipeline = PassPipeline::new(ShellLimits::default());
    assert_eq!(
        pipeline.add_pass(Passes::default()),
        Err(PassPlanError::NestedPassManagerUnsupported(
            "passes".to_owned()
        ))
    );
    assert_eq!(pipeline.pass_order().count(), 0);
}
