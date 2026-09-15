//! V25 footer decoding only. Historical documents have no footer at all.
use super::*;

impl AdmittedInertSemanticMirV1 {
    pub fn decode_exact_v25_canonical(bytes: &[u8], limits: SemanticMirLimitsV1) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(bytes, limits, CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V25))
    }
}

impl CanonicalDecoderV1<'_> {
    fn owned_source_call_site(
        &mut self,
    ) -> Result<SemanticOwnedSourceCallSiteV1, SemanticMirDecodeErrorV1> {
        Ok(SemanticOwnedSourceCallSiteV1 {
            function: SemanticFunctionIdV1(self.u32()?),
            block: SemanticBlockIdV1(self.u32()?),
        })
    }

    pub(super) fn transpose_owned_flows_v25(
        &mut self,
        retained_blocks: usize,
    ) -> Result<Vec<SemanticTransposeOwnedFlowV1>, SemanticMirDecodeErrorV1> {
        if self.wire_version < SemanticMirWireVersionV1::V25 {
            return Ok(Vec::new());
        }
        let count = self.u32()?;
        self.require_count(SemanticMirResourceV1::Blocks, u64::from(count))?;
        let count =
            usize::try_from(count).map_err(|_| SemanticMirDecodeErrorV1::LengthOverflow {
                context: "transpose owned-flow count",
            })?;
        // Each source row owns a distinct Issue call. This caps retained rows
        // using the already bounded body table before reserving any storage.
        if count > retained_blocks {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi.into());
        }
        if count > self.remaining() / 204 {
            return Err(SemanticMirDecodeErrorV1::UnexpectedEnd {
                offset: self.offset,
                requested: count,
            });
        }
        let mut rows = Vec::new();
        rows.try_reserve_exact(count)
            .map_err(|_| SemanticMirDecodeErrorV1::AllocationFailed {
                context: "transpose owned-flow rows",
            })?;
        let mut total_borrows = 0u64;
        for _ in 0..count {
            let issue = self.owned_source_call_site()?;
            let capture = SemanticOwnedSourceStatementSiteV1 {
                function: SemanticFunctionIdV1(self.u32()?),
                block: SemanticBlockIdV1(self.u32()?),
                statement: self.u32()?,
            };
            let capture_field = self.u32()?;
            let matrix_call = self.owned_source_call_site()?;
            let closure_call = self.owned_source_call_site()?;
            let stage = self.owned_source_call_site()?;
            let publish = self.owned_source_call_site()?;
            let workgroup_local = SemanticLocalIdV1(self.u32()?);
            let borrow_count = self.u32()?;
            total_borrows = total_borrows.checked_add(u64::from(borrow_count)).ok_or(
                SemanticMirDecodeErrorV1::LengthOverflow {
                    context: "transpose owned-flow borrows",
                },
            )?;
            self.require_count(SemanticMirResourceV1::CallArguments, total_borrows)?;
            let borrow_count = usize::try_from(borrow_count).map_err(|_| {
                SemanticMirDecodeErrorV1::LengthOverflow {
                    context: "transpose owned-flow borrows",
                }
            })?;
            // 140 fixed bytes remain for the three body commitments and HIR
            // binding. Check multiplication/addition before allocation.
            let needed = borrow_count
                .checked_mul(8)
                .and_then(|n| n.checked_add(140))
                .ok_or(SemanticMirDecodeErrorV1::LengthOverflow {
                    context: "transpose owned-flow byte length",
                })?;
            if needed > self.remaining() {
                return Err(SemanticMirDecodeErrorV1::UnexpectedEnd {
                    offset: self.offset,
                    requested: needed,
                });
            }
            let mut borrows = Vec::new();
            borrows.try_reserve_exact(borrow_count).map_err(|_| {
                SemanticMirDecodeErrorV1::AllocationFailed {
                    context: "transpose owned-flow borrow sites",
                }
            })?;
            for _ in 0..borrow_count {
                borrows.push(self.owned_source_call_site()?);
            }
            let mut bodies = [(SemanticFunctionIdV1(0), [0; 32]); 3];
            for body in &mut bodies {
                *body = (SemanticFunctionIdV1(self.u32()?), self.identity()?);
            }
            let source_binding = self.identity()?;
            let row = SemanticTransposeOwnedFlowV1::from_encoded_parts(
                SemanticTransposeOwnedFlowSitesV1 {
                    issue,
                    capture,
                    capture_field,
                    matrix_call,
                    closure_call,
                    stage,
                    publish,
                    workgroup_local,
                },
                borrows,
                bodies,
                source_binding,
            )?;
            if rows
                .last()
                .is_some_and(|prior: &SemanticTransposeOwnedFlowV1| {
                    prior.canonical_key() >= row.canonical_key()
                })
            {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi.into());
            }
            rows.push(row);
        }
        Ok(rows)
    }
}

#[cfg(test)]
#[path = "transpose_owned_flow_decode_v25/tests.rs"]
mod tests;
