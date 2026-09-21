"""Adverse tests of the evidence importer; all mutations stay in owned scratch."""

import argparse
import json
from pathlib import Path
import runpy
import shutil
import tempfile


def main():
    local = Path(__file__).resolve().parent
    api = runpy.run_path(str(local / 'archive.py'))
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--campaign-only', action='store_true')
    parser.add_argument('--repo', type=Path, default=api['ROOT'])
    parser.add_argument('--archive', type=Path)
    args = parser.parse_args()
    rejected = 0
    with tempfile.TemporaryDirectory(prefix='fe2o3-evidence-selftest-') as owned:
        temporary = Path(owned)
        contents, module = api['signed_sources'](args.repo, temporary / 'source')
        original_campaign = args.archive / 'producer-campaign' if args.archive else api['CAMPAIGN']
        campaign = temporary / 'campaign'
        shutil.copytree(original_campaign, campaign)
        runs = 0

        def campaign_check():
            nonlocal runs
            runs += 1
            return api['validate_campaign'](campaign, module, contents, temporary / f'run-{runs}')

        def adverse(name, path, replacement, check):
            nonlocal rejected
            original = path.read_bytes()
            try:
                path.write_bytes(replacement(original))
                try:
                    check()
                except ValueError:
                    rejected += 1
                else:
                    raise AssertionError(f'accepted adverse evidence: {name}')
            finally:
                path.write_bytes(original)

        def change(edit):
            def replace(data):
                value = json.loads(data)
                edit(value)
                return (json.dumps(value) + '\n').encode()
            return replace

        api['need'](len(campaign_check()) == 21, 'baseline complete campaign')
        adverse('floating obligation count', campaign / 'finished.json',
                change(lambda d: d.update(obligations=254.0)), campaign_check)
        adverse('numeric qualified flag', campaign / 'finished.json',
                change(lambda d: d.update(qualified=1)), campaign_check)
        adverse('empty inputs', campaign / 'inputs.json', lambda _: b'{}', campaign_check)
        adverse('omitted input', campaign / 'inputs.json', change(lambda d: d.pop(next(iter(d)))), campaign_check)
        case = campaign / 'positive_before'
        adverse('omitted generated input', case / 'sources.json', change(lambda d: d.pop(next(iter(d)))), campaign_check)
        adverse('changed candidate', case / 'context_producer_read_lifecycle_v1.rs', lambda b: b + b'\n', campaign_check)
        record = case / 'solver/record.json'
        adverse('partial verification', record, change(lambda d: d['command'].append('--verify-root')), campaign_check)
        adverse('wrong result status', record, change(lambda d: d.update(status=1)), campaign_check)
        adverse('retained group', record, change(lambda d: d.update(group_absent=False)), campaign_check)
        adverse('extra receipt field', record, change(lambda d: d.update(extra=True)), campaign_check)
        adverse('wrong solver summary', case / 'solver/stdout.log',
                change(lambda d: d['verification-results'].update(errors=1)), campaign_check)
        adverse('wrong closure command', campaign / 'closure-before/record.json',
                change(lambda d: d['command'].append('foreign')), campaign_check)
        extra = case / 'solver/unchecked'
        extra.write_bytes(b'extra')
        try:
            try:
                campaign_check()
            except ValueError:
                rejected += 1
            else:
                raise AssertionError('accepted extra solver file')
        finally:
            extra.unlink()
        link = temporary / 'linked-solver'
        link.symlink_to(case / 'solver', target_is_directory=True)
        try:
            api['entries'](link)
        except ValueError:
            rejected += 1
        else:
            raise AssertionError('accepted symlink directory')
        if not args.campaign_only:
            qualification = temporary / 'qualification-data'
            qualification.mkdir()
            original = args.archive or local
            for name in ['qualify.py', 'finished.json']:
                shutil.copyfile(original / name, qualification / name)
            shutil.copytree(original / 'qualification', qualification / 'qualification')

            def qualification_check():
                api['validate_qualification'](qualification, contents)

            qualification_check()
            receipt = qualification / 'qualification/lint/receipt.json'
            for name, edit in [
                ('command', lambda d: d.update(command=['true'])),
                ('cwd', lambda d: d.update(cwd='/foreign')),
                ('environment', lambda d: d.update(environment={})),
                ('stdin', lambda d: d.update(stdin_sha256='0' * 64)),
                ('timeout', lambda d: d.update(timeout_seconds=1)),
                ('exit', lambda d: d.update(exit=True)),
                ('process group', lambda d: d.update(group_absent=False)),
                ('timestamp', lambda d: d.update(finished_ns=0)),
            ]:
                adverse(name, receipt, change(edit), qualification_check)
            adverse('raw stream checksum', qualification / 'qualification/lint/stdout', lambda b: b + b'\n', qualification_check)
            adverse('native claim', qualification / 'finished.json', change(lambda d: d.update(native_execution=True)), qualification_check)
            adverse('changed driver', qualification / 'qualify.py', lambda b: b + b'\n', qualification_check)
            qualification_check()
        api['need'](len(campaign_check()) == 21, 'restored campaign')
    print(json.dumps({'adverse_cases_rejected': rejected, 'campaign_only': args.campaign_only,
                      'source_commit': api['SOURCE'], 'passed': True}))


if __name__ == '__main__':
    main()
