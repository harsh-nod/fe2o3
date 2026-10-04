//! Borrowed payloads and prepaid typed metadata, never imported proof authority.
use super::*;
use fe2o3_functional_proof::FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2;
use fe2o3_lower_mir_kernel::ProductionSourceLaunchInputV1;

pub(super) fn decode<'a>(bytes: &'a [u8], budget: &mut Budget<'_>) -> Result<Packet<'a>, E> {
    if bytes.len() > MAX_NATIVE_COMPILER_SOURCE_PACKET_BYTES_V1 {
        return Err(E::PacketWire("aggregate source packet limit"));
    }
    let mut reader = Reader { bytes, budget };
    if reader.take(8)? != MAGIC
        || reader.array::<2>()? != VERSION.to_le_bytes()
        || reader.array::<2>()? != [0; 2]
    {
        return Err(E::PacketWire("source packet magic/version/flags"));
    }
    if reader.u32()? as usize != bytes.len() {
        return Err(E::PacketWire("source packet total length"));
    }
    let route = reader.array::<1>()?[0];
    if !matches!(route, 1 | 2) {
        return Err(E::PacketWire("source packet route"));
    }
    reader.budget.reserve_storage(size_of::<Packet<'_>>())?;
    let mut frames = [&[][..]; 5];
    for frame in &mut frames {
        *frame = reader.blob()?;
    }
    let erased = reader.blob()?;
    let erased = match (route, erased.is_empty()) {
        (1, true) => None,
        (2, false) => Some(erased),
        _ => return Err(E::PacketWire("source packet route/graph mismatch")),
    };
    let count = reader.count(MAX_ROOTS, 50)?;
    let (mut launches, _) = ranked_source::reserve_vec(count, reader.budget)?;
    for _ in 0..count {
        let name = reader.text()?;
        let binding = reader.array()?;
        let rank = reader.array::<1>()?[0];
        let workgroup = match reader.array::<1>()?[0] {
            0 => None,
            1 => Some(reader.dimensions()?),
            _ => return Err(E::PacketWire("source packet workgroup option")),
        };
        let max_grid = reader.dimensions()?;
        launches.push(ProductionSourceLaunchRootInputV1::new(
            name,
            binding,
            ProductionSourceLaunchInputV1::new(rank, workgroup, max_grid),
        ));
    }
    let count = reader.count(MAX_ROOTS, 8)?;
    let (mut staging, _) = ranked_source::reserve_vec(count, reader.budget)?;
    for _ in 0..count {
        let semantic_root = reader.u32()?;
        let count = reader.count(usize::MAX, 9 * 32)?;
        let (mut commitments, _) = ranked_source::reserve_vec(count, reader.budget)?;
        for _ in 0..count {
            commitments.push(NativeCompilerStagingCommitmentV1 {
                receipt: reader.array()?,
                effect: reader.array()?,
                signer: reader.array()?,
                execution: reader.array()?,
                toolchain: [
                    reader.array()?,
                    reader.array()?,
                    reader.array()?,
                    reader.array()?,
                    reader.array()?,
                ],
            });
        }
        staging.push(StagingRoot {
            semantic_root,
            commitments,
        });
    }
    let count = reader.count(MAX_ROOTS, 21)?;
    let (mut ranked, _) = ranked_source::reserve_vec(count, reader.budget)?;
    for _ in 0..count {
        let semantic_root = reader.u32()?;
        let launch_rank = reader.array::<1>()?[0];
        let recipe = reader.blob()?;
        let source_rows = reader.blob()?;
        let text = reader.text()?;
        let count = reader.count(usize::MAX, FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2 + 32)?;
        let (mut signatures, _) = ranked_source::reserve_vec(count, reader.budget)?;
        for _ in 0..count {
            signatures.push(Signature::from_untrusted_parts(
                reader.array()?,
                reader.array()?,
            ));
        }
        ranked.push(RankedRoot {
            semantic_root,
            launch_rank,
            recipe,
            source_rows,
            text,
            signatures,
        });
    }
    if !reader.bytes.is_empty() {
        return Err(E::PacketWire("source packet trailing bytes"));
    }
    Ok(Packet {
        frames,
        erased,
        launches,
        staging,
        ranked,
    })
}

struct Reader<'a, 'b, 'w> {
    bytes: &'a [u8],
    budget: &'b mut Budget<'w>,
}
impl<'a> Reader<'a, '_, '_> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], E> {
        let (bytes, tail) = self
            .bytes
            .split_at_checked(length)
            .ok_or(E::PacketWire("truncated source packet"))?;
        self.budget
            .charge_work(length.checked_add(1).ok_or(Resource::Arithmetic)?)?;
        self.bytes = tail;
        Ok(bytes)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], E> {
        self.take(N)?
            .try_into()
            .map_err(|_| Resource::Accounting.into())
    }
    fn u32(&mut self) -> Result<u32, E> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn blob(&mut self) -> Result<&'a [u8], E> {
        let length = self.u32()? as usize;
        self.take(length)
    }
    fn text(&mut self) -> Result<&'a str, E> {
        let bytes = self.blob()?;
        self.budget.charge_work(bytes.len())?;
        std::str::from_utf8(bytes).map_err(|_| E::PacketWire("source packet UTF-8"))
    }
    fn count(&mut self, maximum: usize, minimum_bytes: usize) -> Result<usize, E> {
        let count = self.u32()? as usize;
        if count > maximum || count > self.bytes.len() / minimum_bytes {
            return Err(E::PacketWire("source packet count/remaining bytes"));
        }
        Ok(count)
    }
    fn dimensions(&mut self) -> Result<[u32; 3], E> {
        Ok([self.u32()?, self.u32()?, self.u32()?])
    }
}
