<script setup lang="ts">
import { computed, h, nextTick, onBeforeUnmount, reactive, ref, watch } from "vue";
import { open, confirm } from "@tauri-apps/plugin-dialog";
import { ElCheckbox, ElMessage } from "element-plus";
import type { CheckboxValueType, Column, RowEventHandlers } from "element-plus";
import type {
  ApplyResult,
  Direction,
  EngineKind,
  FileConversionPlan,
  FileFilterProfile,
  FilePlanItem,
  FilePlanRequest,
  SettingsV2,
  TextEncoding,
} from "@shared/contracts";
import { core, isCancellationError } from "../lib/coreClient";
import { loadSettings, patchSavedSettings, zhConvertOptions } from "../lib/settings";
import { cliInvocation } from "../lib/cli";
import { summarizeFileApplyWarnings } from "../lib/fileApplyMessages";
import {
  dropIgnoredInputWarnings,
  ignoredInputsConfirmMessage,
  withoutIgnoredInputs,
} from "../lib/ignoredInputs";
import {
  basenameLooksLikeDirectory,
  dialogFileFilters,
  ensureSupportedFilesFilter,
} from "../lib/fileFilters";
import { removeProfile, upsertProfile } from "../lib/filterProfileActions";
import {
  dialogFiltersForProfile,
  filterCollectsNothing,
  modeLabel,
  profileFilter,
  resolveProfile,
  summarizeProfile,
  profileDisplayName,
} from "../lib/filterProfiles";
import { fileConversionDefaults } from "../lib/settingsApply";
import { buildFileDiffSections, nameChangeLabel, type DiffSection } from "../lib/fileDiff";
import { formatProgressLabel, progressPercentage, type ProgressSnapshot } from "../lib/progressEta";
import { t, tn } from "@/i18n";
import PreviewDiffDialog from "../components/PreviewDiffDialog.vue";
import SideBySideDiffView from "../components/SideBySideDiffView.vue";
import FolderPickWizard, { type FolderPickResult } from "../components/FolderPickWizard.vue";
import ProfileEditorDialog from "../components/ProfileEditorDialog.vue";

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
const settingsRoot = ref<SettingsV2>();
const filterProfiles = computed(() => settingsRoot.value?.files.filterProfiles ?? []);
const currentProfileId = ref("");
const currentProfile = computed<FileFilterProfile | undefined>(() =>
  settingsRoot.value
    ? resolveProfile(settingsRoot.value.files, currentProfileId.value || undefined)
    : undefined,
);
const pickedFolder = ref(false);
const wizardVisible = ref(false);
const wizardQuick = ref(false);
const wizardPath = ref("");
const wizardProfileId = ref("");
const profileEditorVisible = ref(false);
const profileEditorSource = ref<FileFilterProfile | null>(null);
const fileFilters = computed(() =>
  ensureSupportedFilesFilter(
    currentProfile.value ? dialogFiltersForProfile(currentProfile.value) : [],
  ),
);
const folderScanMode = computed(
  () => pickedFolder.value || paths.value.some((path) => basenameLooksLikeDirectory(path)),
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
  nameChange: 240,
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
/** 建立預覽時的選項快照；之後選項一變，預覽就不再代表實際會寫入的內容。 */
const plannedInputs = ref("");
/** 預覽建立時的衝突策略；套用時的覆寫確認以它為準，不看畫面目前的值。 */
const plannedConflictPolicy = ref<FilePlanRequest["conflictPolicy"]>("skip");
const applying = ref(false);

function planInputsKey(): string {
  const profile = currentProfile.value;
  return JSON.stringify({
    paths: paths.value,
    outputPath: outputPath.value ?? null,
    outputDirectory: outputDirectory.value ?? null,
    options,
    backup: backup.value,
    previewMaxBytes: previewMaxBytes.value,
    fixCharsetExtensions: fixCharsetExtensions.value,
    filter: profile ? profileFilter(profile) : null,
  });
}

async function invalidatePlan() {
  const stale = plan.value;
  if (!stale) return;
  plan.value = undefined;
  clearPreviewState();
  await core.request("files.cancel", { planId: stale.planId }).catch(() => undefined);
  ElMessage.info(t("files.message.optionsChanged"));
}

watch(planInputsKey, (key) => {
  if (plan.value && !applying.value && key !== plannedInputs.value) void invalidatePlan();
});
let columnResizeCleanup: (() => void) | undefined;

/** 設定預設值套用完成；外部帶入的選項（命令列、拖入）須在它之後套用，才不會被預設值蓋掉。 */
const defaultsApplied = loadSettings().then((settings) => {
  const defaults = fileConversionDefaults(settings);
  options.engine = defaults.engine;
  options.direction = defaults.direction;
  options.addBom = defaults.addBom;
  options.vocabularyCorrection = defaults.vocabularyCorrection;
  options.inputEncoding = defaults.inputEncoding;
  promptAfterConversion.value = defaults.promptAfterConversion;
  backup.value = defaults.autoBackupBeforeConversion;
  defaultPath.value = defaults.defaultPath;
  settingsRoot.value = settings;
  currentProfileId.value = resolveProfile(
    settings.files,
    settings.files.lastFilterProfileId || undefined,
  ).id;
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
  if (!paths.value.length) return t("files.noPaths");
  const separator = t("files.pathSeparator");
  if (paths.value.length <= 2) return paths.value.join(separator);
  return t("files.pathsMore", {
    paths: paths.value.slice(0, 2).join(separator),
    count: paths.value.length,
  });
});

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
        title: t("files.column.resize"),
        onMousedown: (event: MouseEvent) => startColumnResize(key, event),
      }),
    ]);
}

const planColumns = computed<Column<FilePlanItem>[]>(() => [
  {
    key: "selected",
    title: t("files.column.selected"),
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
    title: t("files.column.kind"),
    width: columnWidths.kind,
    headerCellRenderer: resizableHeader("kind", t("files.column.kind")),
  },
  {
    key: "status",
    dataKey: "status",
    title: t("files.column.status"),
    width: columnWidths.status,
    headerCellRenderer: resizableHeader("status", t("files.column.status")),
  },
  {
    key: "nameChange",
    title: t("files.column.name"),
    width: columnWidths.nameChange,
    headerCellRenderer: resizableHeader("nameChange", t("files.column.name")),
    cellRenderer: ({ rowData }) => {
      const label = nameChangeLabel(rowData);
      return h("span", { class: "file-plan-name-change", title: label }, label);
    },
  },
  {
    key: "sourcePath",
    dataKey: "sourcePath",
    title: t("files.column.source"),
    width: columnWidths.sourcePath,
    headerCellRenderer: resizableHeader("sourcePath", t("files.column.source")),
  },
  {
    key: "outputPath",
    dataKey: "outputPath",
    title: t("files.column.output"),
    width: columnWidths.outputPath,
    headerCellRenderer: resizableHeader("outputPath", t("files.column.output")),
  },
  {
    key: "detectedEncoding",
    dataKey: "detectedEncoding",
    title: t("files.column.encoding"),
    width: columnWidths.detectedEncoding,
    headerCellRenderer: resizableHeader("detectedEncoding", t("files.column.encoding")),
    cellRenderer: ({ cellData }) => h("span", null, cellData ? String(cellData) : ""),
  },
  {
    key: "warning",
    dataKey: "warning",
    title: t("files.column.warning"),
    width: columnWidths.warning,
    headerCellRenderer: resizableHeader("warning", t("files.column.warning")),
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
    filters: dialogFileFilters(fileFilters.value),
  });
  if (selected) {
    paths.value = Array.isArray(selected) ? selected : [selected];
    outputPath.value = undefined;
    pickedFolder.value = false;
  }
}

async function chooseFolder() {
  const settings = settingsRoot.value ?? (await loadSettings());
  settingsRoot.value = settings;
  const remembered =
    settings.files.folderPickBehavior === "rememberLast"
      ? settings.files.filterProfiles.find((item) => item.id === settings.files.lastFilterProfileId)
      : undefined;
  wizardProfileId.value = (remembered ?? resolveProfile(settings.files)).id;
  wizardPath.value = "";
  wizardQuick.value = false;
  if (remembered) {
    // 快速路徑：先用系統選擇器取資料夾，再直接顯示步驟 3。
    const selected = await open({
      directory: true,
      multiple: false,
      recursive: options.recursive,
      defaultPath: defaultPath.value,
    });
    if (!selected || Array.isArray(selected)) return;
    wizardPath.value = selected;
    wizardQuick.value = true;
  }
  wizardVisible.value = true;
}

async function confirmFolderPick(result: FolderPickResult): Promise<void> {
  paths.value = [result.path];
  outputPath.value = undefined;
  pickedFolder.value = true;
  currentProfileId.value = result.profile.id;
  if (result.remember) {
    await patchProfiles((files) => {
      files.lastFilterProfileId = result.profile.id;
    });
  }
  await createPlan();
}

/** 只寫入磁碟上的 files 設定欄位，不帶入畫面快取裡尚未儲存的其他欄位。 */
async function patchProfiles(patch: (files: SettingsV2["files"]) => void): Promise<void> {
  settingsRoot.value = await patchSavedSettings((settings) => patch(settings.files));
}

async function saveProfile(profile: FileFilterProfile): Promise<void> {
  await patchProfiles((files) => upsertProfile(files, profile));
}

function openProfileEditor(): void {
  profileEditorSource.value = currentProfile.value ?? null;
  profileEditorVisible.value = true;
}

async function onProfileEdited(profile: FileFilterProfile): Promise<void> {
  await saveProfile(profile);
  currentProfileId.value = profile.id;
}

async function deleteCurrentProfile(): Promise<void> {
  const profile = currentProfile.value;
  if (!profile || profile.builtin) return;
  if (
    !(await confirm(t("files.filter.deleteConfirm", { name: profileDisplayName(profile) }), {
      title: t("files.filter.deleteTitle"),
    }))
  )
    return;
  await patchProfiles((files) => {
    removeProfile(files, profile.id);
  });
  currentProfileId.value = resolveProfile(settingsRoot.value!.files).id;
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
  previewSections.value = sections;
  previewMeta.value = [
    { label: t("files.column.source"), value: item.sourcePath },
    { label: t("files.column.output"), value: item.outputPath },
    ...(item.detectedEncoding
      ? [{ label: t("files.column.encoding"), value: item.detectedEncoding }]
      : []),
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
    { label: t("files.column.source"), value: item.sourcePath },
    { label: t("files.column.output"), value: item.outputPath },
  ];
  previewSections.value = buildFileDiffSections(item);
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
    if (isCancellationError(error)) ElMessage.info(t("files.message.previewCancelled"));
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
  if (!paths.value.length) return ElMessage.warning(t("files.message.selectFirst"));
  busy.value = true;
  progress.value = undefined;
  progressStartedAt.value = Date.now();
  clearPreviewState();
  let requestedInputs = planInputsKey();
  const requestedPolicy = options.conflictPolicy;
  try {
    const settings = await loadSettings();
    const profile = currentProfile.value;
    if (profile && filterCollectsNothing(profile)) {
      return ElMessage.warning(t("files.message.filterEmpty"));
    }
    const requestPlan = (includeIgnored: boolean) =>
      core.request<FileConversionPlan>(
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
          ...(profile ? { extensionFilter: profileFilter(profile) } : {}),
          previewMaxBytes: previewMaxBytes.value,
          conflictPolicy: options.conflictPolicy,
          backup: backup.value,
          preserveModifiedTime: settings.preserveModifiedTime,
          includeIgnored,
          conversion: {
            direction: options.direction,
            engine: options.engine,
            vocabularyCorrection: options.vocabularyCorrection,
            zhconvert: zhConvertOptions(settings, options.direction),
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
    let created = await requestPlan(false);
    const ignored = created.ignoredInputs ?? [];
    if (ignored.length) {
      const accepted = await confirm(ignoredInputsConfirmMessage(ignored), {
        title: t("files.message.ignoredTitle"),
        kind: "warning",
      });
      if (accepted) {
        // 第一份計畫略過了這些路徑；釋放它再以允許的設定重建。
        await core.request("files.cancel", { planId: created.planId }).catch(() => undefined);
        created = await requestPlan(true);
      } else {
        // 取消：從來源清單移除這些路徑；計畫本身已略過它們，不必重建。
        const unchanged = planInputsKey() === requestedInputs;
        paths.value = withoutIgnoredInputs(paths.value, ignored);
        if (unchanged) requestedInputs = planInputsKey();
        created = { ...created, warnings: dropIgnoredInputWarnings(created.warnings, ignored) };
      }
    }
    plannedInputs.value = requestedInputs;
    plannedConflictPolicy.value = requestedPolicy;
    plan.value = created;
    if (planInputsKey() !== requestedInputs) {
      await invalidatePlan();
      return;
    }
    if (!created.items.length) {
      ElMessage.warning(created.warnings[0] ?? t("files.message.noFiles"));
    }
    // 大型清單先讓虛擬表格掛上，再載入第一筆預覽，避免主執行緒連續長任務。
    await nextTick();
    const first = plan.value.items.find((item) => item.status === "ready") ?? plan.value.items[0];
    await loadItemPreview(first);
  } catch (error) {
    if (isCancellationError(error)) ElMessage.info(t("files.message.planCancelled"));
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
  const accepted = await confirm(tn("files.message.applyConfirm", selectedCount.value), {
    title: t("files.message.applyTitle"),
    kind: "warning",
  });
  if (!accepted || !plan.value) return;
  if (
    plannedConflictPolicy.value === "overwrite" &&
    !(await confirm(t("files.message.overwriteConfirm"), {
      title: t("files.message.overwriteTitle"),
      kind: "warning",
    }))
  )
    return;
  if (!plan.value) return;
  busy.value = true;
  applying.value = true;
  progress.value = undefined;
  progressStartedAt.value = Date.now();
  try {
    const selectedPaths = plan.value.items
      .filter((item) => item.selected)
      .map((item) => item.sourcePath);
    const result = await core.request<ApplyResult>(
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
    const skippedCount = result.skipped.length;
    const warnings = result.warnings ?? [];
    for (const warning of summarizeFileApplyWarnings(warnings)) ElMessage.warning(warning);
    if (promptAfterConversion.value) {
      if (result.succeeded.length && skippedCount)
        ElMessage.success(
          t("files.message.doneWithSkipped", {
            done: result.succeeded.length,
            skipped: skippedCount,
          }),
        );
      else if (result.succeeded.length)
        ElMessage.success(tn("files.message.done", result.succeeded.length));
      else if (skippedCount && warnings.length === 0)
        ElMessage.info(t("files.message.stoppedNothingWritten"));
      else if (skippedCount) ElMessage.info(tn("files.message.skipped", skippedCount));
    }
    if (result.failed.length)
      ElMessage.error(
        result.failed
          .map((failure: { path: string; message: string }) =>
            t("files.message.failure", { path: failure.path, message: failure.message }),
          )
          .join("\n"),
      );
    plan.value = undefined;
    clearPreviewState();
  } catch (error) {
    if (isCancellationError(error)) ElMessage.info(t("files.message.applyCancelled"));
    else ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    busy.value = false;
    applying.value = false;
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
    await defaultsApplied.catch(() => undefined);
    if (cliInvocation.value !== invocation) return;
    const cli = invocation.options;
    paths.value = cli.paths;
    pickedFolder.value = cli.paths.some((path) => basenameLooksLikeDirectory(path));
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
        <p class="eyebrow">{{ t("files.eyebrow") }}</p>
        <h1>{{ t("files.title") }}</h1>
        <p>{{ t("files.subtitle") }}</p>
      </div>
      <div class="header-actions">
        <el-button @click="chooseOutputFolder">{{ t("files.chooseOutputFolder") }}</el-button
        ><el-button @click="chooseFolder">{{ t("files.chooseSourceFolder") }}</el-button
        ><el-button type="primary" @click="chooseFiles">{{ t("files.chooseFiles") }}</el-button>
      </div>
    </header>
    <el-card shadow="never">
      <el-form label-position="top" class="option-grid">
        <el-form-item :label="t('files.mode.label')"
          ><el-select v-model="options.mode"
            ><el-option :label="t('files.mode.content')" value="content" /><el-option
              :label="t('files.mode.filename')"
              value="filename" /><el-option :label="t('files.mode.both')" value="both" /></el-select
        ></el-form-item>
        <el-form-item :label="t('files.direction.label')"
          ><el-select v-model="options.direction"
            ><el-option :label="t('common.direction.s2t')" value="s2t" /><el-option
              :label="t('common.direction.t2s')"
              value="t2s" /><el-option :label="t('files.direction.none')" value="none" /></el-select
        ></el-form-item>
        <el-form-item :label="t('files.engine.label')"
          ><el-select v-model="options.engine"
            ><el-option :label="t('files.engine.segmented')" value="segmented" /><el-option
              label="ZhConvert"
              value="zhconvert" /></el-select
        ></el-form-item>
        <el-form-item :label="t('files.inputEncoding')"
          ><el-select v-model="options.inputEncoding"
            ><el-option
              v-for="value in encodings"
              :key="value"
              :value="value"
              :label="value" /></el-select
        ></el-form-item>
        <el-form-item :label="t('files.outputEncoding')"
          ><el-select v-model="options.outputEncoding"
            ><el-option
              v-for="value in encodings"
              :key="value"
              :value="value"
              :label="value" /></el-select
        ></el-form-item>
        <el-form-item :label="t('files.conflict.label')"
          ><el-select v-model="options.conflictPolicy"
            ><el-option :label="t('files.conflict.skip')" value="skip" /><el-option
              :label="t('files.conflict.overwrite')"
              value="overwrite" /></el-select
        ></el-form-item>
      </el-form>
      <div class="switch-row">
        <el-checkbox v-model="options.vocabularyCorrection">{{
          t("files.vocabularyCorrection")
        }}</el-checkbox
        ><el-checkbox v-model="options.recursive">{{ t("files.recursive") }}</el-checkbox
        ><el-checkbox v-model="options.fixCharsetDeclaration">{{
          t("files.fixCharsetDeclaration")
        }}</el-checkbox
        ><el-checkbox v-model="options.addBom">{{ t("files.addBom") }}</el-checkbox
        ><el-checkbox v-model="backup">{{ t("files.backup") }}</el-checkbox>
      </div>
      <div v-if="folderScanMode && currentProfile" class="folder-filter">
        <span class="folder-filter-label">{{ t("files.filter.label") }}</span>
        <el-select
          :model-value="currentProfile.id"
          class="folder-filter-select"
          :aria-label="t('files.filter.selectAria')"
          @update:model-value="(value: string) => (currentProfileId = value)"
        >
          <el-option
            v-for="profile in filterProfiles"
            :key="profile.id"
            :label="
              t('files.filter.option', {
                name: profileDisplayName(profile),
                mode: modeLabel(profile.mode),
              })
            "
            :value="profile.id"
          />
        </el-select>
        <span class="folder-filter-summary">{{ summarizeProfile(currentProfile) }}</span>
        <div class="filter-editor-actions">
          <el-button @click="openProfileEditor">{{ t("files.filter.edit") }}</el-button>
          <el-button v-if="!currentProfile.builtin" @click="deleteCurrentProfile">{{
            t("common.delete")
          }}</el-button>
        </div>
      </div>
      <div class="path-summary">
        <span>{{ pathSummary }}</span>
        <div class="path-summary-actions">
          <el-button :loading="busy" :disabled="busy" @click="createPlan">{{
            t("files.createPreview")
          }}</el-button>
          <el-button v-if="busy || previewBusy" @click="cancelPlan">{{
            t("files.stopJob")
          }}</el-button>
        </div>
      </div>
      <div v-if="outputDirectory" class="path-summary">
        <span>{{ t("files.outputDirectory", { path: outputDirectory }) }}</span>
        <div class="path-summary-actions">
          <el-button @click="outputDirectory = undefined">{{
            t("files.useSourceDirectory")
          }}</el-button>
        </div>
      </div>
      <el-progress
        v-if="(busy || previewBusy) && progress"
        class="job-progress"
        :percentage="progressPercentage(progress)"
        :format="() => formatProgressLabel(progress, progressStartedAt)"
      />
    </el-card>
    <section v-if="plan" class="file-plan-panel">
      <header class="file-plan-panel-header">
        <span>{{ t("files.planTitle", { count: plan.items.length }) }}</span>
        <div class="file-plan-panel-actions">
          <el-button @click="setAllSelected(true)">{{ t("files.selectAll") }}</el-button>
          <el-button @click="setAllSelected(false)">{{ t("files.selectNone") }}</el-button>
          <el-button @click="cancelPlan">{{
            busy ? t("files.stopJob") : t("files.cancelPlan")
          }}</el-button
          ><el-button type="primary" :loading="busy" :disabled="busy" @click="applyPlan">{{
            t("files.applyPlan")
          }}</el-button>
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
                <strong>{{ t("files.preview.title") }}</strong>
                <div class="file-plan-preview-header-actions">
                  <small v-if="previewBusy">{{ t("files.preview.loading") }}</small>
                  <el-button
                    size="small"
                    :disabled="!canOpenFullscreenPreview"
                    @click="openFullscreenPreview"
                  >
                    {{ t("files.preview.fullscreen") }}
                  </el-button>
                </div>
              </div>
              <el-empty v-if="!currentItem" :description="t('files.preview.selectItem')" />
              <template v-else>
                <div v-if="previewMeta.length" class="file-plan-preview-meta">
                  <div v-for="entry in previewMeta" :key="entry.label">
                    <span>{{ entry.label }}</span>
                    <code>{{ entry.value }}</code>
                  </div>
                </div>
                <el-empty
                  v-if="!previewBusy && !currentItem.previewLoaded && options.mode !== 'filename'"
                  :description="t('files.preview.preparing')"
                />
                <p
                  v-else-if="previewSections.length && !previewDiffReady"
                  class="file-plan-preview-pending"
                >
                  {{ t("files.preview.rendering") }}
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
                    !previewBusy
                  "
                  :description="t('files.preview.empty')"
                />
              </template>
            </aside>
          </el-splitter-panel>
        </el-splitter>
      </div>
    </section>
    <FolderPickWizard
      v-model="wizardVisible"
      v-model:recursive="options.recursive"
      v-model:profile-id="wizardProfileId"
      :profiles="filterProfiles"
      :initial-path="wizardPath"
      :quick="wizardQuick"
      :default-path="defaultPath"
      @save-profile="saveProfile"
      @confirm="confirmFolderPick"
    />
    <ProfileEditorDialog
      v-model="profileEditorVisible"
      :profile="profileEditorSource"
      :existing="filterProfiles"
      :scan-start-path="paths[0]"
      @save="onProfileEdited"
    />
    <PreviewDiffDialog
      v-model="previewFullscreenVisible"
      :title="t('files.preview.dialogTitle')"
      :meta="previewMeta"
      :sections="previewSections"
      fullscreen
      enable-nav
    />
  </section>
</template>

<style scoped>
.folder-filter {
  margin-top: 12px;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}
.folder-filter-label {
  display: block;
  margin-bottom: 6px;
  font-weight: 600;
}
.folder-filter-select {
  width: 240px;
}
.folder-filter-summary {
  color: #5b746f;
  font-size: 13px;
}
.file-plan-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
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
  display: flex;
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
}
.file-plan-splitter {
  flex: 1 1 auto;
  width: 100%;
  height: 100%;
  min-height: 0;
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
.file-plan-name-change,
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
