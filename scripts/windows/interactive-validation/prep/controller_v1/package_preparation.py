"""Write an unreviewed candidate pin manifest; never create readiness gates."""
import argparse
import hashlib
import json
from pathlib import Path
from coordinator import ROOT


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--candidate-output', required=True, type=Path)
    parser.add_argument('--include-generated', action='store_true',
                        help='include rebuilt EXE/WAV artifacts after manual provenance review')
    args = parser.parse_args(argv)
    prep = ROOT.parent
    files = list(ROOT.glob('*.py')) + list(ROOT.glob('*.ps1')) + [ROOT/'README.md']
    files += [prep/name for name in (
        'plan.py', 'test_plan.py', 'preflight.ps1', 'lock_probe.py',
        'pixel_redaction_eval.py', 'test_pixel_redaction_eval.py',
    )]
    groups = [('fixtures', ('*.cs','*.html','*.ps1','*.md')),
              ('audio', ('*.json','*.ps1'))]
    if args.include_generated:
        groups = [('fixtures', ('*.exe','*.cs','*.html','*.ps1','*.md')),
                  ('audio', ('*.wav','*.json','*.ps1'))]
    for directory, patterns in groups:
        for pattern in patterns:
            files += list((prep/directory).glob(pattern))
    assets = {file.relative_to(prep).as_posix(): hashlib.sha256(file.read_bytes()).hexdigest().upper()
              for file in sorted(set(files))}
    output_path = args.candidate_output.resolve()
    with output_path.open('x',encoding='utf-8') as output:
        json.dump({'schema':'screenwise.prepared-controller-pins-candidate.v1',
                   'review_status':'candidate_unreviewed',
                   'includes_generated_artifacts':args.include_generated,
                   'assets':assets},output,indent=2)
        output.write('\n')
    print(json.dumps({'candidate':str(output_path),'pinned_files':len(assets),
                      'review_status':'candidate_unreviewed','waiting_sessions':0,
                      'recording_started':False}))


if __name__=='__main__':
    main()
