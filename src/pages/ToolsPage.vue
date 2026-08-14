<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import type { TextEncoding, UtilityConvertRequest } from "@shared/contracts";
import { sidecar } from "../lib/sidecar";

const source = ref("");
const output = ref("");
const kind = ref<UtilityConvertRequest["kind"]>("html-decimal-encode");
const sourceEncoding = ref<TextEncoding>("utf8");
const targetEncoding = ref<TextEncoding>("big5");
const encodings: TextEncoding[] = ["utf8", "utf16le", "utf16be", "big5", "gbk", "shift-jis", "euc-jp", "iso-2022-jp", "hz-gb-2312"];

async function run() {
  try {
    const result = await sidecar.request<{ text: string }>("utility.convert", { kind: kind.value, text: source.value, sourceEncoding: sourceEncoding.value, targetEncoding: targetEncoding.value } satisfies UtilityConvertRequest);
    output.value = result.text;
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  }
}
</script>

<template>
  <section class="page-stack">
    <header class="page-header"><div><p class="eyebrow">UTILITIES</p><h1>文字工具</h1><p>保留舊版的實體字元、編碼與全半形工具。</p></div></header>
    <el-card shadow="never"><div class="control-row"><el-select v-model="kind" style="width:220px"><el-option label="HTML 十進位編碼" value="html-decimal-encode"/><el-option label="HTML 十進位解碼" value="html-decimal-decode"/><el-option label="HTML 十六進位編碼" value="html-hex-encode"/><el-option label="HTML 十六進位解碼" value="html-hex-decode"/><el-option label="Unicode 跳脫編碼" value="unicode-escape-encode"/><el-option label="Unicode 跳脫解碼" value="unicode-escape-decode"/><el-option label="重新解讀文字編碼" value="encoding"/><el-option label="轉為全形" value="fullwidth"/><el-option label="轉為半形" value="halfwidth"/></el-select><template v-if="kind === 'encoding'"><el-select v-model="sourceEncoding" style="width:150px"><el-option v-for="value in encodings" :key="value" :label="value" :value="value"/></el-select><span>→</span><el-select v-model="targetEncoding" style="width:150px"><el-option v-for="value in encodings" :key="value" :label="value" :value="value"/></el-select></template><el-button type="primary" @click="run">執行</el-button></div></el-card>
    <div class="editor-grid"><el-card shadow="never" class="editor-card"><template #header>輸入</template><el-input v-model="source" type="textarea" :rows="20" resize="none"/></el-card><el-card shadow="never" class="editor-card result-card"><template #header>輸出</template><el-input v-model="output" type="textarea" :rows="20" resize="none"/></el-card></div>
  </section>
</template>
