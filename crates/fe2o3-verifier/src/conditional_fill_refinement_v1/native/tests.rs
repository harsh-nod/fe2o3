//! Generator checks use a plain Module checked by the closed private recognizer;
//! they do not forge a recovered V5 owner or a native refinement execution.
use super::*;

#[test]
fn native_theorem_renders_only_actual_final_graph_and_distinct_boundary() {
    let recipe = crate::conditional_fill_program_v1::tests::plain_final_recipe();
    let final_recipe = &recipe;
    let source = generate::native_source(final_recipe, 272).unwrap();
    let text = std::str::from_utf8(source.source()).unwrap();
    for name in [
        "final_store",
        "final_byte",
        "execute_final_refining_group",
        "execute_final_refining_byte",
    ] {
        assert!(text.contains(&format!("fn {name}(")));
    }
    for forbidden in [
        "semantic_store",
        "neutral_store",
        "target_store",
        "include!",
        "external_body",
        "assume(",
        "admit(",
    ] {
        assert!(!text.contains(forbidden), "unexpected {forbidden}");
    }
    assert_ne!(BOUNDARY, super::super::BOUNDARY);
    assert_ne!(DOMAIN, super::super::DOMAIN);
    assert_ne!(
        source.source(),
        generate::source(&[recipe.clone(), recipe.clone(), recipe.clone()], 272)
            .unwrap()
            .source()
    );
    assert_ne!(
        source.identity(),
        generate::native_source(final_recipe, 16)
            .unwrap()
            .identity()
    );
    assert!(generate::native_source(final_recipe, 24).is_err());
    let mut wrong = final_recipe.clone();
    crate::conditional_fill_program_v1::recipe::tests::replace_value_with_zero(&mut wrong);
    let mutant = generate::native_source(&wrong, 272).unwrap();
    assert_ne!(mutant.identity(), source.identity());
    if let Some(directory) = std::env::var_os("FE2O3_NATIVE_FILL_REFINEMENT_CAPTURE") {
        let directory = std::path::Path::new(&directory);
        std::fs::write(directory.join("native-final.rs"), source.source()).unwrap();
        std::fs::write(
            directory.join("native-final-wrong-value.rs"),
            mutant.source(),
        )
        .unwrap();
    }
}
