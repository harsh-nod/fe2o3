use super::{Case, Family, coordinate, locked_supervisor};

#[path = "handoff_native_v3_consuming_tests.rs"]
mod submitter;

#[test]
#[ignore = "opt-in isolated root container; real V3 static readiness and publication"]
fn native_consuming_ready() {
    coordinate(Case::Ready, Family::V3);
}

#[test]
#[ignore = "opt-in isolated root container; V3 readiness without private EOF"]
fn native_consuming_missing_eof() {
    coordinate(Case::MissingEof, Family::V3);
}

#[test]
#[ignore = "opt-in isolated root container; V3 readiness with trailing data"]
fn native_consuming_trailing() {
    coordinate(Case::Trailing, Family::V3);
}

#[test]
#[ignore = "opt-in isolated root container; drop live V3 custody before readiness"]
fn native_consuming_drop_before_ready() {
    coordinate(Case::DropBeforeReady, Family::V3);
}

#[test]
#[ignore = "private locked V3 supervisor, selected only by the consuming coordinator"]
fn locked_supervisor_process_helper() {
    locked_supervisor(Family::V3, crate::launch_v3::tests::exercise_consuming);
}
