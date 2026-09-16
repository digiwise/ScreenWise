import unittest

from coordinator import ControlError
from drm_sequence import DrmRunner


class FakeClock:
    def __init__(self):
        self.now = 0.0

    def clock(self):
        return self.now

    def sleep(self, seconds):
        self.now += seconds


class FakeBackend:
    def __init__(self):
        self.calls = []
        self.phase = None
        self.state = 'unlocked'
        self.focus = True
        self.paused = False
        self.pause_queries = 0
        self.pause_after_queries = 1
        self.marker_counts = {
            'silver cedar': 0,
            'crimson bridge': 0,
            'amber window': 0,
        }
        self.forbidden = 0
        self.leak = False
        self.positive_before = True
        self.positive_after = True
        self.recorder_stop_ok = True
        self.stop_results = []
        self.stop_raises = 0
        self.drm_stop_ok = True
        self.late_leak = False
        self.final_forbidden = 0

    def _call(self, name):
        self.calls.append(name)

    def preflight(self): self._call('preflight')
    def start_fixture(self): self._call('start_fixture')
    def start_recorder(self): self._call('start_recorder')
    def await_api(self): self._call('await_api')
    def api_checks(self): self._call('api_checks'); return {'passed': True}
    def session_state(self): return self.state
    def foreground_valid(self, phase): self._call('foreground:' + phase); return self.focus
    def fixture_action(self, action, phase): self._call('fixture:' + action + ':' + phase); self.phase = phase

    def start_drm(self):
        self._call('start_drm')
        self.phase = 'during'
        self.pause_queries = 0

    def stop_drm(self):
        self._call('stop_drm')
        if self.drm_stop_ok:
            self.paused = False
        return self.drm_stop_ok

    def drm_paused(self):
        self.pause_queries += 1
        if self.phase == 'during' and self.pause_queries >= self.pause_after_queries:
            self.paused = True
        return self.paused

    def forbidden_count(self): return self.forbidden
    def audio_marker_count(self, marker): return self.marker_counts[marker]
    def play_stimulus(self, name): self._call('play:' + name); self.phase = name; return 9

    def sample(self, phase):
        self._call('sample:' + phase)
        if phase == 'before' and self.positive_before:
            self.marker_counts['silver cedar'] += 1
        elif phase == 'after' and self.positive_after:
            self.marker_counts['amber window'] += 1
            if self.late_leak:
                self.forbidden += 1
        elif phase == 'during' and self.leak:
            self.marker_counts['crimson bridge'] += 1
            self.forbidden += 1

    def stop_playback(self): self._call('stop_playback'); return True
    def stop_recorder(self):
        self._call('stop_recorder')
        if self.stop_raises:
            self.stop_raises -= 1
            raise RuntimeError('private stop detail')
        if self.stop_results:
            return self.stop_results.pop(0)
        return self.recorder_stop_ok
    def release_focus(self): self._call('release_focus'); return True
    def close_fixture(self): self._call('close_fixture'); return True
    def processes_stopped(self): self._call('processes_stopped'); return self.recorder_stop_ok
    def clean_shutdown(self): self._call('clean_shutdown'); return self.recorder_stop_ok
    def final_evidence(self): self._call('final_evidence'); return {'forbidden_count': self.final_forbidden}


class DrmRunnerTests(unittest.TestCase):
    def run_batch(self, backend):
        fake = FakeClock()
        return DrmRunner(backend, clock=fake.clock, sleep=fake.sleep).run(
            confirmed=True)

    def test_no_confirmation_refuses_without_backend_calls(self):
        backend = FakeBackend()
        with self.assertRaises(ControlError):
            DrmRunner(backend).run(confirmed=False)
        self.assertEqual(backend.calls, [])

    def test_successful_ordered_transition(self):
        backend = FakeBackend()
        result = self.run_batch(backend)
        self.assertEqual(result['status'], 'passed_scoped_drm_controls')
        self.assertEqual([row['name'] for row in result['controls']],
                         ['before', 'during', 'after'])
        ordered = [
            'play:before', 'start_drm', 'play:during', 'stop_drm',
            'fixture:plain:after', 'play:after', 'stop_recorder',
            'release_focus', 'close_fixture', 'processes_stopped',
            'clean_shutdown',
        ]
        positions = [backend.calls.index(item) for item in ordered]
        self.assertEqual(positions, sorted(positions))

    def test_missing_before_positive_stops_before_drm(self):
        backend = FakeBackend()
        backend.positive_before = False
        result = self.run_batch(backend)
        self.assertEqual(result['status'], 'incomplete')
        self.assertEqual(result['reason'],
                         'persisted_audio_positive_control_missing')
        self.assertNotIn('start_drm', backend.calls)

    def test_unstable_pause_stops_during_protected_phase(self):
        backend = FakeBackend()
        original = backend.drm_paused

        def unstable_pause():
            value = original()
            if backend.phase == 'during' and backend.pause_queries > 2:
                return False
            return value

        backend.drm_paused = unstable_pause
        result = self.run_batch(backend)
        self.assertEqual(result['status'], 'incomplete')
        self.assertEqual(result['reason'], 'drm_pause_lost')
        self.assertNotIn('fixture:plain:after', backend.calls)

    def test_leak_recheck_wins_when_pause_then_becomes_unstable(self):
        backend = FakeBackend()
        backend.leak = True
        original = backend.drm_paused

        def unstable_pause():
            value = original()
            if backend.phase == 'during' and backend.pause_queries > 2:
                return False
            return value

        backend.drm_paused = unstable_pause
        result = self.run_batch(backend)
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['reason'], 'drm_forbidden_capture_persisted')
        self.assertEqual(result['secondary_reason'], 'drm_pause_lost')

    def test_forbidden_leak_remains_failed_when_after_control_is_missing(self):
        backend = FakeBackend()
        backend.leak = True
        backend.positive_after = False
        result = self.run_batch(backend)
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['reason'], 'drm_forbidden_capture_persisted')
        self.assertEqual(result['secondary_reason'],
                         'persisted_audio_positive_control_missing')
        during = result['controls'][1]
        self.assertGreater(during['forbidden_delta'], 0)
        self.assertGreater(during['audio_marker_delta'], 0)

    def test_late_leak_after_resume_fails_completed_controls(self):
        backend = FakeBackend()
        backend.late_leak = True
        result = self.run_batch(backend)
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['reason'], 'drm_forbidden_capture_persisted')
        self.assertGreater(result['controls'][1]
                           ['post_recovery_forbidden_delta'], 0)

    def test_expired_count_query_cannot_satisfy_positive_control(self):
        backend = FakeBackend()
        fake = FakeClock()
        original = backend.audio_marker_count
        silver_queries = 0

        def expiring_count(marker):
            nonlocal silver_queries
            value = original(marker)
            if marker == 'silver cedar':
                silver_queries += 1
                if silver_queries == 2:
                    fake.now = 1000
            return value

        backend.audio_marker_count = expiring_count
        result = DrmRunner(backend, clock=fake.clock, sleep=fake.sleep).run(
            confirmed=True)
        self.assertEqual(result['status'], 'incomplete')
        self.assertEqual(result['reason'], 'active_batch_deadline')
        self.assertEqual(result['controls'], [])

    def test_recorder_stop_failure_preserves_cleanup_order_and_gates_teardown(self):
        backend = FakeBackend()
        backend.focus = False
        backend.recorder_stop_ok = False
        result = self.run_batch(backend)
        self.assertTrue(result['cleanup_required'])
        self.assertLess(backend.calls.index('stop_playback'),
                        backend.calls.index('stop_recorder'))
        self.assertLess(backend.calls.index('stop_recorder'),
                        backend.calls.index('release_focus'))
        self.assertNotIn('stop_drm', backend.calls)
        self.assertEqual(backend.calls.count('stop_recorder'), 2)
        self.assertIn('close_fixture', backend.calls)
        self.assertIn('processes_stopped', backend.calls)
        self.assertIn('clean_shutdown', backend.calls)

    def test_cleanup_stop_retry_recovers_and_runs_all_verification(self):
        backend = FakeBackend()
        backend.stop_results = [False, True]
        result = self.run_batch(backend)
        self.assertNotIn('cleanup_required', result)
        self.assertEqual(backend.calls.count('stop_recorder'), 2)
        self.assertLess(backend.calls.index('release_focus'),
                        backend.calls.index('close_fixture'))
        self.assertIn('final_evidence', backend.calls)
        self.assertIn('processes_stopped', backend.calls)
        self.assertIn('clean_shutdown', backend.calls)

    def test_shutdown_evidence_leak_stays_failed_when_cleanup_fails(self):
        backend = FakeBackend()
        backend.final_forbidden = 1
        backend.drm_stop_ok = False
        result = self.run_batch(backend)
        self.assertEqual(result['status'], 'failed')
        self.assertEqual(result['reason'], 'drm_forbidden_capture_persisted')
        self.assertTrue(result['cleanup_required'])

    def test_backend_exception_is_sanitized(self):
        backend = FakeBackend()

        def private_failure():
            raise RuntimeError('private captured payload')

        backend.start_drm = private_failure
        result = self.run_batch(backend)
        self.assertEqual(result['reason'], 'backend_failure')
        self.assertNotIn('private captured payload', repr(result))


if __name__ == '__main__':
    unittest.main()
