// This index selects candidates only. State::equivalent remains the authority.
#[derive(Clone, Copy, Default)]
struct SourceStorageSnapshotBucketV29 {
    first: Option<usize>,
    last: Option<usize>,
}

#[derive(Default)]
struct SourceStorageSnapshotIndexV29 {
    objects: Vec<Vec<SourceStorageSnapshotBucketV29>>,
    next: Vec<Option<usize>>,
}

impl SourceStorageSnapshotIndexV29 {
    fn prepare_object(
        &mut self,
        arena: &SourceStorageRootArenaV29<'_, '_>,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<SourceStorageSnapshotBucketV29, Error> {
        let lease = &arena.layouts.lease;
        lease.work(4, budget)?;
        let count = arena.instances.instances().len();
        let declaration = arena
            .instances
            .instance(instance)
            .ok_or_else(|| error("source snapshot index changed its original instance"))?
            .declaration();
        let locals = declaration.locals().len();
        if local.index() as usize >= locals {
            return Err(error("source snapshot index changed its original local"));
        }
        if self.objects.is_empty() {
            let mut objects = lease.vector(count, budget)?;
            lease.work(count, budget)?;
            objects.resize_with(count, Vec::new);
            self.objects = objects;
        }
        if self.objects.len() != count {
            return Err(error(
                "source snapshot index changed its original instance roster",
            ));
        }
        let objects = &mut self.objects[instance.index()];
        if objects.is_empty() {
            let mut rows = lease.vector(locals, budget)?;
            lease.work(locals, budget)?;
            rows.resize(locals, SourceStorageSnapshotBucketV29::default());
            *objects = rows;
        }
        if objects.len() != locals {
            return Err(error(
                "source snapshot index changed its original local roster",
            ));
        }
        Ok(objects[local.index() as usize])
    }
}
