"""Closed executable-body mutations; theorem contracts remain unchanged."""


def mutations(body):
    cases = {}

    def add(name, method, before, after):
        marker = 'macro_rules! graph_version_' + method + '_body_v1'
        start = body.index(marker)
        end = body.find('macro_rules!', start + len(marker))
        end = len(body) if end < 0 else end
        fragment = body[start:end]
        if fragment.count(before) != 1:
            raise ValueError('one executable mutation site: ' + name)
        revised = body[:start] + fragment.replace(before, after) + body[end:]
        cases[name] = dict(method=method, before=before, after=after, text=revised)

    add('begin-input-refusal-ignored', 'begin',
        '$ledger.current[usage.segment], input, $ledger.records[input].state,\n                    ) {',
        '$ledger.current[usage.segment], input, $ledger.records[input].state,\n                    ) && false {')
    add('begin-planned-phase-ignored', 'begin', '$ledger.records[output].state, $ledger.pending[usage.segment],',
        'RuntimeGraphVersionStateV1::Planned, $ledger.pending[usage.segment],')
    add('begin-pending-owner-ignored', 'begin', '$ledger.records[output].state, $ledger.pending[usage.segment],',
        '$ledger.records[output].state, None,')
    add('begin-predecessor-ignored', 'begin', '$ledger.current[usage.segment], prior,', 'Some(prior), prior,')
    add('begin-repeated-accepted', 'begin', 'if $ledger.started[$node] {', 'if $ledger.started[$node] && false {')
    add('begin-tail-unchecked', 'begin', 'while $check < $count', 'while $check < $count && $check < 1')
    add('begin-current-retained', 'begin', '$ledger.current[usage.segment] = None;', 'let _ = $ledger.current[usage.segment];')
    add('begin-pending-dropped', 'begin', '$ledger.pending[usage.segment] = Some(output);', '$ledger.pending[usage.segment] = None;')
    add('begin-premature-commit', 'begin', '$ledger.records[output].state = RuntimeGraphVersionStateV1::InFlight;',
        '$ledger.records[output].state = RuntimeGraphVersionStateV1::Committed;')
    add('begin-refusal-mutates-started', 'begin', 'let usage = &$ledger.uses[$node][$check];',
        '$ledger.started[$node] = true;\n                let usage = &$ledger.uses[$node][$check];')
    add('commit-flight-phase-ignored', 'commit', '$ledger.records[output].state, $ledger.pending[usage.segment],',
        'RuntimeGraphVersionStateV1::InFlight, $ledger.pending[usage.segment],')
    add('commit-pending-owner-ignored', 'commit', '$ledger.records[output].state, $ledger.pending[usage.segment],',
        '$ledger.records[output].state, Some(output),')
    add('commit-invalidated-current-ignored', 'commit', 'output, $ledger.current[usage.segment],', 'output, None,')
    add('commit-tail-unchecked', 'commit', 'while $check < $count', 'while $check < $count && $check < 1')
    add('commit-phase-unsettled', 'commit', '$ledger.records[output].state = RuntimeGraphVersionStateV1::Committed;',
        '$ledger.records[output].state = RuntimeGraphVersionStateV1::InFlight;')
    add('commit-current-retained', 'commit', '$ledger.current[usage.segment] = Some(output);',
        'let _ = $ledger.current[usage.segment];')
    add('commit-pending-retained', 'commit', '$ledger.pending[usage.segment] = None;',
        'let _ = $ledger.pending[usage.segment];')
    add('commit-refusal-mutates-started', 'commit', 'let usage = &$ledger.uses[$node][$check];',
        '$ledger.started[$node] = true;\n                let usage = &$ledger.uses[$node][$check];')
    if len(cases) != 18 or len({row['text'] for row in cases.values()}) != 18:
        raise ValueError('exact eighteen distinct executable mutations')
    return cases
