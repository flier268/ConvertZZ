import { describe, expect, it } from "vitest";
import { channelFromReleaseTag as channelFromScript } from "../scripts/prerelease-channel.mjs";
import { channelFromReleaseTag as channelFromUpdate } from "../src/lib/update";

describe("預發佈通道標籤", () => {
  const samples: Array<[string, "alpha" | "beta" | "rc" | null]> = [
    ["v2.0.0-beta1", "beta"],
    ["2.0.0-beta5", "beta"],
    ["v2.0.0-beta.10", "beta"],
    ["v2.1.0-alpha9", "alpha"],
    ["v2.1.0-alpha.1", "alpha"],
    ["v2.0.0-alpha-beta", "alpha"],
    ["v2.0.0-beta.alpha", "beta"],
    ["v2.0.0-beta1+build.1", "beta"],
    ["v2.1.0-rc.1", "rc"],
    ["v2.1.0-rc1", "rc"],
    ["v2.0.0", null],
    ["beta", null],
    ["alpha", null],
    ["rc", null],
    ["", null],
  ];

  it("脚本與前端用同一套 alpha／beta／rc 判斷", () => {
    for (const [tag, expected] of samples) {
      expect(channelFromScript(tag), tag).toBe(expected);
      expect(channelFromUpdate(tag), tag).toBe(expected);
    }
  });
});
