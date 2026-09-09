// 録音・保存済みテイクで共通の固定フルスケール。-60 dBFS未満は表示しない。
export function waveAmplitude(peak:number):number {
 return Number.isFinite(peak)&&peak>=0.001?Math.sqrt(Math.min(1,peak)):0;
}
