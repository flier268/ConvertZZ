import type source from "../zh-TW/app";
import type { LocaleMessages } from "../types";

const messages: LocaleMessages<typeof source> = {
  tagline: "Cross-platform Chinese conversion",
  nav: {
    quick: "Quick convert",
    files: "Files & filenames",
    clipboard: "Clipboard",
    audio: "Audio tags",
    tools: "Text tools",
    dictionary: "User dictionary",
    settings: "Settings",
    about: "About & changes",
  },
  coreVersion: "Core {version} · Rust",
  coreStarting: "Starting core…",
  startupErrorTitle: "ConvertZZ failed to start",
  loadingCore: "Loading conversion core",
  startupFailed: "Startup failed at {step}: {detail}",
  logFile: "Log file: {path}",
  filenamePreviewQueued:
    "A filename conversion preview was also created. See it on the “Files & filenames” page.",
  legacyDictionaryNotice: {
    title: "The legacy dictionary is now the user dictionary",
    message:
      "2.0 removed the legacy dictionary engine that replaced full text using Dictionary.csv. If you had custom entries, import them on the “User dictionary” page. Click here to go there.",
  },
  onboarding: {
    importSuccess: "Legacy settings were read and saved as 2.0 settings.",
    legacyFilterName: "ConvertZZ legacy settings",
    welcome:
      "This is a cross-platform Chinese conversion tool. This tour shows the main screens, the floating ball, and whether to import legacy settings.",
    quick:
      "Paste text here and choose Simplified → Traditional or Traditional → Simplified to convert instantly.",
    files:
      "Batch-convert file contents or filenames. A preview is shown first, and nothing is written until you confirm.",
    clipboard:
      "Watch or read clipboard text, and write the converted result back to the clipboard.",
    audio:
      "Convert text tags in MP3, APE, OGG, and Opus files. Unselected fields and cover art are left untouched.",
    tools:
      "Legacy text tools such as HTML entities, full/half-width, and Unicode escapes are here.",
    desktop:
      "By default the app stays in the tray and the floating ball. Right-click the floating ball for the same conversion menu as before. You can also open the main window from the tray at any time.",
    import:
      "If you have a 1.x ConvertZZ.json, it can be read and saved as 2.0 settings. The source file is not modified.",
    foundPath: "Found: {path}",
    pickLater: "You can also choose a file yourself later.",
    importFound: "Import found settings",
    pickFile: "Choose file",
    settings:
      "Adjust the default engine and whether the main window is shown at startup here. Global shortcuts and floating ball actions have their own tabs.",
  },
  floatingBall: {
    title:
      "Drop files to convert · Drag to move · Right-click for menu · Modifier + click runs the configured action",
    ariaLabel: "ConvertZZ floating ball",
  },
};

export default messages;
