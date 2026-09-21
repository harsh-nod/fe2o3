#!/usr/bin/env python3
"""Qualify production declarations and content views; not whole-runtime refinement."""
import argparse
from collections import namedtuple
import hashlib
import importlib.util
import json
from pathlib import Path
import signal
import sys

sys.dont_write_bytecode = True
COUNT = 452
SOURCE = 'context_journal_representation_v1.rs'
PRIOR = Path('docs/evidence/dev-shared-settlement-commit-2026-09-21/check.py')
PRIOR_SHA = '200b8843917acec5300d3d3d40dec27f631e85936fddfd290b948ab4909e4190'
CRATE = Path('crates/fe2o3-runtime-model')
PINS = {
    "verus/context_journal_representation_v1.rs": "c17586437d5ce3e7930f15b756a9e4b3dc36fb5c720a2e3ca632adaa0ff1fe66",
    "verus/context_journal_value_views_v1.rs": "7e97ad76b017e6227d843c49ff51524722a98bbda3a07fb29621311dc54ac62c",
    "verus/context_journal_error_views_v1.rs": "f56ec78930b3ac68dcd0c339160ea1ae9c563c10697c51613a8acd364732cd52",
    "verus/context_journal_content_views_v1.rs": "1f6b92aa6cf213caa953b7166705e1b97fa5b2e10835a19d8495178167468c7e",
    "verus/context_journal_view_anchors_v1.rs": "9aeba1b74457b98fb0571e3db024419c363806970338047c38f39dfab1595803",
    "src/context_version_journal/declarations.rs": "de362edd368eda151aa2a0a112cf113af91d42a2fb03259707db77c0581e5c16",
    "src/context_version_journal.rs": "a4aa36076ac7b9e5e693bf9d5b0c6c103cc704ef0618ebcf8ec57f3a551a9b13",
    "src/context_version_journal/declaration_tests.rs": "722d7a8cd71631cf0f02e0f4f340d2f5ea3bc7fe089e4c1fbedfaf60bede53a8"
}
SOURCE_SHA = PINS['verus/' + SOURCE]
Mutation = namedtuple('Mutation', 'name path changes proof_file function postcondition')
CONTROLS = [
    [
        "kind",
        "verus/context_journal_value_views_v1.rs",
        [
            [
                "kind_view",
                "ContextWriterKindV1::Synchronous => logical::WriterKindV1::Synchronous,",
                "ContextWriterKindV1::Synchronous => logical::WriterKindV1::Submission,"
            ]
        ],
        "verus/context_journal_view_anchors_v1.rs",
        "kind_projection_exact",
        "match value {\n        ContextWriterKindV1::Synchronous => kind_view(value) == logical::WriterKindV1::Synchronous,\n        ContextWriterKindV1::Submission => kind_view(value) == logical::WriterKindV1::Submission,\n    }"
    ],
    [
        "reference_slot",
        "verus/context_journal_value_views_v1.rs",
        [
            [
                "writer_reference_view",
                "slot: value.slot,",
                "slot: 0,"
            ]
        ],
        "verus/context_journal_view_anchors_v1.rs",
        "writer_reference_projection_exact",
        "writer_reference_view(value).slot == value.slot"
    ],
    [
        "writer_phase",
        "verus/context_journal_value_views_v1.rs",
        [
            [
                "writer_entry_view",
                "logical::WriterEntryV1::Pending { key: writer_key_view(key), head, count }",
                "logical::WriterEntryV1::Unknown { key: writer_key_view(key), head, count }"
            ]
        ],
        "verus/context_journal_view_anchors_v1.rs",
        "writer_entry_projection_exact",
        "match value {\n        WriterEntryV1::Reserved(key) => writer_entry_view(value)\n            == logical::WriterEntryV1::Reserved(writer_key_view(key)),\n        WriterEntryV1::Pending { key, head, count } => writer_entry_view(value)\n            == logical::WriterEntryV1::Pending { key: writer_key_view(key), head, count },\n        WriterEntryV1::Unknown { key, head, count } => writer_entry_view(value)\n            == logical::WriterEntryV1::Unknown { key: writer_key_view(key), head, count },\n    }"
    ],
    [
        "extent",
        "verus/context_journal_value_views_v1.rs",
        [
            [
                "allocation_entry_view",
                "byte_extent: value.byte_extent,",
                "byte_extent: 0,"
            ]
        ],
        "verus/context_journal_view_anchors_v1.rs",
        "allocation_entry_projection_exact",
        "allocation_entry_view(value).byte_extent == value.byte_extent"
    ],
    [
        "paired_lineages",
        "verus/context_journal_value_views_v1.rs",
        [
            [
                "member_entry_view",
                "prior_lineage: value.prior_lineage,\n        attempt_epoch: value.attempt_epoch,",
                "prior_lineage: value.attempt_epoch,\n        attempt_epoch: value.prior_lineage,"
            ],
            [
                "member_entry_from",
                "prior_lineage: value.prior_lineage,\n        attempt_epoch: value.attempt_epoch,",
                "prior_lineage: value.attempt_epoch,\n        attempt_epoch: value.prior_lineage,"
            ]
        ],
        "verus/context_journal_view_anchors_v1.rs",
        "member_entry_projection_exact",
        "(member_entry_view(value).prior_lineage, member_entry_view(value).attempt_epoch)\n            == (value.prior_lineage, value.attempt_epoch)"
    ],
    [
        "capacity",
        "verus/context_journal_content_views_v1.rs",
        [
            [
                "concrete_contents",
                "allocation_capacity: value.allocation_capacity,",
                "allocation_capacity: value.writer_capacity,"
            ]
        ],
        "verus/context_journal_content_views_v1.rs",
        "journal_projection_exact",
        "journal_view(value).allocation_capacity == value.allocation_capacity"
    ],
    [
        "free_order",
        "verus/context_journal_content_views_v1.rs",
        [
            [
                "concrete_contents",
                "free: value.free@,",
                "free: value.free@.reverse(),"
            ]
        ],
        "verus/context_journal_content_views_v1.rs",
        "journal_projection_exact",
        "journal_view(value).free == value.free@"
    ],
    [
        "scratch_tail",
        "verus/context_journal_content_views_v1.rs",
        [
            [
                "concrete_contents",
                "scratch: value.scratch@,",
                "scratch: value.scratch@.map(|_i, _entry| None),"
            ]
        ],
        "verus/context_journal_content_views_v1.rs",
        "journal_projection_exact",
        "forall|i: int| 0 <= i < value.scratch@.len() ==> #[trigger] journal_view(value).scratch[i]\n            == begin_plan_slot_view(value.scratch@[i])"
    ],
    [
        "allocation_failure",
        "verus/context_journal_error_views_v1.rs",
        [
            [
                "read_error_project",
                "ContextVersionJournalErrorV1::StorageAllocationFailed => None,",
                "ContextVersionJournalErrorV1::StorageAllocationFailed => Some(logical::ReadErrorV1::InvalidState),"
            ]
        ],
        "verus/context_journal_error_views_v1.rs",
        "error_domains_are_exact",
        "(journal_error_project(value).is_none() && read_error_project(value).is_none()\n            && enrollment_error_project(value).is_none())\n            <==> value == ContextVersionJournalErrorV1::StorageAllocationFailed"
    ],
    [
        "paired_errors",
        "verus/context_journal_error_views_v1.rs",
        [
            [
                "journal_error_embed",
                "logical::JournalErrorV1::InvalidReference => ContextVersionJournalErrorV1::InvalidReference,\n        logical::JournalErrorV1::InvalidState => ContextVersionJournalErrorV1::InvalidState,",
                "logical::JournalErrorV1::InvalidReference => ContextVersionJournalErrorV1::InvalidState,\n        logical::JournalErrorV1::InvalidState => ContextVersionJournalErrorV1::InvalidReference,"
            ],
            [
                "journal_error_project",
                "ContextVersionJournalErrorV1::InvalidReference => Some(logical::JournalErrorV1::InvalidReference),\n        ContextVersionJournalErrorV1::InvalidState => Some(logical::JournalErrorV1::InvalidState),",
                "ContextVersionJournalErrorV1::InvalidReference => Some(logical::JournalErrorV1::InvalidState),\n        ContextVersionJournalErrorV1::InvalidState => Some(logical::JournalErrorV1::InvalidReference),"
            ]
        ],
        "verus/context_journal_view_anchors_v1.rs",
        "error_variant_anchors",
        "(journal_error_embed(logical::JournalErrorV1::InvalidReference),\n            journal_error_embed(logical::JournalErrorV1::InvalidState))\n            == (ContextVersionJournalErrorV1::InvalidReference, ContextVersionJournalErrorV1::InvalidState)"
    ],
    [
        "writer_comparison",
        "src/context_version_journal/retained_bodies.rs",
        [
            [
                "macro_rules! retained_writer_key_body",
                "$left.context_generation == $right.context_generation",
                "$left.context_generation == $left.context_generation"
            ]
        ],
        "verus/context_journal_representation_v1.rs",
        "production_writer_key_equal",
        "result == logical::same_key_v1(writer_key_view(left), writer_key_view(right))"
    ],
    [
        "allocation_comparison",
        "src/context_version_journal/retained_bodies.rs",
        [
            [
                "macro_rules! retained_allocation_less_body",
                "$left.context_generation < $right.context_generation",
                "$left.context_generation > $right.context_generation"
            ]
        ],
        "verus/context_journal_representation_v1.rs",
        "production_allocation_less",
        "result == logical::enrollment_key_less_v1(allocation_key_view(left), allocation_key_view(right))"
    ]
]


def need(value, message):
    if not value:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load(repo):
    path = repo / PRIOR
    data = path.read_bytes()
    need(digest(data) == PRIOR_SHA, 'pinned shared commit checker')
    spec = importlib.util.spec_from_file_location('representation_parent', path)
    prior = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = prior
    exec(compile(data, str(path), 'exec'), prior.__dict__)
    module = prior.load(repo)
    module.representation_parent = prior
    module.raw_checker.COUNT = COUNT
    return module


def extra_paths(module):
    return [*module.representation_parent.extra_paths(module), PRIOR, *(CRATE / p for p in PINS)]


def mutations(_base=None):
    return [Mutation(*values) for values in CONTROLS]


def candidate(source, mutation):
    if mutation is None:
        return source
    for function, before, after in mutation.changes:
        anchor = function if function.startswith('macro_rules!') else 'spec fn ' + function + '('
        need(source.count(anchor) == 1, 'unique mutation definition')
        start = source.index(anchor)
        end = source.index('\n}\n', start) + 2
        fragment = source[start:end]
        need(before and before != after and fragment.count(before) == 1, 'unique definition mutation')
        offset = fragment.index(before)
        changed = fragment[:offset] + after + fragment[offset + len(before):]
        need(changed[:offset] + before + changed[offset + len(after):] == fragment, 'reversible mutation')
        source = source[:start] + changed + source[end:]
    return source


def stage(module, snapshots, folder, mutation):
    for path, expected in PINS.items():
        need(digest(snapshots[module.HERE.parent / path]) == expected, 'pinned representation source: ' + path)
    module.representation_parent.stage(module, snapshots, folder, None)
    for path in PINS:
        target = folder / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(snapshots[module.HERE.parent / path])
    policy = module.inherited().POLICY
    for path in PINS:
        if path.startswith('verus/') and path != 'verus/' + SOURCE:
            policy.scan(folder / path)
    if mutation:
        target = folder / mutation.path
        target.write_text(candidate(target.read_text(encoding='ascii'), mutation), encoding='ascii')
    generated = {p.relative_to(folder): p.read_bytes() for p in folder.rglob('*.rs')}
    return (folder / 'verus' / SOURCE).read_text(encoding='ascii'), generated, generated


def solver_command(verus, path, mutation):
    result = ['/usr/bin/timeout', '--foreground', '--signal=TERM', '--kill-after=5', '180', str(verus),
              '--crate-type', 'lib', '--triggers-mode', 'silent', '--no-cheating', '--output-json',
              '--error-format=json', '--num-threads', '4']
    if mutation:
        result.extend(['--verify-only-module', 'production', '--verify-function', mutation.function])
    return [*result, str(path)]


def check_result(module, status, stdout, stderr, source, generated, path, mutation):
    base = module.inherited().BASE
    expected = {'encountered-error': bool(mutation), 'encountered-vir-error': False,
                'success': not bool(mutation), 'verified': 0 if mutation else COUNT,
                'errors': 1 if mutation else 0, 'is-verifying-entire-crate': not bool(mutation)}
    need(type(status) is int and status == (1 if mutation else 0), 'normal expected solver exit')
    if mutation:
        del expected['success']  # Verus omits this field for scoped verification.
    actual = base.unique_json(stdout)['verification-results']
    need(json.dumps(actual, sort_keys=True) == json.dumps(expected, sort_keys=True), 'exact verification result')
    diagnostics = [base.unique_json(line) for line in stderr.splitlines()]
    if mutation is None:
        need(not diagnostics, 'clean whole-crate positive')
        return
    errors = [d for d in diagnostics if d['level'] == 'error']
    notes = [d for d in diagnostics if d['level'] == 'note']
    need(len(diagnostics) == len(errors) + len(notes), 'only intended diagnostics')
    need(len(notes) == 1 and notes[0]['$message_type'] == 'diagnostic' and notes[0]['code'] is None
         and notes[0]['message'] == 'verifying module production (selected functions)'
         and notes[0]['spans'] == [] and notes[0]['children'] == [], 'exact scoped verification note')
    need(len(errors) == 2 and errors[1]['$message_type'] == 'diagnostic' and errors[1]['code'] is None
         and errors[1]['message'] == 'aborting due to 1 previous error'
         and errors[1]['spans'] == [] and errors[1]['children'] == [], 'exact negative footer')
    error = errors[0]
    need(error['$message_type'] == 'diagnostic' and error['message'] == 'postcondition not satisfied'
         and error['code'] is None and error['children'] == [], 'intended postcondition failure')
    proof = generated[Path(mutation.proof_file)].decode('ascii')
    anchor = 'fn ' + mutation.function + '('
    need(proof.count(anchor) == 1, 'unique target proof function')
    start = proof.index(anchor)
    post = proof.index(mutation.postcondition, start)
    need(proof.count(mutation.postcondition, start) == 1, 'unique target postcondition')
    helper = module.retained_parent
    original = path.parent.parent / mutation.proof_file
    fragment = {'kind': 'ContextWriterKindV1::Synchronous',
                'writer_phase': 'WriterEntryV1::Reserved(key)'}.get(mutation.name, mutation.postcondition)
    primary_start = proof.index(fragment, post)
    primary = helper.exact_span(proof, original, primary_start, primary_start + len(fragment), True,
                                'failed this postcondition')
    if mutation.proof_file == 'verus/' + SOURCE:
        shared = generated[Path(mutation.path)].decode('ascii')
        shared_path = path.parent / ('../' + mutation.path)
        macro = mutation.changes[0][0].removeprefix('macro_rules! ')
        definition = shared.index('macro_rules! ' + macro)
        definition_end = definition + len('macro_rules! ' + macro)
        block = shared.index('=> {{', definition) + len('=> {')
        block_end = shared.index('\n    }};', block) + len('\n    }')
        secondary = helper.exact_span(shared, shared_path, block, block_end, False,
                                      'at the end of the function body')
        call = macro + '!(left, right)'
        invocation = proof.index(call, start)
        secondary['expansion'] = {
            'span': helper.exact_span(proof, original, invocation, invocation + len(call), False),
            'macro_decl_name': macro + '!',
            'def_site_span': helper.exact_span(shared, shared_path, definition, definition_end, False),
        }
    else:
        # Empty ghost bodies are reported at the function signature by this pinned compiler.
        secondary = helper.exact_span(proof, original, start, proof.index('\n', start), False,
                                      'at the end of the function body')
    need(len(error['spans']) == 2 and all(type(s['is_primary']) is bool for s in error['spans']),
         'exact typed span roster')
    primaries = [s for s in error['spans'] if s['is_primary'] is True]
    secondaries = [s for s in error['spans'] if s['is_primary'] is False]
    need(json.dumps(primaries, sort_keys=True) == json.dumps([primary], sort_keys=True)
         and json.dumps(secondaries, sort_keys=True) == json.dumps([secondary], sort_keys=True),
         'exact proof postcondition and exit spans')


def report():
    return {'development_checks_passed': True, 'whole_crate_positive_obligations': COUNT,
            'inherited_obligations': 398, 'targeted_negative_controls': 12,
            'negative_verification_scope': 'one named production-module function per control',
            'shared_production_declarations': True, 'lossless_sequence_views': True,
            'derived_trait_refinement': False, 'physical_storage_refinement': False,
            'full_runtime_refinement': False, 'native_or_performance_acceptance': False}

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--verus', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    repo, verus = args.repo.resolve(), args.verus.resolve()
    module = load(repo)
    inherited = module.inherited()
    base, here = inherited.BASE, module.HERE
    for signum in base.SIGNALS:
        signal.signal(signum, base.interrupted)
    signal.pthread_sigmask(signal.SIG_UNBLOCK, base.SIGNALS)
    pins = {here / name: pin for name, pin in module.DEPENDENCIES.items()}
    pins[here / 'context_version_journal_enrollment_v1.rs'] = 'CONTEXT_VERSION_JOURNAL_ENROLLMENT_SHA256'
    pins[verus] = 'VERUS_SHA256'
    pins[here / 'pins/VERUS_CLOSURE_MANIFEST'] = 'VERUS_CLOSURE_MANIFEST_SHA256'
    for helper, pin in [(module, 'JOURNAL_ENROLLMENT_CHECKER_SHA256'), (inherited, 'PRODUCER_JOURNAL_ISSUANCE_CHECKER_SHA256'),
        (inherited.PRODUCER, 'PRODUCER_READ_INVARIANT_CHECKER_SHA256'), (inherited.INVARIANT, 'READ_INVARIANT_CHECKER_SHA256'),
        (inherited.COMMIT, 'READ_COMMIT_CHECKER_SHA256'), (inherited.PREFLIGHT, 'READ_PREFLIGHT_CHECKER_SHA256'),
        (base, 'JOURNAL_ISSUANCE_CHECKER_SHA256'), (inherited.POLICY, 'PROOF_SOURCE_CHECKER_SHA256')]:
        pins[Path(helper.__file__)] = pin
    snapshots = inherited.PRODUCER.pinned_snapshots(pins)
    for path in [*(repo / p for p in extra_paths(module)), Path(__file__).resolve(), verus.parent / 'rust_verify', verus.parent / 'z3']:
        snapshots[path] = path.read_bytes()
    need(digest(snapshots[here / SOURCE]) == SOURCE_SHA, 'exact representation root')
    closure = repo / 'examples/row_softmax_v1/verify-verus-closure.sh'
    snapshots[closure] = closure.read_bytes()
    need(digest(snapshots[closure]) == 'c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c', 'closure checker')
    identities = {str(p): digest(data) for p, data in snapshots.items()}
    output = args.output.resolve()
    output.mkdir()
    (output / 'inputs-before.json').write_text(json.dumps(identities, indent=2) + '\n')
    env = {'HOME': '/home/harsh', 'PATH': '/home/harsh/.cargo/bin:/usr/bin:/bin',
           'RUSTUP_HOME': '/home/harsh/.rustup', 'CARGO_HOME': '/home/harsh/.cargo', 'VERUS_Z3_PATH': str(verus.parent / 'z3')}
    (output / 'environment.json').write_text(json.dumps(env, indent=2) + '\n')
    command = ['/bin/sh', str(closure), str(verus.parent), str(here / 'pins/VERUS_CLOSURE_MANIFEST')]
    need(base.run_owned(command, 120, output / 'closure-before', env)[0] == 0, 'closure before')
    cases = [('positive_before', None), *[(m.name, m) for m in mutations(base)], ('positive_after', None)]
    completed = {}
    for name, mutation in cases:
        case = output / name
        case.mkdir()
        source, body, generated = stage(module, snapshots, case, mutation)
        generated_hashes = {str(case / p): digest(data) for p, data in generated.items()}
        (case / 'sources.json').write_text(json.dumps(generated_hashes, indent=2) + '\n')
        need(all(digest(Path(p).read_bytes()) == h for p, h in identities.items()), 'input drift before solver')
        path = case / 'verus' / SOURCE
        cmd = solver_command(verus, path, mutation)
        status, stdout, stderr = base.run_owned(cmd, 190, case / 'solver', env)
        need(all(digest(Path(p).read_bytes()) == h for p, h in generated_hashes.items()), 'generated source drift')
        need(all(digest(Path(p).read_bytes()) == h for p, h in identities.items()), 'input drift after solver')
        check_result(module, status, stdout, stderr, source, body, path, mutation)
        completed[name] = {str(p): digest(p.read_bytes()) for p in case.rglob('*') if p.is_file()}
        print(name + ': PASS', flush=True)
    need(base.run_owned(command, 120, output / 'closure-after', env)[0] == 0, 'closure after')
    after = {p: digest(Path(p).read_bytes()) for p in identities}
    need(after == identities, 'final identities')
    for name, frozen in completed.items():
        need({str(p): digest(p.read_bytes()) for p in (output / name).rglob('*') if p.is_file()} == frozen, 'completed case drift')
    (output / 'completed-cases.json').write_text(json.dumps(completed, indent=2) + '\n')
    (output / 'inputs-after.json').write_text(json.dumps(after, indent=2) + '\n')
    (output / 'report.json').write_text(json.dumps(report(), indent=2) + '\n')
    print(json.dumps(report()), flush=True)


if __name__ == '__main__':
    main()
