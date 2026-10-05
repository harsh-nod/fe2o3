//! Dependency closure for explicitly designated, compiler-owned proof support.
//! Semantic laws and generated cut/trace obligations are never candidates.

use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;

const MARKER: &str = "// fe2o3_optional_support_v97: ";
const PACKETS: [&str; 2] = [
    include_str!("original_semantic_mir_source_constructor_laws_v81.vrs"),
    include_str!("original_semantic_mir_cut_frame_laws_v93.vrs"),
];
const MAX_SUPPORT: usize = 20;

#[derive(Clone, Copy)]
struct Support {
    name: &'static str,
    body: &'static str,
    start: usize,
    end: usize,
    key: u64,
    needed: bool,
    expanded: bool,
}

const EMPTY: Support = Support {
    name: "",
    body: "",
    start: 0,
    end: 0,
    key: 0,
    needed: false,
    expanded: false,
};

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right).ok_or(Resource::Arithmetic.into())
}

// This is only an index accelerator; a matching key always requires exact bytes.
fn key(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

fn identifier(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80
}

fn references(text: &str, support: &mut [Support], budget: &mut Budget<'_>) -> Result<()> {
    budget.charge_work(text.len().checked_mul(4).ok_or(Resource::Arithmetic)?)?;
    let bytes = text.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if !identifier(bytes[cursor]) {
            cursor += 1;
            continue;
        }
        let start = cursor;
        while cursor < bytes.len() && identifier(bytes[cursor]) {
            cursor += 1;
        }
        let token = &bytes[start..cursor];
        let token_key = key(token);
        budget.charge_work(support.len())?;
        for item in support.iter_mut() {
            if item.key == token_key {
                budget.charge_work(token.len().min(item.name.len()))?;
                if token == item.name.as_bytes() {
                    item.needed = true;
                }
            }
        }
    }
    Ok(())
}

fn collect(
    text: &str,
    packets: &[&'static str],
    support: &mut [Support; MAX_SUPPORT],
    budget: &mut Budget<'_>,
) -> Result<usize> {
    let mut count = 0;
    for packet in packets {
        budget.charge_work(
            add(text.len(), packet.len())?
                .checked_mul(2)
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut matches = text.match_indices(*packet);
        let Some((base, _)) = matches.next() else {
            continue;
        };
        if matches.next().is_some() {
            return Err(mismatch());
        }
        let mut remaining = *packet;
        let mut offset = 0;
        loop {
            budget.charge_work(remaining.len().checked_mul(3).ok_or(Resource::Arithmetic)?)?;
            let start = remaining.find(MARKER).ok_or_else(mismatch)?;
            offset = add(offset, start)?;
            remaining = &remaining[start..];
            let newline = remaining.find('\n').ok_or_else(mismatch)?;
            let name = &remaining[MARKER.len()..newline];
            if name == "END" {
                if remaining != "// fe2o3_optional_support_v97: END\n" {
                    return Err(mismatch());
                }
                break;
            }
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            {
                return Err(mismatch());
            }
            let end = add(
                newline + 1,
                remaining[newline + 1..].find(MARKER).ok_or_else(mismatch)?,
            )?;
            let body = &remaining[..end];
            budget.charge_work(body.len().checked_mul(8).ok_or(Resource::Arithmetic)?)?;
            let declaration = body.split_once("proof fn ").ok_or_else(mismatch)?.1;
            if declaration.split_once('(').ok_or_else(mismatch)?.0 != name
                || body.matches("proof fn ").count() != 1
                || body.contains("spec fn ")
                || body.contains("broadcast")
                || body.contains("external_body")
                || count == MAX_SUPPORT
            {
                return Err(mismatch());
            }
            budget.charge_work(
                add(count, 4)?
                    .checked_mul(name.len())
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if support[..count].iter().any(|item| item.name == name) {
                return Err(mismatch());
            }
            let start = add(base, offset)?;
            if count > 0 && support[count - 1].end > start {
                return Err(mismatch());
            }
            support[count] = Support {
                name,
                body,
                start,
                end: add(start, end)?,
                key: key(name.as_bytes()),
                ..EMPTY
            };
            count += 1;
            offset = add(offset, end)?;
            remaining = &remaining[end..];
        }
    }
    Ok(count)
}

pub(super) fn retain_referenced(out: &mut Writer<'_, '_>) -> Result<()> {
    retain_packets(out, &PACKETS)
}

fn retain_packets(out: &mut Writer<'_, '_>, packets: &[&'static str]) -> Result<()> {
    // The owning caller has revalidated source custody before this entry.
    // No new heap allocation is needed; the existing output capacity stays paid.
    out.budget.reserve_storage(
        size_of::<[Support; MAX_SUPPORT]>()
            + 4 * size_of::<Support>()
            + 64 * size_of::<usize>()
            + 24 * size_of::<&str>()
            + 8 * size_of::<Result<usize>>(),
    )?;
    let mut support = [EMPTY; MAX_SUPPORT];
    let count = collect(&out.text, packets, &mut support, out.budget)?;
    let support = &mut support[..count];
    let mut cursor = 0;
    for index in 0..count {
        let item = support[index];
        references(&out.text[cursor..item.start], support, out.budget)?;
        cursor = item.end;
    }
    references(&out.text[cursor..], support, out.budget)?;
    loop {
        out.budget.charge_work(count)?;
        let Some(index) = support
            .iter()
            .position(|item| item.needed && !item.expanded)
        else {
            break;
        };
        support[index].expanded = true;
        references(support[index].body, support, out.budget)?;
    }
    // Charge all possible shifting before changing output, so a short budget
    // cannot leave a partially pruned proof request.
    let mut shifts = 0usize;
    for item in support.iter() {
        out.budget.charge_work(1)?;
        if !item.needed {
            shifts = add(shifts, out.text.len() - item.end)?;
        }
    }
    out.budget.charge_work(shifts)?;
    for item in support.iter().rev().filter(|item| !item.needed) {
        drop(out.text.drain(item.start..item.end));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use std::fmt::Write as _;

    const PACKET: &str = concat!(
        "spec fn required_semantics() -> bool { true }\n",
        "// fe2o3_optional_support_v97: a\nproof fn a() { b(); }\n",
        "// fe2o3_optional_support_v97: b\nproof fn b() { a(); }\n",
        "// fe2o3_optional_support_v97: c\nproof fn c() { }\n",
        "// fe2o3_optional_support_v97: END\n",
    );

    fn run(text: &str, work_limit: usize, storage_limit: usize) -> (Result<String>, usize, usize) {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        let result = (|| {
            budget.reserve_storage(SOURCE_LIMIT)?;
            let mut out = Writer::new(&mut budget)?;
            out.write_str(text).map_err(|_| out.error())?;
            let before = out.text.clone();
            match retain_packets(&mut out, &[PACKET]) {
                Ok(()) => out.finish(),
                Err(error) => {
                    assert_eq!(out.text, before);
                    Err(error)
                }
            }
        })();
        (result, budget.work(), budget.peak_storage())
    }

    #[test]
    fn proof_support_closure_preserves_roots_and_transitive_cycles() {
        let text = format!("{PACKET}proof fn original_cut_all() {{ a(); }}\n");
        let result = run(&text, usize::MAX, usize::MAX).0.unwrap();
        assert!(result.contains("spec fn required_semantics() -> bool { true }"));
        assert!(result.contains("proof fn original_cut_all() { a(); }"));
        assert!(result.contains("proof fn a() { b(); }"));
        assert!(result.contains("proof fn b() { a(); }"));
        assert!(!result.contains("proof fn c()"));
        let unused = run(PACKET, usize::MAX, usize::MAX).0.unwrap();
        assert!(!unused.contains("proof fn "));
        assert!(unused.contains("spec fn required_semantics()"));
        let longer_identifier = format!("{PACKET}proof fn original_cut_all() {{ ab(); }}\n");
        assert!(
            !run(&longer_identifier, usize::MAX, usize::MAX)
                .0
                .unwrap()
                .contains("proof fn a()")
        );
        let comment = format!("{PACKET}// retained external reference: a\n");
        assert!(
            run(&comment, usize::MAX, usize::MAX)
                .0
                .unwrap()
                .contains("proof fn b()")
        );
    }

    #[test]
    fn proof_support_closure_exact_budget_and_denial_are_transactional() {
        let text = format!("{PACKET}proof fn original_trace() {{ a(); }}\n");
        let (measured, work, storage) = run(&text, usize::MAX, usize::MAX);
        let expected = measured.unwrap();
        assert!(work > 0 && storage > 0);
        assert_eq!(run(&text, work, storage).0.unwrap(), expected);
        assert!(matches!(
            run(&text, work - 1, storage).0,
            Err(Error::Resource(Resource::Work(_)))
        ));
        assert!(matches!(
            run(&text, work, storage - 1).0,
            Err(Error::Resource(Resource::Storage(_)))
        ));
    }

    #[test]
    fn proof_support_closure_rejects_duplicate_packets_and_confused_keys() {
        let duplicate = format!("{PACKET}{PACKET}");
        assert!(run(&duplicate, usize::MAX, usize::MAX).0.is_err());
        let mut work = Work::new(1000);
        let mut budget = Budget::new(&mut work, 1000);
        let mut collision = [Support {
            name: "different",
            key: key(b"actual"),
            ..EMPTY
        }];
        references("actual", &mut collision, &mut budget).unwrap();
        assert!(!collision[0].needed);
    }
}
