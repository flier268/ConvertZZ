<script setup lang="ts">
import { computed, h, nextTick, onBeforeUnmount, reactive, ref, watch } from "vue";
import { open, confirm } from "@tauri-apps/plugin-dialog";
import { ElCheckbox, ElMessage } from "element-plus";
import type { CheckboxValueType, Column, RowEventHandlers } from "element-plus";
import type {
  Direction,
  EngineKind,
  FileConversionPlan,
  FilePlanItem,
  FilePlanRequest,
  TextEncoding,
} from "@shared/contracts";
import { core, isCancellationError } from "../lib/coreClient";
import { loadSettings, zhConvertOptions } from "../lib/settings";
import { cliInvocation } from "../lib/cli";
import { ensureSupportedFilesFilter } from "../lib/fileFilters";
import { fileConversionDefaults } from "../lib/settingsApply";
import { buildFileDiffSections, type DiffSection } from "../lib/fileDiff";
import { formatProgressLabel, progressPercentage, type ProgressSnapshot } from "../lib/progressEta";
import PreviewDiffDialog from "../components/PreviewDiffDialog.vue";
import SideBySideDiffView from "../components/SideBySideDiffView.vue";

defineOptions({ name: "FilesPage" });

const paths = ref<string[]>([]);
const outputPath = ref<string>();
const outputDirectory = ref<string>();
const plan = ref<FileConversionPlan>();
const busy = ref(false);
const previewBusy = ref(false);
const previewDiffReady = ref(false);
const previewFullscreenVisible = ref(false);
const currentItem = ref<FilePlanItem>();
const previewSections = ref<DiffSection[]>([]);
const previewMeta = ref<Array<{ label: string; value: string }>>([]);
const previewRequestId = ref(0);
const promptAfterConversion = ref(true);
const backup = ref(true);
const defaultPath = ref<string>();
const fileFilters = ref(
  ensureSupportedFilesFilter([
    {
      name: "文字與網頁文件",
      extensions: ["txt", "log", "ini", "srt", "ass", "html", "htm", "css", "js", "php", "asp"],
    },
  ]),
);
const previewMaxBytes = ref(6 * 1024);
const fixCharsetExtensions = ref<string[]>([]);
const progress = ref<ProgressSnapshot>();
const progressStartedAt = ref<number>();
const activeRequestId = ref<string>();
const listPaneSize = ref<string | number>("55%");
const previewPaneSize = ref<string | number>("45%");
const columnWidths = reactive<Record<string, number>>({
  selected: 64,
  kind: 88,
  status: 88,
  sourcePath: 260,
  outputPath: 260,
  detectedEncoding: 100,
  warning: 160,
});
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

let previewTimer: ReturnType<typeof setTimeout> | undefined;
let columnResizeCleanup: (() => void) | undefined;

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

const selectedCount = computed(
  () => plan.value?.items.filter((item) => item.status === "ready" && item.selected).length ?? 0,
);

const readyCount = computed(
  () => plan.value?.items.filter((item) => item.status === "ready").length ?? 0,
);

const allReadySelected = computed(
  () => readyCount.value > 0 && selectedCount.value === readyCount.value,
);

const someReadySelected = computed(() => selectedCount.value > 0 && !allReadySelected.value);

const pathSummary = computed(() => {
  if (!paths.value.length) return "尚未選取路徑";
  if (paths.value.length <= 2) return paths.value.join("、");
  return `${paths.value.slice(0, 2).join("、")} 等 ${paths.value.length} 項`;
});

const activeSection = computed(
  () =>
    previewSections.value.find((section) => section.title === "內容") ?? previewSections.value[0],
);

const canOpenFullscreenPreview = computed(
  () =>
    Boolean(currentItem.value) &&
    previewDiffReady.value &&
    !previewBusy.value &&
    previewSections.value.some((section) => section.source || section.output),
);

function openFullscreenPreview() {
  if (!canOpenFullscreenPreview.value) return;
  previewFullscreenVisible.value = true;
}

function startColumnResize(key: string, event: MouseEvent) {
  event.preventDefault();
  event.stopPropagation();
  columnResizeCleanup?.();
  const startX = event.clientX;
  const startWidth = columnWidths[key] ?? 80;
  const previousUserSelect = document.body.style.userSelect;
  document.body.style.userSelect = "none";
  document.body.classList.add("file-plan-col-resizing");

  const onMove = (moveEvent: MouseEvent) => {
    columnWidths[key] = Math.max(48, Math.round(startWidth + (moveEvent.clientX - startX)));
  };
  const onUp = () => {
    document.body.style.userSelect = previousUserSelect;
    document.body.classList.remove("file-plan-col-resizing");
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
    columnResizeCleanup = undefined;
  };
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
  columnResizeCleanup = onUp;
}

function resizableHeader(key: string, content: string | ReturnType<typeof h>) {
  return () =>
    h("div", { class: "file-plan-header-cell" }, [
      typeof content === "string"
        ? h("span", { class: "file-plan-header-label" }, content)
        : content,
      h("span", {
        class: "file-plan-col-resizer",
        title: "拖曳調整欄寬",
        onMousedown: (event: MouseEvent) => startColumnResize(key, event),
      }),
    ]);
}

const planColumns = computed<Column<FilePlanItem>[]>(() => [
  {
    key: "selected",
    title: "執行",
    width: columnWidths.selected,
    align: "center",
    cellRenderer: ({ rowData }) =>
      h(ElCheckbox, {
        modelValue: rowData.selected,
        disabled: rowData.status !== "ready",
        "onUpdate:modelValue": (value: CheckboxValueType) => {
          rowData.selected = Boolean(value);
        },
        onClick: (event: MouseEvent) => event.stopPropagation(),
      }),
    headerCellRenderer: resizableHeader(
      "selected",
      h(ElCheckbox, {
        modelValue: allReadySelected.value,
        indeterminate: someReadySelected.value,
        disabled: readyCount.value === 0,
        "onUpdate:modelValue": (value: CheckboxValueType) => {
          setAllSelected(Boolean(value));
        },
      }),
    ),
  },
  {
    key: "kind",
    dataKey: "kind",
    title: "類型",
    width: columnWidths.kind,
    headerCellRenderer: resizableHeader("kind", "類型"),
  },
  {
    key: "status",
    dataKey: "status",
    title: "狀態",
    width: columnWidths.status,
    headerCellRenderer: resizableHeader("status", "狀態"),
  },
  {
    key: "sourcePath",
    dataKey: "sourcePath",
    title: "來源",
    width: columnWidths.sourcePath,
    headerCellRenderer: resizableHeader("sourcePath", "來源"),
  },
  {
    key: "outputPath",
    dataKey: "outputPath",
    title: "輸出",
    width: columnWidths.outputPath,
    headerCellRenderer: resizableHeader("outputPath", "輸出"),
  },
  {
    key: "detectedEncoding",
    dataKey: "detectedEncoding",
    title: "編碼",
    width: columnWidths.detectedEncoding,
    headerCellRenderer: resizableHeader("detectedEncoding", "編碼"),
    cellRenderer: ({ cellData }) => h("span", null, cellData ? String(cellData) : ""),
  },
  {
    key: "warning",
    dataKey: "warning",
    title: "警告",
    width: columnWidths.warning,
    headerCellRenderer: resizableHeader("warning", "警告"),
    cellRenderer: ({ cellData }) =>
      h(
        "span",
        { class: "file-plan-warning", title: cellData ? String(cellData) : "" },
        cellData ? String(cellData) : "",
      ),
  },
]);

const rowEventHandlers: RowEventHandlers = {
  onClick: ({ rowData }) => {
    scheduleItemPreview(rowData as FilePlanItem);
  },
};

function rowClass({ rowData }: { rowData: FilePlanItem }) {
  return rowData.sourcePath === currentItem.value?.sourcePath ? "is-current-row" : "";
}

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

function clearPreviewState() {
  if (previewTimer) {
    clearTimeout(previewTimer);
    previewTimer = undefined;
  }
  currentItem.value = undefined;
  previewSections.value = [];
  previewMeta.value = [];
  previewBusy.value = false;
  previewDiffReady.value = false;
  previewFullscreenVisible.value = false;
}

function updatePreviewPanel(item: FilePlanItem) {
  currentItem.value = item;
  previewDiffReady.value = false;
  const sections = buildFileDiffSections(item);
  previewSections.value = item.previewLoaded
    ? sections
    : sections.filter((section) => section.title === "檔名");
  previewMeta.value = [
    { label: "來源", value: item.sourcePath },
    { label: "輸出", value: item.outputPath },
    ...(item.detectedEncoding ? [{ label: "編碼", value: item.detectedEncoding }] : []),
  ];
  void nextTick(() => {
    requestAnimationFrame(() => {
      if (currentItem.value?.sourcePath === item.sourcePath) previewDiffReady.value = true;
    });
  });
}

function setAllSelected(selected: boolean) {
  if (!plan.value) return;
  for (const item of plan.value.items) {
    if (item.status === "ready") item.selected = selected;
  }
}

function scheduleItemPreview(item: FilePlanItem | undefined) {
  if (previewTimer) clearTimeout(previewTimer);
  if (!item) {
    clearPreviewState();
    return;
  }
  currentItem.value = item;
  previewTimer = setTimeout(() => {
    previewTimer = undefined;
    void loadItemPreview(item);
  }, 100);
}

async function loadItemPreview(item: FilePlanItem | undefined) {
  if (!plan.value || !item) {
    clearPreviewState();
    return;
  }
  if (item.previewLoaded || item.kind === "directory" || options.mode === "filename") {
    if (!item.previewLoaded && (item.kind === "directory" || options.mode === "filename")) {
      item.previewLoaded = true;
    }
    updatePreviewPanel(item);
    return;
  }
  const requestId = ++previewRequestId.value;
  previewBusy.value = true;
  previewDiffReady.value = false;
  currentItem.value = item;
  previewMeta.value = [
    { label: "來源", value: item.sourcePath },
    { label: "輸出", value: item.outputPath },
  ];
  previewSections.value = buildFileDiffSections(item).filter((section) => section.title === "檔名");
  progress.value = undefined;
  progressStartedAt.value = Date.now();
  try {
    const previewed = await core.request<FilePlanItem>(
      "files.preview",
      {
        planId: plan.value.planId,
        sourcePath: item.sourcePath,
      },
      {
        onProgress: (value) => {
          progress.value = value;
        },
        onRequestId: (id) => {
          activeRequestId.value = id;
        },
      },
    );
    if (requestId !== previewRequestId.value || !plan.value) return;
    const index = plan.value.items.findIndex((entry) => entry.sourcePath === item.sourcePath);
    if (index >= 0) plan.value.items[index] = { ...plan.value.items[index], ...previewed };
    updatePreviewPanel(plan.value.items[index] ?? previewed);
  } catch (error) {
    if (requestId !== previewRequestId.value) return;
    if (isCancellationError(error)) ElMessage.info("已取消預覽。");
    else ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    if (requestId === previewRequestId.value) {
      previewBusy.value = false;
      activeRequestId.value = undefined;
      progress.value = undefined;
      progressStartedAt.value = undefined;
    }
  }
}

async function createPlan() {
  if (!paths.value.length) return ElMessage.warning("請先選取檔案或資料夾。");
  busy.value = true;
  progress.value = undefined;
  progressStartedAt.value = Date.now();
  clearPreviewState();
  try {
    const settings = await loadSettings();
    const allowedExtensions = Array.from(
      new Set(
        fileFilters.value
          .flatMap((filter) => filter.extensions)
          .map((extension) => `.${extension.toLowerCase()}`),
      ),
    );
    plan.value = await core.request<FileConversionPlan>(
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
      {
        onProgress: (value) => {
          progress.value = value;
        },
        onRequestId: (id) => {
          activeRequestId.value = id;
        },
      },
    );
    // 大型清單先讓虛擬表格掛上，再載入第一筆預覽，避免主執行緒連續長任務。
    await nextTick();
    const first = plan.value.items.find((item) => item.status === "ready") ?? plan.value.items[0];
    await loadItemPreview(first);
  } catch (error) {
    if (isCancellationError(error)) ElMessage.info("已取消建立預覽。");
    else ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    busy.value = false;
    activeRequestId.value = undefined;
    progress.value = undefined;
    progressStartedAt.value = undefined;
  }
}

async function applyPlan() {
  if (!plan.value) return;
  const accepted = await confirm(`將執行 ${selectedCount.value} 個檔案操作。`, {
    title: "確認檔案轉換",
    kind: "warning",
  });
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
  progressStartedAt.value = Date.now();
  try {
    const selectedPaths = plan.value.items
      .filter((item) => item.selected)
      .map((item) => item.sourcePath);
    const result = await core.request<{
      succeeded: string[];
      skipped?: string[];
      failed: Array<{ path: string; message: string }>;
    }>(
      "files.apply",
      { planId: plan.value.planId, selectedPaths },
      {
        onProgress: (value) => {
          progress.value = value;
        },
        onRequestId: (id) => {
          activeRequestId.value = id;
        },
      },
    );
    const skippedCount = result.skipped?.length ?? 0;
    if (promptAfterConversion.value) {
      if (result.succeeded.length && skippedCount)
        ElMessage.success(
          `已完成 ${result.succeeded.length} 個檔案，另有 ${skippedCount} 個未處理（已停止或略過）。`,
        );
      else if (result.succeeded.length)
        ElMessage.success(`已完成 ${result.succeeded.length} 個檔案。`);
      else if (skippedCount) ElMessage.info("已停止檔案轉換；沒有寫入任何檔案。");
    }
    if (result.failed.length)
      ElMessage.error(
        result.failed
          .map(
            (failure: { path: string; message: string }) => `${failure.path}：${failure.message}`,
          )
          .join("\n"),
      );
    plan.value = undefined;
    clearPreviewState();
  } catch (error) {
    if (isCancellationError(error)) ElMessage.info("已取消檔案轉換。");
    else ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    busy.value = false;
    activeRequestId.value = undefined;
    progress.value = undefined;
    progressStartedAt.value = undefined;
  }
}

async function cancelPlan() {
  const requestId = activeRequestId.value;
  const planId = plan.value?.planId;
  if (requestId) {
    try {
      await core.cancel(requestId);
    } catch {
      // 進行中的作業仍可能被 files.cancel 停住。
    }
  }
  if (planId) {
    await core.request("files.cancel", { planId });
  }
  if (!busy.value && !previewBusy.value) {
    plan.value = undefined;
    clearPreviewState();
  }
}

onBeforeUnmount(() => {
  if (previewTimer) clearTimeout(previewTimer);
  columnResizeCleanup?.();
});

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
        <p>先建立檔案清單，點選後才載入內容預覽；勾選後才會轉換寫入。</p>
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
        <span>{{ pathSummary }}</span>
        <el-button :loading="busy" :disabled="busy" @click="createPlan">建立預覽</el-button>
        <el-button v-if="busy || previewBusy" @click="cancelPlan">停止作業</el-button>
      </div>
      <div v-if="outputDirectory" class="path-summary">
        <span>輸出目錄：{{ outputDirectory }}</span
        ><el-button @click="outputDirectory = undefined">使用原目錄</el-button>
      </div>
      <el-progress
        v-if="(busy || previewBusy) && progress"
        :percentage="progressPercentage(progress)"
        :format="() => formatProgressLabel(progress, progressStartedAt)"
      />
    </el-card>
    <section v-if="plan" class="file-plan-panel">
      <header class="file-plan-panel-header">
        <span>變更預覽（{{ plan.items.length }}）</span>
        <div class="file-plan-panel-actions">
          <el-button @click="setAllSelected(true)">全選</el-button>
          <el-button @click="setAllSelected(false)">取消全選</el-button>
          <el-button @click="cancelPlan">{{ busy ? "停止作業" : "取消計畫" }}</el-button
          ><el-button type="primary" :loading="busy" :disabled="busy" @click="applyPlan"
            >確認執行</el-button
          >
        </div>
      </header>
      <el-alert
        v-for="warning in plan.warnings"
        :key="warning"
        :title="warning"
        type="warning"
        :closable="false"
      />
      <div class="file-plan-layout">
        <el-splitter class="file-plan-splitter">
          <el-splitter-panel v-model:size="listPaneSize" :min="280">
            <div class="file-plan-list">
              <el-auto-resizer>
                <template #default="{ height, width }">
                  <el-table-v2
                    :columns="planColumns"
                    :data="plan.items"
                    :width="width"
                    :height="height"
                    :row-height="44"
                    :header-height="44"
                    row-key="sourcePath"
                    fixed
                    :cache="6"
                    :row-class="rowClass"
                    :row-event-handlers="rowEventHandlers"
                  />
                </template>
              </el-auto-resizer>
            </div>
          </el-splitter-panel>
          <el-splitter-panel v-model:size="previewPaneSize" :min="260">
            <aside class="file-plan-preview">
              <div class="file-plan-preview-header">
                <strong>檔案預覽</strong>
                <div class="file-plan-preview-header-actions">
                  <small v-if="previewBusy">載入中…</small>
                  <el-button
                    size="small"
                    :disabled="!canOpenFullscreenPreview"
                    @click="openFullscreenPreview"
                  >
                    全視窗預覽
                  </el-button>
                </div>
              </div>
              <el-empty v-if="!currentItem" description="選取左側檔案以載入預覽" />
              <template v-else>
                <div v-if="previewMeta.length" class="file-plan-preview-meta">
                  <div v-for="entry in previewMeta" :key="entry.label">
                    <span>{{ entry.label }}</span>
                    <code>{{ entry.value }}</code>
                  </div>
                </div>
                <el-empty
                  v-if="!previewBusy && !currentItem.previewLoaded && options.mode !== 'filename'"
                  description="正在準備內容預覽…"
                />
                <p
                  v-else-if="previewSections.length && !previewDiffReady"
                  class="file-plan-preview-pending"
                >
                  正在繪製差異…
                </p>
                <div
                  v-for="section in previewDiffReady ? previewSections : []"
                  :key="section.title"
                  class="file-plan-section"
                >
                  <h3 v-if="previewSections.length > 1">{{ section.title }}</h3>
                  <SideBySideDiffView
                    :source="section.source"
                    :output="section.output"
                    :source-label="section.sourceLabel"
                    :output-label="section.outputLabel"
                    compact
                  />
                </div>
                <el-empty
                  v-if="
                    currentItem.previewLoaded &&
                    previewDiffReady &&
                    !previewSections.length &&
                    !previewBusy &&
                    !activeSection
                  "
                  description="沒有可顯示的差異內容"
                />
              </template>
            </aside>
          </el-splitter-panel>
        </el-splitter>
      </div>
    </section>
    <PreviewDiffDialog
      v-model="previewFullscreenVisible"
      title="檔案差異預覽"
      :meta="previewMeta"
      :sections="previewSections"
      fullscreen
      enable-nav
    />
  </section>
</template>

<style scoped>
.file-plan-panel {
  display: grid;
  gap: 12px;
}
.file-plan-panel-header {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  font-weight: 600;
}
.file-plan-panel-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.file-plan-layout {
  min-height: 420px;
}
.file-plan-splitter {
  height: 420px;
}
.file-plan-splitter :deep(.el-splitter-bar) {
  width: 10px;
}
.file-plan-splitter :deep(.el-splitter-bar__dragger-horizontal) {
  width: 10px;
  height: 100%;
}
.file-plan-splitter :deep(.el-splitter-bar__dragger-horizontal::before) {
  width: 3px;
  height: 56px;
  border-radius: 999px;
  background-color: #9bb8b1;
}
.file-plan-list {
  height: 100%;
  min-width: 0;
  margin-right: 6px;
  border: 1px solid #d7e4e0;
  border-radius: 12px;
  overflow: hidden;
  background: #fff;
}
.file-plan-list :deep(.el-table-v2__row.is-current-row) {
  background: #e8f5f1;
}
.file-plan-list :deep(.el-table-v2__row) {
  cursor: pointer;
}
.file-plan-list :deep(.el-table-v2__header-cell) {
  position: relative;
  overflow: visible;
}
/* headerCellRenderer 在 table-v2 內渲染，需 :deep 才吃得到 scoped 樣式 */
.file-plan-list :deep(.file-plan-header-cell) {
  position: relative;
  display: flex;
  align-items: center;
  width: 100%;
  height: 100%;
  min-width: 0;
  padding-right: 10px;
  box-sizing: border-box;
}
.file-plan-list :deep(.file-plan-header-label) {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.file-plan-list :deep(.file-plan-col-resizer) {
  position: absolute;
  inset: 0 0 0 auto;
  width: 10px;
  cursor: col-resize;
  z-index: 3;
}
.file-plan-list :deep(.file-plan-col-resizer::after) {
  content: "";
  position: absolute;
  top: 10px;
  bottom: 10px;
  right: 3px;
  width: 2px;
  border-radius: 999px;
  background: #c5d6d1;
}
.file-plan-list :deep(.file-plan-col-resizer:hover::after) {
  background: var(--el-color-primary);
}
.file-plan-warning {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.file-plan-preview {
  height: 100%;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-left: 6px;
  padding: 12px 14px;
  overflow: auto;
  box-sizing: border-box;
  border: 1px solid #d7e4e0;
  border-radius: 12px;
  background: #fff;
}
.file-plan-preview-header {
  flex: 0 0 auto;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 8px;
  padding-bottom: 2px;
}
.file-plan-preview-header strong {
  font-size: 14px;
}
.file-plan-preview-header-actions {
  display: flex;
  align-items: center;
  gap: 8px;
}
.file-plan-preview-header small {
  color: #5b746f;
}
.file-plan-preview-meta {
  flex: 0 0 auto;
  display: grid;
  gap: 6px;
  padding: 8px 10px;
  border-radius: 8px;
  background: #f4f8f7;
}
.file-plan-preview-meta > div {
  display: grid;
  grid-template-columns: 48px minmax(0, 1fr);
  gap: 8px;
  align-items: start;
}
.file-plan-preview-meta span {
  color: #5b746f;
  font-size: 12px;
  line-height: 1.5;
}
.file-plan-preview-meta code {
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 12px;
  line-height: 1.5;
  word-break: break-all;
  white-space: pre-wrap;
  color: #17302d;
}
.file-plan-preview-pending {
  flex: 0 0 auto;
  margin: 0;
  color: #5b746f;
  font-size: 13px;
}
.file-plan-section {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
}
.file-plan-section h3 {
  margin: 0;
  flex: 0 0 auto;
  font-size: 13px;
  color: #5b746f;
  font-weight: 600;
}
.file-plan-preview :deep(.preview-diff-root.compact) {
  flex: 1 1 auto;
  min-height: 0;
}
.file-plan-preview :deep(.preview-diff-root.compact .preview-diff-pane) {
  padding: 0 10px;
  border-radius: 8px;
  background: #f7faf9;
}
</style>

<style>
body.file-plan-col-resizing,
body.file-plan-col-resizing * {
  cursor: col-resize !important;
  user-select: none !important;
}
</style>
