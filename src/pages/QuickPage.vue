<script setup lang="ts">
import { ref } from "vue";
import { ElMessage } from "element-plus";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import type { Direction, EngineKind, ZhConvertOptions } from "@shared/contracts";
import { convertText } from "../lib/actions";
import { core, isCancellationError } from "../lib/coreClient";
import { loadSettings, zhConvertOptions } from "../lib/settings";
import { formatProgressLabel, progressPercentage, type ProgressSnapshot } from "../lib/progressEta";
import SideBySideDiffView from "../components/SideBySideDiffView.vue";
import { t } from "@/i18n";

defineOptions({ name: "QuickPage" });

const source = ref("");
const output = ref("");
const direction = ref<Direction>("s2t");
const engine = ref<EngineKind>("segmented");
const vocabularyCorrection = ref(true);
const busy = ref(false);
const duration = ref<number>();
const promptAfterConversion = ref(true);
const zhconvert = ref<ZhConvertOptions>();
const progress = ref<ProgressSnapshot>();
const progressStartedAt = ref<number>();
const activeRequestId = ref<string>();

loadSettings().then((settings) => {
  engine.value = settings.engine;
  direction.value = settings.direction;
  vocabularyCorrection.value = settings.vocabularyCorrection;
  promptAfterConversion.value = settings.promptAfterConversion;
  zhconvert.value = zhConvertOptions(settings, direction.value);
});

async function convert() {
  busy.value = true;
  progress.value = undefined;
  progressStartedAt.value = Date.now();
  try {
    const settings = await loadSettings();
    zhconvert.value = zhConvertOptions(settings, direction.value);
    const result = await convertText(
      source.value,
      direction.value,
      engine.value,
      vocabularyCorrection.value,
      zhconvert.value,
      {
        onProgress: (value) => {
          progress.value = value;
        },
        onRequestId: (id) => {
          activeRequestId.value = id;
        },
      },
    );
    output.value = result.text;
    duration.value = result.durationMs;
    if (promptAfterConversion.value)
      ElMessage.success(t("quick.converted", { ms: result.durationMs }));
    if (result.warnings.length) ElMessage.warning(result.warnings[0]);
  } catch (error) {
    if (isCancellationError(error)) ElMessage.info(t("quick.cancelled"));
    else ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    busy.value = false;
    activeRequestId.value = undefined;
    progress.value = undefined;
    progressStartedAt.value = undefined;
  }
}

async function cancelConvert() {
  const requestId = activeRequestId.value;
  if (!requestId) return;
  try {
    await core.cancel(requestId);
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  }
}

async function paste() {
  source.value = await readText();
  await convert();
}

async function copy() {
  await writeText(output.value);
  ElMessage.success(t("quick.copied"));
}
</script>

<template>
  <section class="page-stack">
    <header class="page-header">
      <div>
        <p class="eyebrow">CONVERSION</p>
        <h1>{{ t("quick.title") }}</h1>
        <p>{{ t("quick.subtitle") }}</p>
      </div>
      <el-button @click="paste">{{ t("quick.pasteAndConvert") }}</el-button>
    </header>
    <el-card shadow="never" class="control-card">
      <div class="control-row">
        <el-segmented
          v-model="direction"
          :options="[
            { label: t('quick.direction.s2t'), value: 's2t' },
            { label: t('quick.direction.t2s'), value: 't2s' },
            { label: t('quick.direction.none'), value: 'none' },
          ]"
        />
        <el-select v-model="engine" style="width: 180px">
          <el-option :label="t('quick.engine.segmented')" value="segmented" />

          <el-option :label="t('quick.engine.zhconvert')" value="zhconvert" />
        </el-select>
        <el-checkbox v-model="vocabularyCorrection">{{
          t("quick.vocabularyCorrection")
        }}</el-checkbox>
        <el-button type="primary" :loading="busy" :disabled="busy" @click="convert">{{
          t("quick.start")
        }}</el-button>
        <el-button v-if="busy" @click="cancelConvert">{{ t("quick.stop") }}</el-button>
      </div>
      <el-progress
        v-if="busy && progress"
        class="job-progress"
        :percentage="progressPercentage(progress)"
        :format="() => formatProgressLabel(progress, progressStartedAt)"
      />
    </el-card>
    <el-card shadow="never" class="page-fill-main">
      <template #header
        ><div class="card-title">
          <span>{{ t("quick.diffTitle") }}</span>
          <div class="header-actions">
            <small v-if="duration !== undefined">{{ duration }} ms</small>
            <el-button :disabled="!output" @click="copy">{{ t("quick.copyResult") }}</el-button>
          </div>
        </div></template
      >
      <SideBySideDiffView
        v-model:source="source"
        :output="output"
        editable
        fill-height
        :source-label="t('quick.sourceLabel')"
        :output-label="t('quick.outputLabel')"
      />
    </el-card>
  </section>
</template>
