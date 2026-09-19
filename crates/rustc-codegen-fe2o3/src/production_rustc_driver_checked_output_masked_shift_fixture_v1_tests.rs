//! Structural fixture-wiring smoke checks, not Rust cfg execution or admission.
const FIXTURE: &str = include_str!("../tests/fixtures/production-extraction-device/src/lib.rs");

fn has_isolated_masked_fixture(source: &str) -> bool {
    let module = "#[cfg(feature = \"masked-shift\")]\nmod masked_shift;";
    if source.matches(module).count() != 1 {
        return false;
    }
    let Some((_, guard)) = source.split_once("#[cfg(not(any(") else {
        return false;
    };
    let Some((guard, following)) = guard.split_once(")))]") else {
        return false;
    };
    // This fixture's first negative feature guard owns the default fill root.
    // Actual source tests still check the exact two-root roster independently.
    following.trim_start().starts_with("#[kernel")
        && guard
            .lines()
            .map(str::trim)
            .filter(|line| *line == "feature = \"masked-shift\",")
            .count()
            == 1
}

#[test]
fn actual_masked_fixture_excludes_the_default_fill_root() {
    assert!(has_isolated_masked_fixture(FIXTURE));
}

#[test]
fn masked_fixture_wiring_detects_missing_exclusion_module_and_duplicate_exclusion() {
    let exclusion = "    feature = \"masked-shift\",\n";
    let module = "#[cfg(feature = \"masked-shift\")]\nmod masked_shift;";
    assert!(!has_isolated_masked_fixture(
        &FIXTURE.replacen(exclusion, "", 1)
    ));
    assert!(!has_isolated_masked_fixture(
        &FIXTURE.replacen(module, "", 1)
    ));
    assert!(!has_isolated_masked_fixture(&FIXTURE.replacen(
        exclusion,
        &format!("{exclusion}{exclusion}"),
        1
    )));
}
