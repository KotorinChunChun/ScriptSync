"""Whisper単独実行の契約試験。音声データを外部サービスへ渡さない。"""
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import transcribe

class AdapterTest(unittest.TestCase):
    def test_local_result_retains_raw_japanese(self):
        with tempfile.TemporaryDirectory() as root:
            audio = Path(root) / 'test.wav'
            audio.write_bytes(b'fixture')
            out = io.StringIO()
            with patch('sys.argv', ['transcribe.py', '--audio', str(audio), '--provider', 'local']), patch('sys.stdout', out), patch.object(transcribe, 'recognize', return_value={'raw':'原文です。','engine':'faster-whisper'}) as recognize:
                transcribe.main()
            recognize.assert_called_once()
            self.assertEqual(json.loads(out.getvalue())['raw'], '原文です。')

    def test_old_codex_provider_cannot_start_cli(self):
        with patch('sys.argv', ['transcribe.py', '--provider', 'codex']), patch('sys.stderr', io.StringIO()), patch.object(transcribe, 'recognize') as recognize:
            with self.assertRaises(SystemExit):
                transcribe.main()
            recognize.assert_not_called()

if __name__ == '__main__':
    unittest.main()
