use super::{CanonicalGeneratedVerusProofInputV3, ConditionalFillRefinementErrorV1 as Error};
use crate::conditional_fill_program_v1::recipe::{Effect, Expr, Id, Recipe};
use std::fmt::Write as _;

const WAVE_BODY: &str =
    include_str!("../../../fe2o3-kernel-analysis/src/gfx942_fill_wave_v1/body.rs");
const WAVE: &str = include_str!("../../../fe2o3-kernel-analysis/verus/gfx942_fill_wave_v1.rs");
const DISPATCH_BODY: &str =
    include_str!("../../../fe2o3-kernel-analysis/src/gfx942_fill_wave_v1/dispatch_body.rs");
const DISPATCH: &str =
    include_str!("../../../fe2o3-kernel-analysis/verus/gfx942_fill_dispatch_v1.rs");

pub(super) fn source(
    recipes: &[Recipe; 3],
    storage: u64,
) -> Result<CanonicalGeneratedVerusProofInputV3, Error> {
    if !matches!(storage, 16 | 272) {
        return Err(Error::Profile);
    }
    let wave = without_once(WAVE, "include!(\"../src/gfx942_fill_wave_v1/body.rs\");\n")?;
    let dispatch = without_once(DISPATCH, "include!(\"gfx942_fill_wave_v1.rs\");\n")?;
    let dispatch = without_once(
        &dispatch,
        "include!(\"../src/gfx942_fill_wave_v1/dispatch_body.rs\");\n",
    )?;
    let mut source = format!("{WAVE_BODY}\n{wave}\n{DISPATCH_BODY}\n{dispatch}\nverus! {{\n");
    writeln!(source, "spec fn artifact_dispatch_valid(input: DispatchInput) -> bool {{ dispatch_valid(input) && input.kernarg_bytes == {storage} }}").unwrap();
    for (name, recipe) in ["semantic", "neutral", "target"].into_iter().zip(recipes) {
        render(&mut source, name, recipe)?;
    }
    source.push_str(include_str!("proof.rs"));
    source.push_str("}\n");
    for forbidden in [
        "include!",
        "include_str!",
        "include_bytes!",
        "external_body",
        "assume(",
        "admit(",
        "mod ",
        "extern ",
    ] {
        if source.contains(forbidden) {
            return Err(Error::Profile);
        }
    }
    CanonicalGeneratedVerusProofInputV3::new(source.into_bytes()).map_err(Error::Source)
}

fn without_once(source: &str, exact: &str) -> Result<String, Error> {
    if source.matches(exact).count() != 1 {
        return Err(Error::Profile);
    }
    Ok(source.replacen(exact, "", 1))
}

fn render(source: &mut String, name: &str, recipe: &Recipe) -> Result<(), Error> {
    let (_, effect) = recipe.effect.ok_or(Error::Profile)?;
    writeln!(
        source,
        "spec fn {name}_store(base: u64, length: u64, global: u64) -> Option<Gfx942FillStoreV1> {{"
    )
    .unwrap();
    let id = |value: Id| format!("v{}", value.index());
    for (ordinal, node) in recipe.nodes.iter().enumerate() {
        let expression = match node.expression {
            Expr::Output => "(base, length)".to_owned(),
            Expr::ThreadIndex | Expr::GlobalX => "global".to_owned(),
            Expr::Use(a)
            | Expr::SharedBorrow(a)
            | Expr::MutableBorrow(a)
            | Expr::IndexGet(a)
            | Expr::Bitcast(a) => id(a),
            Expr::IntegerTruncate(a) | Expr::Truncate(a) => format!("{} as u32", id(a)),
            Expr::Length(a) => format!("{}.1", id(a)),
            Expr::Less(a, b) => format!("{} < {}", id(a), id(b)),
            Expr::Zero => "0u64".to_owned(),
            Expr::Select(a, b, c) => format!("if {} {{ {} }} else {{ {} }}", id(a), id(b), id(c)),
            Expr::Base(a) => format!("{}.0", id(a)),
            Expr::Offset(a, b) => format!("({} as int + 4 * {} as int) as u64", id(a), id(b)),
            Expr::Unit => "()".to_owned(),
            Expr::WriteAccepted => match effect {
                Effect::MirWrite { output, index, .. } => {
                    format!("{} < {}.1", id(index), id(output))
                }
                _ => return Err(Error::Profile),
            },
        };
        writeln!(source, "    let v{ordinal} = {expression};").unwrap();
    }
    match effect {
        Effect::MirWrite { output, index, value } => writeln!(source,
            "    if {} < {}.1 {{ Some(Gfx942FillStoreV1 {{ address: ({}.0 as int + 4 * {} as int) as u64, value: {} }}) }} else {{ None }}",
            id(index), id(output), id(output), id(index), id(value)).unwrap(),
        Effect::KirStore { pointer, predicate, value } => writeln!(source,
            "    if {} {{ Some(Gfx942FillStoreV1 {{ address: {}, value: {} }}) }} else {{ None }}",
            id(predicate), id(pointer), id(value)).unwrap(),
    }
    source.push_str("}\n");
    writeln!(
        source,
        "spec fn {name}_byte(input: DispatchInput, address: u64, before: u8) -> u8 {{
    if inside_output(input, address) {{
        let index = ((address as int - input.output_base as int) / 4) as u64;
        let store = {name}_store(input.output_base, count(input.kernarg@), index);
        if store.is_some() && covers(store, address) {{
            (store.unwrap().value >> ((8 * (address - store.unwrap().address)) as u64)) as u8
        }} else {{ before }}
    }} else {{ before }}
}}"
    )
    .unwrap();
    Ok(())
}
