<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import type { ExtensionFilterMode, ExtensionStats, FileFilterProfile } from "@shared/contracts";
import { core, isCancellationError } from "../lib/coreClient";
import {
  mergeExtensions,
  newProfileId,
  noExtensionLabel as noExtensionRowLabel,
  validateProfile,
  profileDisplayName,
} from "../lib/filterProfiles";
import { t, tn } from "@/i18n";

const props = defineProps<{
  modelValue: boolean;
  /** 編輯或新增的來源。內建 profile 不會被改動，儲存時另存為自訂 profile。 */
  profile: FileFilterProfile | null;
  existing: FileFilterProfile[];
  /** 掃描目錄的起始路徑。 */
  scanStartPath?: string;
}>();
const emit = defineEmits<{
  (event: "update:modelValue", value: boolean): void;
  (event: "save", profile: FileFilterProfile): void;
}>();

const draft = ref<FileFilterProfile>(blankDraft());
const tab = ref<"typed" | "scan">("typed");
const typedText = ref("");
const duplicateNotice = ref<string[]>([]);
const invalidNotice = ref<string[]>([]);
const allFilesNotice = ref(false);
const scanPath = ref("");
const scanRecursive = ref(true);
const scanning = ref(false);
const scanProgress = ref("");
const scanStats = ref<ExtensionStats>();
const scanRequestId = ref<string>();

function blankDraft(): FileFilterProfile {
  return {
    id: "",
    name: "",
    builtin: false,
    mode: "allow",
    extensions: [],
    includeNoExtension: false,
  };
}

watch(
  () => [props.modelValue, props.profile] as const,
  ([visible, profile]) => {
    if (!visible || !profile) return;
    draft.value = {
      ...profile,
      extensions: [...profile.extensions],
      // 內建 profile 不可改動：以副本編輯，儲存時另存。
      name: profile.builtin
        ? t("components.profileEditor.copyName", { name: profileDisplayName(profile) })
        : profile.name,
    };
    tab.value = "typed";
    typedText.value = "";
    duplicateNotice.value = [];
    invalidNotice.value = [];
    allFilesNotice.value = false;
    scanPath.value = props.scanStartPath ?? "";
    scanStats.value = undefined;
    scanProgress.value = "";
  },
  { immediate: true },
);

const isBuiltinSource = computed(() => Boolean(props.profile?.builtin));
const others = computed(() => props.existing.filter((item) => item.id !== props.profile?.id));
const validation = computed(() =>
  validateProfile(draft.value, isBuiltinSource.value ? props.existing : others.value),
);
const dialogTitle = computed(() => {
  if (isBuiltinSource.value)
    return t("components.profileEditor.saveBuiltinAs", {
      name: props.profile ? profileDisplayName(props.profile) : "",
    });
  return props.profile && props.existing.some((item) => item.id === props.profile?.id)
    ? t("components.profileEditor.editTitle")
    : t("components.profileEditor.createTitle");
});
const noExtensionLabel = computed(() =>
  draft.value.mode === "deny"
    ? t("components.profileEditor.excludeNoExtension")
    : t("components.profileEditor.includeNoExtension"),
);

function commitTyped(): void {
  if (!typedText.value.trim()) return;
  const merged = mergeExtensions(draft.value.extensions, typedText.value);
  draft.value.extensions = merged.extensions;
  duplicateNotice.value = merged.duplicates;
  invalidNotice.value = merged.invalid;
  allFilesNotice.value = merged.hasAllFilesToken;
  typedText.value = "";
}

function onTypedInput(value: string): void {
  typedText.value = value;
  if (/[,;，；、\s]/u.test(value)) commitTyped();
}

function removeExtension(extension: string): void {
  draft.value.extensions = draft.value.extensions.filter((item) => item !== extension);
}

function setMode(mode: ExtensionFilterMode): void {
  draft.value.mode = mode;
  if (mode === "all") draft.value.includeNoExtension = true;
}

async function chooseScanFolder(): Promise<void> {
  const selected = await open({
    directory: true,
    multiple: false,
    defaultPath: scanPath.value || undefined,
  });
  if (!selected || Array.isArray(selected)) return;
  scanPath.value = selected;
  await scan();
}

async function scan(): Promise<void> {
  if (!scanPath.value || scanning.value) return;
  scanning.value = true;
  scanProgress.value = "";
  scanStats.value = undefined;
  try {
    scanStats.value = await core.request<ExtensionStats>(
      "files.scanExtensions",
      { path: scanPath.value, recursive: scanRecursive.value },
      {
        onProgress: (value) => {
          scanProgress.value = value.message;
        },
        onRequestId: (id) => {
          scanRequestId.value = id;
        },
      },
    );
  } catch (error) {
    if (!isCancellationError(error))
      ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    scanning.value = false;
    scanRequestId.value = undefined;
  }
}

async function cancelScan(): Promise<void> {
  if (scanRequestId.value) await core.cancel(scanRequestId.value).catch(() => false);
}

const scanRows = computed(() => {
  const stats = scanStats.value;
  if (!stats) return [];
  const rows: Array<{ key: string; label: string; count: number; extension: string | null }> =
    stats.extensions.map((item) => ({
      key: item.extension,
      label: item.extension,
      count: item.count,
      extension: item.extension,
    }));
  if (stats.noExtensionCount > 0) {
    rows.push({
      key: "",
      label: noExtensionRowLabel(),
      count: stats.noExtensionCount,
      extension: null,
    });
  }
  return rows;
});

function isChecked(extension: string | null): boolean {
  return extension === null
    ? draft.value.includeNoExtension
    : draft.value.extensions.includes(extension);
}

function setChecked(extension: string | null, checked: boolean): void {
  if (extension === null) {
    draft.value.includeNoExtension = checked;
  } else if (checked) {
    if (!draft.value.extensions.includes(extension)) draft.value.extensions.push(extension);
  } else {
    removeExtension(extension);
  }
}

function setAllChecked(checked: boolean): void {
  for (const row of scanRows.value) setChecked(row.extension, checked);
}

function invertChecked(): void {
  for (const row of scanRows.value) setChecked(row.extension, !isChecked(row.extension));
}

function close(): void {
  void cancelScan();
  emit("update:modelValue", false);
}

function save(): void {
  commitTyped();
  if (validation.value.error) return;
  const result: FileFilterProfile = {
    ...draft.value,
    name: draft.value.name.trim(),
    id: isBuiltinSource.value || !draft.value.id ? newProfileId() : draft.value.id,
    builtin: false,
    extensions: draft.value.mode === "all" ? [] : [...draft.value.extensions],
  };
  emit("save", result);
  emit("update:modelValue", false);
}
</script>

<template>
  <el-dialog
    :model-value="modelValue"
    :title="dialogTitle"
    width="640px"
    class="profile-editor-dialog"
    append-to-body
    :close-on-click-modal="false"
    @update:model-value="(value: boolean) => (value ? undefined : close())"
  >
    <el-form label-position="top" @submit.prevent>
      <el-form-item :label="t('components.profileEditor.name')">
        <el-input
          v-model="draft.name"
          :aria-label="t('components.profileEditor.nameAria')"
          maxlength="40"
        />
      </el-form-item>
      <el-form-item :label="t('components.profileEditor.mode')">
        <el-radio-group
          :model-value="draft.mode"
          :aria-label="t('components.profileEditor.modeAria')"
          @update:model-value="(value: unknown) => setMode(value as ExtensionFilterMode)"
        >
          <el-radio-button value="allow">{{ t("components.profileEditor.allow") }}</el-radio-button>
          <el-radio-button value="deny">{{ t("components.profileEditor.deny") }}</el-radio-button>
          <el-radio-button value="all">{{ t("components.profileEditor.all") }}</el-radio-button>
        </el-radio-group>
      </el-form-item>
      <p v-if="draft.mode === 'all'" class="profile-hint">
        {{ t("components.profileEditor.allHint") }}
      </p>
      <template v-else>
        <el-tabs v-model="tab" class="profile-tabs">
          <el-tab-pane :label="t('components.profileEditor.typedTab')" name="typed">
            <div class="ext-tags" :aria-label="t('components.profileEditor.extensionsAria')">
              <el-tag
                disable-transitions
                v-for="extension in draft.extensions"
                :key="extension"
                closable
                @close="removeExtension(extension)"
                >{{ extension }}</el-tag
              >
              <el-input
                :model-value="typedText"
                class="ext-input"
                :placeholder="t('components.profileEditor.typedPlaceholder')"
                :aria-label="t('components.profileEditor.addExtensionAria')"
                @update:model-value="onTypedInput"
                @keydown.enter.prevent="commitTyped"
                @blur="commitTyped"
              />
            </div>
          </el-tab-pane>
          <el-tab-pane :label="t('components.profileEditor.scanTab')" name="scan">
            <div class="scan-bar">
              <el-button :disabled="scanning" @click="chooseScanFolder">{{
                t("components.profileEditor.chooseFolder")
              }}</el-button>
              <el-checkbox v-model="scanRecursive" :disabled="scanning">{{
                t("components.profileEditor.recursive")
              }}</el-checkbox>
              <el-button v-if="scanPath && !scanning" text type="primary" @click="scan">{{
                t("components.profileEditor.rescan")
              }}</el-button>
              <el-button v-if="scanning" text type="danger" @click="cancelScan">{{
                t("common.cancel")
              }}</el-button>
              <span class="scan-path">{{
                scanPath || t("components.profileEditor.noFolder")
              }}</span>
            </div>
            <p v-if="scanning" class="profile-hint">
              {{ scanProgress || t("components.profileEditor.scanning") }}
            </p>
            <template v-else-if="scanStats">
              <div class="scan-actions">
                <el-button size="small" @click="setAllChecked(true)">{{
                  t("components.profileEditor.selectAll")
                }}</el-button>
                <el-button size="small" @click="setAllChecked(false)">{{
                  t("components.profileEditor.selectNone")
                }}</el-button>
                <el-button size="small" @click="invertChecked">{{
                  t("components.profileEditor.invert")
                }}</el-button>
                <span class="profile-hint">{{
                  tn("components.profileEditor.totalFiles", scanStats.totalFiles)
                }}</span>
              </div>
              <p v-if="!scanRows.length" class="profile-hint">
                {{ t("components.profileEditor.noFiles") }}
              </p>
              <ul class="scan-list">
                <li v-for="row in scanRows" :key="row.key">
                  <el-checkbox
                    :model-value="isChecked(row.extension)"
                    @update:model-value="
                      (value: unknown) => setChecked(row.extension, Boolean(value))
                    "
                    >{{ row.label }}</el-checkbox
                  >
                  <span class="scan-count">{{ row.count }}</span>
                </li>
              </ul>
            </template>
          </el-tab-pane>
        </el-tabs>
        <p v-if="duplicateNotice.length" class="profile-warning" role="status">
          {{
            t("components.profileEditor.duplicates", {
              items: duplicateNotice.join(t("components.profileEditor.listSeparator")),
            })
          }}
        </p>
        <p v-if="invalidNotice.length" class="profile-error" role="alert">
          {{
            t("components.profileEditor.invalid", {
              items: invalidNotice.join(t("components.profileEditor.listSeparator")),
            })
          }}
        </p>
        <p v-if="allFilesNotice" class="profile-warning" role="status">
          {{ t("components.profileEditor.allFilesToken") }}
        </p>
        <el-checkbox v-model="draft.includeNoExtension">{{ noExtensionLabel }}</el-checkbox>
        <p class="profile-hint">
          {{ t("components.profileEditor.noExtensionHint")
          }}{{
            draft.mode === "deny"
              ? t("components.profileEditor.denyNoExtensionHint")
              : t("components.profileEditor.allowNoExtensionHint")
          }}
        </p>
      </template>
      <p v-if="validation.error" class="profile-error" role="alert">{{ validation.error }}</p>
      <p v-else-if="validation.warning" class="profile-warning" role="status">
        {{ validation.warning }}
      </p>
      <p v-if="isBuiltinSource" class="profile-hint">
        {{ t("components.profileEditor.builtinHint") }}
      </p>
    </el-form>
    <template #footer>
      <el-button @click="close">{{ t("common.cancel") }}</el-button>
      <el-button type="primary" :disabled="Boolean(validation.error)" @click="save">{{
        t("common.save")
      }}</el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.ext-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 8px;
  border: 1px solid #d7e4e0;
  border-radius: 8px;
  min-height: 56px;
}
.ext-input {
  flex: 1 1 220px;
}
.scan-bar,
.scan-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
}
.scan-path {
  color: #5b746f;
  font-size: 12px;
  word-break: break-all;
}
.scan-list {
  list-style: none;
  margin: 0;
  padding: 0;
  max-height: 220px;
  overflow: auto;
  border: 1px solid #d7e4e0;
  border-radius: 8px;
}
.scan-list li {
  display: flex;
  justify-content: space-between;
  padding: 2px 12px;
  border-bottom: 1px solid #eef3f1;
}
.scan-count {
  color: #5b746f;
  font-variant-numeric: tabular-nums;
}
.profile-hint {
  margin: 6px 0;
  color: #5b746f;
  font-size: 13px;
}
.profile-warning {
  margin: 6px 0;
  color: #b7791f;
  font-size: 13px;
}
.profile-error {
  margin: 6px 0;
  color: #b42318;
  font-size: 13px;
}
</style>
