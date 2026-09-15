//! Each node enters the bounded ready queue once. Every original successor,
//! including duplicate targets and unwind edges, contributes to its degree.
use super::{PhaseResult, rejected, reserve, spend};

pub(super) trait Graph {
    fn node_count(&self) -> usize;
    fn successors(&self, node: usize) -> impl Iterator<Item = usize>;
}

pub(super) fn check(graph: &impl Graph, work: &mut usize) -> PhaseResult<()> {
    let n = graph.node_count();
    spend(work, 1)?;
    let mut degrees = reserve(n, work)?;
    degrees.resize(n, 0usize);
    let mut ready = reserve(n, work)?;
    for node in 0..n {
        spend(work, 1)?;
        for target in graph.successors(node) {
            spend(work, 1)?;
            let degree = degrees.get_mut(target)
                .ok_or_else(|| rejected("phase original CFG target"))?;
            *degree = degree.checked_add(1)
                .ok_or_else(|| rejected("phase CFG degree overflow"))?;
        }
    }
    for (node, degree) in degrees.iter().enumerate() {
        spend(work, 1)?;
        if *degree == 0 { push(&mut ready, node, n)?; }
    }
    let mut cursor = 0;
    while cursor < ready.len() {
        spend(work, 1)?;
        let node = ready[cursor];
        cursor += 1;
        for target in graph.successors(node) {
            spend(work, 1)?;
            let degree = degrees.get_mut(target)
                .ok_or_else(|| rejected("phase original CFG target"))?;
            *degree = degree.checked_sub(1)
                .ok_or_else(|| rejected("phase original CFG degree mismatch"))?;
            if *degree == 0 { push(&mut ready, target, n)?; }
        }
    }
    if cursor != n {
        return Err(rejected("phase first protocol slice requires acyclic original control"));
    }
    Ok(())
}

fn push(queue: &mut Vec<usize>, node: usize, limit: usize) -> PhaseResult<()> {
    if queue.len() >= limit || queue.len() == queue.capacity() {
        return Err(rejected("phase original CFG ready queue bound"));
    }
    queue.push(node);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Edges(Vec<Vec<usize>>);
    impl Graph for Edges {
        fn node_count(&self) -> usize { self.0.len() }
        fn successors(&self, node: usize) -> impl Iterator<Item=usize> { self.0[node].iter().copied() }
    }
    #[test]
    fn ready_queue_keeps_duplicate_successor_degree_exact() {
        check(&Edges(vec![vec![1,1,2],vec![3],vec![3],vec![]]), &mut 1_000).unwrap();
    }
    #[test]
    fn ready_queue_checks_disconnected_components_and_cycles() {
        check(&Edges(vec![vec![1],vec![],vec![3],vec![]]), &mut 1_000).unwrap();
        let error=check(&Edges(vec![vec![1],vec![],vec![3],vec![2]]), &mut 1_000).unwrap_err();
        assert_eq!(format!("{error:?}"),format!("{:?}",rejected("phase first protocol slice requires acyclic original control")));
    }
    #[test]
    fn ready_queue_rejects_out_of_range_edges_and_self_cycles() {
        let error=check(&Edges(vec![vec![1]]), &mut 1_000).unwrap_err();
        assert_eq!(format!("{error:?}"),format!("{:?}",rejected("phase original CFG target")));
        let error=check(&Edges(vec![vec![0]]), &mut 1_000).unwrap_err();
        assert_eq!(format!("{error:?}"),format!("{:?}",rejected("phase first protocol slice requires acyclic original control")));
    }
    #[test]
    fn ready_queue_work_is_linear_for_reverse_numbered_chain() {
        for n in [128,256,512,1024] {
            let edges=Edges((0..n).map(|i| if i==0 {vec![]} else {vec![i-1]}).collect());
            let mut work=20*n;
            check(&edges,&mut work).unwrap();
            assert!(20*n-work<=8*n+16,"n={n}, charged={}",20*n-work);
        }
    }
    #[test]
    fn ready_queue_shares_original_storage_and_work_ceiling() {
        let edges=Edges(vec![vec![1,2],vec![3],vec![3],vec![]]);
        let mut work=1_000;check(&edges,&mut work).unwrap();let used=1_000-work;
        check(&edges,&mut used.clone()).unwrap();
        let error=check(&edges,&mut (used-1)).unwrap_err();
        assert_eq!(format!("{error:?}"),format!("{:?}",rejected("phase live source-work ceiling")));
    }
}
