"""Future live adapter. No interaction occurs without --execute-interactive.

Only the fully wired batches are exposed. Other phase plans remain separate
until their owner-action protocol is integrated and reviewed.
"""
from __future__ import annotations
import argparse
import hashlib
import importlib.util
import ipaddress
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import threading
import time
from datetime import datetime, timezone
from urllib.request import Request, ProxyHandler, HTTPRedirectHandler, build_opener
from urllib.error import HTTPError
from urllib.parse import urlencode

from coordinator import BatchRunner, ControlError, ROOT, consume_confirmation, session_path
from browser_clipboard_sequence import ALL_MARKERS as BROWSER_CLIPBOARD_MARKERS
from browser_runtime import BrowserFixtureRuntime, BrowserRuntimeError
import evidence
from fixture_transport import FixtureClient
import windows_controls as windows

PREP = ROOT.parent
RELEASE = Path(os.environ.get('SCREENWISE_VALIDATION_RELEASE_DIR', PREP.parent / 'release'))
FIXTURE = PREP / 'fixtures' / 'PrivacyFixture.exe'
SCOPED = [RELEASE / name for name in ('screenpipe.exe', 'ffmpeg.exe', 'ffprobe.exe')]
MARKERS = list(dict.fromkeys([
    'public cedar garden', 'hidden tulip waterfall', 'forbidden violet orchard',
    'silver cedar', 'amber window', 'forbidden golden harbour', 'crimson bridge',
    *BROWSER_CLIPBOARD_MARKERS,
]))
OUTPUT = os.environ.get('SCREENWISE_VALIDATION_OUTPUT_DEVICE', '<output-device-not-configured>')
API_PORT = int(os.environ.get('SCREENWISE_VALIDATION_API_PORT', '31479'))
FIXTURE_PORT = int(os.environ.get('SCREENWISE_VALIDATION_FIXTURE_PORT', '31480'))
API_URL = f'http://127.0.0.1:{API_PORT}'
LIVE_MODES = ('privacy', 'audio-output', 'drm', 'browser')


def require_runtime_configuration(mode=None):
    required = ('SCREENWISE_VALIDATION_CONFIG', 'SCREENWISE_VALIDATION_REPO_ROOT',
                'SCREENWISE_VALIDATION_RELEASE_DIR', 'SCREENWISE_VALIDATION_POWERSHELL_EXE',
                'SCREENWISE_VALIDATION_CONFIG_SHA256',
                'SCREENWISE_VALIDATION_INPUT_DEVICE', 'SCREENWISE_VALIDATION_OUTPUT_DEVICE',
                'SCREENWISE_VALIDATION_API_PORT', 'OPENBLAS_PATH', 'ORT_LIB_LOCATION')
    if mode == 'browser':
        required += ('SCREENWISE_VALIDATION_BROWSER_EXE',
                     'SCREENWISE_VALIDATION_BROWSER_SHA256',
                     'SCREENWISE_VALIDATION_FIXTURE_PORT')
    if any(not os.environ.get(name) for name in required):
        raise ControlError('explicit_runtime_configuration_required')


def verify_prepared_pins():
    try:
        pins = json.loads((ROOT/'pins.json').read_text(encoding='utf-8'))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ControlError('prepared_asset_pins_missing_or_invalid') from error
    if pins.get('schema') != 'screenwise.prepared-controller-pins.v1' or not isinstance(pins.get('assets'), dict) or not pins['assets']:
        raise ControlError('prepared_asset_pins_missing_or_invalid')
    for relative, digest in pins['assets'].items():
        target = (PREP/relative).resolve()
        if not target.is_relative_to(PREP.resolve()) or not target.is_file() or not isinstance(digest, str) or len(digest) != 64:
            raise ControlError('prepared_asset_pin_invalid')
        if hashlib.sha256(target.read_bytes()).hexdigest().upper() != digest.upper():
            raise ControlError('prepared_asset_hash_changed')
    return {'asset_count': len(pins['assets'])}


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        raise ControlError('local_api_redirect_rejected')


def utc():
    return datetime.now(timezone.utc).isoformat()


class WindowsBackend:
    def __init__(self, record: dict, path: Path):
        self.record, self.path = record, path
        self.data = path / 'data'
        self.fixture_dir = path / 'fixture'
        self.recorder = self.fixture = self.client = None
        self.drm_fixture = self.drm_client = None
        self.browser_runtime = None
        self.browser_surface_active = False
        self.token = None
        self.expected_hwnd = None
        self.expected_pid = None
        self.counter = 0
        self.started = utc()
        self.last_sample = 0.0
        self.stop_lock = threading.RLock()
        self.watchdog_done = threading.Event()
        self.shutdown = None
        self.shutdown_attempts = []
        self.outbound_attempt_samples = 0
        self.opener = build_opener(ProxyHandler({}), NoRedirect())

    def journal(self, kind: str, value: dict):
        with (self.path / 'controller.jsonl').open('a', encoding='utf-8') as f:
            f.write(json.dumps({'utc': utc(), 'kind': kind, **value}) + '\n')

    def quiescent(self):
        inventory = windows.inventory_known_executables(
            self.scoped_paths() + [FIXTURE, FIXTURE.parent/'Netflix.exe'])
        if inventory.get('error_code') or inventory.get('processes'):
            raise ControlError('existing_or_unverified_test_process')
        return True

    def scoped_paths(self):
        paths = list(SCOPED)
        if self.record.get('mode') == 'browser':
            browser = os.environ.get('SCREENWISE_VALIDATION_BROWSER_EXE')
            if not browser or not Path(browser).is_absolute():
                raise ControlError('browser_runtime_configuration_required')
            paths.append(Path(browser))
        return paths

    def preflight(self):
        self.quiescent()
        expected = {
            'OPENBLAS_PATH': os.environ['OPENBLAS_PATH'],
            'CMAKE_GENERATOR': 'Ninja',
            'ORT_LIB_LOCATION': os.environ['ORT_LIB_LOCATION'],
        }
        if any(os.environ.get(k, '').rstrip('\\').lower() != v.rstrip('\\').lower() for k, v in expected.items()):
            raise ControlError('required_developer_environment_missing')
        if not os.environ.get('VSCMD_VER'):
            raise ControlError('developer_powershell_required')
        for name in ('ffmpeg.exe', 'ffprobe.exe'):
            found = shutil.which(name)
            if not found or Path(found).resolve() != (RELEASE/name).resolve():
                raise ControlError('helper_resolution_mismatch')
        verify_prepared_pins()
        # Keep the same PowerShell generation as the outer launcher. A nested
        # Windows PowerShell 5 session can inherit incompatible PS7 module paths.
        powershell = os.environ['SCREENWISE_VALIDATION_POWERSHELL_EXE']
        command = [powershell, '-NoProfile', '-File', str(PREP/'preflight.ps1'),
                   '-ConfigPath', os.environ['SCREENWISE_VALIDATION_CONFIG'],
                   '-ExpectedConfigSha256', os.environ['SCREENWISE_VALIDATION_CONFIG_SHA256'],
                   '-RequireState', 'unlocked']
        if self.record.get('mode') == 'browser':
            command.append('-RequireBrowser')
        self.journal('command', {'argv': command})
        with (self.path/'preflight-output.txt').open('xb') as output:
            check = subprocess.run(command, stdout=output, stderr=subprocess.STDOUT,
                                   timeout=90, creationflags=subprocess.CREATE_NO_WINDOW)
        if check.returncode:
            raise ControlError('preflight_failed')

    def session_state(self):
        return windows.session_state()['state']

    def start_fixture(self):
        args = [str(FIXTURE), '--run-id', self.record['run_id'], '--out-dir', str(self.fixture_dir)]
        self.journal('command', {'argv': args})
        self.fixture = windows.OwnedProcess.start(args, FIXTURE.parent,
            self.path/'fixture-stdout.txt', self.path/'fixture-stderr.txt', visible=True)
        until = time.monotonic()+10
        while not (self.fixture_dir/'ready.json').is_file():
            if self.fixture.poll() is not None or time.monotonic() >= until:
                raise ControlError('fixture_startup_failed')
            if self.session_state() != 'unlocked':
                raise ControlError('session_changed_during_fixture_start')
            time.sleep(.1)
        self.client = FixtureClient(self.fixture_dir, self.record['run_id'])
        self.fixture_action('plain', 'initial-control')

    def start_recorder(self):
        self.data.mkdir(exist_ok=False)
        (self.data/'.controller-owned').write_text(self.record['run_id'], encoding='utf-8')
        spec = importlib.util.spec_from_file_location('prepared_plan', PREP/'plan.py')
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        plan = module.make_plan(self.record['mode'], self.record['run_id'])
        args = plan['argv']
        args[args.index('--data-dir')+1] = str(self.data)
        if self.record['mode'] == 'audio-output':
            args += ['--disable-vision','--disable-keyboard-capture','--disable-clipboard-capture',
                     '--included-windows','ScreenWise Synthetic Privacy Fixture']
        elif self.record['mode'] == 'privacy':
            args += ['--idle-capture-interval-ms','1000', '--min-capture-interval-ms','500',
                     '--included-windows','ScreenWise Synthetic Privacy Fixture',
                     '--included-windows','::SW EXCLUDED Synthetic Fixture']
        elif self.record['mode'] == 'drm':
            args += ['--disable-keyboard-capture','--disable-clipboard-capture',
                     '--included-windows','ScreenWise Synthetic Privacy Fixture',
                     '--included-windows','ScreenWise SYNTHETIC DRM TEST']
        elif self.record['mode'] == 'browser':
            args += ['--idle-capture-interval-ms','1000', '--min-capture-interval-ms','500',
                     '--included-windows','ScreenWise Synthetic Browser Fixture',
                     '--included-windows','ScreenWise Synthetic Privacy Fixture']
        self.journal('command', {'argv': args})
        self.recorder = windows.OwnedProcess.start(args, RELEASE, self.data/'stdout.log', self.data/'stderr.log')
        self.journal('recorder_identity', {'pid': self.recorder.pid, 'creation_ticks': self.recorder.creation_time_ticks})
        # Independent guard remains effective if an API or fixture wait gets stuck.
        threading.Thread(target=self._watchdog, daemon=True).start()

    def _watchdog(self):
        if not self.watchdog_done.wait(240):
            try:
                self.stop_playback()
            finally:
                for _ in range(2):
                    try:
                        if self.stop_recorder(): break
                    except Exception: pass
                self.journal('watchdog', {'reason': 'recording_safety_deadline'})

    def request(self, path: str, authenticated=True):
        if not path.startswith('/') or path.startswith('//'):
            raise ControlError('invalid_local_route')
        headers = {'Authorization': 'Bearer '+self.token} if authenticated and self.token else {}
        try:
            with self.opener.open(Request(API_URL+path, headers=headers), timeout=3) as response:
                return response.status, response.read(256*1024)
        except HTTPError as error:
            code = error.code
            error.close()
            return code, b''

    def await_api(self):
        until = time.monotonic()+45
        while time.monotonic() < until:
            if self.recorder.poll() is not None or self.session_state() != 'unlocked':
                raise ControlError('recorder_startup_interrupted')
            try:
                if self.request('/health', False)[0] == 200: break
            except OSError: pass
            time.sleep(.25)
        else: raise ControlError('api_startup_timeout')
        command = [str(SCOPED[0]), 'auth', 'token', '--data-dir', str(self.data)]
        self.journal('command', {'argv': command, 'output': 'secret withheld'})
        auth = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                              timeout=10, creationflags=subprocess.CREATE_NO_WINDOW)
        token = auth.stdout.decode('utf-8').strip()
        if auth.returncode or not token.startswith('sp-'):
            raise ControlError('matching_auth_token_unavailable')
        self.token = token

    def api_checks(self):
        result = evidence.verify_api(API_URL, self.token, self.started, utc())
        expected = [OUTPUT] if self.record['mode'] in ('audio-output','drm') else []
        until=time.monotonic()+20
        while True:
            code, raw = self.request('/audio/device/status')
            value = json.loads(raw) if code == 200 else None
            if not isinstance(value, list): raise ControlError('audio_status_schema_invalid')
            running = [row.get('name') for row in value if row.get('is_running') is True]
            if any(name not in expected for name in running):
                raise ControlError('unexpected_audio_device_running')
            if sorted(running)==sorted(expected) or time.monotonic()>=until:break
            if self.session_state()!='unlocked' or not self.foreground_valid('startup'):
                raise ControlError('state_changed_during_audio_startup')
            time.sleep(.25)
        result['exact_audio_selection'] = sorted(running) == sorted(expected)
        result['passed'] = result.get('passed') is True and result['exact_audio_selection']
        if expected and result['passed']:
            # Device/status reflects configured selection before delayed startup.
            # Health's live-handle list is populated only after model setup.
            until=time.monotonic()+60
            while True:
                if self.recorder.poll() is not None or self.session_state()!='unlocked' or not self.foreground_valid('startup'):
                    raise ControlError('state_changed_during_audio_startup')
                code,raw=self.request('/health',False)
                state=audio_health_status(json.loads(raw) if code==200 else {})
                if state['unexpected_device']:raise ControlError('unexpected_audio_device_running')
                if state['capture_handle_ready']:
                    self.journal('audio_capture_ready',state)
                    result['capture_handle_ready']=True
                    break
                if time.monotonic()>=until:raise ControlError('audio_capture_startup_timeout')
                time.sleep(.25)
        return result

    def fixture_action(self, action, phase):
        self.counter += 1
        ack = self.client.send(f'{self.counter:04d}-{phase}', action, expected_pid=self.fixture.pid)
        if ack.get('success') is not True or ack.get('verifiedForeground') is not True:
            raise ControlError('fixture_focus_not_verified')
        self.expected_hwnd = ack.get('foregroundHwnd')
        self.expected_pid = self.fixture.pid
        if type(self.expected_hwnd) is not int or self.expected_hwnd <= 0:
            raise ControlError('fixture_hwnd_missing')
        self.journal('fixture_ack', ack)

    def foreground_valid(self, phase):
        if self.record.get('mode') == 'browser' and self.browser_surface_active:
            return self.browser_runtime is not None and self.browser_runtime.foreground_valid()
        identity = windows.foreground_identity()
        target = self.drm_fixture if self.drm_fixture is not None and self.expected_pid==self.drm_fixture.pid else self.fixture
        return (target is not None and target.poll() is None
                and identity['pid'] == self.expected_pid and identity['hwnd'] == self.expected_hwnd)

    def start_browser_fixture(self):
        if self.record.get('mode') != 'browser' or self.browser_runtime is not None:
            raise ControlError('browser_fixture_state_invalid')
        try:
            self.browser_runtime = BrowserFixtureRuntime(
                os.environ['SCREENWISE_VALIDATION_BROWSER_EXE'], FIXTURE_PORT,
                PREP/'fixtures'/'browser-fixture.html', self.path,
                self.record['run_id'])
            url = self.browser_runtime.start_server()
        except (BrowserRuntimeError, KeyError) as error:
            raise ControlError('browser_fixture_start_failed') from error
        self.journal('browser_server', {'url_origin': f'http://127.0.0.1:{FIXTURE_PORT}',
                                        'route': '/browser-fixture.html'})
        return url

    def _fixture_background_action(self, action, phase):
        self.counter += 1
        ack = self.client.send(f'{self.counter:04d}-{phase}', action,
                               expected_pid=self.fixture.pid)
        if ack.get('success') is not True:
            raise ControlError('fixture_action_failed')
        self.journal('fixture_ack', ack)

    def browser_clipboard_phase(self, phase):
        if self.record.get('mode') != 'browser' or self.browser_runtime is None:
            raise ControlError('browser_fixture_not_started')
        browser_phases = {
            'browser_allowed': ('127.0.0.1', 'ordinary'),
            'browser_password': ('127.0.0.1', 'secret'),
            'browser_excluded': ('localhost', 'ordinary'),
        }
        try:
            if phase in browser_phases:
                self._fixture_background_action('browser-handoff', phase+'-handoff')
                host, focus = browser_phases[phase]
                identity = self.browser_runtime.show_phase(host, phase, focus)
                self.expected_pid = identity['pid']
                self.expected_hwnd = identity['hwnd']
                self.browser_surface_active = True
            elif phase == 'clipboard_plain':
                if self.browser_runtime.stop_browser().get('stopped') is not True:
                    raise ControlError('browser_process_tree_not_stopped')
                self.browser_surface_active = False
                self.fixture_action('plain-copy', phase+'-copy')
                self.fixture_action('plain-paste', phase+'-paste')
            elif phase == 'clipboard_password':
                self.browser_surface_active = False
                self.fixture_action('stage-password-clipboard', phase+'-stage')
                self.fixture_action('password-paste', phase+'-paste')
            else:
                raise ControlError('unknown_browser_clipboard_phase')
        except BrowserRuntimeError as error:
            raise ControlError('browser_phase_setup_failed') from error

    def browser_clipboard_marker_counts(self):
        hits = self.aggregate()['marker_hit_counts']
        return {marker: sum(surface.get(marker, 0) for surface in hits.values())
                for marker in BROWSER_CLIPBOARD_MARKERS}

    def close_browser_fixture(self):
        self.browser_surface_active = False
        if self.browser_runtime is None:
            return {'browser_stopped': True, 'server_stopped': True}
        result = self.browser_runtime.close()
        self.journal('browser_cleanup', {
            'browser_stopped': result.get('browser_stopped') is True,
            'server_stopped': result.get('server_stopped') is True,
        })
        return result

    def start_drm(self):
        executable=FIXTURE.parent/'Netflix.exe'
        out=self.path/'drm-fixture'
        args=[str(executable),'--run-id',self.record['run_id'],'--phase-id','drm-start','--out-dir',str(out)]
        self.journal('command',{'argv':args})
        self.drm_fixture=windows.OwnedProcess.start(args,executable.parent,self.path/'drm-stdout.txt',self.path/'drm-stderr.txt',visible=True)
        until=time.monotonic()+10
        while not (out/'drm-ready-drm-start.json').is_file():
            if self.drm_fixture.poll() is not None or time.monotonic()>=until or self.session_state()!='unlocked':
                raise ControlError('drm_fixture_startup_failed')
            time.sleep(.1)
        self.drm_client=FixtureClient(out,self.record['run_id'],kind='drm',ready_phase_id='drm-start')
        self.counter+=1
        ack=self.drm_client.send(f'{self.counter:04d}-drm-show','show',expected_pid=self.drm_fixture.pid)
        if ack.get('success') is not True or ack.get('verifiedForeground') is not True:
            raise ControlError('drm_fixture_focus_unverified')
        self.expected_hwnd=ack['foregroundHwnd'];self.expected_pid=self.drm_fixture.pid
        self.journal('drm_ack',ack)

    def stop_drm(self):
        if self.drm_fixture is None or self.drm_fixture.poll() is not None:return True
        try:
            self.counter+=1
            self.drm_client.send(f'{self.counter:04d}-drm-close','close',expected_pid=self.drm_fixture.pid)
            until=time.monotonic()+3
            while self.drm_fixture.poll() is None and time.monotonic()<until:time.sleep(.05)
            if self.drm_fixture.poll() is not None:return True
        except Exception:pass
        return self.drm_fixture.terminate_owned().get('stopped') is True

    def drm_paused(self):
        code,raw=self.request('/health',False)
        value=json.loads(raw).get('drm_content_paused') if code==200 else None
        if type(value) is not bool:raise ControlError('drm_state_unknown')
        return value

    def forbidden_count(self):
        hits=self.aggregate()['marker_hit_counts']
        return sum(v.get(m,0) for v in hits.values() for m in MARKERS[5:])

    def aggregate(self):
        return evidence.collect_db(self.data, MARKERS)

    def counts(self):
        data = self.aggregate()
        hits = data['marker_hit_counts']
        return {'allowed': sum(v.get(MARKERS[0],0) for v in hits.values()),
                'forbidden':sum(v.get(m,0) for v in hits.values() for m in MARKERS[1:3])}

    def audio_marker_count(self, marker):
        count=self.aggregate()['marker_hit_counts'].get('audio_transcriptions',{}).get(marker,0)
        if count:
            code,body=self.request('/search?'+urlencode({'content_type':'audio','q':marker,'limit':10}))
            value=json.loads(body) if code==200 else {}
            if not isinstance(value,dict) or not isinstance(value.get('data'),list) or not value['data']:
                return 0
        return count

    def final_evidence(self):
        if not (self.data/'db.sqlite').is_file():return {'available':False}
        result=self.aggregate()
        result['available']=True
        result['outbound_attempt_samples']=self.outbound_attempt_samples
        result['network_limits']='Sampled TCP/UDP metadata only; no packet-drop proof or continuous coverage.'
        if self.record['mode']=='privacy':result['forbidden_count']=self.counts()['forbidden']
        elif self.record['mode']=='drm':result['forbidden_count']=self.forbidden_count()
        elif self.record['mode']=='browser':
            hits=result['marker_hit_counts']
            always_forbidden=('hidden coral orchard','forbidden cyan orchard','hidden tulip waterfall')
            result['forbidden_count']=sum(v.get(m,0) for v in hits.values() for m in always_forbidden)
        return result

    def sample(self, phase):
        if self.recorder.poll() is not None: raise ControlError('recorder_exited')
        if time.monotonic()-self.last_sample < 1: return
        self.last_sample=time.monotonic()
        inventory=windows.inventory_known_executables(self.scoped_paths())
        if inventory.get('error_code'):raise ControlError('process_inventory_unavailable')
        if not any(row['pid']==self.recorder.pid and
                   row['creation_time_ticks']==self.recorder.creation_time_ticks and
                   Path(row['executable_path']).resolve()==SCOPED[0].resolve()
                   for row in inventory['processes']):
            raise ControlError('owned_recorder_missing_from_inventory')
        network=windows.inspect_network_endpoints([row['pid'] for row in inventory['processes']])
        if network.get('error_code'):raise ControlError('network_sample_unavailable')
        self.journal('sample', {'phase':phase, 'os_state':self.session_state(),
            'endpoints':network['endpoints'], 'audio_metrics':evidence.api_snapshot(API_URL,self.token)})
        if self.record['mode'] in ('audio-output','drm'):
            code,raw=self.request('/health',False)
            self.journal('audio_delivery',audio_health_status(json.loads(raw) if code==200 else {}))
        for endpoint in network['endpoints']:
            if endpoint['protocol']=='tcp':
                if endpoint['state']=='listening' and not is_loopback(endpoint['local_address']):
                    raise ControlError('non_loopback_listener_observed')
                remote=endpoint['remote_address']
                if remote not in ('0.0.0.0','::','*') and not is_loopback(remote):
                    if endpoint['state']=='syn_sent':
                        self.outbound_attempt_samples+=1
                        self.journal('network_warning', {'reason':'non_loopback_attempt_observed',
                            'successful_connection':False})
                    elif endpoint['state']!='listening':
                        raise ControlError('non_loopback_connection_observed')

    def play_stimulus(self, name):
        if self.record['mode'] not in ('audio-output','drm') or name not in ('before','during','after'):
            raise ControlError('playback_not_authorized_for_mode')
        import winsound
        manifest=json.loads((PREP/'audio'/'manifest.json').read_text(encoding='utf-8-sig'))
        item=next(v for v in manifest['files'] if v['id']==f'phase-{name}-10s')
        file=PREP/'audio'/item['file']
        if hashlib.sha256(file.read_bytes()).hexdigest().upper()!=item['sha256']:
            raise ControlError('stimulus_hash_mismatch')
        self.journal('stimulus', {'id':item['id'], 'repeat':2, 'seconds':item['durationSeconds']*2,
                                 'route':'Windows default playback; selected capture must prove phrase persistence'})
        winsound.PlaySound(str(file),winsound.SND_FILENAME|winsound.SND_ASYNC|winsound.SND_LOOP|winsound.SND_NODEFAULT)
        return item['durationSeconds']*2

    def stop_playback(self):
        if self.record['mode'] in ('audio-output','drm'):
            import winsound
            winsound.PlaySound(None,0)
        return True

    def stop_recorder(self):
        with self.stop_lock:
            if self.recorder is None:return True
            if self.shutdown is None or self.recorder.poll() is None:
                # Allow the manager's bounded drain and the final safe-notice
                # writer to finish before classifying shutdown as forced.
                self.shutdown=self.recorder.graceful_stop(timeout=40)
                self.shutdown_attempts.append(dict(self.shutdown))
                self.journal('recorder_shutdown', self.shutdown)
            stopped=self.recorder.poll() is not None
            if stopped:self.watchdog_done.set()
            return stopped

    def clean_shutdown(self):
        return self.recorder is None or (self.shutdown is not None and
            not any(row.get('forced') is True for row in self.shutdown_attempts) and
            self.shutdown.get('exit_code') == 0)

    def release_focus(self):
        if self.drm_fixture is not None and self.drm_fixture.poll() is None:
            if self.drm_client is None:return False
            self.counter+=1
            ack=self.drm_client.send(f'{self.counter:04d}-drm-release','release-focus',expected_pid=self.drm_fixture.pid)
            visibility=self.drm_fixture.window_visibility()
            if ack.get('success') is not True or visibility.get('has_visible_window') is not False or visibility.get('error_code'):
                return False
        if self.fixture is None or self.fixture.poll() is not None:return True
        if self.client is None:return False
        self.counter+=1
        ack=self.client.send(f'{self.counter:04d}-release-focus','release-focus',expected_pid=self.fixture.pid)
        visibility=self.fixture.window_visibility()
        return ack.get('success') is True and visibility.get('has_visible_window') is False and not visibility.get('error_code')

    def restore_clipboard(self):
        if self.recorder is not None and self.recorder.poll() is None:
            return False
        if self.record.get('mode') != 'browser':
            return True
        if self.client is None or self.fixture is None or self.fixture.poll() is not None:
            return False
        self.counter += 1
        ack = self.client.send(f'{self.counter:04d}-restore-clipboard',
                               'restore-clipboard-after-recorder-stop',
                               expected_pid=self.fixture.pid,
                               recorder_stopped=True)
        terminal = (ack.get('success') is True or
                    ack.get('errorCode') == 'clipboard_changed_external_preserved')
        if terminal:
            self.journal('clipboard_restore', {
                'restored': ack.get('success') is True,
                'external_change_preserved': ack.get('errorCode') == 'clipboard_changed_external_preserved',
            })
        return terminal

    def close_fixture(self):
        if self.fixture is None or self.fixture.poll() is not None:return True
        try:
            self.counter+=1
            ack=self.client.send(f'{self.counter:04d}-close','close',expected_pid=self.fixture.pid)
            if ack.get('success') is not True:return False
            until=time.monotonic()+3
            while self.fixture.poll() is None and time.monotonic()<until:time.sleep(.05)
            if self.fixture.poll() is not None:return True
        except Exception:pass
        return self.fixture.terminate_owned().get('stopped') is True

    def processes_stopped(self):
        self.token=None
        until=time.monotonic()+5
        while True:
            try:return self.quiescent()
            except ControlError:
                if time.monotonic()>=until:return False
                time.sleep(.1)


def audio_health_status(payload):
    pipeline=payload.get('audio_pipeline') or {}
    devices=pipeline.get('audio_devices') or []
    levels=pipeline.get('per_device_audio_level_rms') or {}
    value=levels.get(OUTPUT) if isinstance(levels,dict) else None
    return {'capture_handle_ready':devices==[OUTPUT],
            'unexpected_device':any(name!=OUTPUT for name in devices),
            'callback_observed':isinstance(levels,dict) and OUTPUT in levels,
            'nonzero_signal_observed':type(value) in (int,float) and value>0}


def is_loopback(address):
    try:
        value=ipaddress.ip_address(address)
        return value.is_loopback or bool(getattr(value,'ipv4_mapped',None) and value.ipv4_mapped.is_loopback)
    except ValueError:return False


def main(argv=None):
    parser=argparse.ArgumentParser(description='Preview only unless explicit interactive execution and fresh owner nonce are supplied')
    parser.add_argument('--run-id',required=True)
    parser.add_argument('--owner-ready')
    parser.add_argument('--execute-interactive',action='store_true')
    args=parser.parse_args(argv)
    path=session_path(args.run_id)
    record=json.loads((path/'waiting.json').read_text(encoding='utf-8'))
    if not args.execute_interactive:
        print(json.dumps({'state':'waiting_for_owner','recording_started':False,'mode':record['mode'],'prompt':record['prompt']}))
        return 0
    require_runtime_configuration(record['mode'])
    if record['mode'] not in LIVE_MODES:raise ControlError('mode_not_yet_wired_for_live_execution')
    verify_prepared_pins()
    backend=WindowsBackend(record,path)
    backend.quiescent()
    if backend.session_state()!='unlocked':raise ControlError('session_not_known_unlocked')
    record=consume_confirmation(args.run_id,args.owner_ready,recorder_stopped=True,fixtures_hidden=True)
    runner=BatchRunner(backend)
    if record['mode']=='drm':
        from drm_sequence import DrmRunner
        result=DrmRunner(backend).run(confirmed=True)
    elif record['mode']=='browser':
        result=runner.run_browser_clipboard(confirmed=True)
    else:
        result=(runner.run_privacy(confirmed=True) if record['mode']=='privacy' else runner.run_audio_output(confirmed=True))
    (path/'result.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
    print(json.dumps(result,indent=2))
    return 0 if result['status'].startswith('passed_') else 1


if __name__=='__main__':
    try:raise SystemExit(main())
    except ControlError as error:raise SystemExit(str(error))
