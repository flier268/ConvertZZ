<script setup lang="ts">
import { reactive, ref, watch } from "vue";
import { open, confirm } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import type {
  Direction,
  EngineKind,
  FileConversionPlan,
  FilePlanRequest,
  TextEncoding,
} from "@shared/contracts";
import { sidecar } from "../lib/sidecar";
import { loadSettings, zhConvertOptions } from "../lib/settings";
import { cliInvocation } from "../lib/cli";
import { fileConversionDefaults } from "../lib/settingsApply";

const paths = ref<string[]>([]);
const outputPath = ref<string>();
const outputDirectory = ref<string>();
const plan = ref<FileConversionPlan>();
const busy = ref(false);
const promptAfterConversion = ref(true);
const backup = ref(true);
const defaultPath = ref<string>();
const fileFilters = ref([
  {
    name: "文字與網頁文件",
    extensions: ["txt", "log", "ini", "srt", "ass", "html", "htm", "css", "js", "php", "asp"],
  },
]);
const previewMaxBytes = ref(6 * 1024);
const fixCharsetExtensions = ref<string[]>([]);
const progress = ref<{ current: number; total: number; message: string }>();
const options = reactive({
  mode: "content" as FilePlanRequest["mode"],
  recursive: true,
  inputEncoding: "auto" as TextEncoding,
  outputEncoding: "auto" as TextEncoding,
  addBom: false,
  fixCharsetDeclaration: true,
  conflictPolicy: "skip" as FilePlanRequest["conflictPolicy"],
  direction: "s2t" as Direction,
  engine: "segmented" as EngineKind,
  vocabularyCorrection: true,
});

loadSettings().then((settings) => {
  const defaults = fileConversionDefaults(settings);
  options.engine = defaults.engine;
  options.direction = defaults.direction;
  options.addBom = defaults.addBom;
  options.vocabularyCorrection = defaults.vocabularyCorrection;
  options.inputEncoding = defaults.inputEncoding;
  promptAfterConversion.value = defaults.promptAfterConversion;
  backup.value = defaults.autoBackupBeforeConversion;
  defaultPath.value = defaults.defaultPath;
  fileFilters.value = defaults.fileFilters.length ? defaults.fileFilters : fileFilters.value;
  previewMaxBytes.value = defaults.previewMaxBytes;
  fixCharsetExtensions.value = defaults.fixCharsetExtensions;
});

const encodings = [
  "auto",
  "utf8",
  "utf8-bom",
  "utf16le",
  "utf16be",
  "big5",
  "gbk",
  "shift-jis",
  "euc-jp",
  "iso-2022-jp",
  "hz-gb-2312",
];

async function chooseFiles() {
  const selected = await open({
    multiple: true,
    defaultPath: defaultPath.value,
    filters: fileFilters.value.length ? fileFilters.value : undefined,
  });
  if (selected) {
    paths.value = Array.isArray(selected) ? selected : [selected];
    outputPath.value = undefined;
  }
}

async function chooseFolder() {
  const selected = await open({
    directory: true,
    multiple: false,
    recursive: options.recursive,
    defaultPath: defaultPath.value,
  });
  if (selected) {
    paths.value = [selected as string];
    outputPath.value = undefined;
  }
}

async function chooseOutputFolder() {
  const selected = await open({
    directory: true,
    multiple: false,
    defaultPath: outputDirectory.value ?? defaultPath.value,
  });
  if (selected) outputDirectory.value = selected as string;
}

async function createPlan() {
  if (!paths.value.length) return ElMessage.warning("請先選取檔案或資料夾。");
  busy.value = true;
  progress.value = undefined;
  try {
    const settings = await loadSettings();
    const allowedExtensions = Array.from(
      new Set(
        fileFilters.value
          .flatMap((filter) => filter.extensions)
          .map((extension) => `.${extension.toLowerCase()}`),
      ),
    );
    plan.value = await sidecar.request<FileConversionPlan>(
      "files.plan",
      {
        paths: paths.value,
        outputPath: outputPath.value,
        outputDirectory: outputDirectory.value,
        mode: options.mode,
        recursive: options.recursive,
        inputEncoding: options.inputEncoding,
        outputEncoding: options.outputEncoding,
        addBom: options.addBom,
        fixCharsetDeclaration: options.fixCharsetDeclaration,
        fixCharsetExtensions: fixCharsetExtensions.value,
        allowedExtensions,
        previewMaxBytes: previewMaxBytes.value,
        conflictPolicy: options.conflictPolicy,
        backup: backup.value,
        conversion: {
          direction: options.direction,
          engine: options.engine,
          vocabularyCorrection: options.vocabularyCorrection,
          zhconvert: zhConvertOptions(settings, options.direction),
          dictionaryPath: settings.dictionaryPath,
        },
      } satisfies FilePlanRequest,
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

async function applyPlan() {
  if (!plan.value) return;
  const accepted = await confirm(
    `將執行 ${plan.value.items.filter((item) => item.status === "ready" && item.selected).length} 個檔案操作。`,
    { title: "確認檔案轉換", kind: "warning" },
  );
  if (!accepted) return;
  if (
    options.conflictPolicy === "overwrite" &&
    !(await confirm("覆寫會取代既有的同名檔案。是否確定繼續？", {
      title: "確認覆寫",
      kind: "warning",
    }))
  )
    return;
  busy.value = true;
  progress.value = undefined;
  try {
    const selectedPaths = plan.value.items
      .filter((item) => item.selected)
      .map((item) => item.sourcePath);
    const result = await sidecar.request<{
      succeeded: string[];
      failed: Array<{ path: string; message: string }>;
    }>("files.apply", { planId: plan.value.planId, selectedPaths }, 600_000, (value) => {
      progress.value = value;
    });
    if (promptAfterConversion.value)
      ElMessage.success(`已完成 ${result.succeeded.length} 個檔案。`);
    if (result.failed.length)
      ElMessage.error(
        result.failed
          .map(
            (failure: { path: string; message: string }) => `${failure.path}：${failure.message}`,
          )
          .join("\n"),
      );
    plan.value = undefined;
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    busy.value = false;
  }
}

async function cancelPlan() {
  if (!plan.value) return;
  await sidecar.request("files.cancel", { planId: plan.value.planId });
  plan.value = undefined;
}

watch(
  cliInvocation,
  async (invocation) => {
    if (invocation?.options.mode !== "file") return;
    const cli = invocation.options;
    paths.value = cli.paths;
    outputPath.value = cli.outputPath;
    options.mode = cli.operation;
    options.inputEncoding = cli.inputEncoding;
    options.outputEncoding = cli.outputEncoding;
    options.direction = cli.direction;
    options.engine = cli.engine;
    backup.value = cli.backup;
    if (cli.vocabularyCorrection !== "settings")
      options.vocabularyCorrection = cli.vocabularyCorrection === "enabled";
    if (paths.value.length) await createPlan();
  },
  { immediate: true },
);
</script>

<template>
  <section class="page-stack">
    <header class="page-header">
      <div>
        <p class="eyebrow">BATCH</p>
        <h1>檔案與檔名</h1>
        <p>所有變更都會先建立預覽計畫。</p>
      </div>
      <div class="header-actions">
        <el-button @click="chooseOutputFolder">選取輸出目錄</el-button
        ><el-button @click="chooseFolder">選取來源資料夾</el-button
        ><el-button type="primary" @click="chooseFiles">選取檔案</el-button>
      </div>
    </header>
    <el-card shadow="never">
      <el-form label-position="top" class="option-grid">
        <el-form-item label="作業"
          ><el-select v-model="options.mode"
            ><el-option label="轉換內容" value="content" /><el-option
              label="轉換檔名"
              value="filename" /><el-option label="內容與檔名" value="both" /></el-select
        ></el-form-item>
        <el-form-item label="方向"
          ><el-select v-model="options.direction"
            ><el-option label="簡轉繁" value="s2t" /><el-option
              label="繁轉簡"
              value="t2s" /><el-option label="不轉換" value="none" /></el-select
        ></el-form-item>
        <el-form-item label="引擎"
          ><el-select v-model="options.engine"
            ><el-option label="新式分詞" value="segmented" /><el-option
              label="舊版字典"
              value="legacy" /><el-option label="ZhConvert" value="zhconvert" /></el-select
        ></el-form-item>
        <el-form-item label="來源編碼"
          ><el-select v-model="options.inputEncoding"
            ><el-option
              v-for="value in encodings"
              :key="value"
              :value="value"
              :label="value" /></el-select
        ></el-form-item>
        <el-form-item label="輸出編碼"
          ><el-select v-model="options.outputEncoding"
            ><el-option
              v-for="value in encodings"
              :key="value"
              :value="value"
              :label="value" /></el-select
        ></el-form-item>
        <el-form-item label="衝突策略"
          ><el-select v-model="options.conflictPolicy"
            ><el-option label="略過" value="skip" /><el-option
              label="覆寫"
              value="overwrite" /></el-select
        ></el-form-item>
      </el-form>
      <div class="switch-row">
        <el-checkbox v-model="options.vocabularyCorrection">詞彙修正</el-checkbox
        ><el-checkbox v-model="options.recursive">包含子資料夾</el-checkbox
        ><el-checkbox v-model="options.fixCharsetDeclaration">修正 charset 宣告</el-checkbox
        ><el-checkbox v-model="options.addBom">加入 BOM</el-checkbox
        ><el-checkbox v-model="backup">轉換前備份（.bak）</el-checkbox>
      </div>
      <div class="path-summary">
        <span>{{ paths.length ? paths.join("、") : "尚未選取路徑" }}</span
        ><el-button :loading="busy" @click="createPlan">建立預覽</el-button>
      </div>
      <div v-if="outputDirectory" class="path-summary">
        <span>輸出目錄：{{ outputDirectory }}</span
        ><el-button @click="outputDirectory = undefined">使用原目錄</el-button>
      </div>
      <el-progress
        v-if="busy && progress"
        :percentage="Math.round((progress.current / Math.max(1, progress.total)) * 100)"
        :format="() => progress?.message ?? ''"
      />
    </el-card>
    <el-card v-if="plan" shadow="never">
      <template #header
        ><div class="card-title">
          <span>變更預覽</span>
          <div>
            <el-button @click="cancelPlan">{{ busy ? "停止作業" : "取消計畫" }}</el-button
            ><el-button type="primary" :loading="busy" :disabled="busy" @click="applyPlan"
              >確認執行</el-button
            >
          </div>
        </div></template
      >
      <el-alert
        v-for="warning in plan.warnings"
        :key="warning"
        :title="warning"
        type="warning"
        :closable="false"
      />
      <el-table :data="plan.items" height="390">
        <el-table-column label="執行" width="70"
          ><template #default="scope"
            ><el-checkbox
              v-model="scope.row.selected"
              :disabled="scope.row.status !== 'ready'" /></template
        ></el-table-column>
        <el-table-column prop="kind" label="類型" width="90" />
        <el-table-column prop="status" label="狀態" width="90" />
        <el-table-column prop="sourcePath" label="來源" min-width="260" show-overflow-tooltip />
        <el-table-column prop="outputPath" label="輸出" min-width="260" show-overflow-tooltip />
        <el-table-column prop="detectedEncoding" label="編碼" width="120" />
        <el-table-column
          prop="sourcePreview"
          label="來源預覽"
          min-width="200"
          show-overflow-tooltip
        />
        <el-table-column
          prop="outputPreview"
          label="輸出預覽"
          min-width="200"
          show-overflow-tooltip
        />
        <el-table-column prop="warning" label="警告" min-width="220" show-overflow-tooltip />
      </el-table>
    </el-card>
  </section>
</template>
