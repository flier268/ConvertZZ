import type source from "../zh-TW/common";
import type { LocaleMessages } from "../types";

const messages: LocaleMessages<typeof source> = {
  ok: "OK",
  cancel: "Cancel",
  close: "Close",
  confirm: "Confirm",
  apply: "Apply",
  save: "Save",
  delete: "Delete",
  browse: "Browse…",
  preview: "Preview",
  stop: "Stop",
  yes: "Yes",
  no: "No",
  unknownError: "Unknown error",
  direction: {
    s2t: "Simplified → Traditional",
    t2s: "Traditional → Simplified",
  },
  language: {
    system: "Follow system",
    "zh-TW": "繁體中文",
    en: "English",
  },
};

export default messages;
