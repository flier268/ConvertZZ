import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const PRE_RELEASE_CHANNEL_TAGS = ["alpha", "beta", "rc"];

export function channelFromReleaseTag(tag) {
  const normalized = String(tag ?? "")
    .trim()
    .replace(/^v/iu, "");
  const plus = normalized.indexOf("+");
  const value = plus === -1 ? normalized : normalized.slice(0, plus);
  if (!value || PRE_RELEASE_CHANNEL_TAGS.includes(value)) return null;
  const dash = value.indexOf("-");
  if (dash === -1) return null;
  const pre = value.slice(dash + 1).toLowerCase();
  let best = null;
  let bestPos = Infinity;
  for (const channel of PRE_RELEASE_CHANNEL_TAGS) {
    const pos = pre.indexOf(channel);
    if (pos >= 0 && pos < bestPos) {
      best = channel;
      bestPos = pos;
    }
  }
  return best;
}

function readArg(name) {
  const index = process.argv.indexOf(`--${name}`);
  if (index === -1) return "";
  return process.argv[index + 1] ?? "";
}

const entry = process.argv[1];
if (entry && fileURLToPath(import.meta.url) === resolve(entry)) {
  const tag = readArg("tag");
  const channel = channelFromReleaseTag(tag);
  if (channel) process.stdout.write(`${channel}\n`);
}
