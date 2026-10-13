"""ローカルASR。原稿・期待読みは入力しない。漢字からのかなは要聴取の候補。"""
import argparse
import importlib.metadata
import json
import sys
from pathlib import Path

def recognize(args):
    from faster_whisper import WhisperModel
    model = WhisperModel(args.model, device='cpu', compute_type='int8',
                         download_root=str(Path('C:/AI/Models/Speech/Whisper/ScriptSync')),
                         local_files_only=not args.prepare)
    if args.prepare:
        return {'prepared': args.model}
    import pykakasi
    kana = pykakasi.kakasi()
    segments, info = model.transcribe(args.audio, language='ja', beam_size=5,
                                     word_timestamps=True, condition_on_previous_text=False,
                                     initial_prompt=None, vad_filter=False)
    words = []
    texts = []
    for segment in segments:
        texts.append(segment.text)
        for word in segment.words or []:
            words.append({'text': word.word, 'kana': ''.join(x['hira'] for x in kana.convert(word.word)),
                          'start': word.start, 'end': word.end, 'confidence': word.probability})
    raw = ''.join(texts)
    result = {'engine': 'faster-whisper', 'version': importlib.metadata.version('faster-whisper'),
              'model': args.model, 'device': 'cpu', 'compute_type': 'int8', 'raw': raw,
              'kana': ''.join(x['hira'] for x in kana.convert(raw)), 'words': words,
              'duration': info.duration, 'needs_review': True,
              'warning': '漢字ASRからのかな候補です。誤読が漢字に復元されて隠れる可能性があります。実音声を聴いて確認してください。'}
    return result


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--audio')
    p.add_argument('--prepare', action='store_true')
    p.add_argument('--model', default='small', choices=['tiny', 'base', 'small', 'medium', 'large-v3'])
    p.add_argument('--provider', default='local', choices=['local'])
    args = p.parse_args()
    if not args.prepare and (not args.audio or not Path(args.audio).is_file()):
        p.error('音声ファイルがありません')
    result = recognize(args)
    print(json.dumps(result, ensure_ascii=True))

if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(json.dumps({'error': str(error)}, ensure_ascii=True), file=sys.stderr)
        raise SystemExit(1)
