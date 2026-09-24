//! Bounded inert rendering/splicing. No parser or source authority is minted here.
use super::{CANDIDATE_CAP, Error, HELPER_CAP, OrderedCompositionTypedEditV1, Result, SOURCE_CAP};
use fe2o3_kernel_ir::{
    Gfx942OrderedProgramRegistersV1, Gfx942ProgramBinaryOpcodeV1 as Op,
    Gfx942ProgramInstructionV1 as Step, Gfx942ProgramRoleV1 as Role, Gfx942U32ProgramV1,
};
use std::fmt::Write as _;
use std::ops::Range;

pub(super) struct Coordinates {
    pub(super) insertion: usize,
    pub(super) selected: Range<usize>,
    pub(super) arguments: [Range<usize>; 3],
}

pub(super) fn helper_name(name: &str) -> Result<()> {
    const PREFIX: &str = "__fe2o3_region_";
    let Some(suffix) = name.strip_prefix(PREFIX) else {
        return Err(Error::refused(
            "publisher helper requires the fixed generated-name grammar",
        ));
    };
    if suffix.len() != 16
        || !suffix
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(Error::refused(
            "publisher helper requires sixteen lowercase hexadecimal digits",
        ));
    }
    Ok(())
}

struct BoundedText {
    value: String,
    limit: usize,
}
impl BoundedText {
    fn new(limit: usize) -> Result<Self> {
        let mut value = String::new();
        value
            .try_reserve_exact(limit)
            .map_err(|_| Error::refused("publisher text allocation refused"))?;
        if value.capacity() > limit {
            return Err(Error::refused("publisher text retained capacity exceeded"));
        }
        Ok(Self { value, limit })
    }
    fn push(&mut self, text: &str) -> Result<()> {
        self.write_str(text)
            .map_err(|_| Error::refused("publisher text output bound exceeded"))
    }
}
impl std::fmt::Write for BoundedText {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        if text.len() > self.limit.saturating_sub(self.value.len()) {
            return Err(std::fmt::Error);
        }
        self.value.push_str(text);
        Ok(())
    }
}

fn role(value: Role) -> &'static str {
    match value {
        Role::Input0 => "input0",
        Role::Input1 => "input1",
        Role::Input2 => "input2",
        Role::Scratch => "scratch",
        Role::Output => "out",
    }
}

fn render_macro(edit: OrderedCompositionTypedEditV1, inputs: [&str; 3]) -> Result<String> {
    if !inputs.iter().all(|input| identifier(input)) {
        return Err(Error::refused(
            "publisher macro inputs require bounded identifiers",
        ));
    }
    // Recheck exact typed values at the rendering boundary; no arbitrary Rust or
    // native instruction strings, includes or attributes are accepted as an edit.
    Gfx942U32ProgramV1::from_descriptors(edit.program.count(), *edit.program.descriptors())
        .map_err(|_| Error::refused("publisher typed program is invalid"))?;
    Gfx942OrderedProgramRegistersV1::new(
        edit.registers.scratch(),
        edit.registers.output(),
        edit.registers.inputs(),
    )
    .map_err(|_| Error::refused("publisher typed register roles are invalid"))?;
    let mut out = BoundedText::new(HELPER_CAP)?;
    write!(&mut out, "fe2o3_device::amdgpu_ordered_program! {{\n            gfx942_xnack_off_wave64;\n            scratch({}); out({});\n",
        edit.registers.scratch(), edit.registers.output())
        .map_err(|_| Error::refused("publisher helper rendering exceeded bound"))?;
    let [a, b, c] = edit.registers.inputs();
    write!(
        &mut out,
        "            in({a}) = {}; in({b}) = {}; in({c}) = {};\n",
        inputs[0], inputs[1], inputs[2]
    )
    .map_err(|_| Error::refused("publisher helper rendering exceeded bound"))?;
    for step in edit.program.instructions() {
        match step {
            Step::Move {
                destination,
                source,
            } => {
                writeln!(
                    &mut out,
                    "            mov({}, {});",
                    role(destination.role()),
                    role(source)
                )
            }
            Step::Binary {
                opcode,
                destination,
                left,
                right,
            } => {
                let opcode = match opcode {
                    Op::Add => "add",
                    Op::Subtract => "sub",
                    Op::And => "and",
                    Op::Or => "or",
                    Op::Xor => "xor",
                };
                writeln!(
                    &mut out,
                    "            {opcode}({}, {}, {});",
                    role(destination.role()),
                    role(left),
                    role(right)
                )
            }
        }
        .map_err(|_| Error::refused("publisher helper rendering exceeded bound"))?;
    }
    out.push("        }")?;
    Ok(out.value)
}

pub(super) fn render(name: &str, edit: OrderedCompositionTypedEditV1) -> Result<String> {
    helper_name(name)?;
    let body = render_macro(edit, ["a", "b", "c"])?;
    let mut out = BoundedText::new(HELPER_CAP)?;
    write!(
        &mut out,
        "\n    #[inline(never)]\n    fn {name}(a: u32, b: u32, c: u32) -> u32 {{\n        "
    )
    .map_err(|_| Error::refused("publisher helper rendering exceeded bound"))?;
    out.push(&body)?;
    out.push("\n    }\n")?;
    Ok(out.value)
}

/// Refuse compile-time repeat/selection captures, comments and alternate token
/// spellings in this first publisher. Compare the original typed program, not an
/// optional requested edit. Whitespace alone may vary; HIR remains the authority.
pub(super) fn require_flat_source(
    original: &str,
    coordinates: &Coordinates,
    original_program: OrderedCompositionTypedEditV1,
) -> Result<()> {
    let mut names = [""; 3];
    for (index, range) in coordinates.arguments.iter().enumerate() {
        names[index] = original
            .get(range.clone())
            .ok_or_else(|| Error::refused("publisher original parameter range differs"))?;
    }
    let expected = render_macro(original_program, names)?;
    let selected = original
        .get(coordinates.selected.clone())
        .ok_or_else(|| Error::refused("publisher original macro range differs"))?;
    if !selected
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .eq(expected.bytes().filter(|b| !b.is_ascii_whitespace()))
    {
        return Err(Error::refused(
            "publisher supports only the exact flat literal program spelling",
        ));
    }
    Ok(())
}

fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || i > 0 && b.is_ascii_digit())
}

pub(super) fn splice(
    original: &str,
    coordinates: &Coordinates,
    helper: &str,
    rendered: &str,
) -> Result<String> {
    helper_name(helper)?;
    let selection = &coordinates.selected;
    if original.len() > SOURCE_CAP
        || rendered.len() > HELPER_CAP
        || coordinates.insertion == 0
        || coordinates.insertion > selection.start
        || selection.start >= selection.end
        || selection.end > original.len()
        || original.as_bytes().get(coordinates.insertion - 1) != Some(&b'{')
        || !original.is_char_boundary(coordinates.insertion)
        || !original.is_char_boundary(selection.start)
        || !original.is_char_boundary(selection.end)
    {
        return Err(Error::refused(
            "publisher HIR insertion or selected range differs",
        ));
    }
    let selected = &original[selection.clone()];
    // First publisher deliberately supports this direct qualified spelling only.
    // Its authenticity comes from HIR+MIR, not from these bytes or this name.
    if !selected.starts_with("fe2o3_device::amdgpu_ordered_program!") || !selected.ends_with('}') {
        return Err(Error::refused(
            "publisher requires the direct qualified brace macro spelling",
        ));
    }
    let mut arguments = [""; 3];
    for (index, range) in coordinates.arguments.iter().enumerate() {
        let argument = original
            .get(range.clone())
            .filter(|name| identifier(name))
            .ok_or_else(|| Error::refused("publisher parameter source bytes differ"))?;
        arguments[index] = argument;
    }
    let mut out = BoundedText::new(CANDIDATE_CAP)?;
    out.push(&original[..coordinates.insertion])?;
    out.push(rendered)?;
    out.push(&original[coordinates.insertion..selection.start])?;
    write!(
        &mut out,
        "{helper}({}, {}, {})",
        arguments[0], arguments[1], arguments[2]
    )
    .map_err(|_| Error::refused("publisher replacement output bound exceeded"))?;
    out.push(&original[selection.end..])?;
    Ok(out.value)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_text_rejects_the_first_extra_byte_without_mutation() {
        let mut text = BoundedText::new(8).unwrap();
        text.push("12345678").unwrap();
        assert!(text.push("9").is_err());
        assert_eq!(text.value, "12345678");
    }
}
