<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { ElMessage } from "element-plus";
import type { Direction, EngineKind, TextEncoding, UtilityConvertRequest } from "@shared/contracts";
import { convertText } from "../lib/actions";
import { loadSettings, zhConvertOptions } from "../lib/settings";
import { core } from "../lib/coreClient";

const source = ref("");
const output = ref("");
const watching = ref(true);
const direction = ref<Direction>("s2t");
const engine = ref<EngineKind>("segmented");
const vocabularyCorrection = ref(true);
const reinterpretEncoding = ref(true);
const lastError = ref("");
const sourceEncoding = ref<TextEncoding>("big5");
const targetEncoding = ref<TextEncoding>("gbk");
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
let timer: ReturnType<typeof setInterval> | undefined;
let mounted = false;

loadSettings().then((settings) => {
  direction.value = settings.direction;
  engine.value = settings.engine;
  vocabularyCorrection.value = settings.vocabularyCorrection;
});

async function refresh(force = false) {
  try {
    const value = await readText();
    if (!force && value === source.value) return;
    source.value = value;
    let converted = value;
    if (reinterpretEncoding.value && direction.value === "s2t")
      converted = await reinterpret(converted);
    const settings = await loadSettings();
    converted = (
      await convertText(
        converted,
        direction.value,
        engine.value,
        vocabularyCorrection.value,
        zhConvertOptions(settings, direction.value),
        settings.dictionaryPath,
      )
    ).text;
    if (reinterpretEncoding.value && direction.value !== "s2t")
      converted = await reinterpret(converted);
    output.value = converted;
    lastError.value = "";
  } catch (error) {
    lastError.value = error instanceof Error ? error.message : String(error);
    throw error;
  }
}

async function reinterpret(text: string): Promise<string> {
  return (
    await core.request<{ text: string }>("utility.convert", {
      kind: "encoding",
      text,
      sourceEncoding: targetEncoding.value,
      targetEncoding: sourceEncoding.value,
    } satisfies UtilityConvertRequest)
  ).text;
}

function toggleWatch(value: boolean) {
  if (timer) clearInterval(timer);
  timer = value ? setInterval(() => refresh().catch(() => undefined), 800) : undefined;
}

async function copy() {
  await writeText(output.value);
  ElMessage.success("結果已寫入剪貼簿");
}

onMounted(async () => {
  mounted = true;
  await refresh();
  toggleWatch(true);
});
watch(
  [direction, engine, vocabularyCorrection, reinterpretEncoding, sourceEncoding, targetEncoding],
  () => {
    if (mounted) void refresh(true).catch(() => undefined);
  },
);
onBeforeUnmount(() => {
  mounted = false;
  if (timer) clearInterval(timer);
});
</script>

<template>
  <section class="page-stack">
    <header class="page-header">
      <div>
        <p class="eyebrow">CLIPBOARD</p>
        <h1>剪貼簿</h1>
        <p>監看模式只讀取文字內容。</p>
      </div>
      <div class="header-actions">
        <el-checkbox v-model="vocabularyCorrection">詞彙修正</el-checkbox
        ><el-switch
          v-model="watching"
          active-text="持續監看"
          @change="toggleWatch(Boolean($event))"
        />
      </div>
    </header>
    <el-alert v-if="lastError" :title="lastError" type="error" :closable="false" />
    <el-card shadow="never"
      ><div class="control-row">
        <el-select v-model="direction" style="width: 150px"
          ><el-option label="不轉換簡繁" value="none" /><el-option
            label="簡轉繁"
            value="s2t" /><el-option label="繁轉簡" value="t2s" /></el-select
        ><el-select v-model="engine" style="width: 180px"
          ><el-option label="新式分詞" value="segmented" /><el-option
            label="舊版字典"
            value="legacy" /><el-option label="ZhConvert" value="zhconvert" /></el-select
        ><el-checkbox v-model="reinterpretEncoding">重新解讀編碼</el-checkbox
        ><template v-if="reinterpretEncoding"
          ><el-select v-model="targetEncoding" style="width: 130px"
            ><el-option
              v-for="value in encodings"
              :key="value"
              :label="value"
              :value="value" /></el-select
          ><span>→</span
          ><el-select v-model="sourceEncoding" style="width: 130px"
            ><el-option
              v-for="value in encodings"
              :key="value"
              :label="value"
              :value="value" /></el-select></template
        ><el-button @click="refresh(true)">立即讀取</el-button
        ><el-button type="primary" :disabled="!output" @click="copy">寫回剪貼簿</el-button>
      </div></el-card
    >
    <div class="editor-grid">
      <el-card shadow="never" class="editor-card"
        ><template #header>剪貼簿文字</template
        ><el-input v-model="source" type="textarea" :rows="20" resize="none" /></el-card
      ><el-card shadow="never" class="editor-card result-card"
        ><template #header>轉換預覽</template
        ><el-input v-model="output" type="textarea" :rows="20" resize="none" readonly
      /></el-card>
    </div>
  </section>
</template>
