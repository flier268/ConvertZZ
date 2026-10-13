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
import { core, isCancellationError } from "../lib/coreClient";
import { loadSettings, zhConvertOptions } from "../lib/settings";
import { cliInvocation } from "../lib/cli";
import { formatProgressLabel, progressPercentage, type ProgressSnapshot } from "../lib/progressEta";
import {
  applyBulkSelection,
  audioBaseName,
  audioReviewStats,
  changedFieldCount,
  charDiff,
  filterAudioFiles,
  selectionTotals,
  settingsSummaryChips,
  fieldValuesChanged,
  type AudioListFilter,
  type DiffPiece,
} from "../lib/audioReview";
import VirtualList from "../components/VirtualList.vue";

defineOptions({ name: "AudioPage" });

const paths = ref<string[]>([]);
const files = ref<AudioTagFile[]>([]);
const plan = ref<AudioTagPlan>();
const busy = ref(false);
const promptAfterConversion = ref(true);
const backup = ref(true);
const conflictPolicy = ref<AudioTagPlanRequest["conflictPolicy"]>("skip");
const progress = ref<ProgressSnapshot>();
const progressStartedAt = ref<number>();
const activeRequestId = ref<string>();
const settingsOpen = ref(false);
const listFilter = ref<AudioListFilter>("changed");
const query = ref("");
const hideUnchanged = ref(true);
const selectedPath = ref("");
const listRef = ref<{ scrollToIndex: (index: number) => void } | null>(null);
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
const stats = computed(() => audioReviewStats(files.value));
const filteredFiles = computed(() => filterAudioFiles(files.value, listFilter.value, query.value));
const selectedFile = computed(() => files.value.find((file) => file.path === selectedPath.value));
const selectedIndex = computed(() =>
  filteredFiles.value.findIndex((file) => file.path === selectedPath.value),
);
const visibleFields = computed(() => {
  const file = selectedFile.value;
  if (!file) return [];
  if (!hideUnchanged.value || !plan.value) return file.fields;
  return file.fields.filter((field) => fieldValuesChanged(field));
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
  }),
);

watch(filteredFiles, (list) => {
  if (!list.some((file) => file.path === selectedPath.value)) {
    selectedPath.value = list[0]?.path ?? "";
  }
});

function joinValues(values: string[] | undefined): string {
  return (values ?? []).join("\n");
}

function pieceClass(piece: DiffPiece): string {
  if (piece.kind === "remove") return "diff-del";
  if (piece.kind === "add") return "diff-ins";
  return "";
}

function containerLabel(container: AudioTagField["container"]): string {
  if (container === "id3v1") return "ID3v1";
  if (container === "id3v2") return "ID3v2";
  if (container === "apev2") return "APEv2";
  return "Vorbis";
}

function openSettings() {
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
  settingsOpen.value = true;
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
}

async function choose() {
  const selected = await open({
    multiple: true,
    filters: [{ name: "音訊標籤", extensions: ["mp3", "ape", "ogg", "oga", "opus"] }],
  });
  if (!selected) return;
  paths.value = Array.isArray(selected) ? selected : [selected];
  plan.value = undefined;
  await scan();
}

async function chooseFolder() {
  const selected = await open({ directory: true, multiple: false });
  if (!selected) return;
  paths.value = Array.isArray(selected) ? selected : [selected];
  plan.value = undefined;
  await scan();
}

async function scan() {
  busy.value = true;
  progress.value = undefined;
  progressStartedAt.value = Date.now();
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
      {
        onProgress: (value) => {
          progress.value = value;
        },
        onRequestId: (id) => {
          activeRequestId.value = id;
        },
      },
    );
  } catch (error) {
    if (isCancellationError(error)) ElMessage.info("已取消掃描。");
    else ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    busy.value = false;
    activeRequestId.value = undefined;
    progress.value = undefined;
    progressStartedAt.value = undefined;
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
  progressStartedAt.value = Date.now();
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
      {
        onProgress: (value) => {
          progress.value = value;
        },
        onRequestId: (id) => {
          activeRequestId.value = id;
        },
      },
    );
    files.value = plan.value.files;
    listFilter.value = "changed";
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

async function applySettings() {
  const hadPlan = Boolean(plan.value);
  applyDraftToOptions();
  settingsOpen.value = false;
  plan.value = undefined;
  if (!paths.value.length) return;
  await scan();
  if (hadPlan && files.value.length) await createPlan();
}

async function applyPlan() {
  if (!plan.value || busy.value || totals.value.files === 0) return;
  if (
    !(await confirm(`將寫入 ${totals.value.files} 個音訊檔案的已選標籤。`, {
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
  progressStartedAt.value = Date.now();
  try {
    const result = await core.request<{ succeeded: string[]; failed: unknown[] }>(
      "audio.apply",
      { planId: plan.value.planId },
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
    await scan();
  } catch (error) {
    if (isCancellationError(error)) ElMessage.info("已取消音訊轉換。");
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
      // 仍嘗試以計畫 ID 取消。
    }
  }
  if (planId) {
    await core.request("audio.cancel", { planId });
  }
  if (!busy.value) plan.value = undefined;
}

function selectFile(file: AudioTagFile) {
  selectedPath.value = file.path;
}

function moveSelection(delta: number) {
  const list = filteredFiles.value;
  if (!list.length) return;
  const next = Math.min(list.length - 1, Math.max(0, selectedIndex.value + delta));
  selectedPath.value = list[next]?.path ?? "";
  listRef.value?.scrollToIndex(next);
}

function onReviewKeydown(event: KeyboardEvent) {
  const target = event.target as HTMLElement | null;
  const tag = target?.tagName;
  if (tag === "INPUT" || tag === "TEXTAREA" || target?.isContentEditable) return;
  if (event.key === "ArrowDown") {
    event.preventDefault();
    moveSelection(1);
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    moveSelection(-1);
  } else if (event.key === " ") {
    const file = selectedFile.value;
    if (!file || file.warning) return;
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
  <section class="page-stack" @keydown="onReviewKeydown">
    <header class="page-header">
      <div>
        <p class="eyebrow">AUDIO TAGS</p>
        <h1>音訊標籤</h1>
        <p>ID3、APEv2 與 Vorbis Comment 都會保留未選欄位與封面。</p>
      </div>
      <div class="card-actions">
        <el-button :disabled="Boolean(plan) || busy" @click="chooseFolder">選取資料夾</el-button>
        <el-button type="primary" :disabled="Boolean(plan) || busy" @click="choose"
          >選取音訊檔案</el-button
        >
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
      <el-check-tag :checked="listFilter === 'changed'" @change="listFilter = 'changed'"
        >只看有變更 {{ stats.listedChanged }}</el-check-tag
      >
      <el-check-tag :checked="listFilter === 'errors'" @change="listFilter = 'errors'"
        >只看錯誤 {{ stats.errors }}</el-check-tag
      >
      <el-check-tag :checked="listFilter === 'all'" @change="listFilter = 'all'"
        >全部 {{ stats.total }}</el-check-tag
      >
      <div class="audio-stats">
        <button type="button" :class="{ on: listFilter === 'all' }" @click="listFilter = 'all'">
          共 {{ stats.total }}
        </button>
        <button
          type="button"
          :class="{ on: listFilter === 'changed' }"
          @click="listFilter = 'changed'"
        >
          有變更 {{ stats.changed }}
        </button>
        <button
          type="button"
          :class="{ on: listFilter === 'unchanged' }"
          @click="listFilter = 'unchanged'"
        >
          不變 {{ stats.unchanged }}
        </button>
        <button
          type="button"
          :class="{ on: listFilter === 'errors' }"
          @click="listFilter = 'errors'"
        >
          錯誤 {{ stats.errors }}
        </button>
      </div>
    </div>

    <div class="page-fill-main audio-review">
      <div v-if="!files.length" class="audio-empty">
        <el-empty description="請選取 MP3、APE、OGG 或 Opus 檔案" />
      </div>
      <template v-else>
        <div class="audio-file-pane">
          <VirtualList ref="listRef" :count="filteredFiles.length" :item-height="40">
            <template #default="{ index }">
              <div
                class="audio-row"
                :class="{
                  active: filteredFiles[index]?.path === selectedPath,
                  warn: Boolean(filteredFiles[index]?.warning),
                }"
                @click="selectFile(filteredFiles[index])"
              >
                <el-checkbox
                  v-model="filteredFiles[index].selected"
                  :disabled="Boolean(filteredFiles[index].warning) || busy"
                  @click.stop
                />
                <span class="audio-name">{{ audioBaseName(filteredFiles[index].path) }}</span>
                <span class="audio-format">{{ filteredFiles[index].format.toUpperCase() }}</span>
                <span v-if="filteredFiles[index].warning" class="audio-bang" title="此檔案有錯誤"
                  >!</span
                >
                <span
                  v-else
                  class="audio-badge"
                  :class="{ zero: changedFieldCount(filteredFiles[index]) === 0 }"
                  >{{ changedFieldCount(filteredFiles[index]) }}</span
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
              <span class="audio-keys"><kbd>↑</kbd><kbd>↓</kbd> 切換檔案 <kbd>空白</kbd> 勾選</span>
            </div>
            <el-alert
              v-if="selectedFile.warning"
              type="error"
              :title="selectedFile.warning"
              description="此檔案有錯誤，不能勾選，也不會寫入。"
              show-icon
              :closable="false"
            />
            <div v-else class="audio-detail-tools">
              <el-tag type="success" effect="plain"
                >{{ changedFieldCount(selectedFile) }} 個變更</el-tag
              >
              <el-check-tag :checked="hideUnchanged" @change="hideUnchanged = !hideUnchanged"
                >隱藏不變欄位</el-check-tag
              >
            </div>
            <el-table
              v-if="!selectedFile.warning"
              :data="visibleFields"
              empty-text="沒有可顯示的欄位"
            >
              <el-table-column label="轉換" width="70">
                <template #default="scope">
                  <el-checkbox v-model="scope.row.selected" :disabled="busy" />
                </template>
              </el-table-column>
              <el-table-column label="標籤" width="110">
                <template #default="scope">{{ containerLabel(scope.row.container) }}</template>
              </el-table-column>
              <el-table-column prop="label" label="欄位" width="120" />
              <el-table-column label="目前內容" min-width="160">
                <template #default="scope">
                  <template v-if="plan && scope.row.convertedValues">
                    <span
                      v-for="(piece, pieceIndex) in charDiff(
                        joinValues(scope.row.values),
                        joinValues(scope.row.convertedValues),
                      ).before"
                      :key="`old-${scope.row.container}-${scope.row.key}-${pieceIndex}`"
                      :class="pieceClass(piece)"
                      >{{ piece.text }}</span
                    >
                  </template>
                  <template v-else>{{ scope.row.values.join(" / ") }}</template>
                </template>
              </el-table-column>
              <el-table-column v-if="plan" label="轉換預覽" min-width="160">
                <template #default="scope">
                  <template v-if="scope.row.convertedValues">
                    <span
                      v-for="(piece, pieceIndex) in charDiff(
                        joinValues(scope.row.values),
                        joinValues(scope.row.convertedValues),
                      ).after"
                      :key="`new-${scope.row.container}-${scope.row.key}-${pieceIndex}`"
                      :class="pieceClass(piece)"
                      >{{ piece.text }}</span
                    >
                  </template>
                  <span v-else class="audio-same">不變</span>
                </template>
              </el-table-column>
            </el-table>
          </template>
          <el-empty v-else description="這個篩選沒有檔案" />
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
      <el-button
        link
        :disabled="!filteredFiles.length || busy"
        @click="applyBulkSelection(files, filteredFiles, 'all')"
        >全選</el-button
      >
      <el-button
        link
        :disabled="!filteredFiles.length || busy"
        @click="applyBulkSelection(files, filteredFiles, 'none')"
        >全不選</el-button
      >
      <el-button
        link
        :disabled="!filteredFiles.length || busy"
        @click="applyBulkSelection(files, filteredFiles, 'invert')"
        >反選</el-button
      >
      <span class="audio-action-spacer" />
      <el-button v-if="plan || busy" @click="cancelPlan">{{
        busy ? "停止作業" : "取消計畫"
      }}</el-button>
      <el-button v-if="plan" :disabled="busy || !totals.files" @click="createPlan"
        >重新建立預覽</el-button
      >
      <el-button v-else :disabled="!paths.length || busy" :loading="busy" @click="scan"
        >重新掃描</el-button
      >
      <el-button v-if="!plan" :disabled="!totals.files || busy" :loading="busy" @click="createPlan"
        >建立標籤預覽</el-button
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

    <el-drawer v-model="settingsOpen" title="轉換設定" size="440px" append-to-body>
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
      </el-form>
      <template #footer>
        <el-button type="primary" :disabled="busy" @click="applySettings"
          >套用並重新建立預覽</el-button
        >
      </template>
    </el-drawer>
  </section>
</template>
