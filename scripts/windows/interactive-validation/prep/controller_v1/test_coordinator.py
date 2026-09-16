import pathlib
import tempfile
import unittest
from unittest.mock import patch
from coordinator import BatchRunner, ControlError, PRIVACY_PHASES, consume_confirmation, prepare, session_path
from browser_clipboard_sequence import ALL_MARKERS


class FakeBackend:
    def __init__(self):
        self.calls=[]; self.phase=None; self.allowed=0; self.forbidden=0
        self.state='unlocked'; self.focus=True; self.stop_ok=True; self.leak=False
        self.stop_results=[]; self.stop_raises=0; self.shutdown_raises=False
    def preflight(self): self.calls.append('preflight')
    def start_fixture(self): self.calls.append('start_fixture')
    def start_recorder(self): self.calls.append('start_recorder')
    def await_api(self): pass
    def api_checks(self): return {'passed':True}
    def session_state(self): return self.state
    def foreground_valid(self, phase): return self.focus
    def fixture_action(self, action, phase): self.phase=phase
    def counts(self): return {'allowed':self.allowed,'forbidden':self.forbidden}
    def sample(self, phase):
        if phase in ('before','after'): self.allowed+=1
        if self.leak and phase=='password': self.forbidden+=1
    def stop_playback(self): self.calls.append('stop_playback'); return True
    def stop_recorder(self):
        self.calls.append('stop_recorder')
        if self.stop_raises:
            self.stop_raises-=1
            raise RuntimeError('private stop detail')
        if self.stop_results:
            return self.stop_results.pop(0)
        return self.stop_ok
    def release_focus(self): self.calls.append('release_focus'); return True
    def restore_clipboard(self): self.calls.append('restore_clipboard'); return True
    def close_fixture(self): self.calls.append('close_fixture'); return True
    def processes_stopped(self): self.calls.append('processes_stopped'); return self.stop_ok
    def clean_shutdown(self):
        self.calls.append('clean_shutdown')
        if self.shutdown_raises: raise RuntimeError('private shutdown detail')
        return self.stop_ok
    def final_evidence(self): return {}
    def audio_marker_count(self, marker): return self.allowed
    def play_stimulus(self, name): self.phase=name; return 16


class BrowserBackend(FakeBackend):
    def __init__(self):
        super().__init__()
        self.marker_counts = {marker: 0 for marker in ALL_MARKERS}
        self.browser_cleanup = {'browser_stopped': True, 'server_stopped': True}
    def start_browser_fixture(self): self.calls.append('start_browser_fixture')
    def browser_clipboard_marker_counts(self): return dict(self.marker_counts)
    def browser_clipboard_phase(self, phase): self.phase=phase; self.calls.append('phase:'+phase)
    def sample(self, phase):
        if phase == 'browser_allowed':
            self.marker_counts['public browser cedar'] += 1
            self.marker_counts['public bronze hill'] += 1
        elif phase == 'clipboard_plain':
            self.marker_counts['public cedar garden'] += 1
        if self.leak and phase == 'clipboard_password':
            self.marker_counts['hidden tulip waterfall'] += 1
    def close_browser_fixture(self):
        self.calls.append('close_browser_fixture')
        return dict(self.browser_cleanup)
    def final_evidence(self): return {'available': True, 'forbidden_count': 0}


class CoordinatorTests(unittest.TestCase):
    def run_batch(self, backend, **kwargs):
        now=[0.0]
        return BatchRunner(backend,clock=lambda:now[0],sleep=lambda s:now.__setitem__(0,now[0]+s)).run_privacy(confirmed=True,**kwargs)
    def test_wait_does_not_expire_or_launch(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=pathlib.Path(tmp); record=prepare('privacy','run-1',root)
            self.assertIsNone(record['readiness_timeout_seconds'])
            with patch('coordinator.time.time',return_value=10**12):
                result=consume_confirmation('run-1',record['nonce'],recorder_stopped=True,fixtures_hidden=True,root=root)
            self.assertFalse(result['recording_started'])
            with self.assertRaises(FileExistsError):
                consume_confirmation('run-1',record['nonce'],recorder_stopped=True,fixtures_hidden=True,root=root)
    def test_stale_reply_focus_and_path_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=pathlib.Path(tmp); record=prepare('privacy','a',root)
            for nonce,hidden in [('old',True),(record['nonce'],False)]:
                with self.assertRaises(ControlError):consume_confirmation('a',nonce,recorder_stopped=True,fixtures_hidden=hidden,root=root)
            with self.assertRaises(ControlError):session_path('../escape',root)
    def test_no_confirmation_never_calls_backend(self):
        backend=FakeBackend()
        with self.assertRaises(ControlError):BatchRunner(backend).run_privacy(confirmed=False)
        self.assertEqual(backend.calls,[])
    def test_success_and_cleanup_order(self):
        backend=FakeBackend(); result=self.run_batch(backend)
        self.assertEqual(result['status'],'passed_scoped_marker_checks')
        self.assertLess(backend.calls.index('stop_recorder'),backend.calls.index('restore_clipboard'))
        self.assertLess(backend.calls.index('release_focus'),backend.calls.index('close_fixture'))
    def test_missing_positive_control_not_pass(self):
        backend=FakeBackend(); backend.sample=lambda phase:None
        self.assertEqual(self.run_batch(backend)['reason'],'positive_control_missing')
    def test_forbidden_marker_fails(self):
        backend=FakeBackend();backend.leak=True
        self.assertEqual(self.run_batch(backend)['status'],'failed')
    def test_focus_loss_stops_and_does_not_advance(self):
        backend=FakeBackend();backend.focus=False
        result=self.run_batch(backend)
        self.assertEqual(result['reason'],'unexpected_foreground')
        self.assertEqual(result['phases'],[])
        self.assertIn('stop_recorder',backend.calls)
    def test_unknown_state_stops_before_start(self):
        backend=FakeBackend();backend.state='unknown'
        self.assertEqual(self.run_batch(backend)['reason'],'session_not_known_unlocked')
        self.assertNotIn('start_recorder',backend.calls)
    def test_unverified_stop_never_restores_clipboard(self):
        backend=FakeBackend();backend.stop_ok=False
        result=self.run_batch(backend)
        self.assertTrue(result['cleanup_required'])
        self.assertNotIn('restore_clipboard',backend.calls)
        self.assertEqual(backend.calls.count('stop_recorder'),2)
        self.assertIn('close_fixture',backend.calls)
        self.assertIn('processes_stopped',backend.calls)
        self.assertIn('clean_shutdown',backend.calls)
    def test_stop_retry_recovers_before_restore(self):
        backend=FakeBackend();backend.stop_results=[False,True]
        result=self.run_batch(backend)
        self.assertNotIn('cleanup_required',result)
        stops=[i for i,value in enumerate(backend.calls) if value=='stop_recorder']
        self.assertEqual(len(stops),2)
        self.assertLess(stops[-1],backend.calls.index('restore_clipboard'))
        self.assertLess(backend.calls.index('release_focus'),backend.calls.index('close_fixture'))
    def test_audio_cleanup_retries_raised_stop_and_always_checks_processes(self):
        backend=FakeBackend();backend.stop_raises=1
        now=[0.0]
        result=BatchRunner(backend,clock=lambda:now[0],sleep=lambda s:now.__setitem__(0,now[0]+s)).run_audio_output(confirmed=True)
        self.assertNotIn('cleanup_required',result)
        self.assertEqual(backend.calls.count('stop_recorder'),2)
        self.assertIn('restore_clipboard',backend.calls)
        self.assertIn('close_fixture',backend.calls)
        self.assertIn('processes_stopped',backend.calls)
        self.assertIn('clean_shutdown',backend.calls)
    def test_clean_shutdown_exception_is_counted_without_raw_detail(self):
        backend=FakeBackend();backend.shutdown_raises=True
        result=self.run_batch(backend)
        self.assertTrue(result['cleanup_required'])
        row=next(row for row in result['cleanup'] if row['step']=='clean_shutdown')
        self.assertFalse(row['ok'])
        self.assertNotIn('private shutdown detail',repr(result))
    def test_timeout_does_not_advance(self):
        backend=FakeBackend();now=[0]
        def sample(phase):now[0]=1000
        backend.sample=sample
        result=BatchRunner(backend,clock=lambda:now[0],sleep=lambda s:None).run_privacy(confirmed=True)
        self.assertEqual(result['reason'],'active_batch_deadline')
        self.assertEqual(result['phases'],[])
    def test_audio_controls_require_persistence(self):
        backend=FakeBackend();now=[0.0]
        runner=BatchRunner(backend,clock=lambda:now[0],sleep=lambda s:now.__setitem__(0,now[0]+s))
        result=runner.run_audio_output(confirmed=True)
        self.assertEqual(result['status'],'passed_scoped_audio_controls')
        self.assertEqual(len(result['controls']),2)
    def test_silent_audio_does_not_pass_running_worker(self):
        backend=FakeBackend();backend.sample=lambda phase:None;now=[0.0]
        runner=BatchRunner(backend,clock=lambda:now[0],sleep=lambda s:now.__setitem__(0,now[0]+s))
        result=runner.run_audio_output(confirmed=True)
        self.assertEqual(result['reason'],'persisted_audio_positive_control_missing')
        self.assertEqual(result['controls'],[])

    def test_browser_clipboard_sequence_passes_and_restores_after_stop(self):
        backend=BrowserBackend();now=[0.0]
        result=BatchRunner(backend,clock=lambda:now[0],sleep=lambda s:now.__setitem__(0,now[0]+s)).run_browser_clipboard(confirmed=True)
        self.assertEqual(result['status'],'passed_scoped_browser_clipboard_checks')
        self.assertEqual([row['name'] for row in result['phases']],
                         ['browser_allowed','browser_password','browser_excluded','clipboard_plain','clipboard_password'])
        self.assertLess(backend.calls.index('stop_recorder'),backend.calls.index('restore_clipboard'))
        self.assertIn('close_browser_fixture',backend.calls)

    def test_browser_clipboard_password_leak_fails(self):
        backend=BrowserBackend();backend.leak=True;now=[0.0]
        result=BatchRunner(backend,clock=lambda:now[0],sleep=lambda s:now.__setitem__(0,now[0]+s)).run_browser_clipboard(confirmed=True)
        self.assertEqual(result['status'],'failed')
        self.assertEqual(result['reason'],'synthetic_forbidden_marker_persisted')

    def test_browser_clipboard_unverified_stop_never_restores(self):
        backend=BrowserBackend();backend.stop_ok=False;now=[0.0]
        result=BatchRunner(backend,clock=lambda:now[0],sleep=lambda s:now.__setitem__(0,now[0]+s)).run_browser_clipboard(confirmed=True)
        self.assertEqual(result['status'],'incomplete')
        self.assertNotIn('restore_clipboard',backend.calls)
        self.assertTrue(result['cleanup_required'])


if __name__=='__main__':unittest.main()
