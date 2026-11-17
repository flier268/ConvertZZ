import type source from "../zh-TW/tools";
import type { LocaleMessages } from "../types";

const messages: LocaleMessages<typeof source> = {
  title: "Text tools",
  subtitle: "Character entity, encoding and full-/half-width tools carried over from 1.x.",
  kind: {
    htmlDecimalEncode: "HTML decimal encode",
    htmlDecimalDecode: "HTML decimal decode",
    htmlHexEncode: "HTML hexadecimal encode",
    htmlHexDecode: "HTML hexadecimal decode",
    unicodeEscapeEncode: "Unicode escape encode",
    unicodeEscapeDecode: "Unicode escape decode",
    encoding: "Reinterpret text encoding",
    fullwidth: "To full-width",
    halfwidth: "To half-width",
  },
  run: "Run",
  input: "Input",
  output: "Output",
};

export default messages;
