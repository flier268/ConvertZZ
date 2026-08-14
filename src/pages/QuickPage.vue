<script setup lang="ts">
import { computed, ref } from "vue";
import { ElMessage } from "element-plus";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
import type { Direction, EngineKind, ZhConvertOptions } from "@shared/contracts";
import { convertText } from "../lib/actions";
import { loadSettings, zhConvertOptions } from "../lib/settings";

const source = ref("");
const output = ref("");
const direction = ref<Direction>("s2t");
const engine = ref<EngineKind>("segmented");
const vocabularyCorrection = ref(true);
const busy = ref(false);
const duration = ref<number>();
const sourceCount = computed(() => Array.from(source.value).length);
const promptAfterConversion = ref(true);
const zhconvert = ref<ZhConvertOptions>();

loadSettings().then((settings) => {
  engine.value = settings.engine;
  direction.value = settings.direction;
  vocabularyCorrection.value = settings.vocabularyCorrection;
  promptAfterConversion.value = settings.promptAfterConversion;
  zhconvert.value = zhConvertOptions(settings, direction.value);
});

async function convert() {
  busy.value = true;
  try {
    const settings = await loadSettings();
    zhconvert.value = zhConvertOptions(settings, direction.value);
    const result = await convertText(source.value, direction.value, engine.value, vocabularyCorrection.value, zhconvert.value, settings.dictionaryPath);
    output.value = result.text;
    duration.value = result.durationMs;
    if (promptAfterConversion.value) ElMessage.success(`轉換完成。耗時 ${result.durationMs} ms。`);
    if (result.warnings.length) ElMessage.warning(result.warnings[0]);
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    busy.value = false;
  }
}

async function paste() {
  source.value = await readText();
  await convert();
}

async function copy() {
  await writeText(output.value);
  ElMessage.success("結果已複製");
}
</script>

<template>
  <section class="page-stack">
    <header class="page-header">
      <div><p class="eyebrow">CONVERSION</p><h1>快速轉換</h1><p>使用分詞語意修正、舊版字典或 ZhConvert。</p></div>
      <el-button @click="paste">貼上並轉換</el-button>
    </header>
    <el-card shadow="never" class="control-card">
      <div class="control-row">
        <el-segmented v-model="direction" :options="[{ label: '簡體 → 繁體', value: 's2t' }, { label: '繁體 → 簡體', value: 't2s' }, { label: '不轉換', value: 'none' }]" />
        <el-select v-model="engine" style="width: 180px">
          <el-option label="新式分詞引擎" value="segmented" />
          <el-option label="舊版字典引擎" value="legacy" />
          <el-option label="ZhConvert API" value="zhconvert" />
        </el-select>
        <el-checkbox v-model="vocabularyCorrection">詞彙修正</el-checkbox>
        <el-button type="primary" :loading="busy" @click="convert">開始轉換</el-button>
      </div>
    </el-card>
    <div class="editor-grid">
      <el-card shadow="never" class="editor-card">
        <template #header><div class="card-title"><span>原始文字</span><small>{{ sourceCount }} 字</small></div></template>
        <el-input v-model="source" type="textarea" resize="none" :autosize="{ minRows: 17, maxRows: 28 }" placeholder="在此輸入或貼上文字" />
      </el-card>
      <el-card shadow="never" class="editor-card result-card">
        <template #header><div class="card-title"><span>轉換結果</span><small v-if="duration !== undefined">{{ duration }} ms</small></div></template>
        <el-input v-model="output" type="textarea" resize="none" :autosize="{ minRows: 17, maxRows: 28 }" readonly placeholder="結果會顯示於此" />
        <el-button class="copy-button" :disabled="!output" @click="copy">複製結果</el-button>
      </el-card>
    </div>
  </section>
</template>
