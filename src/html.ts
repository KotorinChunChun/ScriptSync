/** HTML本文と引用符付き属性へ埋め込む値をエスケープする。URLの検証には使用しない。 */
export function escapeHtml(value: unknown): string {
  const entities: Record<string, string> = {
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;',
  };
  return String(value ?? '').replace(/[&<>"']/g, character => entities[character]);
}
