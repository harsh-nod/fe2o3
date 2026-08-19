use std::sync::atomic::{AtomicUsize, Ordering};

use fe2o3_pliron::{
    ContextBuildError, DiagnosticCode, DialectRegistration, OperationHandleError, PLIRON_REVISION,
    PassPlan, PassPlanError, PlironSession, RegistrationHookError, ShellLimits,
};
use pliron::{
    context::Context,
    dialect::DialectName,
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

struct RecordingPass {
    name: &'static str,
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
        Ok(PassResult::default())
    }
}

fn session(limits: ShellLimits) -> PlironSession {
    let registrations = [
        DialectRegistration::new("kernel", empty_registration).expect("valid dialect"),
        DialectRegistration::new("gpu", empty_registration).expect("valid dialect"),
    ];
    PlironSession::new(limits, registrations).expect("fresh context")
}

#[test]
fn fresh_contexts_and_pass_plans_have_deterministic_metadata() {
    let limits = ShellLimits::default();
    let first = session(limits);
    let second = session(limits);
    assert_eq!(first.manifest(), second.manifest());
    assert_eq!(first.manifest().pliron_revision(), PLIRON_REVISION);
    assert_eq!(
        first.manifest().registration_order(),
        &["kernel".to_owned(), "gpu".to_owned()]
    );

    let mut first_pipeline = PassPlan::new(limits);
    let mut second_pipeline = PassPlan::new(limits);
    for pipeline in [&mut first_pipeline, &mut second_pipeline] {
        pipeline
            .add_pass(RecordingPass { name: "admit" })
            .expect("first pass");
        pipeline
            .add_pass(RecordingPass { name: "normalize" })
            .expect("second pass");
    }

    assert_eq!(
        first_pipeline.pass_order().collect::<Vec<_>>(),
        ["admit", "normalize"]
    );
    assert_eq!(
        second_pipeline.pass_order().collect::<Vec<_>>(),
        ["admit", "normalize"]
    );
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

#[test]
fn pass_plan_rejects_duplicates_and_honors_the_count_bound() {
    let limits = ShellLimits::new(1, 1, 64).expect("valid limits");
    let pass = || RecordingPass { name: "only" };

    let mut duplicate_pipeline = PassPlan::new(ShellLimits::default());
    duplicate_pipeline.add_pass(pass()).expect("first pass");
    assert_eq!(
        duplicate_pipeline.add_pass(pass()),
        Err(PassPlanError::DuplicatePass("only".to_owned()))
    );

    let mut bounded_pipeline = PassPlan::new(limits);
    bounded_pipeline.add_pass(pass()).expect("within bound");
    assert_eq!(
        bounded_pipeline.add_pass(RecordingPass { name: "second" }),
        Err(PassPlanError::TooManyPasses)
    );
}

#[test]
fn pass_plan_rejects_hidden_nested_passes() {
    let mut pipeline = PassPlan::new(ShellLimits::default());
    assert_eq!(
        pipeline.add_pass(Passes::default()),
        Err(PassPlanError::NestedPassManagerUnsupported(
            "passes".to_owned()
        ))
    );
    assert_eq!(pipeline.pass_order().count(), 0);
}

struct PanickingNamePass;

impl Pass for PanickingNamePass {
    fn name(&self) -> &str {
        panic!("hostile pass name")
    }

    fn run(
        &mut self,
        _operation: pliron::context::Ptr<Operation>,
        _context: &mut Context,
        _analyses: &mut AnalysisManager,
    ) -> PlironResult<PassResult> {
        unreachable!("a plan never executes passes")
    }
}

struct PanickingManagerInspectionPass;

impl Pass for PanickingManagerInspectionPass {
    fn name(&self) -> &str {
        "hostile"
    }

    fn run(
        &mut self,
        _operation: pliron::context::Ptr<Operation>,
        _context: &mut Context,
        _analyses: &mut AnalysisManager,
    ) -> PlironResult<PassResult> {
        unreachable!("a plan never executes passes")
    }

    fn as_pass_manager(&mut self) -> Option<&mut dyn pliron::pass::PassManager> {
        panic!("hostile manager inspection")
    }
}

#[test]
fn pass_inspection_panics_are_contained_and_poison_the_plan() {
    for hostile in [
        PassPlan::add_pass(
            &mut PassPlan::new(ShellLimits::default()),
            PanickingNamePass,
        ),
        PassPlan::add_pass(
            &mut PassPlan::new(ShellLimits::default()),
            PanickingManagerInspectionPass,
        ),
    ] {
        assert_eq!(hostile, Err(PassPlanError::PassInspectionPanicked));
    }

    let mut plan = PassPlan::new(ShellLimits::default());
    assert_eq!(
        plan.add_pass(PanickingNamePass),
        Err(PassPlanError::PassInspectionPanicked)
    );
    assert!(plan.is_poisoned());
    assert_eq!(
        plan.add_pass(RecordingPass { name: "later" }),
        Err(PassPlanError::PlanPoisoned)
    );
    assert_eq!(plan.pass_order().count(), 0);
}

struct PanickingRegistrationInput;

impl Iterator for PanickingRegistrationInput {
    type Item = DialectRegistration;

    fn next(&mut self) -> Option<Self::Item> {
        panic!("hostile registration iterator")
    }
}

#[test]
fn registration_input_panics_are_contained() {
    let result = std::panic::catch_unwind(|| {
        PlironSession::new(ShellLimits::default(), PanickingRegistrationInput)
    });
    assert!(matches!(
        result.expect("registration iterator panic is contained"),
        Err(ContextBuildError::RegistrationInputPanicked)
    ));
}

struct CountedUnboundedRegistrations {
    polls: usize,
}

impl Iterator for CountedUnboundedRegistrations {
    type Item = DialectRegistration;

    fn next(&mut self) -> Option<Self::Item> {
        self.polls += 1;
        assert!(self.polls <= 3, "registration input was over-consumed");
        let name = format!("d{}", self.polls);
        Some(DialectRegistration::new(&name, empty_registration).expect("valid dialect"))
    }
}

#[test]
fn registration_collection_stops_at_the_configured_bound() {
    let limits = ShellLimits::new(2, 1, 64).expect("valid limits");
    let result = PlironSession::new(limits, CountedUnboundedRegistrations { polls: 0 });
    assert!(matches!(result, Err(ContextBuildError::TooManyDialects)));
}

#[test]
fn foreign_and_stale_operation_handles_fail_without_unwinding() {
    let limits = ShellLimits::default();
    let mut owner = session(limits);
    let mut foreign = session(limits);
    let handle = owner.create_module("owner").expect("owner module");
    foreign.create_module("foreign").expect("equal-slot module");

    assert_eq!(
        foreign.operation_result_count(&handle),
        Err(OperationHandleError::ForeignSession)
    );
    assert_eq!(owner.operation_result_count(&handle), Ok(0));
    owner.erase_operation(&handle).expect("owner erases module");
    assert_eq!(
        owner.operation_result_count(&handle),
        Err(OperationHandleError::StaleHandle)
    );
}

fn panicking_registration(
    _context: &mut Context,
    _name: &DialectName,
) -> Result<(), RegistrationHookError> {
    panic!("unbounded hostile payload")
}

#[test]
fn registration_hook_panics_are_contained_and_bounded() {
    let registration = DialectRegistration::new("hostile", panicking_registration).expect("valid");
    let result =
        std::panic::catch_unwind(|| PlironSession::new(ShellLimits::default(), [registration]));
    let error = result.expect("hook panic is contained");
    assert!(matches!(
        error,
        Err(ContextBuildError::RegistrationFailed(diagnostic))
            if diagnostic.code() == DiagnosticCode::DialectHookFailed
                && !diagnostic.message().contains("hostile payload")
    ));
}
