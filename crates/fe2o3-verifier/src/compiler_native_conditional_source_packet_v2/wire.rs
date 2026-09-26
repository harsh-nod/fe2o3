//! One bounded count/fill grammar; no leaf admission or signature verification.
use super::*;

pub(super) fn encode(
    input: NativeConditionalSourcePacketInputV2<'_>,
    length: usize,
    output: Option<&mut Vec<u8>>,
    budget: &mut Budget<'_>,
) -> Result<usize, E> {
    let mut w = Writer {
        output,
        position: 0,
        budget,
    };
    w.put(MAGIC)?;
    w.put(&VERSION.to_le_bytes())?;
    w.put(&0_u16.to_le_bytes())?;
    w.count(length)?;
    w.put(&[ROUTE])?;
    w.blob(input.semantic_mir)?;
    w.blob(input.native_module)?;
    w.count(input.canonical_kernel_order.len())?;
    for ordinal in input.canonical_kernel_order {
        w.put(&ordinal.to_le_bytes())?;
    }
    w.count(input.roots.len())?;
    for root in input.roots {
        w.put(&root.semantic_root.to_le_bytes())?;
        w.put(&[root.launch_rank])?;
        w.blob(root.launch.logical_name().as_bytes())?;
        w.put(&root.launch.kernel_binding())?;
        let launch = root.launch.launch();
        w.put(&[launch.rank()])?;
        w.put(&[u8::from(launch.exact_workgroup().is_some())])?;
        if let Some(workgroup) = launch.exact_workgroup() {
            w.dimensions(workgroup)?;
        }
        w.dimensions(launch.max_grid())?;
        for bytes in [
            root.induction_bytes,
            root.recipe_bytes,
            root.source_rows_bytes,
            root.ranked_ir.as_bytes(),
            root.cpu_input_bytes,
        ] {
            w.blob(bytes)?;
        }
        w.budget.charge_work(2)?;
        require(
            !root.staging_commitments.is_empty()
                && root.staging_commitments.len() == root.effect_receipts.len(),
            "staging/signature count",
        )?;
        // Refuse an impossible aggregate before traversing the supplied roster.
        require(
            mul(root.staging_commitments.len(), EFFECT_BYTES)? <= MAX_BYTES,
            "staging count limit",
        )?;
        w.count(root.staging_commitments.len())?;
        for (row, signature) in root.staging_commitments.iter().zip(root.effect_receipts) {
            for digest in [
                &row.receipt,
                &row.effect,
                &row.signer,
                &row.execution,
                &row.toolchain[0],
                &row.toolchain[1],
                &row.toolchain[2],
                &row.toolchain[3],
                &row.toolchain[4],
            ] {
                w.put(digest)?;
            }
            w.signature(signature)?;
        }
        w.count(EVIDENCE_BYTES)?;
        w.put(&VERSION.to_le_bytes())?;
        w.put(&0_u16.to_le_bytes())?;
        w.signature(root.formula_receipt)?;
    }
    Ok(w.position)
}

struct Writer<'a, 'b, 'w> {
    output: Option<&'a mut Vec<u8>>,
    position: usize,
    budget: &'b mut Budget<'w>,
}
impl Writer<'_, '_, '_> {
    fn put(&mut self, bytes: &[u8]) -> Result<(), E> {
        let end = add(self.position, bytes.len())?;
        require(end <= MAX_BYTES, "aggregate packet limit")?;
        self.budget.charge_work(add(bytes.len(), 1)?)?;
        if let Some(output) = &mut self.output {
            if end > output.capacity() {
                return Err(Resource::Accounting.into());
            }
            output.extend_from_slice(bytes);
        }
        self.position = end;
        Ok(())
    }
    fn count(&mut self, count: usize) -> Result<(), E> {
        self.put(
            &u32::try_from(count)
                .map_err(|_| Resource::Arithmetic)?
                .to_le_bytes(),
        )
    }
    fn blob(&mut self, bytes: &[u8]) -> Result<(), E> {
        self.count(bytes.len())?;
        self.put(bytes)
    }
    fn dimensions(&mut self, dimensions: [u32; 3]) -> Result<(), E> {
        for value in dimensions {
            self.put(&value.to_le_bytes())?;
        }
        Ok(())
    }
    fn signature(&mut self, signature: &Signature) -> Result<(), E> {
        self.put(signature.wire())?;
        self.put(signature.verifying_key())
    }
}
