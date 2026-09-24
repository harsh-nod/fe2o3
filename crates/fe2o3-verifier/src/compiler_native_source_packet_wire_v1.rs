//! Count and fill use one grammar; no signature or source admission occurs here.
use super::*;

pub(super) fn encode(
    inputs: NativeCompilerRankedRecipeSourceProofInputsV1<'_>,
    erased: Option<&[u8]>,
    length: usize,
    output: Option<&mut Vec<u8>>,
    budget: &mut Budget<'_>,
) -> Result<usize, E> {
    if erased.is_some_and(<[u8]>::is_empty) {
        return Err(E::PacketWire("empty UnitLocal graph"));
    }
    let mut writer = Writer {
        output,
        position: 0,
        budget,
    };
    writer.put(MAGIC)?;
    writer.put(&VERSION.to_le_bytes())?;
    writer.put(&0_u16.to_le_bytes())?;
    writer.count(length)?;
    writer.put(&[if erased.is_some() { 2 } else { 1 }])?;
    let source = inputs.source;
    for bytes in [
        source.semantic_mir,
        source.native_module,
        source.middle_end_roster,
        source.correspondence_roster,
        source.verus_roster,
        erased.unwrap_or_default(),
    ] {
        writer.blob(bytes)?;
    }
    writer.roots(source.launch_inputs.len())?;
    for root in source.launch_inputs {
        writer.blob(root.logical_name().as_bytes())?;
        writer.put(&root.kernel_binding())?;
        let launch = root.launch();
        writer.put(&[launch.rank()])?;
        writer.put(&[u8::from(launch.exact_workgroup().is_some())])?;
        if let Some(workgroup) = launch.exact_workgroup() {
            writer.dimensions(workgroup)?;
        }
        writer.dimensions(launch.max_grid())?;
    }
    writer.roots(source.staging_roots.len())?;
    for root in source.staging_roots {
        writer.put(&root.semantic_root.to_le_bytes())?;
        writer.count(root.commitments.len())?;
        for row in root.commitments {
            for digest in row.digests() {
                writer.put(digest.as_bytes())?;
            }
        }
    }
    writer.roots(inputs.ranked_roots.len())?;
    for root in inputs.ranked_roots {
        writer.put(&root.semantic_root.to_le_bytes())?;
        writer.put(&[root.launch_rank])?;
        writer.blob(root.recipe_bytes)?;
        writer.blob(root.source_rows_bytes)?;
        writer.blob(root.ranked_ir.as_bytes())?;
        writer.count(root.effect_receipts.len())?;
        for receipt in root.effect_receipts {
            writer.put(receipt.wire())?;
            writer.put(receipt.verifying_key())?;
        }
    }
    Ok(writer.position)
}

struct Writer<'a, 'b, 'w> {
    output: Option<&'a mut Vec<u8>>,
    position: usize,
    budget: &'b mut Budget<'w>,
}
impl Writer<'_, '_, '_> {
    fn put(&mut self, bytes: &[u8]) -> Result<(), E> {
        let end = self
            .position
            .checked_add(bytes.len())
            .ok_or(Resource::Arithmetic)?;
        if end > MAX_NATIVE_COMPILER_SOURCE_PACKET_BYTES_V1 {
            return Err(E::PacketWire("aggregate source packet limit"));
        }
        self.budget
            .charge_work(bytes.len().checked_add(1).ok_or(Resource::Arithmetic)?)?;
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
    fn roots(&mut self, count: usize) -> Result<(), E> {
        if count > MAX_ROOTS {
            return Err(E::PacketWire("source packet root count"));
        }
        self.count(count)
    }
    fn blob(&mut self, bytes: &[u8]) -> Result<(), E> {
        self.count(bytes.len())?;
        self.put(bytes)
    }
    fn dimensions(&mut self, values: [u32; 3]) -> Result<(), E> {
        for value in values {
            self.put(&value.to_le_bytes())?;
        }
        Ok(())
    }
}
