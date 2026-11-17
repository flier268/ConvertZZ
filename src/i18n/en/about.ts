import type source from "../zh-TW/about";
import type { LocaleMessages } from "../types";

const messages: LocaleMessages<typeof source> = {
  subtitle: "Cross-platform Chinese conversion tool under GPL-3.0.",
  replayOnboarding: "Replay tour",
  checkUpdates: "Check for updates",
  reportIssue: "Report an issue",
  hero: {
    title: "A new core.",
    body: "The interface is built with Vue and Element Plus. The in-process Rust conversion core gives the same results on Windows, Linux and macOS.",
  },
  platform: {
    title: "Platform differences",
    subtitle:
      "The conversion core is the same everywhere; desktop features depend on the OS and display server.",
    currentEnvironment: "Current environment: {platform} / {displayServer}",
    noLimitations: "No additional desktop limitations in this environment.",
    feature: "Feature",
    current: "Current",
  },
  features: {
    textFiles: "Text, file and file name conversion",
    audioTags: "ID3, APEv2, OGG and Opus tags",
    globalShortcuts: "Global shortcuts",
    autoCopyPaste: "Automatic copy and paste",
    floatingBallOnTop: "Floating ball always on top",
    sendTo: "SendTo shortcut",
    tray: "System tray",
    credentials: "Credential store",
    autoUpdate: "Automatic updates",
  },
  support: {
    full: "Full",
    disabledThisRelease: "Disabled in this release",
    disabled: "Disabled",
    dependsOnCompositor: "Depends on compositor",
    notApplicable: "N/A",
  },
  details: {
    trayWindows: "Left click and menu fully supported",
    trayLinux: "Requires AppIndicator; open via menu",
    trayMacos: "Menu bar icon and menu",
    credentialsWindows: "Windows Credential Manager",
    credentialsLinux: "Secret Service",
    credentialsMacos: "Keychain",
    updateWindows: "Installer downloads and installs",
    updateLinux: "AppImage downloads and installs; DEB/RPM opens the download page",
    updateMacos: "DMG/app bundle downloads and installs",
  },
  conversion: {
    title: "Conversion differences",
    subtitle: "Compared with 1.x, rules and unmatched characters are handled as follows.",
    legacyEngine: {
      title: "Legacy dictionary engine",
      body: "The 1.x legacy dictionary engine, which replaced text directly using Dictionary.csv, has been removed.",
    },
    unmatched: {
      title: "Unmatched characters",
      body: "Unmatched characters are now handled by cjk-convert-rs.",
    },
    glyphs: {
      title: "Character forms",
      body: "A few character forms may differ from Windows LCMapStringEx.",
    },
    encoding: {
      title: "Encoding",
      body: "The legacy Encoding.Default changed with the Windows system code page. The new encoding tools use explicitly selected encodings.",
    },
    zhconvert: {
      title: "ZhConvert",
      body: "ZhConvert is an optional online service.",
    },
  },
};

export default messages;
