use super::*;

/// Exact actual-pair local relation only. No source lifetime, final target,
/// optimizer-policy, signed proof or publication authority is established.
pub struct CheckedCanonicalKirRedundantStoreV1<'a> {
    input: &'a Owner,
    output: &'a Owner,
    rows: &'a [Row],
    origins: &'a [Retained],
}
impl<'a> CheckedCanonicalKirRedundantStoreV1<'a> {
    pub const fn input(&self) -> &'a Owner {
        self.input
    }
    pub const fn output(&self) -> &'a Owner {
        self.output
    }
    pub const fn rows(&self) -> &'a [Row] {
        self.rows
    }
    pub const fn retained_operations(&self) -> &'a [Retained] {
        self.origins
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Rebuilds the complete input/output inventories and direct-slot census from
/// the actual immutable owners. A two-cursor scan checks every omission and
/// retained operation without trusting a producer plan or MemorySSA. All
/// headers, results, other operations, terminators and CFG occurrences are exact.
pub fn check_canonical_kir_redundant_store_v1<'a>(
    input: &'a Owner,
    output: &'a Owner,
    rows: &'a [Row],
    origins: &'a [Retained],
    budget: &mut Budget<'_>,
) -> Result<(CheckedCanonicalKirRedundantStoreV1<'a>, Storage)> {
    scoped(budget, |budget| {
        budget.charge_work(3)?;
        let bytes = input
            .canonical()
            .canonical_bytes()
            .len()
            .checked_add(output.canonical().canonical_bytes().len())
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(bytes)?;
        budget.reserve_storage(size_of::<CheckedCanonicalKirRedundantStoreV1<'_>>())?;
        let (a, a_size) = Inventory::derive(input, budget)?;
        budget.reserve_storage(a_size.retained_storage())?;
        let (b, b_size) = Inventory::derive(output, budget)?;
        budget.reserve_storage(b_size.retained_storage())?;
        if rows.len() > a.operations().len() || origins.len() != b.operations().len() {
            return Err(Error::Rule(
                "complete bounded deletion and retained rosters",
            ));
        }
        headers(input.module(), output.module(), budget)?;
        let (slots, slot_size) = slots::Slots::derive(&a, budget)?;
        budget.reserve_storage(slot_size)?;
        let mut next = 0;
        let mut mapped = 0;
        for (old, new) in a.blocks().iter().zip(b.blocks()) {
            budget.charge_work(2)?;
            if old.coordinate != new.coordinate {
                return Err(Error::Rule("block coordinate"));
            }
            let mut seed: Option<Store> = None;
            let mut out = new.operations.start;
            for ordinal in old.operations.clone() {
                budget.charge_work(6)?;
                let current = &a.operations()[ordinal];
                let store = slots.store(ordinal, budget)?;
                let repeated =
                    store.and_then(|store| seed.filter(|anchor| anchor.identical(store)));
                if let Some(anchor) = repeated {
                    if rows.get(next)
                        != Some(&Row {
                            anchor: anchor.coordinate,
                            removed: current.coordinate,
                        })
                    {
                        return Err(Error::Rule("exact redundant Store deletion row"));
                    }
                    next += 1;
                } else {
                    let actual = b
                        .operations()
                        .get(out)
                        .filter(|_| out < new.operations.end)
                        .ok_or(Error::Rule("missing retained operation"))?;
                    if current.operation != actual.operation {
                        return Err(Error::Rule("retained operation payload"));
                    }
                    if origins.get(mapped)
                        != Some(&Retained {
                            input: current.coordinate,
                            output: actual.coordinate,
                        })
                    {
                        return Err(Error::Rule("exact retained operation origin"));
                    }
                    out += 1;
                    mapped += 1;
                    if let Some(store) = store {
                        seed = Some(store);
                    } else if !transparent(current.operation) {
                        seed = None;
                    }
                }
            }
            if out != new.operations.end {
                return Err(Error::Rule("extra output operation"));
            }
        }
        if next != rows.len() || mapped != origins.len() {
            return Err(Error::Rule("unordered, duplicate or absent row"));
        }
        drop(slots);
        budget.release_storage(slot_size)?;
        drop(b);
        budget.release_storage(b_size.retained_storage())?;
        drop(a);
        budget.release_storage(a_size.retained_storage())?;
        Ok((
            CheckedCanonicalKirRedundantStoreV1 {
                input,
                output,
                rows,
                origins,
            },
            CanonicalKirRedundantStoreStorageV1(
                size_of::<CheckedCanonicalKirRedundantStoreV1<'_>>(),
            ),
        ))
    })
}

fn headers(input: &Module, output: &Module, budget: &mut Budget<'_>) -> Result<()> {
    let Module {
        id,
        functions,
        kernels,
        required_capabilities,
    } = input;
    if id != &output.id
        || kernels != &output.kernels
        || required_capabilities != &output.required_capabilities
        || functions.len() != output.functions.len()
    {
        return Err(Error::Rule("module payload"));
    }
    for (a, b) in functions.iter().zip(&output.functions) {
        budget.charge_work(1)?;
        let fe2o3_kernel_ir::Function {
            id,
            signature,
            role,
            body,
            required_capabilities,
        } = a;
        if id != &b.id
            || signature != &b.signature
            || role != &b.role
            || required_capabilities != &b.required_capabilities
        {
            return Err(Error::Rule("function payload"));
        }
        let (a, b) = match (body, &b.body) {
            (None, None) => continue,
            (Some(a), Some(b)) => (a, b),
            _ => return Err(Error::Rule("function body")),
        };
        let fe2o3_kernel_ir::FunctionBody { parameters, blocks } = a;
        if parameters != &b.parameters || blocks.len() != b.blocks.len() {
            return Err(Error::Rule("body payload"));
        }
        for (a, b) in blocks.iter().zip(&b.blocks) {
            budget.charge_work(1)?;
            let fe2o3_kernel_ir::BasicBlock {
                id,
                parameters,
                operations: _,
                terminator,
            } = a;
            if id != &b.id || parameters != &b.parameters || terminator != &b.terminator {
                return Err(Error::Rule("block payload"));
            }
        }
    }
    Ok(())
}
