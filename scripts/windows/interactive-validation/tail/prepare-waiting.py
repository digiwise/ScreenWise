"""Create one named, unconsumed readiness record; never activate a run."""
import argparse
import json
import sys
from pathlib import Path

root = Path(__file__).resolve().parent


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run-id', required=True)
    args = parser.parse_args(argv)
    candidates = [root.parent / 'interactive-prep-20260915-01a09e45' / 'controller_v1',
                  root.parent / 'prep' / 'controller_v1']
    controller = next((item.resolve() for item in candidates if (item / 'coordinator.py').is_file()), None)
    if controller is None or not (controller / 'coordinator.py').is_file():
        raise SystemExit('allowlisted staged or source controller root was not found')
    sys.path.insert(0, str(controller))
    import coordinator
    record = coordinator.prepare('audio-output', args.run_id, root=controller)
    print(json.dumps({'run_id': record['run_id'], 'state': record['state'],
                      'readiness_timeout_seconds': record['readiness_timeout_seconds'],
                      'recording_started': record['recording_started'],
                      'consumed': False}, indent=2))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
