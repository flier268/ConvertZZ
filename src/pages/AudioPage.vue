<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { open, confirm } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import type {
  AudioTagField,
  AudioTagFile,
  AudioTagPlan,
  AudioTagPlanRequest,
  Direction,
  EngineKind,
  TextEncoding,
} from "@shared/contracts";
import { core } from "../lib/coreClient";
import { loadSettings, zhConvertOptions } from "../lib/settings";
import { cliInvocation } from "../lib/cli";
import type { DiffSection } from "../lib/fileDiff";
import PreviewDiffDialog from "../components/PreviewDiffDialog.vue";

defineOptions({ name: "AudioPage" });

const paths = ref<string[]>([]);
const files = ref<AudioTagFile[]>([]);
const plan = ref<AudioTagPlan>();
const busy = ref(false);
const diffVisible = ref(false);
const diffTitle = ref("標籤差異");
const diffMeta = ref<Array<{ label: string; value: string }>>([]);
const diffSections = ref<DiffSection[]>([]);
const promptAfterConversion = ref(true);
const backup = ref(true);
const conflictPolicy = ref<AudioTagPlanRequest["conflictPolicy"]>("skip");
const progress = ref<{ current: number; total: number; message: string }>();
const options = reactive({
  direction: "s2t" as Direction,
  engine: "segmented" as EngineKind,
  recursive: true,
  id3v1Enabled: true,
  id3v1Direction: "s2t" as Direction,
  id3v1SourceEncoding: "gbk" as Exclude<TextEncoding, "auto">,
  id3v1OutputEncoding: "big5" as Exclude<TextEncoding, "auto">,
  id3v2Enabled: true,
  id3v2Direction: "s2t" as Direction,
  id3v2SourceEncoding: "gbk" as Exclude<TextEncoding, "auto">,
  id3v2RepairSourceEncoding: true,
  id3v2Version: 4 as 3 | 4,
  id3v2Encoding: "utf8" as AudioTagPlanRequest["id3v2Encoding"],
  vocabularyCorrection: true,
});

loadSettings().then((settings) => {
  options.direction = settings.direction;
  options.id3v1Direction = settings.direction;
  options.id3v2Direction = settings.direction;
  options.engine = settings.engine;
  options.vocabularyCorrection = settings.vocabularyCorrection;
  promptAfterConversion.value = settings.promptAfterConversion;
  backup.value = settings.autoBackupBeforeConversion;
});
const hasMp3 = computed(() => files.value.some((file) => file.format === "mp3"));
const selectedFileCount = computed(
  () =>
    files.value.filter(
      (file) =>
        file.selected &&
        !file.warning &&
        file.fields.some((field) => field.selected && fieldEnabled(file, field.container)),
    ).length,
);

async function choose() {
  const selected = await open({
    multiple: true,
    filters: [{ name: "音訊標籤", extensions: ["mp3", "ape", "ogg", "oga", "opus"] }],
  });
  if (!selected) return;
  paths.value = Array.isArray(selected) ? selected : [selected];
  await scan();
}

async function chooseFolder() {
  const selected = await open({ directory: true, multiple: false });
  if (!selected) return;
  paths.value = Array.isArray(selected) ? selected : [selected];
  await scan();
}

async function scan() {
  busy.value = true;
  progress.value = undefined;
  try {
    files.value = await core.request<AudioTagFile[]>(
      "audio.scan",
      {
        paths: paths.value,
        recursive: options.recursive,
        id3v1SourceEncoding: options.id3v1SourceEncoding,
        id3v2SourceEncoding: options.id3v2SourceEncoding,
        id3v2RepairSourceEncoding: options.id3v2RepairSourceEncoding,
      },
      300_000,
      (value) => {
        progress.value = value;
      },
    );
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    busy.value = false;
  }
}

async function createPlan() {
  const selectedFields = Object.fromEntries(
    files.value.map((file) => [
      file.path,
      file.fields
        .filter((field) => field.selected)
        .map((field) => `${field.container}:${field.key}`),
    ]),
  );
  const selectedPaths = files.value
    .filter((file) => file.selected && !file.warning)
    .map((file) => file.path);
  busy.value = true;
  progress.value = undefined;
  try {
    const settings = await loadSettings();
    plan.value = await core.request<AudioTagPlan>(
      "audio.plan",
      {
        paths: paths.value,
        recursive: options.recursive,
        selectedPaths,
        selectedFields,
        conversion: {
          direction: options.direction,
          engine: options.engine,
          vocabularyCorrection: options.vocabularyCorrection,
          zhconvert: zhConvertOptions(settings, options.direction),
          dictionaryPath: settings.dictionaryPath,
        },
        conflictPolicy: conflictPolicy.value,
        backup: backup.value,
        id3v1Enabled: options.id3v1Enabled,
        id3v1Direction: options.id3v1Direction,
        id3v1Zhconvert: zhConvertOptions(settings, options.id3v1Direction),
        id3v1SourceEncoding: options.id3v1SourceEncoding,
        id3v1OutputEncoding: options.id3v1OutputEncoding,
        id3v2Enabled: options.id3v2Enabled,
        id3v2Direction: options.id3v2Direction,
        id3v2Zhconvert: zhConvertOptions(settings, options.id3v2Direction),
        id3v2SourceEncoding: options.id3v2SourceEncoding,
        id3v2RepairSourceEncoding: options.id3v2RepairSourceEncoding,
        id3v2Version: options.id3v2Version,
        id3v2Encoding: options.id3v2Encoding,
      } satisfies AudioTagPlanRequest,
      600_000,
      (value) => {
        progress.value = value;
      },
    );
    files.value = plan.value.files;
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    busy.value = false;
  }
}

async function applyPlan() {
  if (!plan.value) return;
  if (
    !(await confirm(`將寫入 ${selectedFileCount.value} 個音訊檔案的已選標籤。`, {
      title: "確認標籤轉換",
      kind: "warning",
    }))
  )
    return;
  if (
    backup.value &&
    conflictPolicy.value === "overwrite" &&
    !(await confirm("覆寫會取代既有的 .bak 備份。是否確定繼續？", {
      title: "確認覆寫備份",
      kind: "warning",
    }))
  )
    return;
  busy.value = true;
  progress.value = undefined;
  try {
    const result = await core.request<{ succeeded: string[]; failed: unknown[] }>(
      "audio.apply",
      { planId: plan.value.planId },
      900_000,
      (value) => {
        progress.value = value;
      },
    );
    if (promptAfterConversion.value)
      ElMessage.success(`已完成 ${result.succeeded.length} 個音訊檔案。`);
    if (result.failed.length) ElMessage.warning(`${result.failed.length} 個檔案失敗。`);
    plan.value = undefined;
    await scan();
  } finally {
    busy.value = false;
  }
}

async function cancelPlan() {
  if (!plan.value) return;
  await core.request("audio.cancel", { planId: plan.value.planId });
  plan.value = undefined;
  diffVisible.value = false;
  diffSections.value = [];
  diffMeta.value = [];
  await scan();
}

function openFieldDiff(file: AudioTagFile, field: AudioTagField) {
  const source = field.values.join("\n");
  const output = (field.convertedValues ?? field.values).join("\n");
  diffTitle.value = "標籤差異";
  diffMeta.value = [
    { label: "檔案", value: file.path },
    { label: "標籤", value: field.container },
    { label: "欄位", value: field.label },
  ];
  diffSections.value = [
    {
      title: field.label,
      sourceLabel: "目前內容",
      outputLabel: "轉換預覽",
      source,
      output,
    },
  ];
  diffVisible.value = true;
}

watch(
  cliInvocation,
  async (invocation) => {
    if (invocation?.options.mode !== "audio") return;
    paths.value = invocation.options.paths;
    if (invocation.options.direction !== "none") {
      options.direction = invocation.options.direction;
      options.id3v1Direction = invocation.options.direction;
      options.id3v2Direction = invocation.options.direction;
    }
    options.engine = invocation.options.engine;
    backup.value = invocation.options.backup;
    if (invocation.options.vocabularyCorrection !== "settings")
      options.vocabularyCorrection = invocation.options.vocabularyCorrection === "enabled";
    if (paths.value.length) await scan();
  },
  { immediate: true },
);

function fieldEnabled(
  file: AudioTagFile,
  container: AudioTagFile["fields"][number]["container"],
): boolean {
  if (!file.selected) return false;
  if (container === "id3v1") return options.id3v1Enabled;
  if (container === "id3v2") return options.id3v2Enabled;
  return true;
}
</script>

<template>
  <section class="page-stack">
    <header class="page-header">
      <div>
        <p class="eyebrow">AUDIO TAGS</p>
        <h1>音訊標籤</h1>
        <p>ID3、APEv2 與 Vorbis Comment 都會保留未選欄位與封面。</p>
      </div>
      <div class="card-actions">
        <el-button :disabled="Boolean(plan)" :loading="busy" @click="chooseFolder"
          >選取資料夾</el-button
        >
        <el-button type="primary" :disabled="Boolean(plan)" :loading="busy" @click="choose"
          >選取音訊檔案</el-button
        >
      </div>
    </header>
    <el-card shadow="never">
      <el-form
        label-position="top"
        class="option-grid audio-options"
        :disabled="busy || Boolean(plan)"
      >
        <el-form-item label="APEv2／Vorbis 方向">
          <el-select v-model="options.direction">
            <el-option label="不轉換" value="none" />
            <el-option label="簡轉繁" value="s2t" />
            <el-option label="繁轉簡" value="t2s" />
          </el-select>
        </el-form-item>
        <el-form-item label="引擎">
          <el-select v-model="options.engine">
            <el-option label="新式分詞" value="segmented" />
            <el-option label="舊版字典" value="legacy" />
            <el-option label="ZhConvert" value="zhconvert" />
          </el-select>
        </el-form-item>
        <el-form-item label="詞彙修正"
          ><el-switch v-model="options.vocabularyCorrection"
        /></el-form-item>
        <el-form-item label="轉換前備份"
          ><el-switch v-model="backup" active-text=".bak"
        /></el-form-item>
        <el-form-item label="備份衝突"
          ><el-select v-model="conflictPolicy" :disabled="!backup"
            ><el-option label="略過" value="skip" /><el-option
              label="覆寫"
              value="overwrite" /></el-select
        ></el-form-item>
        <el-form-item label="資料夾掃描"
          ><el-checkbox v-model="options.recursive">包含子資料夾</el-checkbox></el-form-item
        >
        <template v-if="hasMp3">
          <el-form-item label="ID3v1">
            <el-switch v-model="options.id3v1Enabled" active-text="啟用轉換" />
          </el-form-item>
          <el-form-item label="ID3v1 方向">
            <el-select v-model="options.id3v1Direction" :disabled="!options.id3v1Enabled">
              <el-option label="不轉換" value="none" />
              <el-option label="簡轉繁" value="s2t" />
              <el-option label="繁轉簡" value="t2s" />
            </el-select>
          </el-form-item>
          <el-form-item label="ID3v1 來源">
            <el-select v-model="options.id3v1SourceEncoding" :disabled="!options.id3v1Enabled">
              <el-option label="GBK" value="gbk" />
              <el-option label="Big5" value="big5" />
              <el-option label="Shift-JIS" value="shift-jis" />
              <el-option label="UTF-8" value="utf8" />
            </el-select>
          </el-form-item>
          <el-form-item label="ID3v1 輸出">
            <el-select v-model="options.id3v1OutputEncoding" :disabled="!options.id3v1Enabled">
              <el-option label="Big5" value="big5" />
              <el-option label="GBK" value="gbk" />
              <el-option label="Shift-JIS" value="shift-jis" />
              <el-option label="UTF-8" value="utf8" />
            </el-select>
          </el-form-item>
          <el-form-item label="ID3v2">
            <el-switch v-model="options.id3v2Enabled" active-text="啟用轉換" />
          </el-form-item>
          <el-form-item label="ID3v2 方向">
            <el-select v-model="options.id3v2Direction" :disabled="!options.id3v2Enabled">
              <el-option label="不轉換" value="none" />
              <el-option label="簡轉繁" value="s2t" />
              <el-option label="繁轉簡" value="t2s" />
            </el-select>
          </el-form-item>
          <el-form-item label="ID3v2 來源錯碼">
            <el-switch
              v-model="options.id3v2RepairSourceEncoding"
              :disabled="!options.id3v2Enabled"
              active-text="嘗試修復"
            />
          </el-form-item>
          <el-form-item label="ID3v2 錯碼來源編碼">
            <el-select
              v-model="options.id3v2SourceEncoding"
              :disabled="!options.id3v2Enabled || !options.id3v2RepairSourceEncoding"
            >
              <el-option label="GBK" value="gbk" />
              <el-option label="Big5" value="big5" />
              <el-option label="Shift-JIS" value="shift-jis" />
              <el-option label="UTF-8" value="utf8" />
            </el-select>
          </el-form-item>
          <el-form-item label="ID3v2 版本">
            <el-select v-model="options.id3v2Version" :disabled="!options.id3v2Enabled">
              <el-option label="2.4" :value="4" />
              <el-option label="2.3" :value="3" />
            </el-select>
          </el-form-item>
          <el-form-item label="ID3v2 編碼">
            <el-select v-model="options.id3v2Encoding" :disabled="!options.id3v2Enabled">
              <el-option label="UTF-8" value="utf8" />
              <el-option label="UTF-16" value="utf16" />
              <el-option label="UTF-16BE" value="utf16be" />
              <el-option label="Latin-1" value="latin1" />
            </el-select>
          </el-form-item>
        </template>
      </el-form>
      <div class="card-actions">
        <el-button :disabled="!paths.length || Boolean(plan)" :loading="busy" @click="scan"
          >重新掃描</el-button
        >
        <el-button
          :disabled="!selectedFileCount || Boolean(plan)"
          :loading="busy"
          @click="createPlan"
          >建立標籤預覽</el-button
        >
        <el-button v-if="plan" :disabled="busy" @click="cancelPlan">取消計畫</el-button>
        <el-button v-if="plan" type="primary" :loading="busy" @click="applyPlan"
          >確認寫入</el-button
        >
      </div>
      <el-progress
        v-if="busy && progress"
        :percentage="Math.round((progress.current / Math.max(1, progress.total)) * 100)"
        :format="() => progress?.message ?? ''"
      />
    </el-card>
    <el-empty v-if="!files.length" description="請選取 MP3、APE、OGG 或 Opus 檔案" />
    <el-card v-for="file in files" :key="file.path" shadow="never" class="audio-file-card">
      <template #header>
        <div class="card-title">
          <el-checkbox v-model="file.selected" :disabled="Boolean(file.warning) || Boolean(plan)">
            <div>
              <strong>{{ file.path.split(/[\\/]/).at(-1) }}</strong
              ><small
                >{{ file.format.toUpperCase() }} ·
                {{ file.hasCoverArt ? "含封面" : "無封面" }}</small
              >
            </div>
          </el-checkbox>
          <el-tag v-if="file.warning" type="danger">{{ file.warning }}</el-tag>
        </div>
      </template>
      <el-table :data="file.fields" max-height="300">
        <el-table-column label="轉換" width="70"
          ><template #default="scope"
            ><el-checkbox
              v-model="scope.row.selected"
              :disabled="Boolean(plan) || !fieldEnabled(file, scope.row.container)" /></template
        ></el-table-column>
        <el-table-column prop="container" label="標籤" width="150" />
        <el-table-column prop="label" label="欄位" width="160" />
        <el-table-column label="目前內容" min-width="200"
          ><template #default="scope">{{ scope.row.values.join(" / ") }}</template></el-table-column
        >
        <el-table-column v-if="plan" label="轉換預覽" min-width="200"
          ><template #default="scope">{{
            scope.row.convertedValues?.join(" / ")
          }}</template></el-table-column
        >
        <el-table-column v-if="plan" label="差異" width="90" fixed="right"
          ><template #default="scope"
            ><el-button
              link
              type="primary"
              :disabled="!scope.row.convertedValues?.length"
              @click="openFieldDiff(file, scope.row)"
              >檢視</el-button
            ></template
          ></el-table-column
        >
      </el-table>
    </el-card>
    <PreviewDiffDialog
      v-model="diffVisible"
      :title="diffTitle"
      :meta="diffMeta"
      :sections="diffSections"
    />
  </section>
</template>
