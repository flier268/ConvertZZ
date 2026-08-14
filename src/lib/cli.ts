import { shallowRef } from "vue";
import type { ParsedCli } from "@shared/contracts";

interface CliInvocation {
  sequence: number;
  options: ParsedCli;
}

export const cliInvocation = shallowRef<CliInvocation>();

let sequence = 0;

export function setCliInvocation(options: ParsedCli): void {
  cliInvocation.value = { sequence: ++sequence, options };
}
