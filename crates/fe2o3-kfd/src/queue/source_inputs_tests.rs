//! Exact production module inputs for source-wiring regression tests.

const LIVE: &str = include_str!("../queue_live.rs");
const FIXED: &str = include_str!("../queue_live/fixed_dispatch.rs");

fn require_module(parent: &str, path: &str, name: &str) {
    let compact: String = parent.chars().filter(|c| !c.is_whitespace()).collect();
    let declaration = format!("#[path=\"{path}\"]mod{name};");
    assert_eq!(compact.matches(&declaration).count(), 1);
}

pub(crate) fn live_production_source_for_tests_v1() -> &'static str {
    for name in [
        "directional_sdma_window",
        "same_device_sdma_window",
        "same_device_sdma_terminal",
    ] {
        require_module(LIVE, &format!("queue_live/{name}.rs"), name);
    }
    concat!(
        include_str!("../queue_live.rs"),
        include_str!("../queue_live/directional_sdma_window.rs"),
        include_str!("../queue_live/same_device_sdma_window.rs"),
        include_str!("../queue_live/same_device_sdma_terminal.rs"),
    )
}

pub(crate) fn fixed_dispatch_production_source_for_tests_v1() -> &'static str {
    for name in [
        "persistent_attachment",
        "persistent_replay",
        "persistent_three_bind",
        "persistent_single_bind",
        "persistent_submission",
        "persistent_completion",
        "persistent_recycle",
        "persistent_detach",
        "data",
        "fixture",
        "completion",
        "readback",
    ] {
        require_module(FIXED, &format!("fixed_dispatch/{name}.rs"), name);
    }
    concat!(
        include_str!("../queue_live/fixed_dispatch.rs"),
        include_str!("../queue_live/fixed_dispatch/persistent_attachment.rs"),
        include_str!("../queue_live/fixed_dispatch/persistent_replay.rs"),
        include_str!("../queue_live/fixed_dispatch/persistent_three_bind.rs"),
        include_str!("../queue_live/fixed_dispatch/persistent_single_bind.rs"),
        include_str!("../queue_live/fixed_dispatch/persistent_submission.rs"),
        include_str!("../queue_live/fixed_dispatch/persistent_completion.rs"),
        include_str!("../queue_live/fixed_dispatch/persistent_recycle.rs"),
        include_str!("../queue_live/fixed_dispatch/persistent_detach.rs"),
        include_str!("../queue_live/fixed_dispatch/data.rs"),
        include_str!("../queue_live/fixed_dispatch/fixture.rs"),
        include_str!("../queue_live/fixed_dispatch/completion.rs"),
        include_str!("../queue_live/fixed_dispatch/readback.rs"),
    )
}

#[test]
fn production_source_inputs_are_connected_to_the_original_queue_modules() {
    assert!(live_production_source_for_tests_v1().contains("impl ComputeAqlQueueSessionV1"));
    assert!(
        fixed_dispatch_production_source_for_tests_v1().contains("impl ComputeAqlQueueSessionV1")
    );
}
