use std::hint::black_box;
use std::time::{Duration, Instant};

use fe2o3_completion::{
    CompletionGraphV1, CompletionNodeIdV1, CompletionNodeV1, ContextIdentityV1, DeviceIdentityV1,
    FutureIdentityV1, MAX_COMPLETION_GRAPH_NODES_V1, StreamIdentityV1,
};

fn bytes(value: u8) -> [u8; 32] {
    [value; 32]
}

fn indexed_bytes(value: u32) -> [u8; 32] {
    let mut identity = [0; 32];
    identity[..4].copy_from_slice(&value.to_le_bytes());
    identity
}

fn node(value: u32) -> CompletionNodeIdV1 {
    CompletionNodeIdV1::new(value).expect("benchmark node identity is nonzero")
}

fn chain(node_count: usize) -> CompletionGraphV1 {
    let context = ContextIdentityV1::new(DeviceIdentityV1::from_bytes(bytes(1)), bytes(2));
    let stream = StreamIdentityV1::new(context, bytes(3));
    let nodes = (1..=node_count as u32)
        .map(|value| {
            CompletionNodeV1::future(
                node(value),
                FutureIdentityV1::new(stream, indexed_bytes(value)),
                (value > 1).then(|| node(value - 1)),
            )
        })
        .collect();
    CompletionGraphV1::new(context, vec![stream], nodes).expect("benchmark graph is valid")
}

fn measure(node_count: usize) -> (Duration, Duration) {
    let construction_started = Instant::now();
    let mut authority = chain(node_count).into_completion_authority();
    let construction = construction_started.elapsed();

    let transitions_started = Instant::now();
    for value in 1..=node_count as u32 {
        // SAFETY: The benchmark supplies the model's exact next ready node.
        unsafe { authority.mark_succeeded(node(value)) }.expect("benchmark transition succeeds");
    }
    black_box(authority.is_terminal());
    (construction, transitions_started.elapsed())
}

fn main() {
    println!("nodes,construction_ns,transitions_ns,transition_ns_per_node");
    for node_count in [1_024, 4_096, 16_384, MAX_COMPLETION_GRAPH_NODES_V1] {
        let (construction, transitions) = measure(node_count);
        println!(
            "{node_count},{},{},{:.2}",
            construction.as_nanos(),
            transitions.as_nanos(),
            transitions.as_nanos() as f64 / node_count as f64,
        );
    }
}
