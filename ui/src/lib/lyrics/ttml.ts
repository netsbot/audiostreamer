export type Syllable = {
  text: string;
  start: number;
  end: number;
  isBackground: boolean;
};

export type Word = {
  syllables: Syllable[];
  hasTrailingSpace: boolean;
};

export type LyricLine = {
  text: string;
  start: number;
  end: number;
  words: Word[];
  fullLineHighlight: boolean;
};

function parseTtmlTime(input: string | null): number {
  if (!input) return 0;
  const parts = input.split(':');
  if (parts.length === 1) return Number.parseFloat(parts[0]) || 0;
  if (parts.length === 2) {
    const minutes = Number.parseFloat(parts[0]) || 0;
    const seconds = Number.parseFloat(parts[1]) || 0;
    return minutes * 60 + seconds;
  }
  if (parts.length === 3) {
    const hours = Number.parseFloat(parts[0]) || 0;
    const minutes = Number.parseFloat(parts[1]) || 0;
    const seconds = Number.parseFloat(parts[2]) || 0;
    return hours * 3600 + minutes * 60 + seconds;
  }
  return 0;
}

function isBackgroundSpan(node: Element): boolean {
  return node.getAttribute('ttm:role') === 'x-bg';
}

function tokenizeByWhitespace(text: string): string[] {
  return text.match(/\s+|[^\s]+/g) ?? [];
}

export function parseTtmlToLines(ttml: string): LyricLine[] {
  const parser = new DOMParser();
  const doc = parser.parseFromString(ttml, 'application/xml');
  const pNodes = Array.from(doc.getElementsByTagName('p'));
  const result: LyricLine[] = [];

  for (let pIndex = 0; pIndex < pNodes.length; pIndex++) {
    const pNode = pNodes[pIndex];
    const lineStart = parseTtmlTime(pNode.getAttribute('begin'));
    const nextLineStart = pIndex + 1 < pNodes.length
      ? parseTtmlTime(pNodes[pIndex + 1].getAttribute('begin'))
      : 0;
    let lineEnd = parseTtmlTime(pNode.getAttribute('end'));

    if (lineEnd <= lineStart) {
      if (nextLineStart > lineStart) {
        lineEnd = nextLineStart;
      } else {
        lineEnd = lineStart + 4;
      }
    }

    const allSyllables: Syllable[] = [];
    let pendingText = '';

    for (const child of Array.from(pNode.childNodes)) {
      if (child.nodeType === Node.TEXT_NODE) {
        pendingText += child.textContent ?? '';
        continue;
      }
      if (child.nodeType !== Node.ELEMENT_NODE) continue;
      const el = child as Element;
      if (el.tagName !== 'span') continue;
      const isBg = isBackgroundSpan(el);

      const raw = el.textContent ?? '';
      if (!raw) continue;

      const text = `${pendingText}${raw}`;
      pendingText = '';
      allSyllables.push({
        text,
        start: parseTtmlTime(el.getAttribute('begin')) || lineStart,
        end: parseTtmlTime(el.getAttribute('end')) || lineEnd,
        isBackground: isBg,
      });
    }

    if (pendingText.trim()) {
      const lastEnd = allSyllables.length > 0 ? allSyllables[allSyllables.length - 1].end : lineStart;
      const safeStart = Math.min(lastEnd, lineEnd - 0.001);
      allSyllables.push({ text: pendingText, start: safeStart, end: lineEnd, isBackground: false });
    }

    if (allSyllables.length === 0) {
      const text = (pNode.textContent ?? '').trim();
      if (!text) continue;
      result.push({
        text,
        start: lineStart,
        end: lineEnd,
        words: [{ syllables: [{ text, start: lineStart, end: lineEnd, isBackground: false }], hasTrailingSpace: false }],
        fullLineHighlight: true,
      });
      continue;
    }

    const words: Word[] = [];
    let currentWordSyllables: Syllable[] = [];
    let fullLineHighlight = allSyllables.length <= 1;

    for (const rawSyllable of allSyllables) {
      const tokens = tokenizeByWhitespace(rawSyllable.text);
      const textTokens = tokens.filter((token) => !/^\s+$/.test(token));
      if (textTokens.length > 1) fullLineHighlight = true;
      const tokenCount = Math.max(textTokens.length, 1);
      const safeStart = rawSyllable.start;
      const safeEnd = rawSyllable.end > rawSyllable.start ? rawSyllable.end : rawSyllable.start + 0.25;
      const slot = (safeEnd - safeStart) / tokenCount;
      let textTokenIndex = 0;

      for (const token of tokens) {
        if (/^\s+$/.test(token)) {
          if (currentWordSyllables.length > 0) {
            words.push({ syllables: currentWordSyllables, hasTrailingSpace: true });
            currentWordSyllables = [];
          }
          continue;
        }

        currentWordSyllables.push({
          text: token,
          start: safeStart + slot * textTokenIndex,
          end: safeStart + slot * (textTokenIndex + 1),
          isBackground: rawSyllable.isBackground,
        });
        textTokenIndex += 1;
      }
    }

    if (currentWordSyllables.length > 0) {
      words.push({ syllables: currentWordSyllables, hasTrailingSpace: false });
    }

    const lineText = allSyllables.map((s) => s.text).join('').trim();
    result.push({ text: lineText, start: lineStart, end: lineEnd, words, fullLineHighlight });
  }

  return result;
}

export function extractTtml(payload: any): string | null {
  const attrs = payload?.data?.[0]?.attributes;
  if (!attrs) return null;
  if (typeof attrs.ttml === 'string') return attrs.ttml;
  const localizations = attrs.ttmlLocalizations;
  if (!localizations) return null;
  if (typeof localizations === 'string') return localizations;
  if (Array.isArray(localizations)) {
    for (const loc of localizations) {
      if (typeof loc === 'string') return loc;
      if (typeof loc?.ttml === 'string') return loc.ttml;
      if (typeof loc?.value === 'string') return loc.value;
    }
  }
  if (typeof localizations === 'object') {
    for (const key of Object.keys(localizations)) {
      const val = localizations[key];
      if (typeof val === 'string') return val;
      if (typeof val?.ttml === 'string') return val.ttml;
    }
  }
  return null;
}