"""Whisper単独実行の契約試験。音声データを外部サービスへ渡さない。"""
import io
import json
from pathlib import Path
import tempfile
import subprocess
import sys
import unittest
from unittest.mock import patch
import transcribe

class AdapterTest(unittest.TestCase):
    def test_embedded_prepare_needs_no_script_and_preserves_model_root(self):
        with tempfile.TemporaryDirectory(prefix='whisper space ') as root:
            missing = Path(root) / 'transcribe.py'
            self.assertFalse(missing.exists())
            stub = "import types\nm=types.ModuleType('faster_whisper')\nm.WhisperModel=lambda *a,**k: print(__import__('json').dumps(k))\nsys.modules['faster_whisper']=m\n"
            source = Path(transcribe.__file__).read_text(encoding='utf-8-sig')
            module_dir = Path(root) / 'python' / 'Lib' / 'site-packages'
            module_dir.mkdir(parents=True)
            (module_dir / 'faster_whisper.py').write_text("import json\ndef WhisperModel(*a, **k): print(json.dumps(k))", encoding='utf-8')
            stub = ''
            code = "import sys\nfrom pathlib import Path\n__file__ = sys.argv.pop(1)\nsys.path.insert(0, str(Path(__file__).parent / 'python' / 'Lib' / 'site-packages'))\n" + stub + source
            result = subprocess.run([sys.executable, '-I', '-c', code, str(missing), '--prepare', '--model', 'small'], capture_output=True, text=True, check=True)
            lines = result.stdout.splitlines()
            self.assertEqual(json.loads(lines[0])['download_root'], str(Path(root) / 'models'))
            self.assertEqual(json.loads(lines[1]), {'prepared':'small'})
            self.assertFalse(missing.exists())
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
