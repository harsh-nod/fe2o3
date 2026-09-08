//! Axiom-free Verus generation for canonical finite semantic proof plans.

use std::{error::Error, fmt, fmt::Write as _};

use fe2o3_functional_proof::{
    BoundedSemanticRefinementPlanV1, FunctionalFoldOperatorV1, FunctionalNumericalProofV1,
    FunctionalScalarExpressionV1, FunctionalScalarVariableV1, FunctionalScheduleProofV1,
};

use crate::CanonicalGeneratedVerusProofInputV3;

/// Produces one bounded proof program. Compilation identities are emitted as
/// comments and included in the generated-source identity, but no identity is
/// used as a logical premise.
pub(crate) fn generate_bounded_semantic_refinement_verus_v1(
    plan: &BoundedSemanticRefinementPlanV1,
) -> Result<CanonicalGeneratedVerusProofInputV3, BoundedSemanticVerusGenerationErrorV1> {
    let mut source = String::new();
    let context = plan.context();
    writeln!(
        source,
        "// fe2o3 bounded semantic plan: {}",
        hex(plan.canonical_sha256())
    )?;
    for (label, identity) in [
        ("safe-reference-source", context.safe_reference_source()),
        ("safe-reference-mir", context.safe_reference_mir()),
        ("final-ranked-kir", context.final_ranked_kir()),
        ("analysis-epoch", context.analysis_epoch()),
        ("launch-contract", context.launch_contract()),
        ("target-contract", context.target_contract()),
        ("numerical-contract", context.numerical_contract()),
    ] {
        writeln!(source, "// {label}: {}", hex(identity))?;
    }
    source.push_str("use vstd::prelude::*;\n\nverus! {\n");
    source.push_str(VERUS_COMMON_V1);
    for (index, output) in plan.outputs().iter().enumerate() {
        render_output(&mut source, index, output)?;
    }
    source.push_str("}\n\nfn main() {}\n");
    CanonicalGeneratedVerusProofInputV3::new(source.into_bytes())
        .map_err(|error| BoundedSemanticVerusGenerationErrorV1::Source(error.to_string()))
}

fn render_output(
    source: &mut String,
    index: usize,
    output: &fe2o3_functional_proof::FunctionalOutputProofV1,
) -> Result<(), BoundedSemanticVerusGenerationErrorV1> {
    match output.schedule() {
        FunctionalScheduleProofV1::Pointwise { actual, reference } => {
            render_expression_theorem(source, index, "pointwise", output, actual, reference)?;
        }
        FunctionalScheduleProofV1::Permutation {
            extent,
            forward,
            inverse,
            actual,
            reference,
        } => {
            writeln!(
                source,
                "    proof fn fe2o3_output_{index}_permutation_v1() {{"
            )?;
            for point in 0..*extent {
                let forward = render_expression(forward, ExpressionRenderingV1::Step(point))?;
                let inverse =
                    render_expression(inverse, ExpressionRenderingV1::NestedStep(forward.clone()))?;
                writeln!(
                    source,
                    "        assert(0 <= ({forward}) && ({forward}) < {extent});"
                )?;
                writeln!(source, "        assert(({inverse}) == {point});")?;
            }
            source.push_str("    }\n\n");
            render_expression_theorem(
                source,
                index,
                "permutation_value",
                output,
                actual,
                reference,
            )?;
        }
        FunctionalScheduleProofV1::Fold {
            extent,
            identity,
            operator,
            gpu_order,
            reference_order,
        } => {
            writeln!(
                source,
                "    proof fn fe2o3_output_{index}_fold_v1(values: Seq<int>)"
            )?;
            writeln!(source, "        requires values.len() == {extent},")?;
            source.push_str("    {\n");
            let gpu = render_fold(*identity, *operator, gpu_order);
            let reference = render_fold(*identity, *operator, reference_order);
            writeln!(source, "        let gpu: int = {gpu};")?;
            writeln!(source, "        let reference: int = {reference};")?;
            source.push_str("        assert(gpu == reference) by (nonlinear_arith);\n");
            source.push_str("    }\n\n");
        }
        FunctionalScheduleProofV1::BoundedRecurrence {
            maximum_steps,
            state_arity,
            initial_actual,
            initial_reference,
            transition_actual,
            transition_reference,
            outputs_actual,
            outputs_reference,
        } => render_recurrence(
            source,
            index,
            output,
            *maximum_steps,
            *state_arity,
            initial_actual,
            initial_reference,
            transition_actual,
            transition_reference,
            outputs_actual,
            outputs_reference,
        )?,
        FunctionalScheduleProofV1::TensorComponents { actual, reference } => {
            for (component, (actual, reference)) in actual.iter().zip(reference).enumerate() {
                render_expression_theorem(
                    source,
                    index,
                    &format!("tensor_component_{component}"),
                    output,
                    actual,
                    reference,
                )?;
            }
            writeln!(
                source,
                "    proof fn fe2o3_output_{index}_tensor_product_v1() {{"
            )?;
            for component in 0..actual.len() {
                write!(
                    source,
                    "        fe2o3_output_{index}_tensor_component_{component}_v1("
                )?;
                write_scalar_arguments(source, output.input_arity(), "0")?;
                source.push_str(");\n");
            }
            source.push_str("    }\n\n");
        }
    }
    Ok(())
}

fn render_expression_theorem(
    source: &mut String,
    output_index: usize,
    role: &str,
    output: &fe2o3_functional_proof::FunctionalOutputProofV1,
    actual: &FunctionalScalarExpressionV1,
    reference: &FunctionalScalarExpressionV1,
) -> Result<(), BoundedSemanticVerusGenerationErrorV1> {
    write!(
        source,
        "    proof fn fe2o3_output_{output_index}_{role}_v1("
    )?;
    write_scalar_parameters(source, output.input_arity())?;
    source.push_str(")\n");
    render_numerical_requires(source, output.numerical(), output.input_arity(), false)?;
    source.push_str("    {\n");
    let rendering = ExpressionRenderingV1::ScalarInputs;
    let actual = render_expression(actual, rendering.clone())?;
    let reference = render_expression(reference, rendering)?;
    writeln!(source, "        let actual: int = {actual};")?;
    writeln!(source, "        let reference: int = {reference};")?;
    render_numerical_assertion(source, output.numerical())?;
    source.push_str("    }\n\n");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn render_recurrence(
    source: &mut String,
    index: usize,
    output: &fe2o3_functional_proof::FunctionalOutputProofV1,
    maximum_steps: u16,
    state_arity: u16,
    initial_actual: &[FunctionalScalarExpressionV1],
    initial_reference: &[FunctionalScalarExpressionV1],
    transition_actual: &[FunctionalScalarExpressionV1],
    transition_reference: &[FunctionalScalarExpressionV1],
    outputs_actual: &[FunctionalScalarExpressionV1],
    outputs_reference: &[FunctionalScalarExpressionV1],
) -> Result<(), BoundedSemanticVerusGenerationErrorV1> {
    for (side, initial, transition) in [
        ("actual", initial_actual, transition_actual),
        ("reference", initial_reference, transition_reference),
    ] {
        write!(
            source,
            "    pub open spec fn fe2o3_output_{index}_{side}_state_v1(inputs: Seq<Seq<int>>, n: nat) -> Seq<int>\n        decreases n,\n    {{\n"
        )?;
        source.push_str("        if n == 0 {\n            seq![");
        write_expression_list(source, initial, ExpressionRenderingV1::InitialInputs)?;
        source.push_str("]\n        } else {\n");
        writeln!(
            source,
            "            let previous = fe2o3_output_{index}_{side}_state_v1(inputs, (n - 1) as nat);"
        )?;
        source.push_str("            seq![");
        write_expression_list(
            source,
            transition,
            ExpressionRenderingV1::Recurrence {
                input_collection: "inputs",
                state_collection: "previous",
                step: "(n - 1) as int",
            },
        )?;
        source.push_str("]\n        }\n    }\n\n");
    }

    writeln!(
        source,
        "    proof fn fe2o3_output_{index}_recurrence_state_v1(inputs: Seq<Seq<int>>, n: nat)"
    )?;
    writeln!(
        source,
        "        requires n <= {maximum_steps}, inputs.len() == {},",
        output.input_arity()
    )?;
    writeln!(
        source,
        "            forall|slot: int| 0 <= slot < inputs.len() ==> #[trigger] inputs[slot].len() >= {maximum_steps},"
    )?;
    writeln!(
        source,
        "        ensures fe2o3_output_{index}_actual_state_v1(inputs, n) == fe2o3_output_{index}_reference_state_v1(inputs, n),"
    )?;
    source.push_str("        decreases n,\n    {\n");
    writeln!(
        source,
        "        reveal(fe2o3_output_{index}_actual_state_v1);"
    )?;
    writeln!(
        source,
        "        reveal(fe2o3_output_{index}_reference_state_v1);"
    )?;
    source.push_str("        if n > 0 {\n");
    writeln!(
        source,
        "            fe2o3_output_{index}_recurrence_state_v1(inputs, (n - 1) as nat);"
    )?;
    writeln!(
        source,
        "            let previous_actual = fe2o3_output_{index}_actual_state_v1(inputs, (n - 1) as nat);"
    )?;
    writeln!(
        source,
        "            let previous_reference = fe2o3_output_{index}_reference_state_v1(inputs, (n - 1) as nat);"
    )?;
    writeln!(
        source,
        "            assert(previous_actual == previous_reference);"
    )?;
    writeln!(
        source,
        "            let actual = fe2o3_output_{index}_actual_state_v1(inputs, n);"
    )?;
    writeln!(
        source,
        "            let reference = fe2o3_output_{index}_reference_state_v1(inputs, n);"
    )?;
    writeln!(source, "            assert(actual.len() == {state_arity});")?;
    writeln!(
        source,
        "            assert(reference.len() == {state_arity});"
    )?;
    source.push_str("            assert forall|component: int| 0 <= component < actual.len() implies actual[component] == reference[component] by {\n");
    for component in 0..state_arity {
        if component == 0 {
            writeln!(source, "                if component == {component} {{")?;
        } else {
            writeln!(
                source,
                "                }} else if component == {component} {{"
            )?;
        }
        let actual_expression = render_expression(
            &transition_actual[usize::from(component)],
            ExpressionRenderingV1::Recurrence {
                input_collection: "inputs",
                state_collection: "previous_actual",
                step: "(n - 1) as int",
            },
        )?;
        let reference_expression = render_expression(
            &transition_reference[usize::from(component)],
            ExpressionRenderingV1::Recurrence {
                input_collection: "inputs",
                state_collection: "previous_actual",
                step: "(n - 1) as int",
            },
        )?;
        write_expression_equality(source, &actual_expression, &reference_expression, 20)?;
        writeln!(
            source,
            "                    let actual_component: int = {actual_expression};"
        )?;
        writeln!(
            source,
            "                    let reference_component: int = {reference_expression};"
        )?;
        writeln!(
            source,
            "                    assert(actual[component] == actual_component);"
        )?;
        writeln!(
            source,
            "                    assert(reference[component] == reference_component);"
        )?;
        for state in 0..state_arity {
            writeln!(
                source,
                "                    assert(previous_actual[{state}] == previous_reference[{state}]);"
            )?;
        }
        source.push_str("                    assert(actual[component] == reference[component]);\n");
    }
    source.push_str("                }\n            }\n");
    source.push_str("            assert(actual =~= reference);\n        } else {\n");
    writeln!(
        source,
        "            let actual = fe2o3_output_{index}_actual_state_v1(inputs, n);"
    )?;
    writeln!(
        source,
        "            let reference = fe2o3_output_{index}_reference_state_v1(inputs, n);"
    )?;
    writeln!(source, "            assert(actual.len() == {state_arity});")?;
    source.push_str("            assert forall|component: int| 0 <= component < actual.len() implies actual[component] == reference[component] by {\n");
    for component in 0..state_arity {
        if component == 0 {
            writeln!(source, "                if component == {component} {{")?;
        } else {
            writeln!(
                source,
                "                }} else if component == {component} {{"
            )?;
        }
        let actual_expression = render_expression(
            &initial_actual[usize::from(component)],
            ExpressionRenderingV1::InitialInputs,
        )?;
        let reference_expression = render_expression(
            &initial_reference[usize::from(component)],
            ExpressionRenderingV1::InitialInputs,
        )?;
        write_expression_equality(source, &actual_expression, &reference_expression, 20)?;
        writeln!(
            source,
            "                    let actual_component: int = {actual_expression};"
        )?;
        writeln!(
            source,
            "                    let reference_component: int = {reference_expression};"
        )?;
        writeln!(
            source,
            "                    assert(actual[component] == actual_component);"
        )?;
        writeln!(
            source,
            "                    assert(reference[component] == reference_component);"
        )?;
        source.push_str("                    assert(actual[component] == reference[component]);\n");
    }
    source.push_str("                }\n            }\n            assert(actual =~= reference);\n        }\n    }\n\n");

    for (side, expressions) in [("actual", outputs_actual), ("reference", outputs_reference)] {
        writeln!(
            source,
            "    pub open spec fn fe2o3_output_{index}_{side}_values_v1(inputs: Seq<Seq<int>>, n: nat) -> Seq<int> {{"
        )?;
        writeln!(
            source,
            "        let current = fe2o3_output_{index}_{side}_state_v1(inputs, n);"
        )?;
        source.push_str("        seq![");
        write_expression_list(
            source,
            expressions,
            ExpressionRenderingV1::Recurrence {
                input_collection: "inputs",
                state_collection: "current",
                step: "if n == 0 { 0 } else { (n - 1) as int }",
            },
        )?;
        source.push_str("]\n    }\n\n");
    }

    writeln!(
        source,
        "    proof fn fe2o3_output_{index}_bounded_recurrence_v1(inputs: Seq<Seq<int>>, n: nat)"
    )?;
    writeln!(
        source,
        "        requires 0 < n <= {maximum_steps}, inputs.len() == {},",
        output.input_arity()
    )?;
    writeln!(
        source,
        "            forall|slot: int| 0 <= slot < inputs.len() ==> #[trigger] inputs[slot].len() >= {maximum_steps},"
    )?;
    render_sequence_numerical_requires(source, output.numerical(), output.input_arity())?;
    writeln!(
        source,
        "        ensures fe2o3_output_{index}_actual_values_v1(inputs, n) == fe2o3_output_{index}_reference_values_v1(inputs, n),"
    )?;
    source.push_str("    {\n");
    writeln!(
        source,
        "        fe2o3_output_{index}_recurrence_state_v1(inputs, n);"
    )?;
    writeln!(
        source,
        "        reveal(fe2o3_output_{index}_actual_values_v1);"
    )?;
    writeln!(
        source,
        "        reveal(fe2o3_output_{index}_reference_values_v1);"
    )?;
    writeln!(
        source,
        "        let actual = fe2o3_output_{index}_actual_values_v1(inputs, n);"
    )?;
    writeln!(
        source,
        "        let reference = fe2o3_output_{index}_reference_values_v1(inputs, n);"
    )?;
    writeln!(
        source,
        "        assert(actual.len() == {});",
        outputs_actual.len()
    )?;
    writeln!(
        source,
        "        assert(reference.len() == {});",
        outputs_reference.len()
    )?;
    writeln!(
        source,
        "        let current_actual = fe2o3_output_{index}_actual_state_v1(inputs, n);"
    )?;
    writeln!(
        source,
        "        let current_reference = fe2o3_output_{index}_reference_state_v1(inputs, n);"
    )?;
    writeln!(
        source,
        "        assert(current_actual == current_reference);"
    )?;
    source.push_str("        assert forall|component: int| 0 <= component < actual.len() implies actual[component] == reference[component] by {\n");
    for component in 0..outputs_actual.len() {
        if component == 0 {
            writeln!(source, "            if component == {component} {{")?;
        } else {
            writeln!(source, "            }} else if component == {component} {{")?;
        }
        let actual_expression = render_expression(
            &outputs_actual[component],
            ExpressionRenderingV1::Recurrence {
                input_collection: "inputs",
                state_collection: "current_actual",
                step: "(n - 1) as int",
            },
        )?;
        let reference_expression = render_expression(
            &outputs_reference[component],
            ExpressionRenderingV1::Recurrence {
                input_collection: "inputs",
                state_collection: "current_actual",
                step: "(n - 1) as int",
            },
        )?;
        write_expression_equality(source, &actual_expression, &reference_expression, 16)?;
        writeln!(
            source,
            "                let actual_component: int = {actual_expression};"
        )?;
        writeln!(
            source,
            "                let reference_component: int = {reference_expression};"
        )?;
        writeln!(
            source,
            "                assert(actual[component] == actual_component);"
        )?;
        writeln!(
            source,
            "                assert(reference[component] == reference_component);"
        )?;
        for state in 0..state_arity {
            writeln!(
                source,
                "                assert(current_actual[{state}] == current_reference[{state}]);"
            )?;
        }
        source.push_str("                assert(actual[component] == reference[component]);\n");
    }
    source.push_str("            }\n        }\n        assert(actual =~= reference);\n    }\n\n");
    Ok(())
}

fn write_expression_equality(
    source: &mut String,
    actual: &str,
    reference: &str,
    indentation: usize,
) -> Result<(), BoundedSemanticVerusGenerationErrorV1> {
    let spaces = " ".repeat(indentation);
    if actual == reference {
        writeln!(source, "{spaces}assert(({actual}) == ({reference}));")?;
    } else {
        writeln!(
            source,
            "{spaces}assert(({actual}) == ({reference})) by (nonlinear_arith);"
        )?;
    }
    Ok(())
}

fn render_fold(identity: i64, operator: FunctionalFoldOperatorV1, order: &[u16]) -> String {
    order
        .iter()
        .fold(identity.to_string(), |accumulator, index| {
            let value = format!("values[{index}]");
            match operator {
                FunctionalFoldOperatorV1::Add => format!("({accumulator} + {value})"),
                FunctionalFoldOperatorV1::Multiply => format!("({accumulator} * {value})"),
                FunctionalFoldOperatorV1::Minimum => {
                    format!("if {accumulator} <= {value} {{ {accumulator} }} else {{ {value} }}")
                }
                FunctionalFoldOperatorV1::Maximum => {
                    format!("if {accumulator} >= {value} {{ {accumulator} }} else {{ {value} }}")
                }
            }
        })
}

#[derive(Clone)]
enum ExpressionRenderingV1<'a> {
    ScalarInputs,
    InitialInputs,
    Step(u16),
    NestedStep(String),
    Recurrence {
        input_collection: &'a str,
        state_collection: &'a str,
        step: &'a str,
    },
}

fn render_expression(
    expression: &FunctionalScalarExpressionV1,
    rendering: ExpressionRenderingV1<'_>,
) -> Result<String, BoundedSemanticVerusGenerationErrorV1> {
    let rendered = match expression {
        FunctionalScalarExpressionV1::Variable(FunctionalScalarVariableV1::Input(slot)) => {
            match rendering {
                ExpressionRenderingV1::ScalarInputs => format!("i{slot}"),
                ExpressionRenderingV1::InitialInputs => format!("inputs[{slot}][0]"),
                ExpressionRenderingV1::Recurrence {
                    input_collection,
                    step,
                    ..
                } => format!("{input_collection}[{slot}][{step}]"),
                ExpressionRenderingV1::Step(_) | ExpressionRenderingV1::NestedStep(_) => {
                    return Err(BoundedSemanticVerusGenerationErrorV1::InvalidExpressionRole);
                }
            }
        }
        FunctionalScalarExpressionV1::Variable(FunctionalScalarVariableV1::State(slot)) => {
            match rendering {
                ExpressionRenderingV1::Recurrence {
                    state_collection, ..
                } => format!("{state_collection}[{slot}]"),
                _ => return Err(BoundedSemanticVerusGenerationErrorV1::InvalidExpressionRole),
            }
        }
        FunctionalScalarExpressionV1::Variable(FunctionalScalarVariableV1::Step) => match rendering
        {
            ExpressionRenderingV1::Step(step) => step.to_string(),
            ExpressionRenderingV1::NestedStep(step) => step,
            ExpressionRenderingV1::Recurrence { step, .. } => step.to_owned(),
            _ => return Err(BoundedSemanticVerusGenerationErrorV1::InvalidExpressionRole),
        },
        FunctionalScalarExpressionV1::Constant(value) => value.to_string(),
        FunctionalScalarExpressionV1::Add(lhs, rhs) => format!(
            "({} + {})",
            render_expression(lhs, rendering.clone())?,
            render_expression(rhs, rendering)?,
        ),
        FunctionalScalarExpressionV1::Subtract(lhs, rhs) => format!(
            "({} - {})",
            render_expression(lhs, rendering.clone())?,
            render_expression(rhs, rendering)?,
        ),
        FunctionalScalarExpressionV1::Multiply(lhs, rhs) => format!(
            "({} * {})",
            render_expression(lhs, rendering.clone())?,
            render_expression(rhs, rendering)?,
        ),
    };
    if rendered.len() > crate::MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3 {
        return Err(BoundedSemanticVerusGenerationErrorV1::SourceLimit);
    }
    Ok(rendered)
}

fn write_expression_list(
    source: &mut String,
    expressions: &[FunctionalScalarExpressionV1],
    rendering: ExpressionRenderingV1<'_>,
) -> Result<(), BoundedSemanticVerusGenerationErrorV1> {
    for (index, expression) in expressions.iter().enumerate() {
        if index != 0 {
            source.push_str(", ");
        }
        source.push_str(&render_expression(expression, rendering.clone())?);
    }
    Ok(())
}

fn write_scalar_parameters(
    source: &mut String,
    arity: u16,
) -> Result<(), BoundedSemanticVerusGenerationErrorV1> {
    for slot in 0..arity {
        if slot != 0 {
            source.push_str(", ");
        }
        write!(source, "i{slot}: int")?;
    }
    Ok(())
}

fn write_scalar_arguments(
    source: &mut String,
    arity: u16,
    value: &str,
) -> Result<(), BoundedSemanticVerusGenerationErrorV1> {
    for slot in 0..arity {
        if slot != 0 {
            source.push_str(", ");
        }
        source.push_str(value);
    }
    Ok(())
}

fn render_numerical_requires(
    source: &mut String,
    numerical: &FunctionalNumericalProofV1,
    input_arity: u16,
    sequence: bool,
) -> Result<(), BoundedSemanticVerusGenerationErrorV1> {
    if let FunctionalNumericalProofV1::FiniteError { input_bounds, .. } = numerical {
        if sequence {
            return Ok(());
        }
        source.push_str("        requires\n");
        for (slot, (minimum, maximum)) in input_bounds.iter().enumerate() {
            writeln!(source, "            {minimum} <= i{slot} <= {maximum},")?;
        }
        debug_assert_eq!(input_bounds.len(), usize::from(input_arity));
    }
    Ok(())
}

fn render_sequence_numerical_requires(
    source: &mut String,
    numerical: &FunctionalNumericalProofV1,
    input_arity: u16,
) -> Result<(), BoundedSemanticVerusGenerationErrorV1> {
    if let FunctionalNumericalProofV1::FiniteError { input_bounds, .. } = numerical {
        for (slot, (minimum, maximum)) in input_bounds.iter().enumerate() {
            writeln!(
                source,
                "            forall|point: int| 0 <= point < inputs[{slot}].len() ==> {minimum} <= #[trigger] inputs[{slot}][point] <= {maximum},"
            )?;
        }
        debug_assert_eq!(input_bounds.len(), usize::from(input_arity));
    }
    Ok(())
}

fn render_numerical_assertion(
    source: &mut String,
    numerical: &FunctionalNumericalProofV1,
) -> Result<(), BoundedSemanticVerusGenerationErrorV1> {
    match numerical {
        FunctionalNumericalProofV1::ExactInteger => {
            source.push_str("        assert(actual == reference) by (nonlinear_arith);\n");
        }
        FunctionalNumericalProofV1::FiniteError {
            absolute_error_units,
            relative_numerator,
            relative_denominator,
            ..
        } => {
            writeln!(
                source,
                "        assert(fe2o3_abs_v1(actual - reference) * {relative_denominator} <= {absolute_error_units} * {relative_denominator} + {relative_numerator} * fe2o3_abs_v1(reference)) by (nonlinear_arith);"
            )?;
        }
    }
    Ok(())
}

fn hex(identity: fe2o3_proof_contracts::DigestV1) -> String {
    identity
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

const VERUS_COMMON_V1: &str = r#"
    pub open spec fn fe2o3_abs_v1(value: int) -> int {
        if value < 0 { -value } else { value }
    }

"#;

#[derive(Debug, Eq, PartialEq)]
pub enum BoundedSemanticVerusGenerationErrorV1 {
    InvalidExpressionRole,
    SourceLimit,
    Formatting,
    Source(String),
}

impl fmt::Display for BoundedSemanticVerusGenerationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "bounded semantic Verus generation failed: {self:?}"
        )
    }
}

impl Error for BoundedSemanticVerusGenerationErrorV1 {}

impl From<fmt::Error> for BoundedSemanticVerusGenerationErrorV1 {
    fn from(_: fmt::Error) -> Self {
        Self::Formatting
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, process::Command};

    use fe2o3_functional_proof::{
        BoundedSemanticRefinementPlanV1, FunctionalNumericalProofV1, FunctionalOutputProofV1,
        FunctionalScalarExpressionV1 as Expression, FunctionalScheduleProofV1,
        FunctionalSemanticContextV1,
    };
    use fe2o3_proof_contracts::DigestV1;

    use super::*;

    fn digest(tag: u8) -> DigestV1 {
        DigestV1::from_untrusted_bytes([tag; 32])
    }

    fn context() -> FunctionalSemanticContextV1 {
        FunctionalSemanticContextV1::new(
            digest(1),
            digest(2),
            digest(3),
            digest(4),
            digest(5),
            digest(6),
            digest(7),
        )
        .unwrap()
    }

    fn kda_plan(wrong_sign: bool) -> BoundedSemanticRefinementPlanV1 {
        let state = Expression::state(0);
        let key = Expression::input(0);
        let value = Expression::input(1);
        let beta = Expression::input(2);
        let query = Expression::input(3);
        let reference = Expression::sum(
            state.clone(),
            Expression::product(
                beta.clone(),
                Expression::product(
                    Expression::difference(
                        value.clone(),
                        Expression::product(state.clone(), key.clone()),
                    ),
                    key.clone(),
                ),
            ),
        );
        let correction = Expression::product(
            Expression::product(beta.clone(), state.clone()),
            Expression::product(key.clone(), key.clone()),
        );
        let expanded = Expression::sum(
            state.clone(),
            Expression::product(Expression::product(beta, value), key),
        );
        let actual = if wrong_sign {
            Expression::sum(expanded, correction)
        } else {
            Expression::difference(expanded, correction)
        };
        let actual_outputs = vec![
            Expression::state(0),
            Expression::product(query.clone(), Expression::state(0)),
        ];
        let reference_outputs = vec![
            Expression::state(0),
            Expression::product(query, Expression::state(0)),
        ];
        let output = FunctionalOutputProofV1::new(
            digest(8),
            digest(9),
            Some(digest(10)),
            Some(digest(11)),
            4,
            FunctionalScheduleProofV1::BoundedRecurrence {
                maximum_steps: 4,
                state_arity: 1,
                initial_actual: vec![Expression::constant(0)].into(),
                initial_reference: vec![Expression::constant(0)].into(),
                transition_actual: vec![actual].into(),
                transition_reference: vec![reference].into(),
                outputs_actual: actual_outputs.into(),
                outputs_reference: reference_outputs.into(),
            },
            FunctionalNumericalProofV1::ExactInteger,
        )
        .unwrap();
        BoundedSemanticRefinementPlanV1::from_complete_output_product(
            context(),
            &[digest(8)],
            vec![output],
        )
        .unwrap()
    }

    #[test]
    fn kda_delta_plan_generates_bounded_multi_output_recurrence_without_identity_premises() {
        let source = generate_bounded_semantic_refinement_verus_v1(&kda_plan(false)).unwrap();
        let source = std::str::from_utf8(source.source()).unwrap();
        assert!(source.contains("bounded_recurrence_v1"));
        assert!(source.contains("actual.len() == 2"));
        assert!(source.contains("nonlinear_arith"));
        assert!(!source.contains("assume("));
        assert!(!source.contains("admit("));
        assert!(!source.contains("external_body"));
        assert!(!source.contains("kernel_name"));
    }

    #[test]
    fn generated_recurrence_binds_the_exact_transition() {
        let positive = generate_bounded_semantic_refinement_verus_v1(&kda_plan(false)).unwrap();
        let wrong = generate_bounded_semantic_refinement_verus_v1(&kda_plan(true)).unwrap();
        assert_ne!(positive.identity(), wrong.identity());
        assert_ne!(positive.source(), wrong.source());
    }

    #[test]
    fn pinned_rust_verify_accepts_kda_delta_and_rejects_wrong_sign() {
        let rust_verify = std::env::var_os("FE2O3_PINNED_RUST_VERIFY").unwrap_or_else(|| {
            "/home/harsh/.cache/fe2o3-verus-0.2026.08.02/verus-x86-linux/verus".into()
        });
        assert!(
            std::path::Path::new(&rust_verify).is_file(),
            "the reviewed pinned rust_verify executable is mandatory: {}",
            std::path::Path::new(&rust_verify).display(),
        );
        let directory =
            std::env::temp_dir().join(format!("fe2o3-kda-verus-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        for (name, plan, expected_success) in [
            ("correct", kda_plan(false), true),
            ("wrong-sign", kda_plan(true), false),
        ] {
            let source = generate_bounded_semantic_refinement_verus_v1(&plan).unwrap();
            let path = directory.join(format!("{name}.rs"));
            fs::write(&path, source.source()).unwrap();
            let output = Command::new(&rust_verify).arg(&path).output().unwrap();
            assert_eq!(
                output.status.success(),
                expected_success,
                "{name} rust_verify result differed; stdout={} stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
        }
        fs::remove_dir_all(directory).unwrap();
    }
}
