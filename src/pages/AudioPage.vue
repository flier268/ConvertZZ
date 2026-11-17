<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from "vue";
import { open, confirm } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import { t, tn } from "@/i18n";
import type {
  AudioTagField,
  AudioTagFile,
  AudioRescanFileRequest,
  AudioTagPlan,
  AudioTagPlanRequest,
  ApplyResult,
  Direction,
  EngineKind,
  SettingsV2,
  TextEncoding,
} from "@shared/contracts";
import { core, isCancellationError } from "../lib/coreClient";
import { loadSettings, patchSavedSettings, zhConvertOptions } from "../lib/settings";
import { defaultAudioSettings } from "../lib/audioPageSettings";
import { cliInvocation } from "../lib/cli";
import { formatProgressLabel, progressPercentage, type ProgressSnapshot } from "../lib/progressEta";
import type { Progress } from "../lib/coreClient";
import {
  applyBulkSelection,
  audioBaseName,
  audioTagReadError,
  audioWarningDetail,
  id3v1TruncationNotes,
  clearWarningSelection,
  audioRowSection,
  audioSegmentCounts,
  buildAudioListRows,
  buildFieldDiffs,
  changedFieldCount,
  fieldDiffKey,
  filterAudioFiles,
  folderSelectionState,
  planSelectedFields,
  selectableFolderFiles,
  sortAudioFilesWithinFolders,
  replaceAudioFile,
  mergeAudioFiles,
  audioPathKey,
  selectionTotals,
  stepAudioRow,
  toggleFolderCollapsed,
  audioApplySelection,
  audioFileBucket,
  audioPreviewReady,
  fieldVisibleWhenHidingUnchanged,
  fieldPreviewText,
  fieldOriginLabel,
  fileHasTextLoss,
  filenameChanged,
  isMissingAudioWarning,
  pruneMissingAudioFiles,
  dropAudioFiles,
  createAudioPreviewController,
  createTagRule,
  cleanupApeRule,
  tagRuleSummary,
  toTagOperations,
  fromTagOperations,
  tagRuleFields,
  containerName,
  detailContainerNotices,
  type AudioListFilter,
  type AudioListRow,
  type AudioSortKey,
  type DiffPiece,
  type TagRule,
} from "../lib/audioReview";
import VirtualList from "../components/VirtualList.vue";

defineOptions({ name: "AudioPage" });

const paths = ref<string[]>([]);
const files = ref<AudioTagFile[]>([]);
const plan = ref<AudioTagPlan>();
const busy = ref(false);
/** 寫入進行中。選取按鈕只在這段停用，避免取消 audio.apply。 */
const applying = ref(false);
const promptAfterConversion = ref(true);
const backup = ref(true);
const conflictPolicy = ref<AudioTagPlanRequest["conflictPolicy"]>("skip");
const progress = ref<ProgressSnapshot>();
const progressStartedAt = ref<number>();
const activeRequestId = ref<string>();
const listFilter = ref<AudioListFilter>("changed");
const excludedKeys = ref<Set<string>>(new Set());
const settingsReady = ref(false);
const filenameEncodings = [
  "auto",
  "utf8",
  "utf8-bom",
  "utf16le",
  "utf16be",
  "big5",
  "gbk",
  "shift-jis",
] as const;
const sortKey = ref<AudioSortKey>("name");
const listPaneSize = ref<string | number>("36%");
const detailPaneSize = ref<string | number>("64%");
const query = ref("");
const operationError = ref("");
const hideUnchanged = ref(true);
const selectedPath = ref("");
const rescanningPath = ref("");
const collapsedFolders = ref<Set<string>>(new Set());
const focusKind = ref<"file" | "folder">("file");
const focusFolder = ref("");
let streamToken = 0;
const listRef = ref<{ scrollToIndex: (index: number) => void; focusList: () => void } | null>(null);
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
  convertFilename: false,
  filenameInputEncoding: "auto" as TextEncoding,
  filenameOutputEncoding: "utf8" as TextEncoding,
  tagOperations: [] as TagRule[],
});

function applySettings(settings: SettingsV2) {
  const audio = settings.audio ?? defaultAudioSettings();
  options.direction = audio.direction ?? settings.direction;
  options.engine = audio.engine ?? settings.engine;
  options.vocabularyCorrection = audio.vocabularyCorrection ?? settings.vocabularyCorrection;
  promptAfterConversion.value = settings.promptAfterConversion;
  backup.value = audio.backup;
  conflictPolicy.value = audio.conflictPolicy;
  options.recursive = audio.recursive;
  options.id3v1Enabled = audio.id3v1Enabled;
  options.id3v1Direction = audio.id3v1Direction;
  options.id3v1SourceEncoding = audio.id3v1SourceEncoding;
  options.id3v1OutputEncoding = audio.id3v1OutputEncoding;
  options.id3v2Enabled = audio.id3v2Enabled;
  options.id3v2Direction = audio.id3v2Direction;
  options.id3v2SourceEncoding = audio.id3v2SourceEncoding;
  options.id3v2RepairSourceEncoding = audio.id3v2RepairSourceEncoding;
  options.id3v2Version = audio.id3v2Version;
  options.id3v2Encoding = audio.id3v2Encoding;
  options.tagOperations = fromTagOperations(audio.tagOperations ?? []);
  options.convertFilename = audio.convertFilename ?? false;
  options.filenameInputEncoding = audio.filenameInputEncoding ?? "auto";
  options.filenameOutputEncoding = audio.filenameOutputEncoding ?? "utf8";
}

loadSettings().then((settings) => {
  applySettings(settings);
  settingsReady.value = true;
});

const containers = computed(() => ({
  id3v1: options.id3v1Enabled,
  id3v2: options.id3v2Enabled,
}));
const totals = computed(() => selectionTotals(files.value, containers.value));
const segments = computed(() => audioSegmentCounts(files.value));
const filteredFiles = computed(() => filterAudioFiles(files.value, listFilter.value, query.value));
const sortedFiles = computed(() => sortAudioFilesWithinFolders(filteredFiles.value, sortKey.value));
const listRows = computed(() => buildAudioListRows(sortedFiles.value, collapsedFolders.value));
const selectedFile = computed(() => files.value.find((file) => file.path === selectedPath.value));
const fieldDiffs = computed(() => buildFieldDiffs(plan.value ? selectedFile.value : undefined));
const pageSubtitle = computed(() => {
  if (!files.value.length) return t("audio.subtitle.empty");
  if (focusKind.value === "folder" && focusFolder.value) {
    const folder = focusFolder.value.replaceAll("\\", "/");
    const folderKey = audioPathKey(focusFolder.value);
    const prefix = `${folderKey}/`;
    const count = files.value.filter((file) => {
      const path = audioPathKey(file.path);
      return path === folderKey || path.startsWith(prefix);
    }).length;
    return t("audio.subtitle.folder", { folder, files: tn("audio.count.files", count) });
  }
  return t("audio.subtitle.all", {
    paths: tn("audio.count.paths", paths.value.length),
    files: tn("audio.count.files", files.value.length),
  });
});
const visibleFields = computed(() => {
  const file = selectedFile.value;
  if (!file) return [];
  if (!hideUnchanged.value || !plan.value) return file.fields;
  return file.fields.filter((field) => fieldVisibleWhenHidingUnchanged(field, file));
});
const rewriteNotes = computed(() => {
  const file = selectedFile.value;
  if (!file || !plan.value || file.warning) return [];
  return detailContainerNotices(file);
});
const detailRows = computed(() => {
  const file = selectedFile.value;
  if (!file || file.warning) return [];
  const removals = (file.removals ?? []).map((removal) => ({ kind: "removal" as const, removal }));
  return [...removals, ...visibleFields.value.map((field) => ({ kind: "field" as const, field }))];
});
const lossyFileCount = computed(() => files.value.filter((file) => fileHasTextLoss(file)).length);
const editingRuleId = ref("");
const editingRule = computed(() =>
  options.tagOperations.find((rule) => rule.id === editingRuleId.value),
);

watch(filteredFiles, (list) => {
  if (!list.some((file) => file.path === selectedPath.value)) {
    selectedPath.value = list[0]?.path ?? "";
    focusKind.value = "file";
  }
});

function sectionOf(index: number): number | null {
  return audioRowSection(listRows.value, index);
}

function folderAt(index: number): Extract<AudioListRow, { type: "folder" }> | undefined {
  const row = listRows.value[index];
  return row?.type === "folder" ? row : undefined;
}

function folderPath(index: number): string {
  return folderAt(index)?.path ?? "";
}

function focusIndex(): number {
  if (focusKind.value === "folder") {
    return listRows.value.findIndex(
      (row) => row.type === "folder" && row.path === focusFolder.value,
    );
  }
  return listRows.value.findIndex(
    (row) => row.type === "file" && row.file.path === selectedPath.value,
  );
}

watch(
  () =>
    [
      focusKind.value,
      focusFolder.value,
      selectedPath.value,
      sortKey.value,
      listFilter.value,
      query.value,
    ] as const,
  () => {
    // post flush 時清單已掛上。再等一個 nextTick 會落在測試的下一個動作之後，把手動捲動蓋掉。
    const index = focusIndex();
    if (index >= 0) listRef.value?.scrollToIndex(index);
  },
  { flush: "post" },
);

function focusRow(index: number) {
  const row = listRows.value[index];
  if (!row) return;
  if (row.type === "folder") {
    focusKind.value = "folder";
    focusFolder.value = row.path;
  } else {
    focusKind.value = "file";
    selectedPath.value = row.file.path;
  }
  listRef.value?.scrollToIndex(index);
}

function toggleFolder(path: string) {
  focusKind.value = "folder";
  focusFolder.value = path;
  collapsedFolders.value = toggleFolderCollapsed(collapsedFolders.value, path);
}

function resetReviewView() {
  query.value = "";
  collapsedFolders.value = new Set();
  selectedPath.value = "";
  focusKind.value = "file";
  focusFolder.value = "";
}

function pieceClass(piece: DiffPiece): string {
  if (piece.kind === "remove") return "diff-del";
  if (piece.kind === "add") return "diff-ins";
  return "";
}

function containerLabel(container: AudioTagField["container"]): string {
  return containerName(container);
}

function addTagRule() {
  const rule = createTagRule({ action: "merge" });
  options.tagOperations.push(rule);
  editingRuleId.value = rule.id;
}

function addCleanupRule() {
  const rule = cleanupApeRule();
  options.tagOperations.push(rule);
  editingRuleId.value = rule.id;
}

function moveTagRule(index: number, delta: number) {
  const next = index + delta;
  if (next < 0 || next >= options.tagOperations.length) return;
  const [rule] = options.tagOperations.splice(index, 1);
  options.tagOperations.splice(next, 0, rule);
}

function deleteTagRule(index: number) {
  const [removed] = options.tagOperations.splice(index, 1);
  if (editingRuleId.value === removed?.id) {
    editingRuleId.value = options.tagOperations[0]?.id ?? "";
  }
}

function toggleRuleField(rule: TagRule, key: string, checked: boolean) {
  if (checked) {
    if (!rule.fields.includes(key)) rule.fields.push(key);
  } else {
    rule.fields = rule.fields.filter((item) => item !== key);
  }
}

function appendPaths(incoming: readonly string[]): string[] {
  const seen = new Set(paths.value.map((path) => audioPathKey(path)));
  const added: string[] = [];
  for (const path of incoming) {
    const key = audioPathKey(path);
    if (!path || seen.has(key)) continue;
    seen.add(key);
    added.push(path);
  }
  if (added.length) {
    paths.value = [...paths.value, ...added];
    const next = new Set(excludedKeys.value);
    for (const path of added) next.delete(audioPathKey(path));
    excludedKeys.value = next;
  }
  return added;
}

async function addPaths(incoming: readonly string[]) {
  if (applying.value) return;
  const inFlight = scanInFlight;
  const added = appendPaths(incoming);
  if (!added.length) return;
  await scan(inFlight ? undefined : added);
}

async function choose() {
  if (applying.value) return;
  const selected = await open({
    multiple: true,
    filters: [
      { name: t("audio.fileDialogFilter"), extensions: ["mp3", "ape", "ogg", "oga", "opus"] },
    ],
  });
  if (!selected || applying.value) return;
  await addPaths(Array.isArray(selected) ? selected : [selected]);
}

async function chooseFolder() {
  if (applying.value) return;
  const selected = await open({ directory: true, multiple: false });
  if (!selected || applying.value) return;
  await addPaths(Array.isArray(selected) ? selected : [selected]);
}

function selectionSnapshot(list: readonly AudioTagFile[]) {
  return new Map(
    list.map((file) => {
      const labels = file.fields.map((field) => `${field.container}:${field.label}`);
      const uniqueLabels = new Set(
        labels.filter((label) => labels.indexOf(label) === labels.lastIndexOf(label)),
      );
      return [
        audioPathKey(file.path),
        {
          selected: file.selected,
          fields: new Map(
            file.fields.map((field) => [fieldDiffKey(field), field.selected] as const),
          ),
          byLabel: new Map(
            file.fields.flatMap((field) => {
              const label = `${field.container}:${field.label}`;
              return uniqueLabels.has(label) ? [[label, field.selected] as const] : [];
            }),
          ),
        },
      ] as const;
    }),
  );
}

function restoreSelection(list: AudioTagFile[], snapshot: ReturnType<typeof selectionSnapshot>) {
  for (const file of list) {
    const previous = snapshot.get(audioPathKey(file.path));
    if (!previous) continue;
    file.selected = previous.selected;
    for (const field of file.fields) {
      const byKey = previous.fields.get(fieldDiffKey(field));
      const selected = byKey ?? previous.byLabel.get(`${field.container}:${field.label}`);
      if (selected !== undefined) field.selected = selected;
    }
  }
}

function beginStream(): number {
  streamToken += 1;
  return streamToken;
}

function applyStreamProgress(token: number, value: Progress) {
  if (token !== streamToken) return;
  progress.value = value;
  if (!value.batch?.files.length) return;
  files.value = pruneMissingAudioFiles(mergeAudioFiles(files.value, value.batch.files)).filter(
    (file) => !excludedKeys.value.has(audioPathKey(file.path)),
  );
  clearWarningSelection(files.value);
}

watch(selectedPath, (path) => {
  if (!path || !busy.value) return;
  void Promise.resolve(core.request("audio.prioritize", { path })).catch(() => undefined);
});

async function cancelInFlight() {
  const requestId = activeRequestId.value;
  const planId = plan.value?.planId;
  activeRequestId.value = undefined;
  if (requestId) {
    try {
      await core.cancel(requestId);
    } catch {
      // 仍會忽略過期回應。
    }
  }
  if (planId) {
    try {
      await core.request("audio.cancel", { planId });
    } catch {
      // 計畫可能已經結束。
    }
  }
}

function beginAudioWork() {
  busy.value = true;
  operationError.value = "";
  progress.value = undefined;
  progressStartedAt.value = Date.now();
}

function settleAudioWork() {
  busy.value = false;
  activeRequestId.value = undefined;
  progress.value = undefined;
  progressStartedAt.value = undefined;
}

let scanEpoch = 0;
let scanInFlight = false;
let pendingFullSnapshot:
  | {
      epoch: number;
      snapshot: ReturnType<typeof selectionSnapshot>;
      files: AudioTagFile[];
    }
  | undefined;

async function scan(onlyPaths?: readonly string[], extras?: { preview?: boolean }) {
  if (applying.value) return;
  const previewAfter = extras?.preview !== false;
  const subset = Array.isArray(onlyPaths) ? onlyPaths : undefined;
  const merging = subset != null;
  const epoch = ++scanEpoch;
  scanInFlight = true;
  previewController.invalidate();
  previewController.claimJob();
  const token = beginStream();
  await cancelInFlight();
  if (epoch !== scanEpoch || applying.value) {
    if (epoch === scanEpoch) scanInFlight = false;
    return;
  }
  beginAudioWork();
  if (!merging) {
    // 重疊的完整掃描沿用尚未被作廢的那份勾選。世代對不上就重拍，避免還原更早的選擇。
    if (pendingFullSnapshot?.epoch === epoch - 1 || pendingFullSnapshot?.epoch === epoch) {
      pendingFullSnapshot = {
        epoch,
        snapshot: pendingFullSnapshot.snapshot,
        files: pendingFullSnapshot.files,
      };
    } else {
      pendingFullSnapshot = {
        epoch,
        snapshot: selectionSnapshot(files.value),
        files: files.value.map((file) => file),
      };
    }
    resetReviewView();
    plan.value = undefined;
    files.value = [];
  }
  const requestPaths = subset ?? paths.value;
  let completed = false;
  try {
    const scanned = await core.request<AudioTagFile[]>(
      "audio.scan",
      {
        paths: [...requestPaths],
        recursive: options.recursive,
        id3v1SourceEncoding: options.id3v1SourceEncoding,
        id3v2SourceEncoding: options.id3v2SourceEncoding,
        id3v2RepairSourceEncoding: options.id3v2RepairSourceEncoding,
      },
      {
        onProgress: (value) => {
          applyStreamProgress(token, value);
        },
        onRequestId: (id) => {
          if (token === streamToken) activeRequestId.value = id;
        },
      },
    );
    if (token !== streamToken) return;
    const ownedSnapshot =
      !merging && pendingFullSnapshot?.epoch === epoch ? pendingFullSnapshot.snapshot : undefined;
    const snapshot = merging
      ? selectionSnapshot(files.value)
      : (ownedSnapshot ?? selectionSnapshot(files.value));
    const incoming = scanned.filter((file) => !excludedKeys.value.has(audioPathKey(file.path)));
    files.value = pruneMissingAudioFiles(mergeAudioFiles(files.value, incoming));
    restoreSelection(files.value, snapshot);
    clearWarningSelection(files.value);
    completed = true;
  } catch (error) {
    if (token !== streamToken) return;
    if (!merging && pendingFullSnapshot?.epoch === epoch) {
      if (!files.value.length && pendingFullSnapshot.files.length) {
        files.value = pendingFullSnapshot.files;
      }
      restoreSelection(files.value, pendingFullSnapshot.snapshot);
    }
    if (isCancellationError(error)) ElMessage.info(t("audio.messages.scanCancelled"));
    else {
      operationError.value = error instanceof Error ? error.message : String(error);
      ElMessage.error(operationError.value);
    }
    return;
  } finally {
    if (!merging && pendingFullSnapshot?.epoch === epoch && token !== streamToken) {
      pendingFullSnapshot = undefined;
    }
    if (token === streamToken) {
      if (!merging) pendingFullSnapshot = undefined;
      settleAudioWork();
    }
    if (epoch === scanEpoch) scanInFlight = false;
  }
  if (completed && token === streamToken && files.value.length && !applying.value) {
    if (previewAfter) await previewController.run();
  }
}

async function buildPlanRequest(): Promise<AudioTagPlanRequest> {
  const settings = await loadSettings();
  const selectedFields = planSelectedFields(files.value);
  const selectedPaths = files.value
    .filter((file) => file.selected && !file.warning)
    .map((file) => file.path);
  return {
    paths: paths.value,
    recursive: options.recursive,
    selectedPaths,
    selectedFields,
    conversion: {
      direction: options.direction,
      engine: options.engine,
      vocabularyCorrection: options.vocabularyCorrection,
      zhconvert: zhConvertOptions(settings, options.direction),
    },
    conflictPolicy: conflictPolicy.value,
    backup: backup.value,
    preserveModifiedTime: settings.preserveModifiedTime,
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
    tagOperations: toTagOperations(options.tagOperations),
    convertFilename: options.convertFilename,
    filenameInputEncoding: options.filenameInputEncoding,
    filenameOutputEncoding: options.filenameOutputEncoding,
  };
}

function adoptPlan(value: unknown) {
  const next = value as AudioTagPlan;
  const keep = new Set(next.files.map((file) => audioPathKey(file.path)));
  const snapshot = selectionSnapshot(files.value);
  files.value = mergeAudioFiles(
    files.value.filter((file) => keep.has(audioPathKey(file.path))),
    next.files,
  );
  restoreSelection(files.value, snapshot);
  clearWarningSelection(files.value);
  plan.value = { ...next, files: files.value };
}

function reportPreviewError(error: unknown) {
  operationError.value = error instanceof Error ? error.message : String(error);
  ElMessage.error(operationError.value);
}

const previewController = createAudioPreviewController({
  plan: async () => {
    if (!files.value.length) return undefined;
    plan.value = undefined;
    const token = beginStream();
    const result = await core.request<AudioTagPlan>("audio.plan", await buildPlanRequest(), {
      onProgress: (value) => {
        applyStreamProgress(token, value);
      },
      onRequestId: (id) => {
        if (token === streamToken) activeRequestId.value = id;
      },
    });
    if (token !== streamToken) return undefined;
    return result;
  },
  cancelInFlight,
  apply: adoptPlan,
  onError: reportPreviewError,
  isCancellation: isCancellationError,
  onStart: beginAudioWork,
  onSettled: settleAudioWork,
});

async function createPlan() {
  await previewController.run();
}

watch(
  [options, backup, conflictPolicy, settingsReady],
  () => {
    if (!settingsReady.value || applying.value) return;
    previewController.schedule();
    void persistAudioSettings().catch((error: unknown) => {
      ElMessage.error(error instanceof Error ? error.message : String(error));
    });
  },
  { deep: true },
);

async function persistAudioSettings() {
  await patchSavedSettings((settings) => {
    settings.audio = {
      backup: backup.value,
      conflictPolicy: conflictPolicy.value,
      recursive: options.recursive,
      direction: options.direction,
      engine: options.engine,
      vocabularyCorrection: options.vocabularyCorrection,
      id3v1Enabled: options.id3v1Enabled,
      id3v1Direction: options.id3v1Direction,
      id3v1SourceEncoding: options.id3v1SourceEncoding,
      id3v1OutputEncoding: options.id3v1OutputEncoding,
      id3v2Enabled: options.id3v2Enabled,
      id3v2Direction: options.id3v2Direction,
      id3v2SourceEncoding: options.id3v2SourceEncoding,
      id3v2RepairSourceEncoding: options.id3v2RepairSourceEncoding,
      id3v2Version: options.id3v2Version,
      id3v2Encoding: options.id3v2Encoding,
      convertFilename: options.convertFilename,
      filenameInputEncoding: options.filenameInputEncoding,
      filenameOutputEncoding: options.filenameOutputEncoding,
      tagOperations: toTagOperations(options.tagOperations),
    };
  });
}

async function applyPlan() {
  await previewController.flush();
  if (!plan.value || busy.value || totals.value.files === 0) return;
  const planId = plan.value.planId;
  if (
    !(await confirm(tn("audio.messages.confirmApply", totals.value.files), {
      title: t("audio.messages.confirmApplyTitle"),
      okLabel: t("common.ok"),
      cancelLabel: t("common.cancel"),
      kind: "warning",
    }))
  )
    return;
  if (busy.value || plan.value?.planId !== planId) return;
  const overwritten = [
    backup.value ? t("audio.messages.existingBackup") : "",
    options.convertFilename ? t("audio.messages.renamedConflict") : "",
  ].filter(Boolean);
  if (
    conflictPolicy.value === "overwrite" &&
    overwritten.length &&
    !(await confirm(
      t("audio.messages.confirmOverwrite", {
        targets: overwritten.join(t("audio.messages.overwriteJoiner")),
      }),
      {
        title: t("audio.messages.confirmOverwriteTitle"),
        okLabel: t("common.ok"),
        cancelLabel: t("common.cancel"),
        kind: "warning",
      },
    ))
  )
    return;
  if (busy.value || plan.value?.planId !== planId) return;
  previewController.invalidate();
  previewController.setApplying(true);
  applying.value = true;
  busy.value = true;
  progress.value = undefined;
  progressStartedAt.value = Date.now();
  let rescanPaths: string[] | undefined;
  let dropPlan = false;
  try {
    const result = await core.request<ApplyResult>(
      "audio.apply",
      {
        planId: plan.value.planId,
        ...audioApplySelection(files.value, containers.value),
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
    const skipped = result.skipped?.length ?? 0;
    const warnings = result.warnings ?? [];
    if (warnings.length)
      ElMessage.warning(
        warnings.length > 1
          ? t("audio.messages.moreWarnings", { first: warnings[0]!, count: warnings.length - 1 })
          : warnings[0]!,
      );
    if (result.failed.length) {
      const detail = result.failed[0]?.message ?? "";
      ElMessage.warning(
        detail
          ? tn("audio.messages.failedWithDetail", result.failed.length, { detail })
          : tn("audio.messages.failed", result.failed.length),
      );
    }
    if (result.succeeded.length) {
      if (promptAfterConversion.value)
        ElMessage.success(tn("audio.messages.completed", result.succeeded.length));
    } else if (!result.failed.length) {
      ElMessage.warning(
        skipped > 0
          ? t("audio.messages.nothingWrittenSkipped", { count: skipped })
          : t("audio.messages.nothingWritten"),
      );
    }
    if (result.succeeded.length && !result.failed.length) {
      dropPlan = true;
      if (paths.value.length) rescanPaths = undefined;
    } else if (result.failed.length && result.succeeded.length) {
      rescanPaths = result.succeeded;
    }
  } catch (error) {
    if (isCancellationError(error)) ElMessage.info(t("audio.messages.applyCancelled"));
    else ElMessage.error(error instanceof Error ? error.message : String(error));
    dropPlan = !isCancellationError(error);
    if (dropPlan && paths.value.length) rescanPaths = undefined;
  } finally {
    previewController.setApplying(false);
    applying.value = false;
    activeRequestId.value = undefined;
    progress.value = undefined;
    progressStartedAt.value = undefined;
  }
  try {
    if (dropPlan) plan.value = undefined;
    if (dropPlan && paths.value.length) await scan(undefined, { preview: false });
    else if (rescanPaths?.length) {
      await scan(rescanPaths, { preview: false });
      if (plan.value) plan.value = { ...plan.value, files: files.value };
    }
  } finally {
    busy.value = false;
  }
}

async function cancelPlan() {
  previewController.invalidate();
  await cancelInFlight();
}

function adoptRescannedFile(updated: AudioTagFile) {
  const next = replaceAudioFile(files.value, updated);
  files.value = next;
  if (plan.value) plan.value = { ...plan.value, files: next };
}

async function rescanFile(file: AudioTagFile) {
  if (rescanningPath.value || busy.value) return;
  rescanningPath.value = file.path;
  try {
    let updated: AudioTagFile;
    if (plan.value) {
      const request: AudioRescanFileRequest = {
        ...(await buildPlanRequest()),
        path: file.path,
        planId: plan.value.planId,
        paths: [file.path],
        recursive: false,
        selectedPaths: [file.path],
      };
      updated = await core.request<AudioTagFile>("audio.rescanFile", request);
    } else {
      const scanned = await core.request<AudioTagFile[]>("audio.scan", {
        paths: [file.path],
        recursive: false,
        id3v1SourceEncoding: options.id3v1SourceEncoding,
        id3v2SourceEncoding: options.id3v2SourceEncoding,
        id3v2RepairSourceEncoding: options.id3v2RepairSourceEncoding,
      });
      updated = scanned.find((item) => item.path === file.path) ?? scanned[0];
    }
    if (!updated) return;
    updated = { ...updated, path: file.path, selected: updated.warning ? false : updated.selected };
    if (updated.missing || isMissingAudioWarning(updated.warning)) {
      removeFileFromList(file);
      ElMessage.info(t("audio.messages.fileGone", { name: audioBaseName(file.path) }));
      return;
    }
    adoptRescannedFile(updated);
    selectedPath.value = file.path;
    focusKind.value = "file";
    if (updated.warning) ElMessage.warning(updated.warning);
    else ElMessage.success(t("audio.messages.rescanned", { name: audioBaseName(file.path) }));
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    rescanningPath.value = "";
  }
}

function setFileSelected(file: AudioTagFile, selected: boolean) {
  if (file.warning || busy.value) return;
  file.selected = selected;
}

function removeFileFromList(file: AudioTagFile) {
  if (applying.value) return;
  const key = audioPathKey(file.path);
  const next = new Set(excludedKeys.value);
  next.add(key);
  excludedKeys.value = next;
  files.value = dropAudioFiles(files.value, new Set([key]));
  paths.value = paths.value.filter((path) => audioPathKey(path) !== key);
  if (plan.value) plan.value = { ...plan.value, files: files.value };
  if (selectedPath.value === file.path) selectedPath.value = files.value[0]?.path ?? "";
}

function returnListFocus() {
  void nextTick(() => listRef.value?.focusList());
}

function onRowChecked(row: AudioListRow | undefined, selected: boolean) {
  if (row?.type !== "file") return;
  setFileSelected(row.file, selected);
  returnListFocus();
}

function onFolderChecked(path: string, selected: boolean) {
  const group = selectableFolderFiles(filteredFiles.value, path);
  if (!group.length || busy.value) return;
  const mode = selected ? "all" : "none";
  applyBulkSelection(files.value, group, mode);
  returnListFocus();
}

function bindFolderCheck(element: Element | null, path: string) {
  if (!(element instanceof HTMLInputElement)) return;
  const state = folderSelectionState(filteredFiles.value, path);
  element.indeterminate = state === "mixed";
  element.checked = state === "all";
}

function selectFile(file: AudioTagFile) {
  focusKind.value = "file";
  selectedPath.value = file.path;
}

function moveSelection(delta: number) {
  const next = stepAudioRow(listRows.value.length, focusIndex(), delta);
  if (next < 0) return;
  focusRow(next);
}

function onFolderKey(key: string): boolean {
  if (focusKind.value !== "folder") return false;
  if (key !== "Enter" && key !== " " && key !== "ArrowLeft" && key !== "ArrowRight") return false;
  if (!focusFolder.value) return false;
  toggleFolder(focusFolder.value);
  return true;
}

function rowId(index: number): string {
  return `audio-row-${index}`;
}

const activeRowId = computed(() => {
  const index = focusIndex();
  return index >= 0 ? rowId(index) : undefined;
});

function rowActive(index: number): boolean {
  return activeRowId.value === rowId(index);
}

/** 焦點在按鈕、輸入框或其他控制項時，快捷鍵不改清單。 */
function isInteractiveTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  if (target.closest(".audio-file-pane input[type='checkbox']")) return false;
  return Boolean(target.closest("button, input, textarea, select, a, [contenteditable='true']"));
}

function onReviewKeydown(event: KeyboardEvent) {
  if (isInteractiveTarget(event.target)) return;
  if (event.key === "ArrowDown") {
    event.preventDefault();
    moveSelection(1);
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    moveSelection(-1);
  } else if (onFolderKey(event.key)) {
    event.preventDefault();
  } else if (event.key === " ") {
    const file = selectedFile.value;
    if (focusKind.value !== "file" || !file || file.warning) return;
    event.preventDefault();
    file.selected = !file.selected;
  }
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
</script>

<template>
  <section class="page-stack">
    <header class="page-header">
      <div>
        <p class="eyebrow">AUDIO TAGS</p>
        <h1>{{ t("audio.title") }}</h1>
        <p>{{ pageSubtitle }}</p>
      </div>
      <div class="card-actions">
        <el-button :disabled="applying" @click="chooseFolder">{{
          t("audio.chooseFolder")
        }}</el-button>
        <el-button type="primary" :disabled="applying" @click="choose">{{
          t("audio.chooseFiles")
        }}</el-button>
      </div>
    </header>

    <div class="page-fill-main audio-body">
      <el-card shadow="never" class="audio-settings-card">
        <el-form label-position="top" class="option-grid audio-options" :disabled="busy">
          <el-form-item :label="t('audio.options.apeVorbisDirection')">
            <el-select v-model="options.direction">
              <el-option :label="t('audio.options.noConversion')" value="none" />
              <el-option :label="t('common.direction.s2t')" value="s2t" />
              <el-option :label="t('common.direction.t2s')" value="t2s" />
            </el-select>
          </el-form-item>
          <el-form-item :label="t('audio.options.engine')">
            <el-select v-model="options.engine">
              <el-option :label="t('audio.options.segmented')" value="segmented" />

              <el-option label="ZhConvert" value="zhconvert" />
            </el-select>
          </el-form-item>
          <el-form-item
            :label="
              options.convertFilename
                ? t('audio.options.backupAndFilenameConflict')
                : t('audio.options.backupConflict')
            "
          >
            <el-select v-model="conflictPolicy" :disabled="!backup && !options.convertFilename">
              <el-option :label="t('audio.options.skip')" value="skip" />
              <el-option :label="t('audio.options.overwrite')" value="overwrite" />
            </el-select>
          </el-form-item>
          <el-form-item
            v-if="options.convertFilename"
            :label="t('audio.options.filenameInputEncoding')"
          >
            <el-select v-model="options.filenameInputEncoding">
              <el-option
                v-for="value in filenameEncodings"
                :key="value"
                :value="value"
                :label="value"
              />
            </el-select>
          </el-form-item>
          <el-form-item
            v-if="options.convertFilename"
            :label="t('audio.options.filenameOutputEncoding')"
          >
            <el-select v-model="options.filenameOutputEncoding">
              <el-option
                v-for="value in filenameEncodings"
                :key="value"
                :value="value"
                :label="value"
              />
            </el-select>
          </el-form-item>
          <el-form-item :label="t('audio.options.id3v1Direction')">
            <el-select v-model="options.id3v1Direction" :disabled="!options.id3v1Enabled">
              <el-option :label="t('audio.options.noConversion')" value="none" />
              <el-option :label="t('common.direction.s2t')" value="s2t" />
              <el-option :label="t('common.direction.t2s')" value="t2s" />
            </el-select>
          </el-form-item>
          <el-form-item :label="t('audio.options.id3v1Source')">
            <el-select v-model="options.id3v1SourceEncoding" :disabled="!options.id3v1Enabled">
              <el-option label="GBK" value="gbk" />
              <el-option label="Big5" value="big5" />
              <el-option label="Shift-JIS" value="shift-jis" />
              <el-option label="UTF-8" value="utf8" />
            </el-select>
          </el-form-item>
          <el-form-item :label="t('audio.options.id3v1Output')">
            <el-select v-model="options.id3v1OutputEncoding" :disabled="!options.id3v1Enabled">
              <el-option label="Big5" value="big5" />
              <el-option label="GBK" value="gbk" />
              <el-option label="Shift-JIS" value="shift-jis" />
              <el-option label="UTF-8" value="utf8" />
            </el-select>
          </el-form-item>
          <el-form-item :label="t('audio.options.id3v2Direction')">
            <el-select v-model="options.id3v2Direction" :disabled="!options.id3v2Enabled">
              <el-option :label="t('audio.options.noConversion')" value="none" />
              <el-option :label="t('common.direction.s2t')" value="s2t" />
              <el-option :label="t('common.direction.t2s')" value="t2s" />
            </el-select>
          </el-form-item>
          <el-form-item :label="t('audio.options.id3v2Version')">
            <el-select v-model="options.id3v2Version" :disabled="!options.id3v2Enabled">
              <el-option label="2.4" :value="4" />
              <el-option label="2.3" :value="3" />
            </el-select>
          </el-form-item>
          <el-form-item :label="t('audio.options.id3v2Encoding')">
            <el-select v-model="options.id3v2Encoding" :disabled="!options.id3v2Enabled">
              <el-option label="UTF-8" value="utf8" />
              <el-option label="UTF-16" value="utf16" />
              <el-option label="UTF-16BE" value="utf16be" />
              <el-option label="Latin-1" value="latin1" />
            </el-select>
          </el-form-item>
          <el-form-item :label="t('audio.options.id3v2RepairSource')">
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
        </el-form>
        <div class="switch-row">
          <el-checkbox v-model="options.vocabularyCorrection">{{
            t("audio.options.vocabularyCorrection")
          }}</el-checkbox>
          <el-checkbox v-model="options.recursive">{{ t("audio.options.recursive") }}</el-checkbox>
          <el-checkbox v-model="backup">{{ t("audio.options.backup") }}</el-checkbox>
          <el-checkbox v-model="options.convertFilename">{{
            t("audio.options.convertFilename")
          }}</el-checkbox>
          <el-checkbox v-model="options.id3v1Enabled">ID3v1</el-checkbox>
          <el-checkbox v-model="options.id3v2Enabled">ID3v2</el-checkbox>
          <el-checkbox
            v-model="options.id3v2RepairSourceEncoding"
            :disabled="!options.id3v1Enabled && !options.id3v2Enabled"
            >{{ t("audio.options.repairEncoding") }}</el-checkbox
          >
        </div>
        <el-collapse class="audio-rules-collapse">
          <el-collapse-item :title="t('audio.rules.title')" name="rules">
            <p class="audio-rule-lead">
              {{ t("audio.rules.lead") }}
            </p>
            <div
              v-for="(rule, index) in options.tagOperations"
              :key="rule.id"
              class="audio-rule"
              :class="{ 'is-on': rule.id === editingRuleId, 'is-remove': rule.action === 'remove' }"
            >
              <span class="audio-rule-index">{{ index + 1 }}</span>
              <button type="button" class="audio-rule-name" @click="editingRuleId = rule.id">
                <span v-if="rule.action === 'remove'" class="audio-remove">{{
                  t("audio.rules.remove")
                }}</span>
                {{ tagRuleSummary(rule) }}
              </button>
              <el-switch
                v-model="rule.enabled"
                :aria-label="t('audio.rules.enableRule', { index: index + 1 })"
              />
              <el-button link :disabled="index === 0" @click="moveTagRule(index, -1)">{{
                t("audio.rules.moveUp")
              }}</el-button>
              <el-button
                link
                :disabled="index === options.tagOperations.length - 1"
                @click="moveTagRule(index, 1)"
                >{{ t("audio.rules.moveDown") }}</el-button
              >
              <el-button link type="danger" @click="deleteTagRule(index)">{{
                t("common.delete")
              }}</el-button>
            </div>
            <el-button @click="addTagRule">{{ t("audio.rules.add") }}</el-button>
            <el-button link @click="addCleanupRule">{{ t("audio.rules.cleanup") }}</el-button>
            <p class="audio-rule-lead">{{ t("audio.rules.cleanupApeNote") }}</p>
            <div v-if="editingRule" class="audio-rule-editor">
              <strong>{{
                t("audio.rules.editRule", {
                  index: options.tagOperations.findIndex((rule) => rule.id === editingRule?.id) + 1,
                })
              }}</strong>
              <div class="audio-rule-grid">
                <label
                  >{{ t("audio.rules.action") }}
                  <el-select v-model="editingRule.action">
                    <el-option :label="t('audio.rules.merge')" value="merge" />
                    <el-option :label="t('audio.rules.remove')" value="remove" />
                  </el-select>
                </label>
                <label
                  >{{ t("audio.rules.onConflict") }}
                  <el-select
                    v-model="editingRule.onConflict"
                    :disabled="editingRule.action === 'remove'"
                  >
                    <el-option :label="t('audio.rules.overwriteTarget')" value="overwrite" />
                    <el-option :label="t('audio.rules.fillEmpty')" value="fillEmpty" />
                  </el-select>
                </label>
                <label
                  >{{ t("audio.rules.source") }}
                  <el-select v-model="editingRule.source">
                    <el-option label="ID3v2" value="id3v2" />
                    <el-option label="ID3v1" value="id3v1" />
                    <el-option label="APEv2" value="apev2" />
                  </el-select>
                </label>
                <label
                  >{{ t("audio.rules.target") }}
                  <el-select
                    v-model="editingRule.target"
                    :disabled="editingRule.action === 'remove'"
                  >
                    <el-option label="ID3v1" value="id3v1" />
                    <el-option label="ID3v2" value="id3v2" />
                    <el-option label="APEv2" value="apev2" />
                  </el-select>
                </label>
              </div>
              <p v-if="editingRule.action !== 'remove'">{{ t("audio.rules.fields") }}</p>
              <div v-if="editingRule.action !== 'remove'" class="audio-rule-fields">
                <el-checkbox
                  v-for="[key, label] in tagRuleFields()"
                  :key="key"
                  :model-value="editingRule.fields.includes(key)"
                  @change="
                    (checked: boolean) => editingRule && toggleRuleField(editingRule, key, checked)
                  "
                  >{{ label }}</el-checkbox
                >
              </div>
            </div>
          </el-collapse-item>
        </el-collapse>
      </el-card>

      <div class="audio-toolbar">
        <el-input
          v-model="query"
          clearable
          class="audio-search"
          :placeholder="t('audio.toolbar.search')"
          :disabled="!files.length"
        />
        <el-select
          v-model="sortKey"
          class="audio-sort"
          :disabled="!files.length"
          :aria-label="t('audio.toolbar.sort')"
        >
          <el-option :label="t('audio.toolbar.sortName')" value="name" />
          <el-option :label="t('audio.toolbar.sortChanged')" value="changed" />
          <el-option :label="t('audio.toolbar.sortFormat')" value="format" />
        </el-select>
        <el-radio-group
          v-model="listFilter"
          class="audio-segments"
          :disabled="!files.length"
          :aria-label="t('audio.toolbar.filter')"
        >
          <el-radio-button value="changed" :aria-pressed="listFilter === 'changed'">{{
            t("audio.toolbar.changed", { count: segments.changed })
          }}</el-radio-button>
          <el-radio-button value="unchanged" :aria-pressed="listFilter === 'unchanged'">{{
            t("audio.toolbar.unchanged", { count: segments.unchanged })
          }}</el-radio-button>
          <el-radio-button
            value="warnings"
            :aria-pressed="listFilter === 'warnings'"
            :class="{
              'is-warn': segments.warnings > 0,
              'is-filled': listFilter === 'warnings' && segments.warnings > 0,
            }"
            >{{ t("audio.toolbar.warnings", { count: segments.warnings }) }}</el-radio-button
          >
          <el-radio-button
            value="errors"
            :aria-pressed="listFilter === 'errors'"
            :class="{
              'is-alert': segments.errors > 0,
              'is-filled': listFilter === 'errors' && segments.errors > 0,
            }"
            >{{ t("audio.toolbar.errors", { count: segments.errors }) }}</el-radio-button
          >
          <el-radio-button value="all" :aria-pressed="listFilter === 'all'">{{
            t("audio.toolbar.all", { count: segments.all })
          }}</el-radio-button>
        </el-radio-group>
      </div>

      <div class="page-fill-main audio-review">
        <div v-if="operationError && !files.length" class="audio-empty">
          <el-result icon="error" :title="t('audio.list.scanFailed')" :sub-title="operationError">
            <template #extra>
              <el-button type="primary" :disabled="!paths.length || busy" @click="scan()">{{
                t("audio.list.retry")
              }}</el-button>
            </template>
          </el-result>
        </div>
        <div v-else-if="!files.length" class="audio-empty">
          <el-empty :description="t('audio.list.empty')" />
        </div>
        <template v-else>
          <el-splitter class="audio-review-splitter">
            <el-splitter-panel v-model:size="listPaneSize" :min="280">
              <div class="audio-file-pane">
                <el-result
                  v-if="operationError"
                  class="audio-inline-error"
                  icon="warning"
                  :title="t('audio.list.previewFailed')"
                  :sub-title="operationError"
                />
                <el-empty v-if="!listRows.length" :description="t('audio.list.noMatch')" />
                <VirtualList
                  v-else
                  ref="listRef"
                  role="listbox"
                  :aria-label="t('audio.list.ariaLabel')"
                  tabindex="0"
                  :aria-activedescendant="activeRowId"
                  :count="listRows.length"
                  :item-height="40"
                  :section-of="sectionOf"
                  @keydown="onReviewKeydown"
                >
                  <template #sticky="{ index }">
                    <div
                      v-if="folderAt(index)"
                      class="audio-folder"
                      :aria-expanded="folderAt(index) ? !folderAt(index)!.collapsed : undefined"
                      @click="toggleFolder(folderAt(index)!.path)"
                    >
                      <input
                        type="checkbox"
                        class="audio-folder-check"
                        tabindex="-1"
                        :aria-label="t('audio.list.selectFolder', { path: folderPath(index) })"
                        :disabled="
                          busy || folderSelectionState(filteredFiles, folderPath(index)) === 'empty'
                        "
                        :ref="
                          (element) => bindFolderCheck(element as Element | null, folderPath(index))
                        "
                        @click.stop
                        @change="
                          onFolderChecked(
                            folderPath(index),
                            ($event.target as HTMLInputElement).checked,
                          )
                        "
                      />
                      <span class="audio-caret">{{ folderAt(index)!.collapsed ? "▸" : "▾" }}</span>
                      <span class="audio-folder-path">{{ folderAt(index)!.path }}</span>
                      <span class="audio-folder-counts">
                        {{
                          t("audio.list.folderCounts", {
                            files: folderAt(index)!.counts.files,
                            changed: folderAt(index)!.counts.changed,
                            errors: folderAt(index)!.counts.errors,
                          })
                        }}
                      </span>
                    </div>
                  </template>
                  <template #default="{ index }">
                    <div
                      v-if="listRows[index]?.type === 'folder'"
                      :id="rowId(index)"
                      class="audio-folder"
                      role="option"
                      :aria-selected="rowActive(index)"
                      :aria-expanded="!listRows[index].collapsed"
                      :class="{ active: rowActive(index) }"
                      @click="toggleFolder(listRows[index].path)"
                    >
                      <input
                        type="checkbox"
                        class="audio-folder-check"
                        tabindex="-1"
                        :aria-label="t('audio.list.selectFolder', { path: folderPath(index) })"
                        :disabled="
                          busy || folderSelectionState(filteredFiles, folderPath(index)) === 'empty'
                        "
                        :ref="
                          (element) => bindFolderCheck(element as Element | null, folderPath(index))
                        "
                        @click.stop
                        @change="
                          onFolderChecked(
                            folderPath(index),
                            ($event.target as HTMLInputElement).checked,
                          )
                        "
                      />
                      <span class="audio-caret">{{ listRows[index].collapsed ? "▸" : "▾" }}</span>
                      <span class="audio-folder-path">{{ listRows[index].path }}</span>
                      <span class="audio-folder-counts">
                        {{
                          t("audio.list.folderCounts", {
                            files: listRows[index].counts.files,
                            changed: listRows[index].counts.changed,
                            errors: listRows[index].counts.errors,
                          })
                        }}
                      </span>
                    </div>
                    <div
                      v-else-if="listRows[index]?.type === 'file'"
                      :id="rowId(index)"
                      class="audio-row"
                      role="option"
                      :aria-selected="rowActive(index)"
                      :class="{
                        active: rowActive(index),
                        warn: Boolean(listRows[index].file.warning),
                      }"
                      @click="selectFile(listRows[index].file)"
                    >
                      <el-checkbox
                        :model-value="
                          !listRows[index].file.warning && Boolean(listRows[index].file.selected)
                        "
                        :disabled="Boolean(listRows[index].file.warning) || busy"
                        tabindex="-1"
                        @change="(checked: boolean) => onRowChecked(listRows[index], checked)"
                        @click.stop
                      />
                      <span class="audio-name">{{ audioBaseName(listRows[index].file.path) }}</span>
                      <span class="audio-format">{{
                        listRows[index].file.format.toUpperCase()
                      }}</span>
                      <span
                        v-if="listRows[index].file.warning"
                        class="audio-bang"
                        :title="t('audio.list.fileHasError')"
                        >!</span
                      >
                      <span
                        v-else
                        class="audio-badge"
                        :class="{ zero: changedFieldCount(listRows[index].file) === 0 }"
                        >{{ changedFieldCount(listRows[index].file) }}</span
                      >
                      <button
                        type="button"
                        class="audio-remove-file"
                        :disabled="busy"
                        :aria-label="
                          t('audio.list.removeFromList', {
                            name: audioBaseName(listRows[index].file.path),
                          })
                        "
                        @click.stop="removeFileFromList(listRows[index].file)"
                      >
                        ×
                      </button>
                    </div>
                  </template>
                </VirtualList>
              </div>
            </el-splitter-panel>
            <el-splitter-panel v-model:size="detailPaneSize" :min="260">
              <div class="audio-detail">
                <template v-if="selectedFile">
                  <div class="audio-detail-head">
                    <div>
                      <strong>{{ audioBaseName(selectedFile.path) }}</strong>
                      <p>
                        {{ selectedFile.path }} · {{ selectedFile.format.toUpperCase() }} ·
                        {{
                          selectedFile.hasCoverArt
                            ? t("audio.detail.hasCover")
                            : t("audio.detail.noCover")
                        }}
                      </p>
                      <p v-if="filenameChanged(selectedFile)" class="audio-filename-preview">
                        {{
                          t("audio.detail.filenamePreview", {
                            name: audioBaseName(selectedFile.convertedPath ?? selectedFile.path),
                          })
                        }}
                      </p>
                    </div>
                    <el-tooltip placement="bottom-end">
                      <template #content>
                        <div class="audio-shortcut-tip">
                          <div>{{ t("audio.detail.shortcutSwitch") }}</div>
                          <div>{{ t("audio.detail.shortcutFolder") }}</div>
                          <div>{{ t("audio.detail.shortcutCheck") }}</div>
                        </div>
                      </template>
                      <el-button
                        class="audio-help"
                        circle
                        size="small"
                        :aria-label="t('audio.detail.shortcutHelp')"
                        >?</el-button
                      >
                    </el-tooltip>
                  </div>
                  <template v-if="selectedFile.warning">
                    <el-alert type="error" :title="audioTagReadError()" show-icon :closable="false">
                      <p v-if="audioWarningDetail(selectedFile.warning)" class="audio-error-reason">
                        {{ audioWarningDetail(selectedFile.warning) }}
                      </p>
                      <p class="audio-error-path">{{ selectedFile.path }}</p>
                      <p>{{ t("audio.detail.errorHint") }}</p>
                    </el-alert>
                    <el-button
                      class="audio-rescan"
                      :loading="rescanningPath === selectedFile.path"
                      :disabled="busy"
                      @click="rescanFile(selectedFile)"
                      >{{ t("audio.detail.rescanFile") }}</el-button
                    >
                  </template>
                  <div v-else class="audio-detail-tools">
                    <el-alert
                      v-for="note in id3v1TruncationNotes(selectedFile)"
                      :key="note"
                      class="audio-id3v1-truncation"
                      type="warning"
                      :title="note"
                      show-icon
                      :closable="false"
                    />
                    <el-alert
                      v-for="note in rewriteNotes"
                      :key="note"
                      class="audio-container-rewrite"
                      type="info"
                      :title="note"
                      show-icon
                      :closable="false"
                    />
                    <el-tag type="success" effect="plain">{{
                      tn("audio.detail.changes", changedFieldCount(selectedFile))
                    }}</el-tag>
                    <button
                      type="button"
                      class="audio-hide-unchanged"
                      :aria-pressed="hideUnchanged"
                      @click="hideUnchanged = !hideUnchanged"
                    >
                      {{ t("audio.detail.hideUnchanged") }}
                    </button>
                  </div>
                  <el-table
                    v-if="!selectedFile.warning"
                    :key="hideUnchanged ? 'hide-unchanged' : 'show-all'"
                    :data="detailRows"
                    :empty-text="t('audio.detail.noFields')"
                  >
                    <el-table-column :label="t('audio.detail.columnConvert')" width="70">
                      <template #default="scope">
                        <el-checkbox
                          v-if="scope.row.kind === 'removal'"
                          v-model="scope.row.removal.selected"
                          :disabled="busy || Boolean(scope.row.removal.refused)"
                        />
                        <el-checkbox v-else v-model="scope.row.field.selected" :disabled="busy" />
                      </template>
                    </el-table-column>
                    <el-table-column :label="t('audio.detail.columnTag')" width="110">
                      <template #default="scope">{{
                        containerLabel(
                          scope.row.kind === "removal"
                            ? scope.row.removal.container
                            : scope.row.field.container,
                        )
                      }}</template>
                    </el-table-column>
                    <el-table-column :label="t('audio.detail.columnField')" width="120">
                      <template #default="scope">{{
                        scope.row.kind === "removal"
                          ? t("audio.detail.wholeTag")
                          : scope.row.field.label
                      }}</template>
                    </el-table-column>
                    <el-table-column :label="t('audio.detail.columnCurrent')" min-width="160">
                      <template #default="scope">
                        <template v-if="scope.row.kind === 'removal'">
                          {{ tn("audio.detail.hiddenFields", scope.row.removal.hiddenFieldCount) }}
                          <span v-if="scope.row.removal.refused"
                            >・{{ scope.row.removal.refused }}</span
                          >
                        </template>
                        <template v-else-if="plan && fieldDiffs.get(fieldDiffKey(scope.row.field))">
                          <span
                            v-for="(piece, pieceIndex) in fieldDiffs.get(
                              fieldDiffKey(scope.row.field),
                            )!.before"
                            :key="`old-${scope.row.field.container}-${scope.row.field.key}-${pieceIndex}`"
                            :class="pieceClass(piece)"
                            >{{ piece.text }}</span
                          >
                        </template>
                        <template v-else>{{ scope.row.field.values.join(" / ") }}</template>
                      </template>
                    </el-table-column>
                    <el-table-column
                      v-if="plan"
                      :label="t('audio.detail.columnPreview')"
                      min-width="160"
                    >
                      <template #default="scope">
                        <template v-if="scope.row.kind === 'removal'">
                          <span class="audio-remove">{{ t("audio.detail.removeTag") }}</span>
                        </template>
                        <template v-else>
                          <template v-if="fieldDiffs.get(fieldDiffKey(scope.row.field))">
                            <span
                              v-for="(piece, pieceIndex) in fieldDiffs.get(
                                fieldDiffKey(scope.row.field),
                              )!.after"
                              :key="`new-${scope.row.field.container}-${scope.row.field.key}-${pieceIndex}`"
                              :class="pieceClass(piece)"
                              >{{ piece.text }}</span
                            >
                          </template>
                          <span v-else>{{ fieldPreviewText(scope.row.field) }}</span>
                          <span
                            v-for="warning in scope.row.field.warnings ?? []"
                            :key="warning.message"
                            class="audio-field-warning"
                            >{{ warning.message }}</span
                          >
                        </template>
                      </template>
                    </el-table-column>
                    <el-table-column v-if="plan" :label="t('audio.detail.columnOrigin')" width="90">
                      <template #default="scope">
                        <span v-if="scope.row.kind === 'removal'" class="audio-remove">{{
                          fieldOriginLabel(scope.row.removal.origin)
                        }}</span>
                        <span v-else>{{ fieldOriginLabel(scope.row.field.origin) }}</span>
                      </template>
                    </el-table-column>
                  </el-table>
                </template>
                <el-empty v-else :description="t('audio.list.noMatch')" />
              </div>
            </el-splitter-panel>
          </el-splitter>
        </template>
      </div>

      <footer class="audio-action-bar">
        <el-progress
          v-if="busy && progress"
          class="job-progress"
          :percentage="progressPercentage(progress)"
          :format="() => formatProgressLabel(progress, progressStartedAt)"
        />
        <span>{{
          t("audio.footer.selected", {
            files: tn("audio.count.files", totals.files),
            fields: tn("audio.count.fields", totals.fields),
          })
        }}</span>
        <span v-if="lossyFileCount" class="audio-lossy">{{
          tn("audio.footer.lossy", lossyFileCount)
        }}</span>
        <el-button
          link
          :disabled="!filteredFiles.length || busy"
          :title="t('audio.footer.bulkHint')"
          @click="applyBulkSelection(files, filteredFiles, 'all')"
          >{{ t("audio.footer.selectAll") }}</el-button
        >
        <el-button
          link
          :disabled="!filteredFiles.length || busy"
          :title="t('audio.footer.bulkHint')"
          @click="applyBulkSelection(files, filteredFiles, 'none')"
          >{{ t("audio.footer.selectNone") }}</el-button
        >
        <el-button
          link
          :disabled="!filteredFiles.length || busy"
          :title="t('audio.footer.bulkHint')"
          @click="applyBulkSelection(files, filteredFiles, 'invert')"
          >{{ t("audio.footer.invert") }}</el-button
        >
        <span class="audio-action-spacer" />
        <el-button v-if="busy" @click="cancelPlan">{{ t("audio.footer.stop") }}</el-button>
        <el-button v-if="plan" :disabled="busy || !totals.files" @click="createPlan">{{
          t("audio.footer.reload")
        }}</el-button>
        <el-button v-else :disabled="!paths.length || busy" :loading="busy" @click="scan()">{{
          t("audio.footer.rescan")
        }}</el-button>
        <el-button
          v-if="plan"
          type="primary"
          :disabled="busy || totals.files === 0"
          :loading="busy"
          @click="applyPlan"
          >{{
            t("audio.footer.apply", { files: tn("audio.count.files", totals.files) })
          }}</el-button
        >
      </footer>
    </div>
  </section>
</template>
