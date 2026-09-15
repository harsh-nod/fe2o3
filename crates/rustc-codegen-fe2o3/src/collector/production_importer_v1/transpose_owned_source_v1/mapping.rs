use super::*;

struct Body<'p, 'a> {
    original: SemanticFunctionIdV1,
    canonical: SemanticFunctionIdV1,
    declaration: &'a SemanticFunctionDeclV1,
    correspondence: Correspondence<'p, 'a>,
}

impl Body<'_, '_> {
    fn site(
        &self,
        site: Site,
        work: &mut usize,
    ) -> PlanResult<(SemanticFunctionIdV1, SemanticBlockIdV1)> {
        if site.function != self.original {
            return Err(Error::Source("transpose source site changed original body").into());
        }
        // Correspondence uses identity binary search in the admitted roster.
        bounded::charge(work, bounded::search_work(self.declaration.blocks().len()))?;
        Ok((
            self.canonical,
            self.correspondence
                .block(u32::try_from(site.block.index()).map_err(|_| Error::Work)?)?,
        ))
    }

    fn local(&self, local: mir::Local, work: &mut usize) -> PlanResult<SemanticLocalIdV1> {
        bounded::charge(work, bounded::search_work(self.declaration.locals().len()))?;
        Ok(self
            .correspondence
            .local(u32::try_from(local.index()).map_err(|_| Error::Work)?)?)
    }
}

fn body<'p, 'a, 'tcx>(
    auth: &Authentication<'a, 'tcx>,
    replay: &mut Replay<'p, 'tcx>,
    original: SemanticFunctionIdV1,
    work: &mut usize,
) -> PlanResult<Body<'p, 'a>> {
    let producer = auth
        .retained
        .function_producers()
        .get(original.index() as usize)
        .ok_or(Error::Source("source function outside retained table"))?;
    let mut found = None;
    for (index, candidate) in auth.semantic.functions().iter().enumerate() {
        bounded::charge(work, 1)?;
        if candidate.identity() == producer.identities.function() {
            if found.replace((index, candidate)).is_some() {
                return Err(Error::Source("duplicate canonical source function identity").into());
            }
        }
    }
    let (index, declaration) = found.ok_or(Error::Source("missing canonical source function"))?;
    let abi = auth
        .retained
        .function_abi_producers()
        .get(original.index() as usize)
        .filter(|abi| abi.function == original)
        .ok_or(Error::Source("source function lost retained ABI"))?;
    if declaration.abi().identity() != abi.identity {
        return Err(Error::Source("source canonical ABI changed").into());
    }
    let correspondence = replay.check(auth.retained, original, declaration, work)?;
    Ok(Body {
        original,
        canonical: SemanticFunctionIdV1::from_index(u32::try_from(index).map_err(|_| Error::Work)?),
        declaration,
        correspondence,
    })
}

pub(super) fn flow<'a, 'p, 'tcx>(
    auth: &Authentication<'a, 'tcx>,
    replay: &mut Replay<'p, 'tcx>,
    checked: CheckedFlow<'tcx>,
    work: &mut usize,
) -> PlanResult<MappedFlow<'a>> {
    let c = &checked.coordinates;
    let functions = [
        c.publish.function,
        c.closure_call.function,
        c.stage.function,
    ];
    if functions[0] == functions[1] || functions[0] == functions[2] || functions[1] == functions[2]
    {
        return Err(Error::Source("transpose source body roles collapsed").into());
    }
    let outer = body(auth, replay, functions[0], work)?;
    let helper = body(auth, replay, functions[1], work)?;
    let closure = body(auth, replay, functions[2], work)?;
    let capture = outer.site(c.capture.0, work)?;
    let block = &outer.declaration.blocks()[capture.1.index() as usize];
    bounded::charge(work, 1)?;
    if c.capture.1 >= block.statements().len() {
        return Err(Error::Source("source capture statement lost original correspondence").into());
    }
    let mut workgroup_borrows = Vec::new();
    for borrow in &checked.nodes.borrows {
        // Source visitor order is not canonical order. Each insertion charges
        // its search, retained key and shifts before publication.
        bounded::insert_unique(&mut workgroup_borrows, outer.site(borrow.site, work)?, work)?;
    }
    Ok(MappedFlow {
        issue: outer.site(c.issue, work)?,
        capture: (
            capture.0,
            capture.1,
            u32::try_from(c.capture.1).map_err(|_| Error::Work)?,
        ),
        capture_field: checked.nodes.capture_field,
        matrix_call: outer.site(c.matrix_call, work)?,
        closure_call: helper.site(c.closure_call, work)?,
        stage: closure.site(c.stage, work)?,
        publish: outer.site(c.publish, work)?,
        workgroup_local: outer.local(c.workgroup_local, work)?,
        workgroup_borrows,
        source_binding: checked.source_binding,
        bodies: [
            (outer.canonical, outer.declaration),
            (helper.canonical, helper.declaration),
            (closure.canonical, closure.declaration),
        ],
    })
}
