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
  profileDisplayName,
} from "../lib/filterProfiles";
import { t, tn } from "@/i18n";
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
    ElMessage.success(t("components.filterProfiles.copied"));
  } catch {
    ElMessage.warning(t("components.filterProfiles.copyFailed"));
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
  ElMessage.success(tn("components.filterProfiles.imported", result.profiles.length));
}

function setBehavior(value: unknown): void {
  props.files.folderPickBehavior = value as FolderPickBehavior;
}
</script>

<template>
  <div class="filter-profiles" data-testid="filter-profiles">
    <div class="profiles-head">
      <div class="section-title">{{ t("components.filterProfiles.title") }}</div>
      <div class="profiles-actions">
        <el-button @click="create">{{ t("components.filterProfiles.create") }}</el-button>
        <el-button @click="openImport">{{ t("components.filterProfiles.import") }}</el-button>
        <el-button @click="openExport">{{ t("components.filterProfiles.export") }}</el-button>
      </div>
    </div>
    <el-table :data="profiles" row-key="id" class="profiles-table" size="small">
      <el-table-column :label="t('components.filterProfiles.default')" width="70">
        <template #default="{ row }">
          <el-radio
            :model-value="files.defaultFilterProfileId"
            :value="row.id"
            :aria-label="
              t('components.filterProfiles.setDefaultAria', { name: profileDisplayName(row) })
            "
            @update:model-value="setDefault(row)"
            ><span class="sr-only">{{ t("components.filterProfiles.default") }}</span></el-radio
          >
        </template>
      </el-table-column>
      <el-table-column :label="t('components.filterProfiles.name')" min-width="140">
        <template #default="{ row }"
          >{{ profileDisplayName(row)
          }}<el-tag disable-transitions v-if="row.builtin" size="small" class="builtin-tag">{{
            t("components.filterProfiles.builtin")
          }}</el-tag>
        </template>
      </el-table-column>
      <el-table-column :label="t('components.filterProfiles.mode')" width="90">
        <template #default="{ row }"
          ><el-tag
            disable-transitions
            size="small"
            :type="row.mode === 'deny' ? 'danger' : 'success'"
            >{{ modeLabel(row.mode) }}</el-tag
          ></template
        >
      </el-table-column>
      <el-table-column :label="t('components.filterProfiles.extensions')" min-width="200">
        <template #default="{ row }">{{ summarizeProfile(row) }}</template>
      </el-table-column>
      <el-table-column :label="t('components.filterProfiles.actions')" width="180">
        <template #default="{ row }">
          <el-button text type="primary" @click="edit(row)">{{
            t("components.filterProfiles.edit")
          }}</el-button>
          <el-button text type="primary" @click="copy(row)">{{
            t("components.filterProfiles.copy")
          }}</el-button>
          <el-button v-if="!row.builtin" text type="danger" @click="remove(row)">{{
            t("common.delete")
          }}</el-button>
        </template>
      </el-table-column>
    </el-table>
    <div class="settings-section">
      <div class="section-title">{{ t("components.filterProfiles.folderPickTitle") }}</div>
      <el-radio-group
        :model-value="files.folderPickBehavior"
        :aria-label="t('components.filterProfiles.folderPickAria')"
        @update:model-value="setBehavior"
      >
        <el-radio value="rememberLast">{{ t("components.filterProfiles.rememberLast") }}</el-radio>
        <el-radio value="alwaysWizard">{{ t("components.filterProfiles.alwaysWizard") }}</el-radio>
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
      :title="
        transferKind === 'export'
          ? t('components.filterProfiles.exportTitle')
          : t('components.filterProfiles.importTitle')
      "
      width="560px"
      append-to-body
    >
      <p class="muted">
        {{
          transferKind === "export"
            ? t("components.filterProfiles.exportHint")
            : t("components.filterProfiles.importHint")
        }}
      </p>
      <el-input
        v-model="transferText"
        type="textarea"
        :rows="10"
        :readonly="transferKind === 'export'"
        :aria-label="t('components.filterProfiles.jsonAria')"
      />
      <input
        v-if="transferKind === 'import'"
        type="file"
        accept=".json,application/json"
        class="import-file"
        @change="onImportFile"
      />
      <template #footer>
        <el-button @click="transferVisible = false">{{ t("common.close") }}</el-button>
        <el-button v-if="transferKind === 'export'" type="primary" @click="copyExport">{{
          t("components.filterProfiles.copy")
        }}</el-button>
        <el-button v-else type="primary" :disabled="!transferText.trim()" @click="runImport">{{
          t("components.filterProfiles.import")
        }}</el-button>
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
