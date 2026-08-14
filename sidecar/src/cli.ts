import type { EngineKind, ParsedCli } from "../../shared/contracts.js";

export function parseLegacyCli(args: string[], defaultEngine: EngineKind = "segmented"): ParsedCli {
  let explicitMode = false;
  const parsed: ParsedCli = {
    mode: "interactive",
    paths: [],
    inputEncoding: "auto",
    outputEncoding: "auto",
    direction: "none",
    engine: defaultEngine,
    operation: "content",
    vocabularyCorrection: "settings",
  };
  for (const raw of args) {
    const argument = raw.toLowerCase();
    if (argument === "/file") {
      parsed.mode = "file";
      explicitMode = true;
    }
    else if (argument === "/audio") {
      parsed.mode = "audio";
      explicitMode = true;
    }
    else if (argument === "/i:ule") parsed.inputEncoding = "utf16le";
    else if (argument === "/i:ube") parsed.inputEncoding = "utf16be";
    else if (argument === "/i:utf8") parsed.inputEncoding = "utf8";
    else if (argument === "/i:gbk") parsed.inputEncoding = "gbk";
    else if (argument === "/i:big5") parsed.inputEncoding = "big5";
    else if (argument === "/o:ule") parsed.outputEncoding = "utf16le";
    else if (argument === "/o:ube") parsed.outputEncoding = "utf16be";
    else if (argument === "/o:utf8") parsed.outputEncoding = "utf8";
    else if (argument === "/o:gbk") parsed.outputEncoding = "gbk";
    else if (argument === "/o:big5") parsed.outputEncoding = "big5";
    else if (argument === "/f:t") parsed.direction = "s2t";
    else if (argument === "/f:s") parsed.direction = "t2s";
    else if (argument === "/f:d") parsed.direction = "none";
    else if (argument === "/d:t") parsed.vocabularyCorrection = "enabled";
    else if (argument === "/d:f") parsed.vocabularyCorrection = "disabled";
    else if (argument === "/d:s") parsed.vocabularyCorrection = "settings";
    else if (argument === "/e:l") parsed.engine = "legacy";
    else if (argument === "/e:f") parsed.engine = "zhconvert";
    else if (argument === "/e:n") parsed.engine = "segmented";
    else parsed.paths.push(raw);
  }
  if (!explicitMode && parsed.paths.length > 0) {
    parsed.mode = "file";
    if (parsed.paths.length > 1) {
      parsed.outputPath = parsed.paths[1];
      parsed.paths = [parsed.paths[0]];
    }
  }
  return parsed;
}
