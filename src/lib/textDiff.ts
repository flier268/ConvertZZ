export type DiffKind = "equal" | "insert" | "delete";

export interface DiffPart {
  value: string;
  kind: DiffKind;
}

export interface SideBySideSpan {
  text: string;
  kind: "equal" | "change";
}

const CHAR_DIFF_LIMIT = 4_000;
/** 超過此面積改走錨點／對齊，避免 O(n·m) 爆開。 */
const DENSE_AREA_LIMIT = 250_000;

/** 以 Unicode 碼點做字元級 diff；過長時改為逐行比對。 */
export function diffText(source: string, output: string): DiffPart[] {
  if (source === output) return source ? [{ value: source, kind: "equal" }] : [];
  const sourceUnits = Array.from(source);
  const outputUnits = Array.from(output);
  if (sourceUnits.length + outputUnits.length <= CHAR_DIFF_LIMIT) {
    return mergeAdjacent(diffUnits(sourceUnits, outputUnits));
  }
  return mergeAdjacent(diffByLines(source, output));
}

export function buildSideBySideDiff(
  source: string,
  output: string,
): { left: SideBySideSpan[]; right: SideBySideSpan[] } {
  const left: SideBySideSpan[] = [];
  const right: SideBySideSpan[] = [];
  for (const part of diffText(source, output)) {
    if (part.kind === "equal") {
      pushSpan(left, part.value, "equal");
      pushSpan(right, part.value, "equal");
    } else if (part.kind === "delete") {
      pushSpan(left, part.value, "change");
    } else {
      pushSpan(right, part.value, "change");
    }
  }
  return { left, right };
}

/** 長文並排預覽預設每頁碼點數；過長時分頁避免一次捲動過深。 */
export const DEFAULT_DIFF_PAGE_SIZE = 2_000;

export interface DiffPage {
  left: SideBySideSpan[];
  right: SideBySideSpan[];
  hasChanges: boolean;
}

type DiffSegment =
  { kind: "equal"; text: string } | { kind: "change"; left: string; right: string };

/** 依碼點預算切成多頁，變更段盡量整段放入同一頁，避免簡繁成對差異被拆開。 */
export function buildPagedSideBySideDiff(
  source: string,
  output: string,
  pageSize = DEFAULT_DIFF_PAGE_SIZE,
): DiffPage[] {
  if (pageSize <= 0) {
    const sides = buildSideBySideDiff(source, output);
    return [
      {
        left: sides.left,
        right: sides.right,
        hasChanges:
          sides.left.some((span) => span.kind === "change") ||
          sides.right.some((span) => span.kind === "change"),
      },
    ];
  }

  const segments = partsToSegments(diffText(source, output));
  if (!segments.length) return [{ left: [], right: [], hasChanges: false }];

  const pages: DiffPage[] = [];
  let left: SideBySideSpan[] = [];
  let right: SideBySideSpan[] = [];
  let weight = 0;
  let hasChanges = false;

  const flush = () => {
    if (!left.length && !right.length) return;
    pages.push({ left, right, hasChanges });
    left = [];
    right = [];
    weight = 0;
    hasChanges = false;
  };

  const appendEqual = (text: string) => {
    if (!text) return;
    pushSpan(left, text, "equal");
    pushSpan(right, text, "equal");
  };

  const appendChange = (leftText: string, rightText: string) => {
    if (leftText) {
      pushSpan(left, leftText, "change");
      hasChanges = true;
    }
    if (rightText) {
      pushSpan(right, rightText, "change");
      hasChanges = true;
    }
  };

  for (const segment of segments) {
    if (segment.kind === "equal") {
      const units = Array.from(segment.text);
      let offset = 0;
      while (offset < units.length) {
        if (weight >= pageSize) flush();
        const take = Math.min(pageSize - weight, units.length - offset);
        appendEqual(units.slice(offset, offset + take).join(""));
        weight += take;
        offset += take;
      }
      continue;
    }

    const leftUnits = Array.from(segment.left);
    const rightUnits = Array.from(segment.right);
    const segmentWeight = Math.max(leftUnits.length, rightUnits.length);
    if (!segmentWeight) continue;

    if (segmentWeight <= pageSize) {
      if (weight > 0 && weight + segmentWeight > pageSize) flush();
      appendChange(segment.left, segment.right);
      weight += segmentWeight;
      continue;
    }

    if (weight > 0) flush();
    let cursor = 0;
    while (cursor < segmentWeight) {
      if (weight >= pageSize) flush();
      const take = Math.min(pageSize - weight, segmentWeight - cursor);
      const next = cursor + take;
      const leftFrom = mapProportion(cursor, segmentWeight, leftUnits.length);
      const leftTo = mapProportion(next, segmentWeight, leftUnits.length);
      const rightFrom = mapProportion(cursor, segmentWeight, rightUnits.length);
      const rightTo = mapProportion(next, segmentWeight, rightUnits.length);
      appendChange(
        leftUnits.slice(leftFrom, leftTo).join(""),
        rightUnits.slice(rightFrom, rightTo).join(""),
      );
      weight += take;
      cursor = next;
    }
  }

  flush();
  return pages.length ? pages : [{ left: [], right: [], hasChanges: false }];
}

function partsToSegments(parts: DiffPart[]): DiffSegment[] {
  const segments: DiffSegment[] = [];
  let index = 0;
  while (index < parts.length) {
    const part = parts[index]!;
    if (part.kind === "equal") {
      segments.push({ kind: "equal", text: part.value });
      index += 1;
      continue;
    }
    let left = "";
    let right = "";
    while (index < parts.length && parts[index]!.kind !== "equal") {
      const current = parts[index]!;
      if (current.kind === "delete") left += current.value;
      else right += current.value;
      index += 1;
    }
    segments.push({ kind: "change", left, right });
  }
  return segments;
}

function mapProportion(position: number, total: number, length: number): number {
  if (!total || !length) return 0;
  if (position >= total) return length;
  return Math.floor((position * length) / total);
}

export function escapeHtml(text: string): string {
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

/** 將並排 span 轉成可安全塞進 v-html 的標記字串。 */
export function sideBySideToHtml(spans: SideBySideSpan[], changeClass: string): string {
  let html = "";
  for (const span of spans) {
    const text = escapeHtml(span.text);
    if (span.kind === "change") {
      html += `<mark class="diff-change ${changeClass}">${text}</mark>`;
    } else {
      html += text;
    }
  }
  return html;
}

function pushSpan(spans: SideBySideSpan[], text: string, kind: SideBySideSpan["kind"]) {
  if (!text) return;
  const last = spans[spans.length - 1];
  if (last && last.kind === kind) last.text += text;
  else spans.push({ text, kind });
}

function diffByLines(source: string, output: string): DiffPart[] {
  const sourceLines = splitLines(source);
  const outputLines = splitLines(output);
  const lineParts = diffUnits(sourceLines, outputLines);
  const parts: DiffPart[] = [];
  let index = 0;
  while (index < lineParts.length) {
    const part = lineParts[index]!;
    if (part.kind === "equal") {
      parts.push(part);
      index += 1;
      continue;
    }
    const deleted: string[] = [];
    const inserted: string[] = [];
    while (index < lineParts.length && lineParts[index]!.kind !== "equal") {
      const current = lineParts[index]!;
      if (current.kind === "delete") deleted.push(current.value);
      else inserted.push(current.value);
      index += 1;
    }
    const deletedText = deleted.join("");
    const insertedText = inserted.join("");
    // 長段也走 diffUnits（錨點／對齊），避免整行被標成單一變更。
    parts.push(...diffUnits(Array.from(deletedText), Array.from(insertedText)));
  }
  return parts;
}

function splitLines(text: string): string[] {
  if (!text) return [];
  const lines: string[] = [];
  let start = 0;
  for (let index = 0; index < text.length; index += 1) {
    if (text[index] === "\n") {
      lines.push(text.slice(start, index + 1));
      start = index + 1;
    }
  }
  if (start < text.length) lines.push(text.slice(start));
  return lines;
}

function diffUnits(source: string[], output: string[]): DiffPart[] {
  const n = source.length;
  const m = output.length;
  if (!n && !m) return [];
  if (!n) return [{ value: output.join(""), kind: "insert" }];
  if (!m) return [{ value: source.join(""), kind: "delete" }];

  const area = n * m;
  if (area <= DENSE_AREA_LIMIT) return diffUnitsDense(source, output);

  const anchors = findUniqueAnchors(source, output);
  if (!anchors.length) return diffUnitsAligned(source, output);

  const parts: DiffPart[] = [];
  let sourceIndex = 0;
  let outputIndex = 0;
  for (const anchor of anchors) {
    if (anchor.sourceIndex > sourceIndex || anchor.outputIndex > outputIndex) {
      parts.push(
        ...diffUnits(
          source.slice(sourceIndex, anchor.sourceIndex),
          output.slice(outputIndex, anchor.outputIndex),
        ),
      );
    }
    parts.push({ value: source[anchor.sourceIndex]!, kind: "equal" });
    sourceIndex = anchor.sourceIndex + 1;
    outputIndex = anchor.outputIndex + 1;
  }
  if (sourceIndex < n || outputIndex < m) {
    parts.push(...diffUnits(source.slice(sourceIndex), output.slice(outputIndex)));
  }
  return parts;
}

function findUniqueAnchors(
  source: string[],
  output: string[],
): Array<{ sourceIndex: number; outputIndex: number }> {
  const sourceCounts = countOccurrences(source);
  const outputCounts = countOccurrences(output);
  const outputIndexByValue = new Map<string, number>();
  for (let index = 0; index < output.length; index += 1) {
    const value = output[index]!;
    if (sourceCounts.get(value) === 1 && outputCounts.get(value) === 1) {
      outputIndexByValue.set(value, index);
    }
  }

  const anchors: Array<{ sourceIndex: number; outputIndex: number }> = [];
  let lastOutputIndex = -1;
  for (let sourceIndex = 0; sourceIndex < source.length; sourceIndex += 1) {
    const outputIndex = outputIndexByValue.get(source[sourceIndex]!);
    if (outputIndex === undefined || outputIndex <= lastOutputIndex) continue;
    anchors.push({ sourceIndex, outputIndex });
    lastOutputIndex = outputIndex;
  }
  return anchors;
}

function countOccurrences(values: string[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const value of values) counts.set(value, (counts.get(value) ?? 0) + 1);
  return counts;
}

/** 去頭尾相同前綴／後綴後，等長中段做位置對齊，否則必要時再跑密集 DP。 */
function diffUnitsAligned(source: string[], output: string[]): DiffPart[] {
  let start = 0;
  while (start < source.length && start < output.length && source[start] === output[start]) {
    start += 1;
  }
  let sourceEnd = source.length;
  let outputEnd = output.length;
  while (
    sourceEnd > start &&
    outputEnd > start &&
    source[sourceEnd - 1] === output[outputEnd - 1]
  ) {
    sourceEnd -= 1;
    outputEnd -= 1;
  }

  const parts: DiffPart[] = [];
  if (start > 0) parts.push({ value: source.slice(0, start).join(""), kind: "equal" });

  const middleSource = source.slice(start, sourceEnd);
  const middleOutput = output.slice(start, outputEnd);
  if (!middleSource.length && !middleOutput.length) {
    // only prefix/suffix
  } else if (middleSource.length * middleOutput.length <= DENSE_AREA_LIMIT) {
    parts.push(...diffUnitsDense(middleSource, middleOutput));
  } else if (middleSource.length === middleOutput.length) {
    for (let index = 0; index < middleSource.length; index += 1) {
      const left = middleSource[index]!;
      const right = middleOutput[index]!;
      if (left === right) parts.push({ value: left, kind: "equal" });
      else {
        parts.push({ value: left, kind: "delete" });
        parts.push({ value: right, kind: "insert" });
      }
    }
  } else {
    if (middleSource.length) parts.push({ value: middleSource.join(""), kind: "delete" });
    if (middleOutput.length) parts.push({ value: middleOutput.join(""), kind: "insert" });
  }

  if (sourceEnd < source.length) {
    parts.push({ value: source.slice(sourceEnd).join(""), kind: "equal" });
  }
  return parts;
}

function diffUnitsDense(source: string[], output: string[]): DiffPart[] {
  const n = source.length;
  const m = output.length;
  if (!n && !m) return [];
  if (!n) return [{ value: output.join(""), kind: "insert" }];
  if (!m) return [{ value: source.join(""), kind: "delete" }];

  // Levenshtein：同等長度的簡繁替換會對齊成一對一變更，而不是整段刪除再插入。
  const table: number[][] = Array.from({ length: n + 1 }, (_, i) => {
    const row = new Array<number>(m + 1);
    row[0] = i;
    return row;
  });
  for (let j = 0; j <= m; j += 1) table[0]![j] = j;
  for (let i = 1; i <= n; i += 1) {
    for (let j = 1; j <= m; j += 1) {
      const substitution = table[i - 1]![j - 1]! + (source[i - 1] === output[j - 1] ? 0 : 1);
      table[i]![j] = Math.min(substitution, table[i - 1]![j]! + 1, table[i]![j - 1]! + 1);
    }
  }

  const reversed: DiffPart[] = [];
  let i = n;
  let j = m;
  while (i > 0 || j > 0) {
    if (i > 0 && j > 0 && source[i - 1] === output[j - 1]) {
      reversed.push({ value: source[i - 1]!, kind: "equal" });
      i -= 1;
      j -= 1;
      continue;
    }
    const cost = table[i]![j]!;
    const canSubstitute =
      i > 0 && j > 0 && table[i - 1]![j - 1]! + 1 === cost && source[i - 1] !== output[j - 1];
    if (canSubstitute) {
      reversed.push({ value: output[j - 1]!, kind: "insert" });
      reversed.push({ value: source[i - 1]!, kind: "delete" });
      i -= 1;
      j -= 1;
      continue;
    }
    if (j > 0 && table[i]![j - 1]! + 1 === cost) {
      reversed.push({ value: output[j - 1]!, kind: "insert" });
      j -= 1;
      continue;
    }
    reversed.push({ value: source[i - 1]!, kind: "delete" });
    i -= 1;
  }
  return reversed.reverse();
}

function mergeAdjacent(parts: DiffPart[]): DiffPart[] {
  const merged: DiffPart[] = [];
  for (const part of parts) {
    if (!part.value) continue;
    const last = merged[merged.length - 1];
    if (last && last.kind === part.kind) last.value += part.value;
    else merged.push({ ...part });
  }
  return merged;
}
