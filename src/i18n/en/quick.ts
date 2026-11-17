import type source from "../zh-TW/quick";
import type { LocaleMessages } from "../types";

const messages: LocaleMessages<typeof source> = {
  title: "Quick convert",
  subtitle: "Convert with semantic segmentation correction or ZhConvert.",
  pasteAndConvert: "Paste and convert",
  direction: {
    s2t: "Simplified → Traditional",
    t2s: "Traditional → Simplified",
    none: "No conversion",
  },
  engine: {
    segmented: "Segmentation engine",
    zhconvert: "ZhConvert API",
  },
  vocabularyCorrection: "Vocabulary correction",
  start: "Convert",
  stop: "Stop",
  diffTitle: "Changes",
  copyResult: "Copy result",
  sourceLabel: "Original text",
  outputLabel: "Converted text",
  converted: "Conversion finished in {ms} ms.",
  cancelled: "Conversion cancelled.",
  copied: "Result copied",
};

export default messages;
