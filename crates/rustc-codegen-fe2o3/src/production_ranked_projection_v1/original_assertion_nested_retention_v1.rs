//! cfg(test)-only ORIGINAL nested-constructor retention; no source authority.
use super::*;
use std::any::Any;
use std::panic::resume_unwind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) enum Stage {
    Start,
    Strict,
    FixedFrames,
    FirstWork,
    Ledger,
    Source,
    EvaluatorFrames,
    ProofFrame,
    ProofWork,
    Shape,
    NestedWork,
    NestedReserve,
    NestedFill,
    BlockWork,
    RowPush,
    Dominance,
    Zero,
    Ready,
}
pub(in crate::production_ranked_projection_v1) struct Cuts {
    pub seen: usize,
    pub last: Stage,
    pub panic_at: Option<usize>,
    payload: Option<Box<[u64; 2]>>,
    pub original_address: Option<usize>,
}
impl Cuts {
    pub fn new(panic_at: Option<usize>) -> Self {
        Self {
            seen: 0,
            last: Stage::Start,
            panic_at,
            payload: None,
            original_address: None,
        }
    }
    // Called only AFTER the enclosing component's typed prefix is admitted.
    pub fn prepare_payload(&mut self) {
        if self.panic_at.is_some() {
            let payload = Box::new([0x6f726967696e616cu64, 0x72657461696e6564u64]);
            self.original_address = Some(payload.as_ref() as *const [u64; 2] as usize);
            self.payload = Some(payload);
        }
    }
    pub fn checkpoint(&mut self, stage: Stage) {
        self.seen = self
            .seen
            .checked_add(1)
            .expect("bounded constructor checkpoint roster");
        self.last = stage;
        if self.panic_at == Some(self.seen) {
            let payload: Box<dyn Any + Send> =
                self.payload.take().expect("prepaid original panic payload");
            resume_unwind(payload);
        }
    }
}

pub(in crate::production_ranked_projection_v1) fn nested_into_original<T>(
    resources: &mut AssertionResourcesV1<'_>,
    values: &mut Vec<Vec<T>>,
    count: usize,
    cuts: &mut Cuts,
) -> Result<()> {
    resources.available()?;
    if !resources.is_strict() {
        *values = Vec::with_capacity(count);
        values.resize_with(count, Vec::new);
        return Ok(());
    }
    let work = resources.product(count, size_of::<Vec<T>>())?;
    let work = work
        .checked_add(count)
        .ok_or_else(|| resources.arithmetic())?;
    resources.extra_work(work)?;
    cuts.checkpoint(Stage::NestedWork);
    resources.reserve_vec(
        values,
        count,
        LegacyReserve::Exact,
        "assertion nested table storage cannot be reserved",
    )?;
    cuts.checkpoint(Stage::NestedReserve);
    values.resize_with(count, Vec::new);
    cuts.checkpoint(Stage::NestedFill);
    Ok(())
}

fn frame<T>(locals: usize) -> Result<usize> {
    locals
        .checked_add(
            size_of::<T>()
                .checked_mul(2)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )
        .and_then(|n| {
            size_of::<Result<T>>()
                .checked_mul(2)
                .and_then(|r| n.checked_add(r))
        })
        .ok_or_else(|| resource(Resource::Arithmetic))
}
pub(in crate::production_ranked_projection_v1) fn additional_frame<T>() -> Result<usize> {
    const N: usize = 12;
    let rows: [usize; N] = [
        frame::<()>(size_of::<(
            &mut AssertionResourcesV1<'static>,
            &mut Vec<Vec<T>>,
            usize,
            &mut Cuts,
            usize,
            Option<usize>,
        )>())?,
        frame::<()>(size_of::<(
            &mut Cuts,
            Stage,
            usize,
            Option<usize>,
            Option<Box<[u64; 2]>>,
            Box<dyn Any + Send>,
        )>())?,
        frame::<Cuts>(size_of::<(Cuts, Option<usize>, Stage)>())?,
        frame::<()>(size_of::<(
            &mut Cuts,
            Box<[u64; 2]>,
            [u64; 2],
            Option<usize>,
            *const [u64; 2],
        )>())?,
        // Existing nonempty-capable cache snapshot bodies, named here because
        // their private CacheStorage enum is visible only in this module tree.
        frame::<(u8, usize, usize, usize)>(size_of::<(
            &AssertionCacheV1<'static>,
            &CacheStorage,
            &Vec<((usize, usize), bool)>,
            &HashMap<(usize, usize), bool>,
            *const ((usize, usize), bool),
            usize,
            usize,
        )>())?,
        frame::<(u8, usize, usize, usize)>(size_of::<(
            &RetiredAssertionCacheV1,
            &CacheStorage,
            &Vec<((usize, usize), bool)>,
            &HashMap<(usize, usize), bool>,
            *const ((usize, usize), bool),
            usize,
            usize,
        )>())?,
        frame::<[usize; N]>(size_of::<([usize; N], Option<usize>)>())?,
        frame::<usize>(size_of::<([usize; N], &[usize], Result<usize>)>())?,
        frame::<usize>(size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            usize,
            &usize,
            Option<usize>,
        )>())?,
        frame::<usize>(size_of::<(
            usize,
            usize,
            Option<usize>,
            Result<usize>,
            Error,
            Resource,
        )>())?,
        frame::<()>(size_of::<(&mut Vec<Vec<T>>, usize, Vec<T>, fn() -> Vec<T>)>())?,
        frame::<()>(size_of::<(
            Option<Box<[u64; 2]>>,
            Box<[u64; 2]>,
            Box<dyn Any + Send>,
            &(dyn Any + Send),
        )>())?,
    ];
    rows.iter().try_fold(0usize, |n, r| {
        n.checked_add(*r)
            .ok_or_else(|| resource(Resource::Arithmetic))
    })
}
