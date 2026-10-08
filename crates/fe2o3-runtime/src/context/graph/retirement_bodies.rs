// Shared executable retirement gate and its concrete retained-owner scans.

// The proof calls verus_exec_expr instead; native callers use this identity.
#[allow(unused_macros)]
macro_rules! graph_retirement_runtime_expr {
    ($expression:expr) => {
        $expression
    };
}

macro_rules! graph_reservation_retirement_body_v1 {
    ($syntax:ident, $context:ident, $token:ident,
        ($stream:ident, $values:ident, $found:ident),
        [$($closure_spec:tt)*], [$($before_scan:tt)*], [$($after_scan:tt)*]) => {
        $syntax!({
        if !fe2o3_runtime_model::r63_graph_can_release_v1(
            $context.terminal,
            $context.graph_reservation == Some($token),
            $context.graph_issue_closed,
            $context.submissions.len(),
            $context.events.len(),
        ) || $context.has_unpublished_holds_v1()
            || $context.pending_replicas_v1() != 0
            || !$context.generated_issues.is_empty()
            || {
                let mut $values = $context.streams.values();
                $($before_scan)*
                let $found = $values.any(|$stream| $($closure_spec)* { $stream.generated.is_some() });
                $($after_scan)*
                $found
            }
            || $context.completion_callback_count != 0
            || !$context.backend_submissions.is_empty()
            || !$context.backend_events.is_empty()
        {
            return Err(RuntimeValidationErrorV1::SubmissionPending);
        }
        $context.graph_reservation = None;
        Ok(())
        })
    };
}

macro_rules! graph_unpublished_holds_body_v1 {
    ($syntax:ident, $context:ident, ($record:ident, $values:ident, $found:ident),
        [$($closure_spec:tt)*], [$($before_scan:tt)*], [$($after_scan:tt)*]) => {
        $syntax!({
        let mut $values = $context.streams.values();
        $($before_scan)*
        let $found = $values.any(|$record| $($closure_spec)* { $record.unpublished.is_some() });
        $($after_scan)*
        $found
        })
    };
}

macro_rules! graph_pending_replicas_body_v1 {
    ($syntax:ident, $context:ident) => {
        $syntax!({
            match &$context.replicas {
                None => 0,
                Some(table) => table.usage().pending,
            }
        })
    };
}

macro_rules! graph_replica_usage_body_v1 {
    ($syntax:ident, $table:ident, ($index:ident, $pending:ident, $settled:ident, $count:ident),
        [$($pending_invariants:tt)*], [$($settled_invariants:tt)*]) => {
        $syntax!({
        let $count = $table.slots.len();
        let mut $index = 0usize;
        let mut $pending = 0usize;
        while $index < $count
            $($pending_invariants)*
        {
            if matches!($table.slots[$index].state, ReplicaStateV1::Pending(_)) {
                $pending += 1;
            }
            $index += 1;
        }
        $index = 0;
        let mut $settled = 0usize;
        while $index < $count
            $($settled_invariants)*
        {
            if matches!($table.slots[$index].state, ReplicaStateV1::Settled(_)) {
                $settled += 1;
            }
            $index += 1;
        }
        RuntimeReplicaUsageV1 {
            capacity: $count,
            pending: $pending,
            settled: $settled,
        }
        })
    };
}

pub(crate) use graph_pending_replicas_body_v1;
pub(crate) use graph_replica_usage_body_v1;
pub(crate) use graph_reservation_retirement_body_v1;
pub(crate) use graph_retirement_runtime_expr;
pub(crate) use graph_unpublished_holds_body_v1;
