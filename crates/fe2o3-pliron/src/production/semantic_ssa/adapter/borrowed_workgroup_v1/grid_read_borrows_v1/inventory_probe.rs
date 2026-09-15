//! Test-only access to the live common dispatcher, never source admission.
use super::super::{Budget, SemanticBorrowCandidateV1, SemanticTransparentBorrowSiteV1};
use super::*;

type UseSite = SemanticTransparentBorrowSiteV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct Observation {
    pub site: UseSite,
    pub candidate: usize,
    pub remaining: usize,
    pub uses: Option<Vec<UseSite>>,
    pub consumers: u32,
    pub intrinsic: bool,
}

pub(in super::super) struct Probe {
    function: SemanticFunctionIdentityV1,
    reference: SemanticTypeIdV1,
    owner: SemanticLocalIdV1,
    receiver: SemanticLocalIdV1,
    borrow: UseSite,
    reads: [UseSite; 3],
    pub before: Vec<Observation>,
    pub after: Vec<Observation>,
    pub final_uses: Vec<Vec<UseSite>>,
    pub finished: bool,
}

impl Probe {
    pub fn new(
        function: &SemanticFunctionDeclV1,
        reference: SemanticTypeIdV1,
        owner: SemanticLocalIdV1,
        receiver: SemanticLocalIdV1,
        borrow: UseSite,
        reads: [UseSite; 3],
    ) -> Self {
        Self {
            function: function.identity(),
            reference,
            owner,
            receiver,
            borrow,
            reads,
            before: Vec::new(),
            after: Vec::new(),
            final_uses: Vec::new(),
            finished: false,
        }
    }

    // This component fixture deliberately does not fabricate a Grid source
    // contract. It exercises bookkeeping after source recognition; production
    // and the ordinary tests always pass None at the cfg(test) call boundary.
    pub fn facts<'a>(
        &self,
        function: &'a SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
        budget: &mut Budget,
    ) -> Result<Facts<'a>, Error> {
        assert_eq!(function.identity(), self.function);
        let mut charge = |work| budget.charge(work);
        let owned = primitive_read::shared_snapshot(types, self.reference, &mut charge)?
            .expect("closed typed snapshot in the component fixture");
        assert_eq!(function.locals()[self.owner.index() as usize].ty(), owned);
        assert_eq!(
            function.locals()[self.receiver.index() as usize].ty(),
            self.reference
        );
        let statement = |site: UseSite| {
            function.blocks()[site.block as usize].statements()[site.statement as usize].kind()
        };
        assert!(
            matches!(statement(self.borrow), SemanticStatementKindV1::Assign(a)
            if a.destination().local() == self.receiver
                && a.destination().projections().is_empty()
                && a.destination().ty() == self.reference
                && matches!(a.value().kind(), SemanticRvalueKindV1::Borrow {kind:SemanticBorrowKindV1::Shared,place}
                    if place.local() == self.owner && place.projections().is_empty() && place.ty() == owned))
        );
        assert!(guarded_grid_results::initialized_shared_owner(
            function,
            self.owner,
            &mut |work, words| { charge(work.checked_add(words).ok_or(Error::ResourceOverflow)?) }
        )?);
        let mut facts = Facts::default();
        for site in self.reads {
            let kind = statement(site);
            let SemanticStatementKindV1::Assign(a) = kind else {
                panic!("original primitive read")
            };
            assert!(primitive_read::copied_field(
                a,
                self.receiver,
                owned,
                types,
                &mut charge
            )?);
            charge(12 + lookup_work(facts.reads.len()))?;
            assert!(
                facts
                    .reads
                    .insert((site.block, site.statement), (kind, self.receiver.index()))
                    .is_none()
            );
        }
        charge(33)?;
        facts.pairs.insert(self.reference, owned);
        facts.roots.insert(
            (self.borrow.block, self.borrow.statement),
            (self.owner.index(), owned),
        );
        Ok(facts)
    }

    pub fn before_push(
        &mut self,
        site: UseSite,
        index: usize,
        candidates: &[SemanticBorrowCandidateV1],
        uses: &[Vec<UseSite>],
        remaining: usize,
    ) {
        assert!(self.reads.contains(&site));
        assert!(self.before.len() < self.reads.len());
        self.before
            .push(observe(site, index, candidates, uses, remaining));
    }
    pub fn after_push(
        &mut self,
        site: UseSite,
        index: usize,
        candidates: &[SemanticBorrowCandidateV1],
        uses: &[Vec<UseSite>],
        remaining: usize,
    ) {
        assert!(self.after.len() < self.reads.len());
        self.after
            .push(observe(site, index, candidates, uses, remaining));
    }
    pub fn finish(&mut self, uses: &[Vec<UseSite>]) {
        assert!(!self.finished);
        self.finished = true;
        self.final_uses = uses.to_vec();
    }
}

fn observe(
    site: UseSite,
    index: usize,
    candidates: &[SemanticBorrowCandidateV1],
    uses: &[Vec<UseSite>],
    remaining: usize,
) -> Observation {
    let candidate = &candidates[index];
    Observation {
        site,
        candidate: index,
        remaining,
        uses: uses.get(index).cloned(),
        consumers: candidate.consumers,
        intrinsic: candidate.intrinsic_consumer,
    }
}
