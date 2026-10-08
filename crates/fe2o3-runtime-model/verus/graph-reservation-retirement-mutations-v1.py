"""Mutate only the actual shared executable suffix; never its specification."""


def mutations(body):
    cases = {}

    def add(name, method, before, after):
        marker = 'macro_rules! graph_' + method + '_body_v1'
        start = body.index(marker)
        end = body.find('macro_rules!', start + len(marker))
        end = len(body) if end < 0 else end
        fragment = body[start:end]
        if fragment.count(before) != 1:
            raise ValueError('one executable mutation site: ' + name)
        revised = body[:start] + fragment.replace(before, after) + body[end:]
        cases[name] = dict(method=method, before=before, after=after, text=revised)

    gate = 'reservation_retirement'
    for name, before, after in (
        ('terminal-ignored', '$context.terminal,', 'false,'),
        ('exact-token-ignored', '$context.graph_reservation == Some($token),', 'true,'),
        ('issue-close-ignored', '$context.graph_issue_closed,', 'true,'),
        ('submissions-ignored', '$context.submissions.len(),', '0,'),
        ('events-ignored', '$context.events.len(),', '0,'),
        ('unpublished-holds-ignored', '$context.has_unpublished_holds_v1()', 'false'),
        ('pending-replicas-ignored', '$context.pending_replicas_v1() != 0', 'false'),
        ('generated-issues-ignored', '!$context.generated_issues.is_empty()', 'false'),
        ('generated-streams-ignored', '$($after_scan)*\n                $found', '$($after_scan)*\n                false'),
        ('completion-callbacks-ignored', '$context.completion_callback_count != 0', 'false'),
        ('backend-submissions-ignored', '!$context.backend_submissions.is_empty()', 'false'),
        ('backend-events-ignored', '!$context.backend_events.is_empty()', 'false'),
        ('refusal-clears-reservation', 'return Err(RuntimeValidationErrorV1::SubmissionPending);',
         '$context.graph_reservation = None; return Err(RuntimeValidationErrorV1::SubmissionPending);'),
        ('wrong-refusal-error', 'Err(RuntimeValidationErrorV1::SubmissionPending)',
         'Err(RuntimeValidationErrorV1::ContextReserved)'),
        ('success-retains-reservation', '$context.graph_reservation = None;', 'let _ = &$context.graph_reservation;'),
        ('success-mutates-owner-frame', '$context.graph_reservation = None;',
         '$context.graph_reservation = None; $context.next_identity = 0;'),
    ):
        add(name, gate, before, after)
    add('unpublished-scan-ignored', 'unpublished_holds',
        '$($after_scan)*\n        $found', '$($after_scan)*\n        false')
    add('optional-pending-table-ignored', 'pending_replicas',
        'Some(table) => table.usage().pending,', 'Some(_table) => 0,')
    add('pending-count-wrong-state', 'replica_usage',
        'ReplicaStateV1::Pending(_)', 'ReplicaStateV1::Settled(_)')
    add('settled-count-wrong-state', 'replica_usage',
        'ReplicaStateV1::Settled(_)', 'ReplicaStateV1::Vacant')
    add('pending-count-not-incremented', 'replica_usage', '$pending += 1;', '$pending += 0;')
    add('settled-count-not-incremented', 'replica_usage', '$settled += 1;', '$settled += 0;')
    add('table-capacity-substituted', 'replica_usage', 'capacity: $count,', 'capacity: 0,')
    if len(cases) != 23 or len({row['text'] for row in cases.values()}) != 23:
        raise ValueError('exact 23 distinct executable mutations')
    return cases
