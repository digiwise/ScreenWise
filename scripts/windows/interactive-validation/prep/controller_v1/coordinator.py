"""Owner-paced controller policy. Importing this module performs no OS actions.

CLI defaults to preparing a waiting record. It cannot launch a recorder or UI.
The live adapter must be invoked separately after the owner's explicit reply.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import pathlib
import re
import secrets
import time
from dataclasses import dataclass
from typing import Any

ROOT = pathlib.Path(__file__).resolve().parent
MODES = ('privacy', 'audio-output', 'audio-microphone', 'lock', 'drm', 'browser', 'uac')
SAFE_ID = re.compile(r'[A-Za-z0-9][A-Za-z0-9_-]{0,63}\Z')


class ControlError(RuntimeError):
    pass


def exclusive_json(path: pathlib.Path, value: dict) -> None:
    with path.open('x', encoding='utf-8') as stream:
        json.dump(value, stream, indent=2)


def session_path(run_id: str, root: pathlib.Path = ROOT) -> pathlib.Path:
    if not isinstance(run_id, str) or not SAFE_ID.fullmatch(run_id):
        raise ControlError('invalid_run_id')
    sessions = (root / 'sessions').resolve()
    path = (sessions / run_id).resolve()
    if path.parent != sessions:
        raise ControlError('run_path_escape')
    return path


def prepare(mode: str, run_id: str, root: pathlib.Path = ROOT) -> dict:
    if mode not in MODES:
        raise ControlError('invalid_mode')
    path = session_path(run_id, root)
    path.mkdir(parents=True, exist_ok=False)
    record = {
        'schema': 'screenwise.owner-paced.v1', 'run_id': run_id, 'mode': mode,
        'nonce': secrets.token_hex(24), 'state': 'waiting_for_owner',
        'readiness_timeout_seconds': None, 'recording_started': False,
        'prompt': prompt_for(mode), 'created_unix': time.time(),
    }
    exclusive_json(path / 'waiting.json', record)
    return record


def prompt_for(mode: str) -> str:
    base = 'Test recording is stopped and owned test windows must be hidden. Reply Ready when convenient; there is no response deadline. '
    details = {
        'privacy': 'Ready for a compact synthetic privacy batch (about 90 seconds), with no audio or further actions from you?',
        'audio-output': 'Ready for local speech clips through your selected USB headphones (about two minutes), with no speaking required?',
        'audio-microphone': 'Ready to read the displayed synthetic passage once, for about 45 seconds after the start cue?',
        'lock': 'Ready for one pre-briefed lock, hold, unlock and synthetic recovery sequence? You control locking and unlocking.',
        'drm': 'Ready for a compact synthetic DRM-identity and audio pause/recovery batch (about two minutes)?',
        'browser': 'Ready for a local synthetic browser/password/exclusion batch in a compact browser window?',
        'uac': 'Ready for one intentional owner-controlled UAC transition? Do not approve unrelated installer prompts.',
    }
    return base + details[mode]


def consume_confirmation(run_id: str, nonce: str, *, recorder_stopped: bool,
                         fixtures_hidden: bool, root: pathlib.Path = ROOT) -> dict:
    path = session_path(run_id, root)
    record = json.loads((path / 'waiting.json').read_text(encoding='utf-8'))
    if record.get('run_id') != run_id or record.get('schema') != 'screenwise.owner-paced.v1':
        raise ControlError('waiting_identity_mismatch')
    if record.get('mode') not in MODES or record.get('state') != 'waiting_for_owner':
        raise ControlError('not_waiting')
    if not isinstance(nonce, str) or not secrets.compare_digest(record.get('nonce', ''), nonce):
        raise ControlError('stale_or_missing_confirmation')
    if recorder_stopped is not True or fixtures_hidden is not True:
        raise ControlError('readiness_surface_not_safe')
    # Exclusive creation is also the concurrent/replay guard. A failed launch
    # needs a newly prepared run and a fresh owner confirmation.
    exclusive_json(path / 'consumed.json', {'run_id': run_id, 'confirmed_unix': time.time()})
    return record


@dataclass(frozen=True)
class Phase:
    name: str
    action: str
    seconds: float
    state: str = 'unlocked'


PRIVACY_PHASES = (
    Phase('before', 'plain', 10),
    Phase('password', 'password', 10),
    Phase('excluded_foreground', 'excluded-foreground', 10),
    Phase('excluded_background', 'excluded-background', 10),
    Phase('after', 'plain', 10),
)


def assess_privacy(phases: list[dict]) -> dict:
    """Only marker deltas within verified phases count as positive controls."""
    expected = [p.name for p in PRIVACY_PHASES]
    if [p.get('name') for p in phases] != expected:
        return {'status': 'incomplete', 'reason': 'phase_sequence_incomplete'}
    if any(p.get('verified') is not True for p in phases):
        return {'status': 'incomplete', 'reason': 'os_or_fixture_state_unverified'}
    if any(p.get('forbidden_delta', 0) > 0 for p in phases):
        return {'status': 'failed', 'reason': 'synthetic_forbidden_marker_persisted'}
    for name in ('before', 'after'):
        if next(p for p in phases if p['name'] == name).get('allowed_delta', 0) <= 0:
            return {'status': 'incomplete', 'reason': 'positive_control_missing'}
    return {'status': 'passed_scoped_marker_checks',
            'limits': 'Synthetic text admission only; image pixels, clipboard, other monitors and real DRM need separate evidence.'}


class BatchRunner:
    """Injected backend; real implementations are never selected implicitly.

    Backend methods: preflight, start_fixture, start_recorder, await_api,
    api_checks, session_state, fixture_action, foreground_valid, counts,
    sample, stop_playback, stop_recorder, release_focus, restore_clipboard,
    close_fixture, processes_stopped. Every stop is idempotent and identity-guarded.
    """
    def __init__(self, backend, *, clock=time.monotonic, sleep=time.sleep):
        self.backend, self.clock, self.sleep = backend, clock, sleep
        self.deadline = None
        self.phases: list[dict] = []

    def guard(self, phase=None):
        if self.clock() >= self.deadline:
            raise ControlError('active_batch_deadline')
        if self.backend.session_state() != 'unlocked':
            raise ControlError('session_not_known_unlocked')
        if phase and not self.backend.foreground_valid(phase):
            raise ControlError('unexpected_foreground')

    def run_privacy(self, *, confirmed: bool, max_seconds: float = 150) -> dict:
        if confirmed is not True:
            raise ControlError('owner_readiness_required')
        if not 60 <= max_seconds <= 240:
            raise ControlError('invalid_watchdog_budget')
        result: dict[str, Any] = {'status': 'incomplete', 'phases': self.phases}
        cleanup = []
        self.deadline = self.clock() + max_seconds
        try:
            self.backend.preflight()
            self.guard()
            self.backend.start_fixture()
            self.backend.start_recorder()
            self.backend.await_api()
            self.guard()
            result['api'] = self.backend.api_checks()
            if result['api'].get('passed') is not True:
                raise ControlError('api_preconditions_failed')
            for phase in PRIVACY_PHASES:
                self.guard()
                before = self.backend.counts()
                self.backend.fixture_action(phase.action, phase.name)
                self.guard(phase.name)
                until = self.clock() + phase.seconds
                samples = 0
                while self.clock() < until:
                    self.guard(phase.name)
                    self.backend.sample(phase.name)
                    samples += 1
                    self.sleep(min(.5, max(0, until-self.clock())))
                self.guard(phase.name)
                after = self.backend.counts()
                self.phases.append({'name': phase.name, 'verified': samples > 0,
                                    'allowed_delta': after['allowed']-before['allowed'],
                                    'forbidden_delta': after['forbidden']-before['forbidden']})
            result.update(assess_privacy(self.phases))
        except Exception as error:
            # Never serialize raw exception messages from HTTP, windows or capture.
            result.update(status='incomplete', reason=str(error) if isinstance(error, ControlError) else 'backend_failure')
        finally:
            # Always stop capture before private clipboard restoration or questions.
            stopped = False
            try:
                cleanup.append({'step': 'stop_playback',
                                'ok': self.backend.stop_playback() is True})
            except Exception:
                cleanup.append({'step': 'stop_playback', 'ok': False})
            for attempt in (1, 2):
                try:
                    stopped = self.backend.stop_recorder() is True
                except Exception:
                    stopped = False
                cleanup.append({'step': 'stop_recorder', 'attempt': attempt,
                                'ok': stopped})
                if stopped:
                    break
            try:
                cleanup.append({'step': 'release_focus',
                                'ok': self.backend.release_focus() is True})
            except Exception:
                cleanup.append({'step': 'release_focus', 'ok': False})
            if stopped:
                try:
                    final = getattr(self.backend, 'final_evidence', lambda: {})()
                    result['final_evidence'] = final
                    if final.get('forbidden_count', 0) > 0:
                        result.update(status='failed', reason='synthetic_forbidden_marker_persisted')
                except Exception:
                    cleanup.append({'step': 'final_evidence', 'ok': False})
                try:
                    cleanup.append({'step': 'restore_clipboard',
                                    'ok': self.backend.restore_clipboard() is True})
                except Exception:
                    cleanup.append({'step': 'restore_clipboard', 'ok': False})
            else:
                cleanup.append({'step': 'restore_clipboard', 'ok': False, 'reason': 'recorder_stop_unverified'})
            try:
                cleanup.append({'step': 'close_fixture',
                                'ok': self.backend.close_fixture() is True})
            except Exception:
                cleanup.append({'step': 'close_fixture', 'ok': False})
            try:
                cleanup.append({'step': 'processes_stopped', 'ok': self.backend.processes_stopped() is True})
            except Exception:
                cleanup.append({'step': 'processes_stopped', 'ok': False})
            try:
                cleanup.append({'step': 'clean_shutdown',
                                'ok': self.backend.clean_shutdown() is True})
            except Exception:
                cleanup.append({'step': 'clean_shutdown', 'ok': False})
            result['cleanup'] = cleanup
            cleanup_ok = (stopped and all(
                row['ok'] for row in cleanup
                if row['step'] != 'stop_recorder'))
            if not cleanup_ok:
                if result['status'] != 'failed': result['status'] = 'incomplete'
                result['cleanup_required'] = True
        return result

    def run_audio_output(self, *, confirmed: bool, max_seconds: float = 210) -> dict:
        if confirmed is not True:
            raise ControlError('owner_readiness_required')
        if not 90 <= max_seconds <= 240:
            raise ControlError('invalid_watchdog_budget')
        self.deadline = self.clock() + max_seconds
        result = {'status': 'incomplete', 'controls': []}
        try:
            self.backend.preflight()
            self.guard()
            self.backend.start_fixture()
            self.backend.start_recorder()
            self.backend.await_api()
            api = self.backend.api_checks()
            if api.get('passed') is not True:
                raise ControlError('api_preconditions_failed')
            result['api'] = api
            for name in ('before', 'after'):
                self.guard()
                self.backend.fixture_action('plain', name)
                self.guard(name)
                marker = {'before': 'silver cedar', 'after': 'amber window'}[name]
                before = self.backend.audio_marker_count(marker)
                duration = self.backend.play_stimulus(name)
                if not 9 <= duration <= 45:
                    raise ControlError('invalid_stimulus_duration')
                until = self.clock() + duration
                while self.clock() < until:
                    self.guard(name)
                    self.backend.sample(name)
                    self.sleep(min(.5, max(0, until-self.clock())))
                self.backend.stop_playback()
                self.guard(name)
                until = self.clock() + 45
                while self.backend.audio_marker_count(marker) <= before:
                    self.guard(name)
                    self.backend.sample(name)
                    if self.clock() >= until:
                        raise ControlError('persisted_audio_positive_control_missing')
                    self.sleep(.5)
                self.guard(name)
                result['controls'].append({'name': name, 'persisted': True})
            result['status'] = 'passed_scoped_audio_controls'
            result['limits'] = 'Known phrases persisted; exact speech completeness and shutdown tail preservation not validated.'
        except Exception as error:
            result.update(status='incomplete', reason=str(error) if isinstance(error, ControlError) else 'backend_failure')
        finally:
            cleanup = []
            stopped = False
            try:
                cleanup.append({'step': 'stop_playback',
                                'ok': self.backend.stop_playback() is True})
            except Exception:
                cleanup.append({'step': 'stop_playback', 'ok': False})
            for attempt in (1, 2):
                try:
                    stopped = self.backend.stop_recorder() is True
                except Exception:
                    stopped = False
                cleanup.append({'step': 'stop_recorder', 'attempt': attempt,
                                'ok': stopped})
                if stopped:
                    break
            try:
                cleanup.append({'step': 'release_focus',
                                'ok': self.backend.release_focus() is True})
            except Exception:
                cleanup.append({'step': 'release_focus', 'ok': False})
            if stopped:
                try:
                    result['final_evidence'] = getattr(self.backend, 'final_evidence', lambda: {})()
                except Exception:
                    cleanup.append({'step': 'final_evidence', 'ok': False})
                try:
                    cleanup.append({'step': 'restore_clipboard',
                                    'ok': self.backend.restore_clipboard() is True})
                except Exception:
                    cleanup.append({'step': 'restore_clipboard', 'ok': False})
            else:
                cleanup.append({'step': 'restore_clipboard', 'ok': False, 'reason': 'recorder_stop_unverified'})
            for name in ('close_fixture', 'processes_stopped', 'clean_shutdown'):
                try:
                    cleanup.append({'step': name,
                                    'ok': getattr(self.backend, name)() is True})
                except Exception:
                    cleanup.append({'step': name, 'ok': False})
            result['cleanup'] = cleanup
            cleanup_ok = (stopped and all(
                row['ok'] for row in cleanup
                if row['step'] != 'stop_recorder'))
            if not cleanup_ok:
                result.update(status='incomplete', cleanup_required=True)
        return result


def main():
    parser = argparse.ArgumentParser(description='Prepare an indefinite waiting gate; never launches tests')
    parser.add_argument('--mode', required=True, choices=MODES)
    parser.add_argument('--run-id', required=True)
    args = parser.parse_args()
    print(json.dumps(prepare(args.mode, args.run_id), indent=2))


if __name__ == '__main__':
    main()
