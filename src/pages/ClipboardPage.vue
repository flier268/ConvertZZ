<script setup lang="ts">
import { onActivated, onBeforeUnmount, onDeactivated, onMounted, ref, watch } from "vue";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import { ElMessage } from "element-plus";
import type { Direction, EngineKind, TextEncoding, UtilityConvertRequest } from "@shared/contracts";
import { convertText } from "../lib/actions";
import { loadSettings, zhConvertOptions } from "../lib/settings";
import { core } from "../lib/coreClient";
import SideBySideDiffView from "../components/SideBySideDiffView.vue";
import { t } from "@/i18n";

defineOptions({ name: "ClipboardPage" });

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
  ElMessage.success(t("clipboard.written"));
}

onMounted(async () => {
  await refresh();
});

onActivated(() => {
  mounted = true;
  if (watching.value) toggleWatch(true);
});

onDeactivated(() => {
  mounted = false;
  if (timer) {
    clearInterval(timer);
    timer = undefined;
  }
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
        <h1>{{ t("clipboard.title") }}</h1>
        <p>{{ t("clipboard.subtitle") }}</p>
      </div>
      <div class="header-actions">
        <el-checkbox v-model="vocabularyCorrection">{{
          t("clipboard.vocabularyCorrection")
        }}</el-checkbox
        ><el-switch
          v-model="watching"
          :active-text="t('clipboard.watch')"
          @change="toggleWatch(Boolean($event))"
        />
      </div>
    </header>
    <el-alert v-if="lastError" :title="lastError" type="error" :closable="false" />
    <el-card shadow="never"
      ><div class="control-row">
        <el-select v-model="direction" style="width: 150px"
          ><el-option :label="t('clipboard.direction.none')" value="none" /><el-option
            :label="t('common.direction.s2t')"
            value="s2t" /><el-option :label="t('common.direction.t2s')" value="t2s" /></el-select
        ><el-select v-model="engine" style="width: 180px"
          ><el-option :label="t('clipboard.engine.segmented')" value="segmented" /><el-option
            :label="t('clipboard.engine.zhconvert')"
            value="zhconvert" /></el-select
        ><el-checkbox v-model="reinterpretEncoding">{{
          t("clipboard.reinterpretEncoding")
        }}</el-checkbox
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
        ><el-button @click="refresh(true)">{{ t("clipboard.readNow") }}</el-button
        ><el-button type="primary" :disabled="!output" @click="copy">{{
          t("clipboard.writeBack")
        }}</el-button>
      </div></el-card
    >
    <el-card shadow="never" class="page-fill-main">
      <template #header
        ><div class="card-title">
          <span>{{ t("clipboard.diffTitle") }}</span>
        </div></template
      >
      <SideBySideDiffView
        v-model:source="source"
        :output="output"
        editable
        fill-height
        :source-label="t('clipboard.sourceLabel')"
        :output-label="t('clipboard.outputLabel')"
        :source-placeholder="t('clipboard.sourcePlaceholder')"
        :output-placeholder="t('clipboard.outputPlaceholder')"
      />
    </el-card>
  </section>
</template>
