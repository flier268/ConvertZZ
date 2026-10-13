<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from "vue";
import { open, confirm } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import type {
  AudioContainer,
  AudioTagField,
  AudioTagFile,
  AudioRescanFileRequest,
  AudioTagPlan,
  AudioTagPlanRequest,
  Direction,
  EngineKind,
  TagOperation,
  TextEncoding,
} from "@shared/contracts";
import { ElMessageBox } from "element-plus";
import { core, isCancellationError } from "../lib/coreClient";
import { loadSettings, zhConvertOptions } from "../lib/settings";
import { cliInvocation } from "../lib/cli";
import { formatProgressLabel, progressPercentage, type ProgressSnapshot } from "../lib/progressEta";
import type { Progress } from "../lib/coreClient";
import {
  applyBulkSelection,
  audioBaseName,
  AUDIO_TAG_READ_ERROR,
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
  settingsSummaryChips,
  fieldVisibleWhenHidingUnchanged,
  fieldOriginLabel,
  fileHasTextLoss,
  createAudioPreviewController,
  createTagRule,
  legacyPlayerRule,
  cleanupApeRule,
  tagRuleSummary,
  toTagOperations,
  TAG_COLUMN_KEYS,
  tagColumnLabel,
  tagColumnsForFile,
  columnFieldValue,
  hiddenColumnCount,
  copyTargetsFor,
  defaultColumnPicks,
  columnPicksToOperations,
  mergedColumnValue,
  onConflictForSource,
  id3v1OverwriteWarning,
  manualCopyOperation,
  overwriteOtherOperations,
  manualRemoveOperation,
  type ColumnPick,
  TAG_RULE_FIELDS,
  LEGACY_PLAYER_NOTE,
  CLEANUP_APE_NOTE,
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
const settingsOpen = ref(false);
const listFilter = ref<AudioListFilter>("changed");
const sortKey = ref<AudioSortKey>("name");
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
  tagOperations: [] as TagRule[],
});
const draft = reactive({
  direction: options.direction,
  engine: options.engine,
  recursive: options.recursive,
  vocabularyCorrection: options.vocabularyCorrection,
  backup: backup.value,
  conflictPolicy: conflictPolicy.value,
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
  tagOperations: [] as TagRule[],
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
  if (!files.value.length) return "ID3、APEv2 與 Vorbis Comment 都會保留未選欄位與封面。";
  if (focusKind.value === "folder" && focusFolder.value) {
    const folder = focusFolder.value.replaceAll("\\", "/");
    const folderKey = audioPathKey(focusFolder.value);
    const prefix = `${folderKey}/`;
    const count = files.value.filter((file) => {
      const path = audioPathKey(file.path);
      return path === folderKey || path.startsWith(prefix);
    }).length;
    return `${folder} · ${count} 個檔案`;
  }
  return `${paths.value.length} 個路徑 · 全部 ${files.value.length} 個檔案`;
});
const visibleFields = computed(() => {
  const file = selectedFile.value;
  if (!file) return [];
  if (!hideUnchanged.value || !plan.value) return file.fields;
  return file.fields.filter((field) => fieldVisibleWhenHidingUnchanged(field));
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
const summaryChips = computed(() =>
  settingsSummaryChips({
    direction: options.direction,
    vocabularyCorrection: options.vocabularyCorrection,
    backup: backup.value,
    hasMp3: hasMp3.value,
    id3v1Enabled: options.id3v1Enabled,
    id3v1SourceEncoding: options.id3v1SourceEncoding,
    id3v1OutputEncoding: options.id3v1OutputEncoding,
    id3v2Enabled: options.id3v2Enabled,
    id3v2Direction: options.id3v2Direction,
    id3v2RepairSourceEncoding: options.id3v2RepairSourceEncoding,
    id3v2SourceEncoding: options.id3v2SourceEncoding,
    id3v2Version: options.id3v2Version,
    id3v2Encoding: options.id3v2Encoding,
    tagOperationCount: options.tagOperations.filter((rule) => rule.enabled).length,
  }),
);
const lossyFileCount = computed(() => files.value.filter((file) => fileHasTextLoss(file)).length);
const editingRuleId = ref("");
const editingRule = computed(() =>
  draft.tagOperations.find((rule) => rule.id === editingRuleId.value),
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
  listFilter.value = "changed";
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

const detailView = ref<"table" | "columns">("table");
const mergeTarget = ref<AudioContainer>("id3v2");
const columnOps = ref<TagOperation[]>([]);
const headerOps = ref<TagOperation[]>([]);
const picksByPath = ref<Record<string, ColumnPick[]>>({});
const menuFor = ref<AudioContainer | "">("");
const removeDialog = ref<{ container: AudioContainer; salvage: boolean; onlyAck: boolean } | null>(
  null,
);

const selectedColumns = computed(() =>
  selectedFile.value ? tagColumnsForFile(selectedFile.value) : [],
);
const selectedPicks = computed(() => {
  const file = selectedFile.value;
  if (!file) return [];
  return (
    picksByPath.value[file.path] ??
    defaultColumnPicks(file, mergeTarget.value, selectedColumns.value)
  );
});
const presentContainers = computed(() => {
  const file = selectedFile.value;
  if (!file) return [];
  return [...new Set(file.fields.map((field) => field.container))];
});
const columnNotes = computed(() => {
  const file = selectedFile.value;
  if (!file) return [];
  const notes = new Set<string>();
  for (const operation of [...columnOps.value, ...headerOps.value]) {
    if (!operation.paths?.includes(file.path)) continue;
    const warning = id3v1OverwriteWarning(operation.source, operation.onConflict);
    if (warning) notes.add(warning);
  }
  if (
    selectedPicks.value.some((pick) => pick.source === "id3v1" && pick.source !== mergeTarget.value)
  ) {
    notes.add("ID3v1 當來源時只補空白欄位。");
  }
  return [...notes];
});

function commitColumnPicks(file: AudioTagFile, picks: ColumnPick[]) {
  picksByPath.value = { ...picksByPath.value, [file.path]: picks };
  columnOps.value = [
    ...columnOps.value.filter((operation) => !operation.paths?.includes(file.path)),
    ...columnPicksToOperations(file.path, mergeTarget.value, picks),
  ];
  previewController.schedule();
}

function chooseColumn(key: string, source: AudioContainer) {
  const file = selectedFile.value;
  if (!file || busy.value) return;
  commitColumnPicks(
    file,
    selectedPicks.value.map((pick) => (pick.key === key ? { key, source } : pick)),
  );
}

function onMergeTarget(value: AudioContainer) {
  mergeTarget.value = value;
  const file = selectedFile.value;
  if (!file) return;
  commitColumnPicks(file, selectedPicks.value);
}

function clearColumnPicks() {
  const file = selectedFile.value;
  if (!file) return;
  commitColumnPicks(file, defaultColumnPicks(file, mergeTarget.value, selectedColumns.value));
}

function pickFor(key: string): AudioContainer {
  return selectedPicks.value.find((pick) => pick.key === key)?.source ?? mergeTarget.value;
}

async function confirmId3v1Overwrite(): Promise<boolean> {
  const message = id3v1OverwriteWarning("id3v1", "overwrite");
  if (!message) return true;
  try {
    await ElMessageBox.confirm(message, "ID3v1 覆蓋", {
      type: "warning",
      confirmButtonText: "仍要覆蓋",
      cancelButtonText: "取消",
    });
    return true;
  } catch {
    return false;
  }
}

async function copyColumnTo(source: AudioContainer, target: AudioContainer, overwrite: boolean) {
  menuFor.value = "";
  const file = selectedFile.value;
  if (!file || busy.value) return;
  if (overwrite && source === "id3v1" && !(await confirmId3v1Overwrite())) return;
  if (busy.value) return;
  const next = manualCopyOperation(file.path, source, target, overwrite);
  headerOps.value = [
    ...headerOps.value.filter(
      (operation) =>
        !(
          operation.paths?.includes(file.path) &&
          operation.action === "copy" &&
          operation.source === source &&
          operation.target === target
        ),
    ),
    next,
  ];
  previewController.schedule();
}

async function overwriteColumn(source: AudioContainer) {
  menuFor.value = "";
  const file = selectedFile.value;
  if (!file || busy.value) return;
  if (source === "id3v1" && !(await confirmId3v1Overwrite())) return;
  if (busy.value) return;
  const next = overwriteOtherOperations(file.path, source);
  headerOps.value = [
    ...headerOps.value.filter(
      (operation) =>
        !(
          operation.paths?.includes(file.path) &&
          operation.action === "copy" &&
          operation.source === source
        ),
    ),
    ...next,
  ];
  previewController.schedule();
}

function openRemoveDialog(container: AudioContainer) {
  menuFor.value = "";
  removeDialog.value = {
    container,
    salvage: container !== "id3v2",
    onlyAck: false,
  };
}

function confirmRemoveDialog() {
  const dialog = removeDialog.value;
  const file = selectedFile.value;
  if (!dialog || !file || busy.value) return;
  if (presentContainers.value.length <= 1 && !dialog.onlyAck) return;
  const next = manualRemoveOperation(file.path, dialog.container, dialog.salvage);
  headerOps.value = [
    ...headerOps.value.filter(
      (operation) =>
        !(
          operation.paths?.includes(file.path) &&
          operation.source === dialog.container &&
          (operation.action === "remove" || operation.target === "id3v2")
        ),
    ),
    ...next,
  ];
  removeDialog.value = null;
  previewController.schedule();
}

function cloneRules(rules: TagRule[]): TagRule[] {
  return rules.map((rule) => ({ ...rule, fields: [...rule.fields] }));
}

function openSettings() {
  draftMute += 1;
  draft.direction = options.direction;
  draft.engine = options.engine;
  draft.recursive = options.recursive;
  draft.vocabularyCorrection = options.vocabularyCorrection;
  draft.backup = backup.value;
  draft.conflictPolicy = conflictPolicy.value;
  draft.id3v1Enabled = options.id3v1Enabled;
  draft.id3v1Direction = options.id3v1Direction;
  draft.id3v1SourceEncoding = options.id3v1SourceEncoding;
  draft.id3v1OutputEncoding = options.id3v1OutputEncoding;
  draft.id3v2Enabled = options.id3v2Enabled;
  draft.id3v2Direction = options.id3v2Direction;
  draft.id3v2SourceEncoding = options.id3v2SourceEncoding;
  draft.id3v2RepairSourceEncoding = options.id3v2RepairSourceEncoding;
  draft.id3v2Version = options.id3v2Version;
  draft.id3v2Encoding = options.id3v2Encoding;
  draft.tagOperations = cloneRules(options.tagOperations);
  editingRuleId.value = draft.tagOperations[0]?.id ?? "";
  settingsOpen.value = true;
  void nextTick(() => {
    draftMute -= 1;
  });
}

function applyDraftToOptions() {
  options.direction = draft.direction;
  options.engine = draft.engine;
  options.recursive = draft.recursive;
  options.vocabularyCorrection = draft.vocabularyCorrection;
  backup.value = draft.backup;
  conflictPolicy.value = draft.conflictPolicy;
  options.id3v1Enabled = draft.id3v1Enabled;
  options.id3v1Direction = draft.id3v1Direction;
  options.id3v1SourceEncoding = draft.id3v1SourceEncoding;
  options.id3v1OutputEncoding = draft.id3v1OutputEncoding;
  options.id3v2Enabled = draft.id3v2Enabled;
  options.id3v2Direction = draft.id3v2Direction;
  options.id3v2SourceEncoding = draft.id3v2SourceEncoding;
  options.id3v2RepairSourceEncoding = draft.id3v2RepairSourceEncoding;
  options.id3v2Version = draft.id3v2Version;
  options.id3v2Encoding = draft.id3v2Encoding;
  options.tagOperations = cloneRules(draft.tagOperations);
}

function addTagRule() {
  const rule = createTagRule({ action: "copy" });
  draft.tagOperations.push(rule);
  editingRuleId.value = rule.id;
}

function addLegacyPlayerRule() {
  draft.id3v1OutputEncoding = "big5";
  draft.id3v1Enabled = true;
  const rule = legacyPlayerRule();
  draft.tagOperations.push(rule);
  editingRuleId.value = rule.id;
}

function addCleanupRule() {
  const rule = cleanupApeRule();
  draft.tagOperations.push(rule);
  editingRuleId.value = rule.id;
}

function moveTagRule(index: number, delta: number) {
  const next = index + delta;
  if (next < 0 || next >= draft.tagOperations.length) return;
  const [rule] = draft.tagOperations.splice(index, 1);
  draft.tagOperations.splice(next, 0, rule);
}

function deleteTagRule(index: number) {
  const [removed] = draft.tagOperations.splice(index, 1);
  if (editingRuleId.value === removed?.id) {
    editingRuleId.value = draft.tagOperations[0]?.id ?? "";
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
  if (added.length) paths.value = [...paths.value, ...added];
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
    filters: [{ name: "音訊標籤", extensions: ["mp3", "ape", "ogg", "oga", "opus"] }],
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
  files.value = mergeAudioFiles(files.value, value.batch.files);
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
    }
  | undefined;

async function scan(onlyPaths?: readonly string[]) {
  if (applying.value) return;
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
      pendingFullSnapshot = { epoch, snapshot: pendingFullSnapshot.snapshot };
    } else {
      pendingFullSnapshot = { epoch, snapshot: selectionSnapshot(files.value) };
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
    files.value = mergeAudioFiles(files.value, scanned);
    restoreSelection(files.value, snapshot);
    clearWarningSelection(files.value);
    completed = true;
  } catch (error) {
    if (token !== streamToken) return;
    if (!merging && pendingFullSnapshot?.epoch === epoch) {
      restoreSelection(files.value, pendingFullSnapshot.snapshot);
    }
    if (isCancellationError(error)) ElMessage.info("已取消掃描。");
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
    await previewController.run();
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
    tagOperations: [
      ...toTagOperations(options.tagOperations),
      ...columnOps.value,
      ...headerOps.value,
    ],
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
  listFilter.value = "changed";
}

function reportPreviewError(error: unknown) {
  operationError.value = error instanceof Error ? error.message : String(error);
  ElMessage.error(operationError.value);
}

let draftMute = 0;
const previewController = createAudioPreviewController({
  plan: async () => {
    applyDraftToOptions();
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
  draft,
  () => {
    if (draftMute > 0 || !settingsOpen.value) return;
    previewController.schedule();
  },
  { deep: true },
);

async function closeSettings() {
  settingsOpen.value = false;
  applyDraftToOptions();
  await previewController.flush();
}

async function applyPlan() {
  await previewController.flush();
  if (!plan.value || busy.value || totals.value.files === 0) return;
  const planId = plan.value.planId;
  if (
    !(await confirm(`將寫入 ${totals.value.files} 個音訊檔案的已選標籤。`, {
      title: "確認標籤轉換",
      kind: "warning",
    }))
  )
    return;
  if (busy.value || plan.value?.planId !== planId) return;
  if (
    backup.value &&
    conflictPolicy.value === "overwrite" &&
    !(await confirm("覆寫會取代既有的 .bak 備份。是否確定繼續？", {
      title: "確認覆寫備份",
      kind: "warning",
    }))
  )
    return;
  if (busy.value || plan.value?.planId !== planId) return;
  previewController.invalidate();
  previewController.setApplying(true);
  applying.value = true;
  busy.value = true;
  progress.value = undefined;
  progressStartedAt.value = Date.now();
  try {
    const result = await core.request<{ succeeded: string[]; failed: unknown[] }>(
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
    if (promptAfterConversion.value)
      ElMessage.success(`已完成 ${result.succeeded.length} 個音訊檔案。`);
    if (result.failed.length) ElMessage.warning(`${result.failed.length} 個檔案失敗。`);
    plan.value = undefined;
  } catch (error) {
    if (isCancellationError(error)) ElMessage.info("已取消音訊轉換。");
    else ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    previewController.setApplying(false);
    applying.value = false;
    busy.value = false;
    activeRequestId.value = undefined;
    progress.value = undefined;
    progressStartedAt.value = undefined;
  }
  if (!plan.value && paths.value.length) await scan();
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
    adoptRescannedFile(updated);
    if (listFilter.value === "errors") {
      const bucket = audioFileBucket(updated, audioPreviewReady(files.value));
      if (bucket !== "errors") listFilter.value = bucket;
    }
    selectedPath.value = file.path;
    focusKind.value = "file";
    if (updated.warning) ElMessage.warning(updated.warning);
    else ElMessage.success(`已重新掃描 ${audioBaseName(file.path)}。`);
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
        <h1>音訊標籤</h1>
        <p>{{ pageSubtitle }}</p>
      </div>
      <div class="card-actions">
        <el-button :disabled="applying" @click="chooseFolder">選取資料夾</el-button>
        <el-button type="primary" :disabled="applying" @click="choose">選取音訊檔案</el-button>
      </div>
    </header>

    <div class="audio-settings-bar">
      <strong>轉換設定</strong>
      <div class="audio-settings-chips">
        <el-tag v-for="chip in summaryChips" :key="chip" effect="plain">{{ chip }}</el-tag>
      </div>
      <el-button :disabled="busy" @click="openSettings">編輯設定</el-button>
    </div>

    <div class="audio-toolbar">
      <el-input
        v-model="query"
        clearable
        class="audio-search"
        placeholder="搜尋檔名、專輯、標題"
        :disabled="!files.length"
      />
      <el-select v-model="sortKey" class="audio-sort" :disabled="!files.length" aria-label="排序">
        <el-option label="檔名" value="name" />
        <el-option label="變更數" value="changed" />
        <el-option label="格式" value="format" />
      </el-select>
      <el-radio-group
        v-model="listFilter"
        class="audio-segments"
        :disabled="!files.length"
        aria-label="檔案篩選"
      >
        <el-radio-button value="changed" :aria-pressed="listFilter === 'changed'"
          >有變更 {{ segments.changed }}</el-radio-button
        >
        <el-radio-button value="unchanged" :aria-pressed="listFilter === 'unchanged'"
          >不變 {{ segments.unchanged }}</el-radio-button
        >
        <el-radio-button
          value="warnings"
          :aria-pressed="listFilter === 'warnings'"
          :class="{
            'is-warn': segments.warnings > 0,
            'is-filled': listFilter === 'warnings' && segments.warnings > 0,
          }"
          >警告 {{ segments.warnings }}</el-radio-button
        >
        <el-radio-button
          value="errors"
          :aria-pressed="listFilter === 'errors'"
          :class="{
            'is-alert': segments.errors > 0,
            'is-filled': listFilter === 'errors' && segments.errors > 0,
          }"
          >錯誤 {{ segments.errors }}</el-radio-button
        >
        <el-radio-button value="all" :aria-pressed="listFilter === 'all'"
          >全部 {{ segments.all }}</el-radio-button
        >
      </el-radio-group>
    </div>

    <div class="page-fill-main audio-review">
      <div v-if="operationError && !files.length" class="audio-empty">
        <el-result icon="error" title="掃描失敗" :sub-title="operationError">
          <template #extra>
            <el-button type="primary" :disabled="!paths.length || busy" @click="scan()"
              >再試一次</el-button
            >
          </template>
        </el-result>
      </div>
      <div v-else-if="!files.length" class="audio-empty">
        <el-empty description="尚未掃描音訊檔案。請選取 MP3、APE、OGG 或 Opus。" />
      </div>
      <template v-else>
        <div class="audio-file-pane">
          <el-result
            v-if="operationError"
            class="audio-inline-error"
            icon="warning"
            title="建立預覽失敗"
            :sub-title="operationError"
          />
          <el-empty v-if="!listRows.length" description="沒有符合的檔案" />
          <VirtualList
            v-else
            ref="listRef"
            role="listbox"
            aria-label="音訊檔案"
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
                  :aria-label="`選取資料夾 ${folderPath(index)}`"
                  :disabled="
                    busy || folderSelectionState(filteredFiles, folderPath(index)) === 'empty'
                  "
                  :ref="(element) => bindFolderCheck(element as Element | null, folderPath(index))"
                  @click.stop
                  @change="
                    onFolderChecked(folderPath(index), ($event.target as HTMLInputElement).checked)
                  "
                />
                <span class="audio-caret">{{ folderAt(index)!.collapsed ? "▸" : "▾" }}</span>
                <span class="audio-folder-path">{{ folderAt(index)!.path }}</span>
                <span class="audio-folder-counts">
                  {{ folderAt(index)!.counts.files }} 檔 · 變更
                  {{ folderAt(index)!.counts.changed }} · 錯誤
                  {{ folderAt(index)!.counts.errors }}
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
                  :aria-label="`選取資料夾 ${folderPath(index)}`"
                  :disabled="
                    busy || folderSelectionState(filteredFiles, folderPath(index)) === 'empty'
                  "
                  :ref="(element) => bindFolderCheck(element as Element | null, folderPath(index))"
                  @click.stop
                  @change="
                    onFolderChecked(folderPath(index), ($event.target as HTMLInputElement).checked)
                  "
                />
                <span class="audio-caret">{{ listRows[index].collapsed ? "▸" : "▾" }}</span>
                <span class="audio-folder-path">{{ listRows[index].path }}</span>
                <span class="audio-folder-counts">
                  {{ listRows[index].counts.files }} 檔 · 變更
                  {{ listRows[index].counts.changed }} · 錯誤
                  {{ listRows[index].counts.errors }}
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
                <span class="audio-format">{{ listRows[index].file.format.toUpperCase() }}</span>
                <span v-if="listRows[index].file.warning" class="audio-bang" title="此檔案有錯誤"
                  >!</span
                >
                <span
                  v-else
                  class="audio-badge"
                  :class="{ zero: changedFieldCount(listRows[index].file) === 0 }"
                  >{{ changedFieldCount(listRows[index].file) }}</span
                >
              </div>
            </template>
          </VirtualList>
        </div>
        <div class="audio-detail">
          <template v-if="selectedFile">
            <div class="audio-detail-head">
              <div>
                <strong>{{ audioBaseName(selectedFile.path) }}</strong>
                <p>
                  {{ selectedFile.path }} · {{ selectedFile.format.toUpperCase() }} ·
                  {{ selectedFile.hasCoverArt ? "含封面" : "無封面" }}
                </p>
              </div>
              <el-tooltip placement="bottom-end">
                <template #content>
                  <div class="audio-shortcut-tip">
                    <div>↑/↓ 切換檔案</div>
                    <div>Enter / ←→ 摺疊或展開資料夾</div>
                    <div>空白 勾選</div>
                  </div>
                </template>
                <el-button class="audio-help" circle size="small" aria-label="快捷鍵說明"
                  >?</el-button
                >
              </el-tooltip>
            </div>
            <template v-if="selectedFile.warning">
              <el-alert type="error" :title="AUDIO_TAG_READ_ERROR" show-icon :closable="false">
                <p v-if="audioWarningDetail(selectedFile.warning)" class="audio-error-reason">
                  {{ audioWarningDetail(selectedFile.warning) }}
                </p>
                <p class="audio-error-path">{{ selectedFile.path }}</p>
                <p>這個檔案不能勾選，也不會寫入。修復後可按「重新掃描這個檔案」。</p>
              </el-alert>
              <el-button
                class="audio-rescan"
                :loading="rescanningPath === selectedFile.path"
                :disabled="busy"
                @click="rescanFile(selectedFile)"
                >重新掃描這個檔案</el-button
              >
            </template>
            <div v-else class="audio-detail-tools">
              <el-radio-group v-model="detailView" size="small" class="audio-view-switch">
                <el-radio-button value="table">現有欄位表</el-radio-button>
                <el-radio-button value="columns">並排標籤欄</el-radio-button>
              </el-radio-group>
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
              <el-tag type="success" effect="plain"
                >{{ changedFieldCount(selectedFile) }} 個變更</el-tag
              >
              <button
                type="button"
                class="audio-hide-unchanged"
                :aria-pressed="hideUnchanged"
                @click="hideUnchanged = !hideUnchanged"
              >
                隱藏不變欄位
              </button>
            </div>
            <div v-if="!selectedFile.warning && detailView === 'columns'" class="audio-columns">
              <p class="audio-keys">
                點每一列要保留的來源。結果寫入合併目標，只作用於這個檔案，並加入轉換計畫。
              </p>
              <table class="audio-column-table">
                <thead>
                  <tr>
                    <th>欄位</th>
                    <th v-for="container in selectedColumns" :key="container">
                      <div class="audio-column-head">
                        <span>{{ containerLabel(container) }}</span>
                        <button
                          type="button"
                          class="audio-column-menu-button"
                          :aria-label="`${containerLabel(container)} 欄位選單`"
                          :aria-expanded="menuFor === container"
                          @click="menuFor = menuFor === container ? '' : container"
                        >
                          ⋯
                        </button>
                        <div v-if="menuFor === container" class="audio-column-menu" role="menu">
                          <template v-if="copyTargetsFor(container).length">
                            <button
                              v-for="target in copyTargetsFor(container)"
                              :key="target"
                              type="button"
                              role="menuitem"
                              :disabled="busy"
                              @click="copyColumnTo(container, target, container !== 'id3v1')"
                            >
                              複製到… {{ containerLabel(target) }}
                              <template v-if="container === 'id3v1'">（只補空白）</template>
                            </button>
                            <button
                              v-if="container === 'id3v1'"
                              type="button"
                              role="menuitem"
                              :disabled="busy"
                              @click="copyColumnTo(container, 'id3v2', true)"
                            >
                              複製到… ID3v2（覆蓋）
                            </button>
                          </template>
                          <button
                            type="button"
                            role="menuitem"
                            :disabled="busy || !copyTargetsFor(container).length"
                            @click="overwriteColumn(container)"
                          >
                            以此標籤覆蓋其他標籤
                          </button>
                          <button
                            type="button"
                            role="menuitem"
                            class="audio-column-danger"
                            :disabled="busy || presentContainers.length <= 1"
                            @click="openRemoveDialog(container)"
                          >
                            移除這個標籤…
                          </button>
                        </div>
                      </div>
                    </th>
                    <th>合併後</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="key in TAG_COLUMN_KEYS" :key="key">
                    <td>{{ tagColumnLabel(key) }}</td>
                    <td v-for="container in selectedColumns" :key="container">
                      <label>
                        <input
                          type="radio"
                          :name="`tag-col-${selectedFile.path}-${key}`"
                          :checked="pickFor(key) === container"
                          :aria-label="`${tagColumnLabel(key)} 採用 ${containerLabel(container)}`"
                          :disabled="busy"
                          @change="chooseColumn(key, container)"
                        />
                        {{ columnFieldValue(selectedFile, container, key) || "—" }}
                      </label>
                    </td>
                    <td>
                      {{
                        mergedColumnValue(
                          columnFieldValue(selectedFile, mergeTarget, key),
                          pickFor(key),
                          columnFieldValue(selectedFile, pickFor(key), key),
                          onConflictForSource(pickFor(key), false),
                        ) || "—"
                      }}
                    </td>
                  </tr>
                </tbody>
              </table>
              <div class="audio-column-foot">
                <label>
                  合併目標
                  <select
                    :value="mergeTarget"
                    aria-label="合併目標"
                    :disabled="busy"
                    @change="
                      onMergeTarget(($event.target as HTMLSelectElement).value as AudioContainer)
                    "
                  >
                    <option
                      v-for="container in selectedColumns"
                      :key="container"
                      :value="container"
                    >
                      {{ containerLabel(container) }}
                    </option>
                  </select>
                </label>
                <button type="button" class="audio-hide-unchanged" @click="clearColumnPicks">
                  清除挑選
                </button>
              </div>
              <el-alert
                v-for="note in columnNotes"
                :key="note"
                type="warning"
                :title="note"
                show-icon
                :closable="false"
              />
            </div>
            <el-table
              v-if="!selectedFile.warning && detailView === 'table'"
              :key="hideUnchanged ? 'hide-unchanged' : 'show-all'"
              :data="detailRows"
              empty-text="沒有可顯示的欄位"
            >
              <el-table-column label="轉換" width="70">
                <template #default="scope">
                  <el-checkbox
                    v-if="scope.row.kind === 'removal'"
                    v-model="scope.row.removal.selected"
                    :disabled="busy || Boolean(scope.row.removal.refused)"
                  />
                  <el-checkbox v-else v-model="scope.row.field.selected" :disabled="busy" />
                </template>
              </el-table-column>
              <el-table-column label="標籤" width="110">
                <template #default="scope">{{
                  containerLabel(
                    scope.row.kind === "removal"
                      ? scope.row.removal.container
                      : scope.row.field.container,
                  )
                }}</template>
              </el-table-column>
              <el-table-column label="欄位" width="120">
                <template #default="scope">{{
                  scope.row.kind === "removal" ? "整個標籤" : scope.row.field.label
                }}</template>
              </el-table-column>
              <el-table-column label="目前內容" min-width="160">
                <template #default="scope">
                  <template v-if="scope.row.kind === 'removal'">
                    含 {{ scope.row.removal.hiddenFieldCount }} 個隱藏欄位
                    <span v-if="scope.row.removal.refused">・{{ scope.row.removal.refused }}</span>
                  </template>
                  <template v-else-if="plan && fieldDiffs.get(fieldDiffKey(scope.row.field))">
                    <span
                      v-for="(piece, pieceIndex) in fieldDiffs.get(fieldDiffKey(scope.row.field))!
                        .before"
                      :key="`old-${scope.row.field.container}-${scope.row.field.key}-${pieceIndex}`"
                      :class="pieceClass(piece)"
                      >{{ piece.text }}</span
                    >
                  </template>
                  <template v-else>{{ scope.row.field.values.join(" / ") }}</template>
                </template>
              </el-table-column>
              <el-table-column v-if="plan" label="轉換預覽" min-width="160">
                <template #default="scope">
                  <template v-if="scope.row.kind === 'removal'">
                    <span class="audio-remove">移除標籤</span>
                  </template>
                  <template v-else-if="fieldDiffs.get(fieldDiffKey(scope.row.field))">
                    <span
                      v-for="(piece, pieceIndex) in fieldDiffs.get(fieldDiffKey(scope.row.field))!
                        .after"
                      :key="`new-${scope.row.field.container}-${scope.row.field.key}-${pieceIndex}`"
                      :class="pieceClass(piece)"
                      >{{ piece.text }}</span
                    >
                    <span
                      v-for="warning in scope.row.field.warnings ?? []"
                      :key="warning.message"
                      class="audio-field-warning"
                      >{{ warning.message }}</span
                    >
                  </template>
                  <span v-else class="audio-same">不變</span>
                </template>
              </el-table-column>
              <el-table-column v-if="plan" label="來源" width="90">
                <template #default="scope">
                  <span v-if="scope.row.kind === 'removal'" class="audio-remove">{{
                    fieldOriginLabel(scope.row.removal.origin)
                  }}</span>
                  <span v-else>{{ fieldOriginLabel(scope.row.field.origin) }}</span>
                </template>
              </el-table-column>
            </el-table>
          </template>
          <el-empty v-else description="沒有符合的檔案" />
        </div>
      </template>
    </div>

    <footer class="audio-action-bar">
      <el-progress
        v-if="busy && progress"
        class="job-progress"
        :percentage="progressPercentage(progress)"
        :format="() => formatProgressLabel(progress, progressStartedAt)"
      />
      <span>已勾選 {{ totals.files }} 個檔案・{{ totals.fields }} 個欄位</span>
      <span v-if="lossyFileCount" class="audio-lossy">{{ lossyFileCount }} 個檔案有截斷或缺字</span>
      <el-button
        link
        :disabled="!filteredFiles.length || busy"
        title="作用於目前篩選的全部檔案，包含已摺疊資料夾裡的檔案"
        @click="applyBulkSelection(files, filteredFiles, 'all')"
        >全選</el-button
      >
      <el-button
        link
        :disabled="!filteredFiles.length || busy"
        title="作用於目前篩選的全部檔案，包含已摺疊資料夾裡的檔案"
        @click="applyBulkSelection(files, filteredFiles, 'none')"
        >全不選</el-button
      >
      <el-button
        link
        :disabled="!filteredFiles.length || busy"
        title="作用於目前篩選的全部檔案，包含已摺疊資料夾裡的檔案"
        @click="applyBulkSelection(files, filteredFiles, 'invert')"
        >反選目前篩選</el-button
      >
      <span class="audio-action-spacer" />
      <el-button v-if="busy" @click="cancelPlan">停止作業</el-button>
      <el-button v-if="plan" :disabled="busy || !totals.files" @click="createPlan"
        >重新載入</el-button
      >
      <el-button v-else :disabled="!paths.length || busy" :loading="busy" @click="scan()"
        >重新掃描</el-button
      >
      <el-button
        v-if="plan"
        type="primary"
        :disabled="busy || totals.files === 0"
        :loading="busy"
        @click="applyPlan"
        >確認寫入 {{ totals.files }} 個檔案</el-button
      >
    </footer>

    <el-drawer v-model="settingsOpen" title="轉換設定" size="480px" append-to-body>
      <el-form label-position="top" class="option-grid audio-options" :disabled="busy">
        <h3 class="audio-drawer-section">一般</h3>
        <el-form-item label="APEv2／Vorbis 方向">
          <el-select v-model="draft.direction">
            <el-option label="不轉換" value="none" />
            <el-option label="簡轉繁" value="s2t" />
            <el-option label="繁轉簡" value="t2s" />
          </el-select>
        </el-form-item>
        <el-form-item label="引擎">
          <el-select v-model="draft.engine">
            <el-option label="新式分詞" value="segmented" />
            <el-option label="舊版字典" value="legacy" />
            <el-option label="ZhConvert" value="zhconvert" />
          </el-select>
        </el-form-item>
        <el-form-item label="詞彙修正">
          <el-switch v-model="draft.vocabularyCorrection" />
        </el-form-item>
        <el-form-item label="資料夾掃描">
          <el-checkbox v-model="draft.recursive">包含子資料夾</el-checkbox>
        </el-form-item>
        <h3 class="audio-drawer-section">備份</h3>
        <el-form-item label="轉換前備份">
          <el-switch v-model="draft.backup" active-text=".bak" />
        </el-form-item>
        <el-form-item label="備份衝突">
          <el-select v-model="draft.conflictPolicy" :disabled="!draft.backup">
            <el-option label="略過" value="skip" />
            <el-option label="覆寫" value="overwrite" />
          </el-select>
        </el-form-item>
        <template v-if="hasMp3">
          <h3 class="audio-drawer-section">ID3v1</h3>
          <el-form-item label="ID3v1">
            <el-switch v-model="draft.id3v1Enabled" active-text="啟用轉換" />
          </el-form-item>
          <el-form-item label="ID3v1 方向">
            <el-select v-model="draft.id3v1Direction" :disabled="!draft.id3v1Enabled">
              <el-option label="不轉換" value="none" />
              <el-option label="簡轉繁" value="s2t" />
              <el-option label="繁轉簡" value="t2s" />
            </el-select>
          </el-form-item>
          <el-form-item label="ID3v1 來源">
            <el-select v-model="draft.id3v1SourceEncoding" :disabled="!draft.id3v1Enabled">
              <el-option label="GBK" value="gbk" />
              <el-option label="Big5" value="big5" />
              <el-option label="Shift-JIS" value="shift-jis" />
              <el-option label="UTF-8" value="utf8" />
            </el-select>
          </el-form-item>
          <el-form-item label="ID3v1 輸出">
            <el-select v-model="draft.id3v1OutputEncoding" :disabled="!draft.id3v1Enabled">
              <el-option label="Big5" value="big5" />
              <el-option label="GBK" value="gbk" />
              <el-option label="Shift-JIS" value="shift-jis" />
              <el-option label="UTF-8" value="utf8" />
            </el-select>
          </el-form-item>
          <h3 class="audio-drawer-section">ID3v2</h3>
          <el-form-item label="ID3v2">
            <el-switch v-model="draft.id3v2Enabled" active-text="啟用轉換" />
          </el-form-item>
          <el-form-item label="ID3v2 方向">
            <el-select v-model="draft.id3v2Direction" :disabled="!draft.id3v2Enabled">
              <el-option label="不轉換" value="none" />
              <el-option label="簡轉繁" value="s2t" />
              <el-option label="繁轉簡" value="t2s" />
            </el-select>
          </el-form-item>
          <el-form-item label="ID3v2 來源錯碼">
            <el-switch
              v-model="draft.id3v2RepairSourceEncoding"
              :disabled="!draft.id3v2Enabled"
              active-text="嘗試修復"
            />
          </el-form-item>
          <el-form-item label="ID3v2 錯碼來源編碼">
            <el-select
              v-model="draft.id3v2SourceEncoding"
              :disabled="!draft.id3v2Enabled || !draft.id3v2RepairSourceEncoding"
            >
              <el-option label="GBK" value="gbk" />
              <el-option label="Big5" value="big5" />
              <el-option label="Shift-JIS" value="shift-jis" />
              <el-option label="UTF-8" value="utf8" />
            </el-select>
          </el-form-item>
          <el-form-item label="ID3v2 版本">
            <el-select v-model="draft.id3v2Version" :disabled="!draft.id3v2Enabled">
              <el-option label="2.4" :value="4" />
              <el-option label="2.3" :value="3" />
            </el-select>
          </el-form-item>
          <el-form-item label="ID3v2 編碼">
            <el-select v-model="draft.id3v2Encoding" :disabled="!draft.id3v2Enabled">
              <el-option label="UTF-8" value="utf8" />
              <el-option label="UTF-16" value="utf16" />
              <el-option label="UTF-16BE" value="utf16be" />
              <el-option label="Latin-1" value="latin1" />
            </el-select>
          </el-form-item>
        </template>
        <h3 class="audio-drawer-section">
          標籤操作
          <span v-if="draft.tagOperations.length" class="audio-rule-count">{{
            draft.tagOperations.filter((rule) => rule.enabled).length
          }}</span>
        </h3>
        <p class="audio-rule-lead">
          規則由上到下執行，套用到所有已勾選的檔案。來源一律用讀到的原始標籤，所以先移除再從該標籤複製，仍然複製得到移除前的內容。結果一樣先進預覽。
        </p>
        <div
          v-for="(rule, index) in draft.tagOperations"
          :key="rule.id"
          class="audio-rule"
          :class="{ 'is-on': rule.id === editingRuleId, 'is-remove': rule.action === 'remove' }"
        >
          <span class="audio-rule-index">{{ index + 1 }}</span>
          <button type="button" class="audio-rule-name" @click="editingRuleId = rule.id">
            <span v-if="rule.action === 'remove'" class="audio-remove">移除</span>
            {{ tagRuleSummary(rule, draft.id3v1OutputEncoding) }}
          </button>
          <el-switch v-model="rule.enabled" :aria-label="`啟用規則 ${index + 1}`" />
          <el-button link :disabled="index === 0" @click="moveTagRule(index, -1)">上移</el-button>
          <el-button
            link
            :disabled="index === draft.tagOperations.length - 1"
            @click="moveTagRule(index, 1)"
            >下移</el-button
          >
          <el-button link type="danger" @click="deleteTagRule(index)">刪除</el-button>
        </div>
        <el-button @click="addTagRule">＋ 新增規則</el-button>
        <div v-if="editingRule" class="audio-rule-editor">
          <strong
            >編輯規則
            {{ draft.tagOperations.findIndex((rule) => rule.id === editingRule?.id) + 1 }}</strong
          >
          <div class="audio-rule-grid">
            <label
              >動作
              <el-select v-model="editingRule.action">
                <el-option label="複製" value="copy" />
                <el-option label="合併" value="merge" />
                <el-option label="移除" value="remove" />
              </el-select>
            </label>
            <label
              >衝突時
              <el-select
                v-model="editingRule.onConflict"
                :disabled="editingRule.action === 'remove'"
              >
                <el-option label="覆蓋目標" value="overwrite" />
                <el-option label="只補空白" value="fillEmpty" />
              </el-select>
            </label>
            <label
              >來源
              <el-select v-model="editingRule.source">
                <el-option label="ID3v2" value="id3v2" />
                <el-option label="ID3v1" value="id3v1" />
                <el-option label="APEv2" value="apev2" />
              </el-select>
            </label>
            <label
              >目標
              <el-select v-model="editingRule.target" :disabled="editingRule.action === 'remove'">
                <el-option label="ID3v1" value="id3v1" />
                <el-option label="ID3v2" value="id3v2" />
                <el-option label="APEv2" value="apev2" />
              </el-select>
            </label>
          </div>
          <p v-if="editingRule.action !== 'remove'">欄位</p>
          <div v-if="editingRule.action !== 'remove'" class="audio-rule-fields">
            <el-checkbox
              v-for="[key, label] in TAG_RULE_FIELDS"
              :key="key"
              :model-value="editingRule.fields.includes(key)"
              @change="
                (checked: boolean) => editingRule && toggleRuleField(editingRule, key, checked)
              "
              >{{ label }}</el-checkbox
            >
          </div>
          <p
            v-if="editingRule.target === 'id3v1' && editingRule.action !== 'remove'"
            class="audio-rule-lead"
          >
            ID3v1 輸出編碼沿用「ID3v1」區塊：{{ draft.id3v1OutputEncoding }}
          </p>
        </div>
        <p class="audio-rule-lead">預設範本</p>
        <el-button link @click="addLegacyPlayerRule">舊播放器相容</el-button>
        <el-button link @click="addCleanupRule">清理多餘標籤</el-button>
        <p class="audio-rule-lead">{{ LEGACY_PLAYER_NOTE }}</p>
        <p class="audio-rule-lead">{{ CLEANUP_APE_NOTE }}</p>
      </el-form>
      <template #footer>
        <el-button type="primary" :disabled="busy" @click="closeSettings">完成</el-button>
      </template>
    </el-drawer>

    <el-dialog
      :model-value="Boolean(removeDialog)"
      :title="removeDialog ? `移除 ${containerLabel(removeDialog.container)} 標籤` : ''"
      width="600px"
      append-to-body
      class="audio-remove-dialog"
      @close="removeDialog = null"
    >
      <template v-if="removeDialog && selectedFile">
        <el-alert
          type="error"
          :closable="false"
          show-icon
          :title="`這會刪除整個 ${containerLabel(removeDialog.container)} 標籤`"
          description="包含畫面上沒列出的自訂欄位（例如 ReplayGain、歌詞）。只會加入轉換計畫，按「確認寫入」才會動檔案。寫入前會先建立 .bak 備份。"
        />
        <p class="audio-rule-lead">套用範圍</p>
        <el-radio-group model-value="file" class="audio-remove-scope">
          <el-radio value="file">只有這個檔案（{{ audioBaseName(selectedFile.path) }}）</el-radio>
          <el-radio value="checked" disabled>已勾選的檔案（第二版）</el-radio>
        </el-radio-group>
        <p class="audio-rule-lead">
          含 {{ hiddenColumnCount(selectedFile, removeDialog.container) }} 個隱藏欄位
        </p>
        <el-checkbox
          v-model="removeDialog.salvage"
          :disabled="removeDialog.container === 'id3v2' || presentContainers.length <= 1"
        >
          移除前先補到 ID3v2
        </el-checkbox>
        <el-checkbox v-if="presentContainers.length <= 1" v-model="removeDialog.onlyAck">
          我確定要移除檔案唯一的標籤
        </el-checkbox>
      </template>
      <template #footer>
        <el-button @click="removeDialog = null">取消</el-button>
        <el-button
          type="danger"
          :disabled="
            busy || !removeDialog || (presentContainers.length <= 1 && !removeDialog.onlyAck)
          "
          @click="confirmRemoveDialog"
        >
          加入計畫：移除 {{ removeDialog ? containerLabel(removeDialog.container) : "" }}
        </el-button>
      </template>
    </el-dialog>
  </section>
</template>
