/** 變更行與相等錨點交錯，確保產生 N 個獨立差異 span，而非整段合併。 */
export function buildInterleavedDiffPair(changes: number): { source: string; output: string } {
  const source: string[] = [];
  const output: string[] = [];
  for (let index = 0; index < changes; index += 1) {
    source.push(`变${index}\n`, `锚${index}\n`);
    output.push(`變${index}\n`, `锚${index}\n`);
  }
  return { source: source.join(""), output: output.join("") };
}
