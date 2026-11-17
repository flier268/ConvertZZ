<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import type { TextEncoding, UtilityConvertRequest } from "@shared/contracts";
import { core } from "../lib/coreClient";
import { t } from "@/i18n";

defineOptions({ name: "ToolsPage" });

const source = ref("");
const output = ref("");
const kind = ref<UtilityConvertRequest["kind"]>("html-decimal-encode");
const sourceEncoding = ref<TextEncoding>("utf8");
const targetEncoding = ref<TextEncoding>("big5");
const encodings: TextEncoding[] = [
  "utf8",
  "utf16le",
  "utf16be",
  "big5",
  "gbk",
  "shift-jis",
  "euc-jp",
  "iso-2022-jp",
  "hz-gb-2312",
];

async function run() {
  try {
    const result = await core.request<{ text: string }>("utility.convert", {
      kind: kind.value,
      text: source.value,
      sourceEncoding: sourceEncoding.value,
      targetEncoding: targetEncoding.value,
    } satisfies UtilityConvertRequest);
    output.value = result.text;
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  }
}
</script>

<template>
  <section class="page-stack">
    <header class="page-header">
      <div>
        <p class="eyebrow">UTILITIES</p>
        <h1>{{ t("tools.title") }}</h1>
        <p>{{ t("tools.subtitle") }}</p>
      </div>
    </header>
    <el-card shadow="never"
      ><div class="control-row">
        <el-select v-model="kind" style="width: 220px"
          ><el-option
            :label="t('tools.kind.htmlDecimalEncode')"
            value="html-decimal-encode" /><el-option
            :label="t('tools.kind.htmlDecimalDecode')"
            value="html-decimal-decode" /><el-option
            :label="t('tools.kind.htmlHexEncode')"
            value="html-hex-encode" /><el-option
            :label="t('tools.kind.htmlHexDecode')"
            value="html-hex-decode" /><el-option
            :label="t('tools.kind.unicodeEscapeEncode')"
            value="unicode-escape-encode" /><el-option
            :label="t('tools.kind.unicodeEscapeDecode')"
            value="unicode-escape-decode" /><el-option
            :label="t('tools.kind.encoding')"
            value="encoding" /><el-option
            :label="t('tools.kind.fullwidth')"
            value="fullwidth" /><el-option
            :label="t('tools.kind.halfwidth')"
            value="halfwidth" /></el-select
        ><template v-if="kind === 'encoding'"
          ><el-select v-model="sourceEncoding" style="width: 150px"
            ><el-option
              v-for="value in encodings"
              :key="value"
              :label="value"
              :value="value" /></el-select
          ><span>→</span
          ><el-select v-model="targetEncoding" style="width: 150px"
            ><el-option
              v-for="value in encodings"
              :key="value"
              :label="value"
              :value="value" /></el-select></template
        ><el-button type="primary" @click="run">{{ t("tools.run") }}</el-button>
      </div></el-card
    >
    <div class="editor-grid page-fill-main">
      <el-card shadow="never" class="editor-card"
        ><template #header>{{ t("tools.input") }}</template
        ><el-input v-model="source" type="textarea" :rows="8" resize="none" /></el-card
      ><el-card shadow="never" class="editor-card result-card"
        ><template #header>{{ t("tools.output") }}</template
        ><el-input v-model="output" type="textarea" :rows="8" resize="none"
      /></el-card>
    </div>
  </section>
</template>
