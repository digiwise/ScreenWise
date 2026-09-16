"""Injected, owner-confirmed DRM validation sequence.

Importing this module performs no operating-system, device, process, or file
actions.  A live adapter must supply every backend operation explicitly.
"""
from __future__ import annotations

import math
import time
from typing import Any

from coordinator import ControlError

SAFE_REASONS = frozenset({
    'active_batch_deadline',
    'api_preconditions_failed',
    'drm_pause_lost',
    'drm_pause_not_observed',
    'drm_resume_not_observed',
    'drm_stop_unverified',
    'invalid_stimulus_duration',
    'persisted_audio_positive_control_missing',
    'session_not_known_unlocked',
    'unexpected_foreground',
})


class DrmRunner:
    """Run bounded DRM pause and recovery checks through an injected backend.

    In addition to the ``BatchRunner`` backend surface, the backend supplies
    ``start_drm()``, ``stop_drm()``, ``drm_paused()``,
    ``forbidden_count()``, ``play_stimulus(name)`` and
    ``audio_marker_count(marker)``.  ``play_stimulus`` accepts ``before``,
    ``during`` and ``after``.  Stop methods are idempotent and identity-guarded;
    ``release_focus()`` must hide every owned surface.
    """

    def __init__(self, backend, *, clock=time.monotonic, sleep=time.sleep):
        self.backend = backend
        self.clock = clock
        self.sleep = sleep
        self.deadline: float | None = None

    def _guard(self, phase: str | None = None) -> None:
        if self.deadline is None or self.clock() >= self.deadline:
            raise ControlError('active_batch_deadline')
        if self.backend.session_state() != 'unlocked':
            raise ControlError('session_not_known_unlocked')
        if phase is not None and self.backend.foreground_valid(phase) is not True:
            raise ControlError('unexpected_foreground')

    def _wait_pause(self, expected: bool, phase: str) -> None:
        until = min(self.deadline, self.clock() + 10.0)
        while True:
            self._guard(phase)
            if self.backend.drm_paused() is expected:
                return
            if self.clock() >= until:
                reason = 'drm_pause_not_observed' if expected else 'drm_resume_not_observed'
                raise ControlError(reason)
            self.sleep(min(0.25, max(0.0, until - self.clock())))

    def _duration(self, phase: str) -> float:
        duration = self.backend.play_stimulus(phase)
        if (isinstance(duration, bool) or not isinstance(duration, (int, float))
                or not math.isfinite(duration) or not 9 <= duration <= 45):
            raise ControlError('invalid_stimulus_duration')
        return float(duration)

    def _positive_control(self, phase: str, marker: str) -> dict[str, Any]:
        self.backend.fixture_action('plain', phase)
        self._guard(phase)
        before = self.backend.audio_marker_count(marker)
        self._guard(phase)
        duration = self._duration(phase)
        playback_until = self.clock() + duration
        while self.clock() < playback_until:
            self._guard(phase)
            self.backend.sample(phase)
            self.sleep(min(0.5, max(0.0, playback_until - self.clock())))
        self.backend.stop_playback()
        self._guard(phase)
        persisted_until = min(self.deadline, self.clock() + 45.0)
        while True:
            self._guard(phase)
            after = self.backend.audio_marker_count(marker)
            if after > before:
                self._guard(phase)
                return {'name': phase, 'marker_delta': after - before,
                        'persisted': True}
            if self.clock() >= persisted_until:
                raise ControlError('persisted_audio_positive_control_missing')
            self.backend.sample(phase)
            self.sleep(min(0.5, max(0.0, persisted_until - self.clock())))

    def _during_control(self) -> dict[str, Any]:
        marker = 'crimson bridge'
        audio_before = self.backend.audio_marker_count(marker)
        forbidden_before = self.backend.forbidden_count()
        duration = self._duration('during')
        playback_until = self.clock() + duration
        samples = 0
        while self.clock() < playback_until:
            self._guard('during')
            if self.backend.drm_paused() is not True:
                raise ControlError('drm_pause_lost')
            self.backend.sample('during')
            samples += 1
            self.sleep(min(0.5, max(0.0, playback_until - self.clock())))
        self.backend.stop_playback()
        self._guard('during')
        if self.backend.drm_paused() is not True:
            raise ControlError('drm_pause_lost')
        forbidden_after = self.backend.forbidden_count()
        audio_after = self.backend.audio_marker_count(marker)
        return {
            'name': 'during',
            'verified_samples': samples,
            'forbidden_delta': forbidden_after - forbidden_before,
            'audio_marker_delta': audio_after - audio_before,
        }

    def run(self, *, confirmed: bool, max_seconds: float = 240) -> dict[str, Any]:
        if confirmed is not True:
            raise ControlError('owner_readiness_required')
        if (isinstance(max_seconds, bool) or not isinstance(max_seconds, (int, float))
                or not math.isfinite(max_seconds) or not 60 <= max_seconds <= 240):
            raise ControlError('invalid_watchdog_budget')

        self.deadline = self.clock() + float(max_seconds)
        result: dict[str, Any] = {'status': 'incomplete', 'controls': []}
        confirmed_leak = False
        protected_started = False
        protected_before: int | None = None
        try:
            self.backend.preflight()
            self._guard()
            self.backend.start_fixture()
            self.backend.start_recorder()
            self.backend.await_api()
            self._guard()
            api = self.backend.api_checks()
            result['api'] = api
            if api.get('passed') is not True:
                raise ControlError('api_preconditions_failed')

            result['controls'].append(
                self._positive_control('before', 'silver cedar'))

            # This aggregate baseline is deliberately taken before the DRM
            # surface starts, so both newly admitted frames and audio can be
            # attributed to the protected interval by the backend.
            protected_before = self.backend.forbidden_count()
            protected_started = True
            self.backend.start_drm()
            self._guard('during')
            self._wait_pause(True, 'during')
            during = self._during_control()
            during['forbidden_delta'] = (
                self.backend.forbidden_count() - protected_before)
            result['controls'].append(during)
            if during['forbidden_delta'] > 0 or during['audio_marker_delta'] > 0:
                confirmed_leak = True
                result.update(status='failed', reason='drm_forbidden_capture_persisted')

            if self.backend.stop_drm() is not True:
                raise ControlError('drm_stop_unverified')
            self.backend.fixture_action('plain', 'after')
            self._guard('after')
            self._wait_pause(False, 'after')
            result['controls'].append(
                self._positive_control('after', 'amber window'))
            self._guard('after')
            final_forbidden_delta = (
                self.backend.forbidden_count() - protected_before)
            self._guard('after')
            during['forbidden_delta'] = final_forbidden_delta
            during['post_recovery_forbidden_delta'] = final_forbidden_delta
            if final_forbidden_delta > 0:
                confirmed_leak = True
                result.update(status='failed',
                              reason='drm_forbidden_capture_persisted')
            if not confirmed_leak:
                result.update(
                    status='passed_scoped_drm_controls',
                    limits=('Synthetic aggregate frame/audio markers only; real '
                            'protected-media behavior needs separate evidence.'),
                )
        except Exception as error:
            candidate = str(error) if isinstance(error, ControlError) else ''
            reason = candidate if candidate in SAFE_REASONS else 'backend_failure'
            if protected_started and protected_before is not None:
                try:
                    final_forbidden_delta = (
                        self.backend.forbidden_count() - protected_before)
                    result['protected_forbidden_delta'] = final_forbidden_delta
                    if final_forbidden_delta > 0:
                        confirmed_leak = True
                except Exception:
                    result['forbidden_recheck'] = 'backend_failure'
            if confirmed_leak:
                result.update(status='failed', reason='drm_forbidden_capture_persisted',
                              secondary_reason=reason)
            else:
                result.update(status='incomplete', reason=reason)
        finally:
            cleanup: list[dict[str, Any]] = []
            recorder_stopped = False
            try:
                cleanup.append({'step': 'stop_playback',
                                'ok': self.backend.stop_playback() is True})
            except Exception:
                cleanup.append({'step': 'stop_playback', 'ok': False})
            for attempt in (1, 2):
                try:
                    recorder_stopped = self.backend.stop_recorder() is True
                except Exception:
                    recorder_stopped = False
                cleanup.append({'step': 'stop_recorder', 'attempt': attempt,
                                'ok': recorder_stopped})
                if recorder_stopped:
                    break
            try:
                cleanup.append({'step': 'release_focus',
                                'ok': self.backend.release_focus() is True})
            except Exception:
                cleanup.append({'step': 'release_focus', 'ok': False})

            if recorder_stopped:
                try:
                    final = getattr(self.backend, 'final_evidence', lambda: {})()
                    result['final_evidence'] = final
                    if final.get('forbidden_count', 0) > 0:
                        confirmed_leak = True
                        result.update(
                            status='failed',
                            reason='drm_forbidden_capture_persisted')
                except Exception:
                    cleanup.append({'step': 'final_evidence', 'ok': False})
                try:
                    cleanup.append({'step': 'stop_drm',
                                    'ok': self.backend.stop_drm() is True})
                except Exception:
                    cleanup.append({'step': 'stop_drm', 'ok': False})
            else:
                cleanup.append({'step': 'stop_drm', 'ok': False,
                                'reason': 'recorder_stop_unverified'})

            # Owned synthetic fixtures contain no private clipboard state and
            # are closed after focus release even if recorder stop is unverified.
            try:
                cleanup.append({'step': 'close_fixture',
                                'ok': self.backend.close_fixture() is True})
            except Exception:
                cleanup.append({'step': 'close_fixture', 'ok': False})

            for name in ('processes_stopped', 'clean_shutdown'):
                try:
                    ok = getattr(self.backend, name)() is True
                except Exception:
                    ok = False
                cleanup.append({'step': name, 'ok': ok})
            result['cleanup'] = cleanup
            cleanup_ok = (recorder_stopped and all(
                row['ok'] for row in cleanup
                if row['step'] != 'stop_recorder'))
            if not cleanup_ok:
                if not confirmed_leak:
                    result['status'] = 'incomplete'
                result['cleanup_required'] = True
        return result
