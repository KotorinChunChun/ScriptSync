// 出力データを読み取り、試作専用の対応表へ変換する。元プロジェクトは変更しない。
export function ensure(condition, message) { if (!condition) throw new Error(message); }
export function text(value, max = 100000) {
  ensure(typeof value === 'string' && value.length <= max, '文字列の形式または長さが不正です');
  return value;
}
export function validateMap(map) {
  ensure(map?.schema === 'scriptsync.block-map' && map.schema_version === 1, '対応するblock-map形式は1です');
  text(map.project_id, 200); text(map.export_id, 200);
  ensure(Array.isArray(map.blocks) && map.blocks.length > 0 && map.blocks.length <= 500, 'ブロック数は1〜500件です');
  for (const block of map.blocks) {
    text(block.text); text(block.audio_file, 255);
    ensure(!/[\\/:\x00-\x1f]/.test(block.audio_file) && !block.audio_file.startsWith('.') && /\.(wav|flac)$/i.test(block.audio_file), '音声名は同じフォルダーのWAV/FLACに限ります');
    ensure(Number.isFinite(block.duration_seconds) && block.duration_seconds > 0, '音声時間が不正です');
    ensure(/^[a-f0-9]{64}$/.test(block.audio_hash), '音声ハッシュが不正です');
    ensure(Array.isArray(block.utterances), '発話一覧が必要です');
    block.utterances.forEach(u => text(u.segment_id, 200));
  }
  return map;
}
export function makePlan(map) {
  validateMap(map);
  return { schema: 'scriptsync.presentation-plan', schema_version: 1, project_id: map.project_id, export_id: map.export_id,
    title: 'プレゼンテーション', mode: 'manual', powerpoint_path: '', powerpoint_enabled: false, obs_enabled: false,
    cues: map.blocks.map((b, i) => {
      const lines = b.text.split(/\r?\n/).filter(Boolean);
      return { block_index: i, segment_ids: b.utterances.map(u => u.segment_id), slide_id: null, scene: '',
        title: (lines[0] || `スライド ${i + 1}`).slice(0, 50), bullets: lines.slice(0, 5).map(s => s.slice(0, 100)) };
    }) };
}
export function validatePlan(plan, map) {
  ensure(plan?.schema === 'scriptsync.presentation-plan' && plan.schema_version === 1, '対応表の形式が不正です');
  ensure(plan.project_id === map.project_id && plan.export_id === map.export_id, '別の出力の対応表です。音声を読み込み直してください');
  ensure(['manual', 'auto'].includes(plan.mode), '進行方式が不正です');
  text(plan.title, 100); text(plan.powerpoint_path, 4096);
  ensure(typeof plan.powerpoint_enabled === 'boolean' && typeof plan.obs_enabled === 'boolean', '連携設定が不正です');
  ensure(Array.isArray(plan.cues) && plan.cues.length === map.blocks.length, '対応表の件数が一致しません');
  plan.cues.forEach((c, i) => {
    ensure(c.block_index === i && JSON.stringify(c.segment_ids) === JSON.stringify(map.blocks[i].utterances.map(u => u.segment_id)), '発話の対応が一致しません');
    ensure(c.slide_id === null || (Number.isSafeInteger(c.slide_id) && c.slide_id > 0), 'スライドIDが不正です');
    text(c.scene, 200); text(c.title, 100);
    ensure(Array.isArray(c.bullets) && c.bullets.length <= 6, '箇条書きは6件以内です'); c.bullets.forEach(b => text(b, 140));
  });
  return structuredClone(plan);
}
export function demo() {
  const scripts = ['はじめに\n台本と音声からプレゼンテーションを組み立てます。', '操作の実演\n音声に合わせてスライドとOBSシーンを切り替えます。', 'まとめ\n既存の録音を使って説明資料を作れます。'];
  return {schema:'scriptsync.block-map',schema_version:1,project_id:'demo',export_id:'demo-v1',blocks:scripts.map((s,i)=>({block_index:i,text:s,audio_file:`demo-${i}.wav`,audio_hash:'0'.repeat(64),duration_seconds:4,utterances:[{segment_id:`demo-${i}`,text:s}]}))};
}
export function demoWave(index) {
  const rate = 16000, count = rate * 4, data = Buffer.alloc(44 + count * 2);
  data.write('RIFF'); data.writeUInt32LE(data.length - 8, 4); data.write('WAVEfmt ', 8); data.writeUInt32LE(16,16);
  data.writeUInt16LE(1,20); data.writeUInt16LE(1,22); data.writeUInt32LE(rate,24); data.writeUInt32LE(rate*2,28);
  data.writeUInt16LE(2,32); data.writeUInt16LE(16,34); data.write('data',36); data.writeUInt32LE(count*2,40);
  for(let i=0;i<count;i++){const t=i/rate, envelope=Math.max(0,1-t/0.25);data.writeInt16LE(Math.round(Math.sin(t*Math.PI*2*(440+index*110))*envelope*1800),44+i*2);}
  return data;
}
