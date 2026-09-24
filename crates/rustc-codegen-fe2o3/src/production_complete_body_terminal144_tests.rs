use super::*;

#[test]
fn complete_body_runtime_argument_roster_is_exactly_ten() {
    assert_eq!(
        terminal_argument_count_v1(ProductionTerminalExpansionV1::Gfx942CompleteBodyE32),
        Some(10)
    );
    assert_eq!(
        terminal_argument_count_v1(ProductionTerminalExpansionV1::Gfx942OrderedProgramE32),
        Some(8)
    );
    assert_eq!(
        terminal_argument_count_v1(ProductionTerminalExpansionV1::Gfx942OrderedXorAddE32),
        Some(8)
    );
}
