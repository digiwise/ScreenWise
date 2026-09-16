"""Inspect only newly synthesized speech preparation, never captured media."""
import array
import hashlib
import json
import math
import wave
from pathlib import Path

root = Path(__file__).resolve().parent
speech = root / 'speech-retry'
manifest = json.loads((speech / 'manifest.json').read_text(encoding='utf-8-sig'))
results = []
for record in manifest['files']:
    path = speech / record['file']
    digest = hashlib.sha256(path.read_bytes()).hexdigest().upper()
    assert digest == record['sha256']
    with wave.open(str(path), 'rb') as wav:
        assert wav.getsampwidth() == 2 and wav.getnchannels() == 1
        assert wav.getcomptype() == 'NONE'
        rate = wav.getframerate()
        duration = wav.getnframes() / rate
        samples = array.array('h', wav.readframes(wav.getnframes()))
    peak = max(abs(sample) for sample in samples)
    rms = math.sqrt(sum(sample * sample for sample in samples) / len(samples)) / 32768
    assert rms > 0.005 and peak < 32767
    if record['name'] == 'tail':
        assert 1.0 < duration < 5.0
    else:
        assert 5.0 < duration < 15.0
    results.append({'name': record['name'], 'duration_seconds': round(duration, 3),
                    'sample_rate_hz': rate, 'rms': round(rms, 5), 'peak': peak,
                    'sha256': digest})
report = {'status': 'synthetic_assets_validated', 'playback_performed': False,
          'files': results, 'limitation': 'No live capture or ASR quality proof.'}
(root / 'speech-validation.json').write_text(json.dumps(report, indent=2), encoding='utf-8')
print(json.dumps(report, indent=2))
