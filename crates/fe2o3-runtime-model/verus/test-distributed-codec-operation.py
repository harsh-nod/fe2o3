#!/usr/bin/env python3
"""Read-only whole-operation source controls; no compiler or verifier process."""
import importlib.util
import copy
from pathlib import Path
import types

path = Path(__file__).with_name('check-distributed-codec-operation.py')
spec = importlib.util.spec_from_file_location('codec_operation_source_controls', path)
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
    assert len(c.FILES) == 11 and c.EXPECTED_VERIFIED == 99
    for key in ('isolated', 'dont_write_bytecode'):
        flags = dict(isolated=1, dont_write_bytecode=1, optimize=0)
        flags[key] = 0
        refused(lambda: c.invocation_guard(types.SimpleNamespace(**flags)))
    refused(lambda: c.invocation_guard(types.SimpleNamespace(isolated=1, dont_write_bytecode=1, optimize=1)))
    for path in c.PINS:
        changed = dict(sources)
        changed[path] += '\n'
        refused(lambda: c.audit(changed))
    for path in (c.OWNER, c.IDENTITY, c.BODY, c.NATIVE, c.PROOF):
        changed = dict(sources)
        del changed[path]
        refused(lambda: c.audit(changed))
    changed = dict(sources)
    changed[c.SRC / 'unreviewed.rs'] = ''
    refused(lambda: c.audit(changed))
    for kind in c.KINDS:
        for path in (c.NATIVE, c.PROOF, c.BODY):
            changed = dict(sources)
            before = 'distributed_codec_' + kind + '_body_v1'
            assert changed[path].count(before) == 1
            changed[path] = changed[path].replace(before, 'proof_only_replacement')
            refused(lambda: c.structural(changed))
    for name in ('write_digest', 'read_digest', 'encode', 'decode'):
        changed = dict(sources)
        before = 'pub(super) fn ' + name + '('
        assert sources[c.NATIVE].count(before) == 1
        changed[c.NATIVE] = changed[c.NATIVE].replace(before, 'pub(super) fn unrelated(')
        refused(lambda: c.structural(changed))
    edges = (
        (c.OWNER, 'codec_operation::encode(self)', 'other::encode(self)'),
        (c.OWNER, 'codec_operation::decode(bytes)', 'other::decode(bytes)'),
        (c.NATIVE, 'use super::codec_fields::', 'use proof_only_fields::'),
        (c.NATIVE, 'use super::codec_primitives::', 'use proof_only_primitives::'),
        (c.NATIVE, 'debug_assert_eq,', 'drop_predicate,'),
        (c.PROOF, 'include!("distributed_codec_fields_v1.rs");', 'include!("trusted_fields.rs");'),
        (c.PROOF, 'fn decode(', '#[verifier::external_body]\nfn decode('),
        (c.PROOF, '    distributed_codec_decode_operation_body_v1!', '    assume(false);\n    distributed_codec_decode_operation_body_v1!'),
        (c.PROOF, 'assert($left == $right)', 'assert(true)'),
        (c.BODY, '$check!(offset, DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1);', '$check!(offset, offset);'),
        (c.OWNER, 'Self(digest)', 'Self(IdentityDigestV1::from_untrusted_bytes([0; 32]))'),
        (c.IDENTITY, 'Self(digest)', 'Self(IdentityDigestV1::from_untrusted_bytes([0; 32]))'),
        (c.IDENTITY, '&self.0', '&[0; IDENTITY_DIGEST_BYTES_V1]'),
        (c.PROOF, '{ Self(digest) }', '{ Self(IdentityDigestV1([0; 32])) }'),
        (c.PROOF, '{ &self.0 }', '{ &[0; 32] }'),
    )
    for path, before, after in edges:
        changed = dict(sources)
        assert before in changed[path]
        changed[path] = changed[path].replace(before, after)
        refused(lambda: c.structural(changed))
    for paths in ((c.OWNER,), (c.PROOF,), (c.OWNER, c.PROOF)):
        changed = dict(sources)
        for path in paths:
            changed[path] = changed[path].replace('11 * 32 + 4 * 8', '11 * 32 + 3 * 8')
        refused(lambda: c.structural(changed))
    for before, after in (
        ("DOMAIN_V1: &'static [u8; 43]", "DOMAIN_V1: &'other [u8; 43]"),
        ("DOMAIN_V1: &'static [u8; 43]", "DOMAIN_V1: &'static [u8]"),
        ("DOMAIN_V1: &'static [u8; 43]", "DOMAIN_V1: &'static [u8; 42]"),
        ("DOMAIN_V1: &'static [u8; 43]", "DOMAIN_V1: &'static [u8; 44]"),
        ('43 + 4 + COORDINATE_BYTES;', '42 + 4 + COORDINATE_BYTES;'),
        ('43 + 4 + COORDINATE_BYTES;', 'DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1.len() + 4 + COORDINATE_BYTES;'),
        ('fn operation_roundtrip(', 'fn unrelated_roundtrip('),
        ('fn successful_decode_canonicality(', 'fn unrelated_canonicality('),
        ('let bytes = encode(binding);', 'let bytes = [0; DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1];'),
        ('decode(&bytes)', 'Ok(binding)'),
        ('let binding = decode(bytes)?;', 'let binding = unverified_decode(bytes)?;'),
        ('Ok(encode(binding))', 'Ok(unverified_encode(binding))'),
        ('wire@ == bytes@ && operation_decision(bytes@).is_ok()', 'true'),
    ):
        changed = dict(sources)
        assert before in changed[c.PROOF]
        changed[c.PROOF] = changed[c.PROOF].replace(before, after)
        refused(lambda: c.structural(changed))
    for paths in ((c.OWNER,), (c.PROOF,), (c.OWNER, c.PROOF)):
        changed = dict(sources)
        for path in paths:
            before = 'b"FE2O3/DISTRIBUTED-OPERATION-DESCRIPTION/V1\\0"'
            assert changed[path].count(before) == 1
            changed[path] = changed[path].replace(before, before[:-1] + '\\0"')
        refused(lambda: c.structural(changed))
    calibration, roster, buffers = c.verification_metadata()
    classifier = c.operation_classifier()
    mutants = c.mutations(sources[c.BODY])
    assert len(mutants) == 40 and classifier.SELECTORS == c.SELECTORS
    for name, (body, selector) in mutants.items():
        case = roster['logical_candidates'][name]
        assert c.sha(body) == calibration['cases'][name]['body_sha256']
        assert selector == c.SELECTORS[case['family']]
        changed = {str(path): sources[path].encode() for path in c.FILES}
        changed[str(c.BODY)] = body.encode()
        context = classifier.context(changed, str(c.PROOF), str(c.BODY), case['family'])
        assert context['family'] == case['family']
        for key, value in (('before', ''), ('after', case['before']), ('body_sha256', '0' * 64),
                           ('family', 'unrelated'), ('selector', 'unrelated')):
            modified = copy.deepcopy(roster)
            modified['logical_candidates'][name][key] = value
            refused(lambda: c.construct_mutations(sources[c.BODY], modified))
        for key, value in (('qualified_kill', True), ('negative_classification_accepted', True),
                           ('body_sha256', '0' * 64), ('family', 'unrelated'), ('selector', 'unrelated')):
            modified = copy.deepcopy(roster)
            modified['logical_candidates'][name][key] = value
            refused(lambda: c.metadata_checks(calibration, modified))
    for key, value in (('body_sha256', '0' * 64), ('negative_count_qualified', 40),
                       ('negative_count_qualified', False), ('compiler_or_solver_executed', True),
                       ('expected_full_positive_verified', 98), ('equivalent_omissions', {})):
        modified = copy.deepcopy(roster)
        modified[key] = value
        refused(lambda: c.metadata_checks(calibration, modified))
    for key in ('source_capture_accepted_negative_classifications', 'source_capture_qualified_kills',
                'qualified_kills', 'fresh_verifier_executions', 'new_cpu_executions'):
        for value in (1, False):
            modified = copy.deepcopy(calibration)
            modified[key] = value
            refused(lambda: c.metadata_checks(modified, roster))
    for key in ('classifier_sha256', 'field_span_helper_sha256', 'capture_prepared_sha256'):
        modified = copy.deepcopy(calibration)
        modified[key] = '0' * 64
        refused(lambda: c.metadata_checks(modified, roster))
    for key in calibration['proof_inputs']:
        modified = copy.deepcopy(calibration)
        del modified['proof_inputs'][key]
        refused(lambda: c.metadata_checks(modified, roster))
    missing = copy.deepcopy(roster)
    del missing['logical_candidates']['digest_write_value']
    refused(lambda: c.metadata_checks(calibration, missing))
    refused(lambda: c.construct_mutations(sources[c.BODY], missing))
    refused(lambda: c.mutations(sources[c.BODY] + '\n'))
    print('PASS: operation source controls (twelve groups, actual bodies, constants, composed inverses and forty calibrated mutations; no compiler or verifier execution)')


if __name__ == '__main__':
    main()
