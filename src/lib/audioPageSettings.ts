import type { SettingsV2 } from "@shared/contracts";

export function defaultAudioSettings(): SettingsV2["audio"] {
  return {
    backup: true,
    conflictPolicy: "skip",
    recursive: true,
    id3v1Enabled: true,
    id3v1Direction: "s2t",
    id3v1SourceEncoding: "gbk",
    id3v1OutputEncoding: "big5",
    id3v2Enabled: true,
    id3v2Direction: "s2t",
    id3v2SourceEncoding: "gbk",
    id3v2RepairSourceEncoding: true,
    id3v2Version: 4,
    id3v2Encoding: "utf8",
    tagOperations: [],
  };
}
