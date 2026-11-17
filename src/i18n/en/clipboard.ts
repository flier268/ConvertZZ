import type source from "../zh-TW/clipboard";
import type { LocaleMessages } from "../types";

const messages: LocaleMessages<typeof source> = {
  title: "Clipboard",
  subtitle: "Watch mode reads text content only.",
  vocabularyCorrection: "Vocabulary correction",
  watch: "Keep watching",
  direction: {
    none: "No conversion",
  },
  engine: {
    segmented: "Segmentation engine",
    zhconvert: "ZhConvert",
  },
  reinterpretEncoding: "Reinterpret encoding",
  readNow: "Read now",
  writeBack: "Write to clipboard",
  diffTitle: "Changes",
  sourceLabel: "Clipboard text",
  outputLabel: "Converted preview",
  sourcePlaceholder: "Clipboard text appears here",
  outputPlaceholder: "Converted preview appears here",
  written: "Result written to clipboard",
};

export default messages;
