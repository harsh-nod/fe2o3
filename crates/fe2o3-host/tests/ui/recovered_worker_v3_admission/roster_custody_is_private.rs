use fe2o3_host::RecoveredWorkerV3PinnedRosterV1;

fn decompose<R>(value: RecoveredWorkerV3PinnedRosterV1<R>) {
    let RecoveredWorkerV3PinnedRosterV1 {
        artifact: _artifact,
        entrypoints: _entrypoints,
        lineage: _lineage,
        _roster,
    } = value;
}

fn main() {}
