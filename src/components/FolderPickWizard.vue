<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import type { ExtensionStats, FileFilterProfile } from "@shared/contracts";
import { core, isCancellationError } from "../lib/coreClient";
import {
  applyFilterToStats,
  duplicateProfile,
  emptyProfile,
  filterCollectsNothing,
  modeLabel,
  NO_EXTENSION_LABEL,
  profileFilter,
  summarizeProfile,
} from "../lib/filterProfiles";
import ProfileEditorDialog from "./ProfileEditorDialog.vue";

export interface FolderPickResult {
  path: string;
  recursive: boolean;
  profile: FileFilterProfile;
  remember: boolean;
  fileCount: number;
}

const props = defineProps<{
  modelValue: boolean;
  profiles: FileFilterProfile[];
  initialPath: string;
  recursive: boolean;
  profileId: string;
  /** 記住上次 profile 的快速路徑：直接從步驟 3 開始。 */
  quick: boolean;
  defaultPath?: string;
}>();
const emit = defineEmits<{
  (event: "update:modelValue", value: boolean): void;
  (event: "update:recursive", value: boolean): void;
  (event: "update:profileId", value: string): void;
  (event: "saveProfile", profile: FileFilterProfile): void;
  (event: "confirm", result: FolderPickResult): void;
}>();

const step = ref<1 | 2 | 3>(1);
const path = ref("");
const remember = ref(true);
const stats = ref<ExtensionStats>();
const scanning = ref(false);
const scanProgress = ref("");
const scanSeconds = ref<number>();
const scanRequestId = ref<string>();
const editorOpen = ref(false);
const editorProfile = ref<FileFilterProfile | null>(null);
const quickPath = ref(false);
let scanToken = 0;

const selected = computed(
  () => props.profiles.find((item) => item.id === props.profileId) ?? props.profiles[0],
);
const filtered = computed(() =>
  stats.value && selected.value
    ? applyFilterToStats(stats.value, profileFilter(selected.value))
    : undefined,
);
const emptyAllowList = computed(
  () => Boolean(selected.value) && filterCollectsNothing(selected.value),
);
const folderName = computed(() => path.value.split(/[\\/]/u).filter(Boolean).pop() ?? path.value);
const steps = [
  { id: 1, label: "資料夾" },
  { id: 2, label: "篩選 profile" },
  { id: 3, label: "預覽與確認" },
] as const;

watch(
  () => props.modelValue,
  (visible) => {
    if (!visible) {
      scanToken += 1;
      void cancelScan();
      return;
    }
    path.value = props.initialPath;
    stats.value = undefined;
    quickPath.value = props.quick && Boolean(props.initialPath);
    step.value = quickPath.value ? 3 : 1;
    if (quickPath.value) void scan();
  },
  { immediate: true },
);

async function browse(): Promise<void> {
  const chosen = await open({
    directory: true,
    multiple: false,
    defaultPath: path.value || props.defaultPath,
  });
  if (chosen && !Array.isArray(chosen)) {
    path.value = chosen;
    stats.value = undefined;
  }
}

async function scan(): Promise<void> {
  if (!path.value) return;
  const token = ++scanToken;
  await cancelScan();
  scanning.value = true;
  scanProgress.value = "";
  scanSeconds.value = undefined;
  stats.value = undefined;
  const started = Date.now();
  try {
    const result = await core.request<ExtensionStats>(
      "files.scanExtensions",
      { path: path.value, recursive: props.recursive },
      {
        onProgress: (value) => {
          if (token === scanToken) scanProgress.value = value.message;
        },
        onRequestId: (id) => {
          scanRequestId.value = id;
        },
      },
    );
    if (token !== scanToken) return;
    stats.value = result;
    scanSeconds.value = (Date.now() - started) / 1000;
  } catch (error) {
    if (token === scanToken && !isCancellationError(error))
      ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    if (token === scanToken) {
      scanning.value = false;
      scanRequestId.value = undefined;
    }
  }
}

async function cancelScan(): Promise<void> {
  const id = scanRequestId.value;
  if (id) await core.cancel(id).catch(() => false);
}

function goTo(target: 1 | 2 | 3): void {
  if (target === 1) {
    quickPath.value = false;
    step.value = 1;
  } else if (target === 2 && path.value) {
    quickPath.value = false;
    step.value = 2;
  } else if (target === 3 && path.value) {
    step.value = 3;
    if (!stats.value && !scanning.value) void scan();
  }
}

function back(): void {
  if (step.value === 3) goTo(2);
  else if (step.value === 2) goTo(1);
}

function next(): void {
  if (step.value === 1) goTo(2);
  else if (step.value === 2) goTo(3);
}

function setRecursive(value: boolean): void {
  emit("update:recursive", value);
  stats.value = undefined;
}

function openEditor(profile: FileFilterProfile | null): void {
  editorProfile.value = profile ?? emptyProfile(props.profiles);
  editorOpen.value = true;
}

function copyProfile(profile: FileFilterProfile): void {
  const copy = duplicateProfile(profile, props.profiles);
  emit("saveProfile", copy);
  emit("update:profileId", copy.id);
}

function onEditorSave(profile: FileFilterProfile): void {
  emit("saveProfile", profile);
  emit("update:profileId", profile.id);
}

function confirmPick(): void {
  if (!selected.value || !filtered.value || !filtered.value.included) return;
  emit("confirm", {
    path: path.value,
    recursive: props.recursive,
    profile: selected.value,
    remember: remember.value,
    fileCount: filtered.value.included,
  });
  emit("update:modelValue", false);
}

function close(): void {
  emit("update:modelValue", false);
}

function formatCount(value: number): string {
  return value.toLocaleString("en-US");
}

const confirmLabel = computed(() =>
  filtered.value ? `加入 ${formatCount(filtered.value.included)} 個檔案` : "加入檔案",
);
const canConfirm = computed(() => Boolean(filtered.value && filtered.value.included > 0));
</script>

<template>
  <el-dialog
    :model-value="modelValue"
    :title="step === 3 && path ? `選取來源資料夾 — ${folderName}` : '選取來源資料夾'"
    width="720px"
    class="folder-wizard"
    append-to-body
    @update:model-value="(value: boolean) => (value ? undefined : close())"
  >
    <ol class="wizard-steps" aria-label="步驟">
      <li
        v-for="item in steps"
        :key="item.id"
        :class="{ active: step === item.id, done: step > item.id }"
        :aria-current="step === item.id ? 'step' : undefined"
      >
        {{ step > item.id ? "✓ " : "" }}{{ item.id }} {{ item.label }}
      </li>
    </ol>

    <section v-if="step === 1" class="wizard-body" aria-label="步驟 1 資料夾">
      <div class="path-row">
        <el-input
          v-model="path"
          placeholder="資料夾路徑"
          aria-label="來源資料夾路徑"
          @update:model-value="stats = undefined"
        />
        <el-button @click="browse">瀏覽…</el-button>
      </div>
      <el-checkbox
        :model-value="recursive"
        @update:model-value="(value: unknown) => setRecursive(Boolean(value))"
        >包含子資料夾</el-checkbox
      >
      <p class="wizard-hint">隱藏資料夾（以 . 開頭）與版本控制資料夾不會掃描。</p>
    </section>

    <section v-else-if="step === 2" class="wizard-body" aria-label="步驟 2 篩選 profile">
      <div class="profile-head">
        <span class="wizard-hint">{{ path }}{{ recursive ? "（含子資料夾）" : "" }}</span>
        <el-button @click="openEditor(null)">＋ 新增 profile</el-button>
      </div>
      <el-radio-group
        :model-value="profileId"
        class="profile-list"
        @update:model-value="(value: unknown) => emit('update:profileId', String(value))"
      >
        <div
          v-for="profile in profiles"
          :key="profile.id"
          class="profile-row"
          :class="{ selected: profile.id === selected?.id }"
          data-testid="profile-row"
          @click="emit('update:profileId', profile.id)"
        >
          <el-radio :value="profile.id" :aria-label="profile.name">
            <span class="profile-name">{{ profile.name }}</span>
            <el-tag
              disable-transitions
              size="small"
              :type="profile.mode === 'deny' ? 'danger' : 'success'"
              >{{ modeLabel(profile.mode) }}</el-tag
            >
            <small v-if="profile.builtin" class="wizard-hint">內建・不可刪除</small>
            <span class="profile-summary">{{ summarizeProfile(profile) }}</span>
          </el-radio>
          <span class="profile-row-actions" @click.stop>
            <el-button text type="primary" @click="openEditor(profile)">編輯</el-button>
            <el-button v-if="!profile.builtin" text type="primary" @click="copyProfile(profile)"
              >複製</el-button
            >
          </span>
        </div>
      </el-radio-group>
      <el-checkbox v-model="remember">下次選資料夾時記住這個 profile，直接到預覽</el-checkbox>
    </section>

    <section v-else class="wizard-body" aria-label="步驟 3 預覽與確認">
      <el-alert
        v-if="quickPath && selected"
        type="success"
        :closable="false"
        class="quick-banner"
        show-icon
      >
        已套用上次的 profile「{{ selected.name }}」，略過步驟 2。
        <el-button text type="primary" @click="goTo(2)">更換 profile</el-button>
      </el-alert>
      <p v-if="scanning" class="wizard-hint" role="status">
        {{ scanProgress || "掃描中…" }}
        <el-button text type="danger" @click="cancelScan">取消</el-button>
      </p>
      <template v-else-if="filtered && stats">
        <div class="stat-cards">
          <div>
            <strong>{{ formatCount(filtered.included) }}</strong
            ><span>將包含的檔案</span>
          </div>
          <div>
            <strong>{{ filtered.includedKinds }}</strong
            ><span>副檔名種類</span>
          </div>
          <div>
            <strong>{{ formatCount(filtered.excluded) }}</strong
            ><span>排除的檔案</span>
          </div>
          <div v-if="recursive">
            <strong>{{ formatCount(stats.directoryCount) }}</strong
            ><span>子資料夾</span>
          </div>
        </div>
        <el-alert
          v-if="!canConfirm"
          type="warning"
          :closable="false"
          show-icon
          title="這個 profile 沒有符合的檔案。請按「上一步」換一個 profile。"
        />
        <el-alert
          v-for="warning in stats.warnings"
          :key="warning"
          type="warning"
          :closable="false"
          :title="warning"
        />
        <div class="stat-table-wrap">
          <table class="stat-table" aria-label="副檔名統計">
            <thead>
              <tr>
                <th>副檔名</th>
                <th>檔案數</th>
                <th>狀態</th>
              </tr>
            </thead>
            <tbody>
              <tr
                v-for="row in filtered.rows"
                :key="row.extension ?? ''"
                :class="{ off: !row.included }"
              >
                <td>{{ row.extension ?? NO_EXTENSION_LABEL }}</td>
                <td>{{ formatCount(row.count) }}</td>
                <td>
                  <el-tag
                    disable-transitions
                    size="small"
                    :type="row.included ? 'success' : 'info'"
                    >{{ row.included ? "包含" : "排除" }}</el-tag
                  >
                </td>
              </tr>
            </tbody>
          </table>
        </div>
        <p class="wizard-hint profile-line">
          profile：<strong>{{ selected?.name }}</strong>
          <el-tag disable-transitions size="small">{{
            selected ? modeLabel(selected.mode) : ""
          }}</el-tag>
          <el-button text type="primary" @click="goTo(2)">更換</el-button>
          <span v-if="scanSeconds !== undefined" class="scan-time"
            >掃描 {{ scanSeconds.toFixed(1) }} 秒</span
          >
        </p>
        <p v-if="emptyAllowList" class="wizard-hint">此 profile 是空白名單。</p>
      </template>
    </section>

    <template #footer>
      <div class="wizard-footer">
        <el-button :disabled="step === 1" @click="back">上一步</el-button>
        <span class="spacer" />
        <el-button @click="close">取消</el-button>
        <el-button v-if="step === 1" type="primary" :disabled="!path" @click="next"
          >下一步</el-button
        >
        <el-button v-else-if="step === 2" type="primary" @click="next">下一步</el-button>
        <el-button v-else type="primary" :disabled="!canConfirm" @click="confirmPick">{{
          confirmLabel
        }}</el-button>
      </div>
    </template>
    <ProfileEditorDialog
      v-model="editorOpen"
      :profile="editorProfile"
      :existing="profiles"
      :scan-start-path="path"
      @save="onEditorSave"
    />
  </el-dialog>
</template>

<style scoped>
.wizard-steps {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
  margin: 0 0 16px;
  padding: 0;
  list-style: none;
}
.wizard-steps li {
  padding: 10px;
  text-align: center;
  border-radius: 8px;
  background: #e8efed;
  color: #5b746f;
}
.wizard-steps li.done {
  background: #e8f5f1;
  color: #0f8f82;
}
.wizard-steps li.active {
  background: #0f8f82;
  color: #fff;
}
.wizard-body {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 220px;
}
.path-row,
.profile-head {
  display: flex;
  gap: 8px;
  align-items: center;
  justify-content: space-between;
}
.wizard-hint {
  margin: 0;
  color: #5b746f;
  font-size: 13px;
}
.profile-list {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 8px;
}
.profile-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  border: 1px solid #d7e4e0;
  border-radius: 10px;
}
.profile-row.selected {
  border-color: #0f8f82;
  background: #f4faf8;
}
.profile-name {
  margin-right: 8px;
  font-weight: 600;
}
.profile-summary {
  margin-left: 8px;
  color: #5b746f;
  font-size: 12px;
}
.stat-cards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
  gap: 10px;
}
.stat-cards div {
  display: flex;
  flex-direction: column;
  padding: 12px 14px;
  border: 1px solid #d7e4e0;
  border-radius: 10px;
  background: #f4faf8;
}
.stat-cards strong {
  font-size: 24px;
  color: #0f8f82;
}
.stat-table-wrap {
  max-height: 260px;
  overflow: auto;
}
.stat-table {
  width: 100%;
  border-collapse: collapse;
}
.stat-table th,
.stat-table td {
  padding: 6px 10px;
  text-align: left;
  border-bottom: 1px solid #eef3f1;
}
.stat-table tr.off td {
  color: #8aa19b;
}
.profile-line {
  display: flex;
  align-items: center;
  gap: 8px;
}
.scan-time {
  margin-left: auto;
}
.wizard-footer {
  display: flex;
  gap: 8px;
  align-items: center;
}
.wizard-footer .spacer {
  flex: 1;
}
</style>
