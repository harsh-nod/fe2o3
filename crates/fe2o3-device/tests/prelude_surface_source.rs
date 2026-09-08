const NEUTRAL_PRELUDE_SOURCE: &str = include_str!("../src/prelude.rs");
const GFX950_PRELUDE_SOURCE: &str = include_str!("../src/gfx950/prelude.rs");

#[test]
fn neutral_prelude_excludes_target_specific_capabilities() {
    let public_surface = NEUTRAL_PRELUDE_SOURCE
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<String>();
    for target_name in ["Gfx", "GFX"] {
        assert!(
            !public_surface.contains(target_name),
            "target-neutral prelude leaked {target_name}",
        );
    }
}

#[test]
fn gfx950_prelude_directly_reexports_branded_capabilities() {
    for capability in [
        "Gfx950Subgroup",
        "GlobalGfx950MfmaMatrix",
        "GlobalGfx950Fp4MfmaAMatrix",
        "GlobalGfx950Fp4MfmaBMatrix",
        "GlobalGfx950Fp8MfmaAMatrix",
        "GlobalGfx950Fp8MfmaBMatrix",
        "PolicyGfx950Matrix",
    ] {
        assert!(
            GFX950_PRELUDE_SOURCE.contains(capability),
            "gfx950 prelude omitted {capability}",
        );
    }

    assert_eq!(GFX950_PRELUDE_SOURCE.matches("pub use super::{").count(), 1);
    assert!(
        !GFX950_PRELUDE_SOURCE
            .lines()
            .any(|line| line.trim_start().starts_with("pub type "))
    );
}
