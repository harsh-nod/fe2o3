#!/usr/bin/env python3
"""Read-only field binding controls; no compiler, test subprocess or verifier."""
import importlib.util
from pathlib import Path
import types

path = Path(__file__).with_name('check-distributed-codec-fields.py')
spec = importlib.util.spec_from_file_location('codec_fields_source_controls', path)
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)


def refused(call):
    try:
        call()
    except (ValueError, KeyError):
        return
    raise AssertionError('expected strict refusal')


def main():
    sources = c.snapshot()
    c.audit(sources)
    assert len(c.FILES) == 9 and c.EXPECTED_VERIFIED == 63
    for key in ('isolated', 'dont_write_bytecode'):
        flags = dict(isolated=1, dont_write_bytecode=1, optimize=0)
        flags[key] = 0
        refused(lambda: c.invocation_guard(types.SimpleNamespace(**flags)))
    refused(lambda: c.invocation_guard(types.SimpleNamespace(isolated=1, dont_write_bytecode=1, optimize=1)))
    for path in c.PINS:
        changed = dict(sources)
        changed[path] += '\n'
        refused(lambda: c.audit(changed))
    for path in (c.PROOF, c.BODY, c.OWNER):
        changed = dict(sources)
        del changed[path]
        refused(lambda: c.audit(changed))
    changed = dict(sources)
    changed[c.SRC / 'unreviewed.rs'] = ''
    refused(lambda: c.audit(changed))
    for paths in ((c.OWNER,), (c.PROOF,), (c.OWNER, c.PROOF)):
        changed = dict(sources)
        for path in paths:
            changed[path] = changed[path].replace('SCHEMA_V1: u16 = 1;', 'SCHEMA_V1: u16 = 2;')
        refused(lambda: c.structural(changed))
    for name in c.KINDS:
        changed = dict(sources)
        before = 'codec_fields::' + name
        assert sources[c.OWNER].count(before) == 1
        changed[c.OWNER] = changed[c.OWNER].replace(before, 'other::' + name)
        refused(lambda: c.structural(changed))
        for path in (c.NATIVE, c.PROOF, c.BODY):
            changed = dict(sources)
            changed[path] = changed[path].replace('distributed_codec_' + name + '_body_v1', 'proof_only_replacement')
            refused(lambda: c.structural(changed))
    for path, before, after in (
        (c.OWNER, 'codec_fields::write_header(self.bytes', 'codec_fields::write_header(other.bytes'),
        (c.PROOF, 'fn read_header(', '#[verifier::external_body]\nfn read_header('),
        (c.PROOF, '    distributed_codec_read_header_body_v1!', '    assume(false);\n    distributed_codec_read_header_body_v1!'),
        (c.PROOF, 'include!("distributed_codec_primitives_v1.rs");', 'include!("trusted_adapter.rs");'),
        (c.NATIVE, 'use super::codec_primitives::', 'use proof_only_primitives::'),
        (c.BODY, 'if !bytes_equal(actual_domain, $domain) {', 'if actual_domain != $domain {'),
        (c.BODY, 'if reserved[0] != 0 || reserved[1] != 0 {', 'if reserved != [0; 2] {'),
        (c.NATIVE, 'left, right, index, []', 'left, left, index, []'),
    ):
        changed = dict(sources)
        assert before in changed[path]
        changed[path] = changed[path].replace(before, after)
        refused(lambda: c.structural(changed))
    for path in (c.NATIVE, c.PROOF, c.BODY):
        changed = dict(sources)
        changed[path] = changed[path].replace('distributed_codec_bytes_equal_body_v1', 'proof_only_equality')
        refused(lambda: c.structural(changed))
    calibration, roster, buffers = c.verification_metadata()
    assert len(calibration['cases']) == len(roster['logical_candidates']) == 34
    assert calibration['source_capture_accepted_negative_classifications'] == calibration['source_capture_qualified_kills'] == 0
    classifier = c.field_classifier()
    assert len(classifier.RECOMMENDATION_CASES) == 2
    assert classifier.VSTD_SEQ_SHA == calibration['vstd_seq_sha256']
    original_pins = dict(c.VERIFICATION_PINS)
    for path in original_pins:
        try:
            c.VERIFICATION_PINS[path] = '0' * 64
            refused(c.verification_metadata)
        finally:
            c.VERIFICATION_PINS[path] = original_pins[path]
    mutants = c.mutations(sources[c.BODY])
    assert set(mutants) == set(calibration['cases'])
    for name, (body, selector) in mutants.items():
        assert c.sha(body) == calibration['cases'][name]['body_sha256']
        assert selector == calibration['cases'][name]['selector']
    refused(lambda: c.mutations(sources[c.BODY] + '\n'))
    print('PASS: codec field source controls (nine groups; 34 calibrated mutants constructed; no compiler or verifier execution)')


if __name__ == '__main__':
    main()
