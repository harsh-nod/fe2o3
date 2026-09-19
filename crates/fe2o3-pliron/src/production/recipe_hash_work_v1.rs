//! Shared hash transport. Encoding belongs to the existing recipe encoders.

use std::convert::Infallible;

use sha2::{Digest, Sha256};

use crate::production_analysis::{
    ProductionAnalysisResourceContractV1 as Resources, ProductionAnalysisResourceLimitV1 as Limit,
    ProductionAnalysisResourcePhaseV1 as Phase, ProductionAnalysisResourceUpperBoundV1 as Bound,
};

use super::{
    MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 as MAX_DEPTH,
    MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2 as MAX_NODES,
};

const PHASE: Phase = Phase::StructuralIdentity;
const HASH_STORAGE: usize = std::mem::size_of::<Sha256>() + std::mem::size_of::<[u8; 32]>();

pub(super) trait HashMeterV1 {
    type Error;

    fn work(&mut self, units: usize) -> Result<(), Self::Error>;
    fn reserve(&mut self, bytes: usize) -> Result<(), Self::Error>;
    fn release(&mut self, bytes: usize);
    fn expression_node(&mut self, nodes: &mut usize, depth: usize) -> Result<(), Self::Error>;
}

pub(super) struct MeteredV1<'a> {
    resources: &'a mut Resources,
    cleanup_error: Option<Limit>,
}

impl HashMeterV1 for MeteredV1<'_> {
    type Error = Limit;

    fn work(&mut self, units: usize) -> Result<(), Limit> {
        if let Some(error) = self.cleanup_error {
            return Err(error);
        }
        self.resources
            .admit_retained(PHASE, Bound::checked_phase(PHASE, units, 0, 0)?)
    }

    fn reserve(&mut self, bytes: usize) -> Result<(), Limit> {
        if let Some(error) = self.cleanup_error {
            return Err(error);
        }
        self.resources
            .admit_retained(PHASE, Bound::checked_phase(PHASE, 0, bytes, 0)?)
    }

    fn release(&mut self, bytes: usize) {
        if self.cleanup_error.is_none() {
            self.cleanup_error = Bound::checked_phase(PHASE, 0, 0, 0)
                .and_then(|empty| self.resources.admit_replacement(PHASE, bytes, empty))
                .err();
        }
    }

    fn expression_node(&mut self, nodes: &mut usize, depth: usize) -> Result<(), Limit> {
        self.work(1)?;
        if *nodes >= MAX_NODES || depth > MAX_DEPTH {
            return Err(Limit {
                phase: PHASE,
                resource: "recipe expression nodes or depth",
            });
        }
        *nodes += 1;
        Ok(())
    }
}

pub(super) struct UnmeteredV1;

impl HashMeterV1 for UnmeteredV1 {
    type Error = Infallible;

    fn work(&mut self, _: usize) -> Result<(), Infallible> {
        Ok(())
    }
    fn reserve(&mut self, _: usize) -> Result<(), Infallible> {
        Ok(())
    }
    fn release(&mut self, _: usize) {}
    fn expression_node(&mut self, _: &mut usize, _: usize) -> Result<(), Infallible> {
        Ok(())
    }
}

struct Scratch<'a, M: HashMeterV1> {
    meter: &'a mut M,
    bytes: usize,
}

impl<M: HashMeterV1> Drop for Scratch<'_, M> {
    fn drop(&mut self) {
        if self.bytes != 0 {
            self.meter.release(self.bytes);
        }
    }
}

fn with_scratch<M: HashMeterV1, R>(
    meter: &mut M,
    bytes: usize,
    use_scratch: impl FnOnce(&mut M) -> Result<R, M::Error>,
) -> Result<R, M::Error> {
    meter.reserve(bytes)?;
    let scratch = Scratch { meter, bytes };
    use_scratch(scratch.meter)
}

pub(super) struct TranscriptV1<'a, M: HashMeterV1> {
    digest: &'a mut Sha256,
    meter: &'a mut M,
}

impl<M: HashMeterV1> TranscriptV1<'_, M> {
    pub(super) fn update(&mut self, bytes: impl AsRef<[u8]>) -> Result<(), M::Error> {
        let bytes = bytes.as_ref();
        self.meter.work(1)?;
        self.meter.work(bytes.len())?;
        self.digest.update(bytes);
        Ok(())
    }

    pub(super) fn visit(&mut self) -> Result<(), M::Error> {
        self.meter.work(1)
    }

    pub(super) fn expression_node(
        &mut self,
        nodes: &mut usize,
        depth: usize,
    ) -> Result<(), M::Error> {
        self.meter.expression_node(nodes, depth)
    }

    pub(super) fn append_nested(
        &mut self,
        encode: impl FnOnce(&mut TranscriptV1<'_, M>) -> Result<(), M::Error>,
    ) -> Result<(), M::Error> {
        // The outer hash's reservation retains this digest slot throughout
        // child execution. Pay insertion before doing any child work.
        self.meter.work(33)?;
        let bytes = hash(self.meter, encode)?;
        self.digest.update(bytes);
        Ok(())
    }

    pub(super) fn scratch<R>(
        &mut self,
        bytes: usize,
        encode: impl FnOnce(&mut TranscriptV1<'_, M>) -> Result<R, M::Error>,
    ) -> Result<R, M::Error> {
        with_scratch(self.meter, bytes, |meter| {
            encode(&mut TranscriptV1 {
                digest: self.digest,
                meter,
            })
        })
    }
}

pub(super) fn hash<M: HashMeterV1>(
    meter: &mut M,
    encode: impl FnOnce(&mut TranscriptV1<'_, M>) -> Result<(), M::Error>,
) -> Result<[u8; 32], M::Error> {
    let result = with_scratch(meter, HASH_STORAGE, |meter| {
        meter.work(1)?;
        let mut digest = Sha256::new();
        encode(&mut TranscriptV1 {
            digest: &mut digest,
            meter,
        })?;
        meter.work(1)?;
        Ok(digest.finalize().into())
    });
    // Scratch cleanup has happened. A cleanup fault cannot return a digest.
    meter.work(0)?;
    result
}

/// The returned digest's 32-byte reservation remains in the caller's ledger.
/// It is conservatively retained by the pending owner, not refunded on copying.
pub(super) fn metered<'a>(
    resources: &'a mut Resources,
    encode: impl FnOnce(&mut TranscriptV1<'_, MeteredV1<'a>>) -> Result<(), Limit>,
) -> Result<[u8; 32], Limit> {
    let mut meter = MeteredV1 {
        resources,
        cleanup_error: None,
    };
    meter.reserve(32)?;
    let mut output = Scratch {
        meter: &mut meter,
        bytes: 32,
    };
    let digest = hash(output.meter, encode)?;
    output.meter.work(0)?;
    output.bytes = 0;
    Ok(digest)
}

pub(super) fn infallible<T>(result: Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(never) => match never {},
    }
}

pub(super) fn unmetered(
    encode: impl FnOnce(&mut TranscriptV1<'_, UnmeteredV1>) -> Result<(), Infallible>,
) -> [u8; 32] {
    infallible(hash(&mut UnmeteredV1, encode))
}

pub(super) fn unmetered_into(
    digest: &mut Sha256,
    encode: impl FnOnce(&mut TranscriptV1<'_, UnmeteredV1>) -> Result<(), Infallible>,
) {
    infallible(encode(&mut TranscriptV1 {
        digest,
        meter: &mut UnmeteredV1,
    }));
}

/// V2 refinement framing differs from ranked and expression value encodings.
pub(super) struct RefinementTranscriptV1<'a, 'b, M: HashMeterV1> {
    digest: &'a mut TranscriptV1<'b, M>,
}

impl<'a, 'b, M: HashMeterV1> RefinementTranscriptV1<'a, 'b, M> {
    pub(super) fn new(
        digest: &'a mut TranscriptV1<'b, M>,
        domain: &[u8],
    ) -> Result<Self, M::Error> {
        digest.update((domain.len() as u64).to_le_bytes())?;
        digest.update(domain)?;
        Ok(Self::fields(digest))
    }

    pub(super) fn fields(digest: &'a mut TranscriptV1<'b, M>) -> Self {
        Self { digest }
    }

    fn header(&mut self, tag: u16, len: usize) -> Result<(), M::Error> {
        self.digest.update(tag.to_le_bytes())?;
        self.digest.update((len as u64).to_le_bytes())
    }

    pub(super) fn field(&mut self, tag: u16, bytes: &[u8]) -> Result<(), M::Error> {
        self.header(tag, bytes.len())?;
        self.digest.update(bytes)
    }

    pub(super) fn value(
        &mut self,
        tag: u16,
        value: super::ProductionRankedValueV1,
    ) -> Result<(), M::Error> {
        self.digest.visit()?;
        let (bytes, len) = refinement_value(value);
        self.field(tag, &bytes[..len])
    }

    pub(super) fn values(
        &mut self,
        tag: u16,
        values: &[super::ProductionRankedValueV1],
    ) -> Result<(), M::Error> {
        // Owning refinement constructors bound these lists by the ranked rank.
        self.header(tag, 8 + values.len() * 9)?;
        self.digest.update((values.len() as u64).to_le_bytes())?;
        for value in values {
            self.digest.visit()?;
            let (bytes, _) = refinement_value(*value);
            self.digest.update(bytes)?;
        }
        Ok(())
    }
}

fn refinement_value(value: super::ProductionRankedValueV1) -> ([u8; 9], usize) {
    use super::ProductionRankedValueV1 as V;
    let mut bytes = [0; 9];
    let len = match value {
        V::Argument(index) => {
            bytes[0] = 1;
            bytes[1..5].copy_from_slice(&index.to_le_bytes());
            5
        }
        V::BlockArgument { block, argument } => {
            bytes[0] = 2;
            bytes[1..5].copy_from_slice(&block.to_le_bytes());
            bytes[5..9].copy_from_slice(&argument.to_le_bytes());
            9
        }
        V::Local(identity) => {
            bytes[0] = 3;
            bytes[1..5].copy_from_slice(&identity.get().to_le_bytes());
            5
        }
    };
    (bytes, len)
}

#[cfg(test)]
#[path = "recipe_hash_work_v1_tests.rs"]
mod tests;
