<script setup lang="ts">
import { computed, ref } from "vue";
import { ElMessage } from "element-plus";
import type { FileFilterProfile, FolderPickBehavior, SettingsV2 } from "@shared/contracts";
import { removeProfile, upsertProfile } from "../lib/filterProfileActions";
import {
  duplicateProfile,
  emptyProfile,
  exportProfilesJson,
  modeLabel,
  parseImportedProfiles,
  summarizeProfile,
} from "../lib/filterProfiles";
import ProfileEditorDialog from "./ProfileEditorDialog.vue";

/** 直接修改傳入的 `settings.files`；儲存由設定頁的儲存列負責。 */
const props = defineProps<{ files: SettingsV2["files"] }>();

const editorVisible = ref(false);
const editorSource = ref<FileFilterProfile | null>(null);
const transferVisible = ref(false);
const transferKind = ref<"import" | "export">("export");
const transferText = ref("");

const profiles = computed(() => props.files.filterProfiles);

function create(): void {
  editorSource.value = emptyProfile(profiles.value);
  editorVisible.value = true;
}

function edit(profile: FileFilterProfile): void {
  editorSource.value = profile;
  editorVisible.value = true;
}

function onSave(profile: FileFilterProfile): void {
  upsertProfile(props.files, profile);
}

function copy(profile: FileFilterProfile): void {
  upsertProfile(props.files, duplicateProfile(profile, profiles.value));
}

function remove(profile: FileFilterProfile): void {
  removeProfile(props.files, profile.id);
}

function setDefault(profile: FileFilterProfile): void {
  props.files.defaultFilterProfileId = profile.id;
}

function openExport(): void {
  transferKind.value = "export";
  transferText.value = exportProfilesJson(profiles.value);
  transferVisible.value = true;
}

function openImport(): void {
  transferKind.value = "import";
  transferText.value = "";
  transferVisible.value = true;
}

async function copyExport(): Promise<void> {
  try {
    await navigator.clipboard.writeText(transferText.value);
    ElMessage.success("已複製到剪貼簿。");
  } catch {
    ElMessage.warning("無法寫入剪貼簿，請手動選取文字複製。");
  }
}

async function onImportFile(event: Event): Promise<void> {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  if (file) transferText.value = await file.text();
  input.value = "";
}

function runImport(): void {
  const result = parseImportedProfiles(transferText.value, profiles.value);
  if (result.error) {
    ElMessage.error(result.error);
    return;
  }
  for (const profile of result.profiles) upsertProfile(props.files, profile);
  transferVisible.value = false;
  ElMessage.success(`已匯入 ${result.profiles.length} 個 profile。`);
}

function setBehavior(value: unknown): void {
  props.files.folderPickBehavior = value as FolderPickBehavior;
}
</script>

<template>
  <div class="filter-profiles" data-testid="filter-profiles">
    <div class="profiles-head">
      <div class="section-title">副檔名篩選 profile</div>
      <div class="profiles-actions">
        <el-button @click="create">新增</el-button>
        <el-button @click="openImport">匯入</el-button>
        <el-button @click="openExport">匯出</el-button>
      </div>
    </div>
    <el-table :data="profiles" row-key="id" class="profiles-table" size="small">
      <el-table-column label="預設" width="70">
        <template #default="{ row }">
          <el-radio
            :model-value="files.defaultFilterProfileId"
            :value="row.id"
            :aria-label="`設 ${row.name} 為預設`"
            @update:model-value="setDefault(row)"
            ><span class="sr-only">預設</span></el-radio
          >
        </template>
      </el-table-column>
      <el-table-column label="名稱" min-width="140">
        <template #default="{ row }"
          >{{ row.name }}<el-tag v-if="row.builtin" size="small" class="builtin-tag">內建</el-tag>
        </template>
      </el-table-column>
      <el-table-column label="模式" width="90">
        <template #default="{ row }"
          ><el-tag size="small" :type="row.mode === 'deny' ? 'danger' : 'success'">{{
            modeLabel(row.mode)
          }}</el-tag></template
        >
      </el-table-column>
      <el-table-column label="副檔名" min-width="200">
        <template #default="{ row }">{{ summarizeProfile(row) }}</template>
      </el-table-column>
      <el-table-column label="操作" width="180">
        <template #default="{ row }">
          <el-button text type="primary" @click="edit(row)">編輯</el-button>
          <el-button text type="primary" @click="copy(row)">複製</el-button>
          <el-button v-if="!row.builtin" text type="danger" @click="remove(row)">刪除</el-button>
        </template>
      </el-table-column>
    </el-table>
    <div class="settings-section">
      <div class="section-title">選取來源資料夾的流程</div>
      <el-radio-group
        :model-value="files.folderPickBehavior"
        aria-label="選取資料夾流程"
        @update:model-value="setBehavior"
      >
        <el-radio value="rememberLast">記住上次的 profile，選完資料夾直接到預覽</el-radio>
        <el-radio value="alwaysWizard">每次都走完整流程</el-radio>
      </el-radio-group>
    </div>
    <ProfileEditorDialog
      v-model="editorVisible"
      :profile="editorSource"
      :existing="profiles"
      @save="onSave"
    />
    <el-dialog
      v-model="transferVisible"
      :title="transferKind === 'export' ? '匯出 profile' : '匯入 profile'"
      width="560px"
      append-to-body
    >
      <p class="muted">
        {{
          transferKind === "export"
            ? "自訂 profile 的 JSON。內建 profile 不會匯出。"
            : "貼上匯出的 JSON，或選擇檔案載入。同名 profile 會自動改名。"
        }}
      </p>
      <el-input
        v-model="transferText"
        type="textarea"
        :rows="10"
        :readonly="transferKind === 'export'"
        aria-label="profile JSON"
      />
      <input
        v-if="transferKind === 'import'"
        type="file"
        accept=".json,application/json"
        class="import-file"
        @change="onImportFile"
      />
      <template #footer>
        <el-button @click="transferVisible = false">關閉</el-button>
        <el-button v-if="transferKind === 'export'" type="primary" @click="copyExport"
          >複製</el-button
        >
        <el-button v-else type="primary" :disabled="!transferText.trim()" @click="runImport"
          >匯入</el-button
        >
      </template>
    </el-dialog>
  </div>
</template>

<style scoped>
.profiles-head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 8px;
}
.builtin-tag {
  margin-left: 6px;
}
.import-file {
  margin-top: 8px;
}
.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0 0 0 0);
}
</style>
