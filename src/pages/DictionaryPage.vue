<script setup lang="ts">
import { computed, h, reactive, ref, shallowRef, watch } from "vue";
import { confirm, open, save as chooseSavePath } from "@tauri-apps/plugin-dialog";
import { ElButton, ElCheckbox, ElInput, ElMessage, ElOption, ElSelect } from "element-plus";
import type { CheckboxValueType, Column, TableV2Instance } from "element-plus";
import type {
  DictionaryImportPreview,
  DictionaryImportRow,
  UserDictionaryDirection,
  UserDictionaryEntry,
  UserDictionaryReadResult,
} from "@shared/contracts";
import { core } from "../lib/coreClient";
import { formatUnknownError } from "../lib/errors";
import {
  DICTIONARY_FILE_EXTENSIONS,
  findConflicts,
  MANUAL_SOURCE,
  matchesFilter,
  mergeImport,
  type DictionaryFilter,
} from "../lib/userDictionary";
import { locale, t, tn } from "@/i18n";

defineOptions({ name: "DictionaryPage" });

interface Row extends UserDictionaryEntry {
  key: number;
}

const directions: UserDictionaryDirection[] = ["both", "s2t", "t2s"];

function directionLabel(direction: UserDictionaryDirection): string {
  return t(`dictionary.direction.${direction}`);
}

function statusLabel(status: DictionaryImportRow["status"]): string {
  return t(`dictionary.status.${status}`);
}

function fileFilters() {
  return [{ name: t("dictionary.fileFilterName"), extensions: DICTIONARY_FILE_EXTENSIONS }];
}

const path = ref("");
const loadError = ref("");
/** 每列是 reactive 物件：編輯只重繪該格，不觸發整表重新篩選。 */
const rows = shallowRef<Row[]>([]);
const filter = reactive<DictionaryFilter>({ keyword: "", direction: "", source: "" });
const visibleRows = shallowRef<Row[]>([]);
const sourceOptions = shallowRef<string[]>([]);
const revision = ref(0);
const savedRevision = ref(0);
const busy = ref(false);
const importPreview = ref<DictionaryImportPreview>();
const importVisible = ref(false);
const table = ref<TableV2Instance>();
let nextKey = 0;

/** 勾選的列；只保留目前篩選結果中的列，刪除時不會動到看不到的詞條。 */
const selected = reactive(new Set<number>());
const selectedCount = computed(() => selected.size);
const allVisibleSelected = computed(
  () => visibleRows.value.length > 0 && selected.size === visibleRows.value.length,
);

const dirty = computed(() => revision.value !== savedRevision.value);
const skippedImportRows = computed(
  () => importPreview.value?.rows.filter((row) => row.status !== "importable") ?? [],
);

function toRows(entries: UserDictionaryEntry[]): Row[] {
  return entries.map((entry) => reactive({ ...entry, key: nextKey++ }));
}

function entries(): UserDictionaryEntry[] {
  return rows.value.map(({ simplified, traditional, direction, importedAt, sourceFile }) => ({
    simplified,
    traditional,
    direction,
    ...(importedAt ? { importedAt } : {}),
    ...(sourceFile ? { sourceFile } : {}),
  }));
}

/** 只在篩選條件或列數變動時重算；編輯中的列不會因不再符合關鍵字而消失。 */
function refreshView() {
  visibleRows.value = rows.value.filter((row) => matchesFilter(row, filter));
  if (selected.size) {
    const visible = new Set(visibleRows.value.map((row) => row.key));
    for (const key of [...selected]) if (!visible.has(key)) selected.delete(key);
  }
  sourceOptions.value = [
    ...new Set(rows.value.flatMap((row) => (row.sourceFile ? [row.sourceFile] : []))),
  ].sort((a, b) => a.localeCompare(b, "zh-Hant"));
  if (filter.source && filter.source !== MANUAL_SOURCE) {
    if (!sourceOptions.value.includes(filter.source)) filter.source = "";
  }
}

watch(filter, refreshView);

function replaceRows(next: Row[], edited: boolean) {
  rows.value = next;
  if (edited) revision.value += 1;
  refreshView();
}

function touch() {
  revision.value += 1;
}

function formatImportedAt(value?: string): string {
  if (!value) return "";
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : date.toLocaleString(locale.value, { hour12: false });
}

function setAllVisibleSelected(value: boolean) {
  selected.clear();
  if (value) for (const row of visibleRows.value) selected.add(row.key);
}

function setSelected(row: Row, value: boolean) {
  if (value) selected.add(row.key);
  else selected.delete(row.key);
}

const columns = computed<Column<Row>[]>(() => [
  {
    key: "selected",
    title: t("dictionary.column.selected"),
    width: 48,
    align: "center",
    headerCellRenderer: () =>
      h(ElCheckbox, {
        modelValue: allVisibleSelected.value,
        indeterminate: selected.size > 0 && !allVisibleSelected.value,
        disabled: visibleRows.value.length === 0,
        "aria-label": t("dictionary.column.selectAllVisible"),
        "onUpdate:modelValue": (value: CheckboxValueType) => setAllVisibleSelected(Boolean(value)),
      }),
    cellRenderer: ({ rowData }) =>
      h(ElCheckbox, {
        modelValue: selected.has(rowData.key),
        "aria-label": t("dictionary.column.selectRow"),
        "onUpdate:modelValue": (value: CheckboxValueType) => setSelected(rowData, Boolean(value)),
      }),
  },
  {
    key: "simplified",
    title: t("dictionary.column.simplified"),
    width: 200,
    flexGrow: 1,
    cellRenderer: ({ rowData }) =>
      h(ElInput, {
        modelValue: rowData.simplified,
        "onUpdate:modelValue": (value: string) => {
          rowData.simplified = value;
          touch();
        },
      }),
  },
  {
    key: "traditional",
    title: t("dictionary.column.traditional"),
    width: 200,
    flexGrow: 1,
    cellRenderer: ({ rowData }) =>
      h(ElInput, {
        modelValue: rowData.traditional,
        "onUpdate:modelValue": (value: string) => {
          rowData.traditional = value;
          touch();
        },
      }),
  },
  {
    key: "direction",
    title: t("dictionary.column.direction"),
    width: 150,
    cellRenderer: ({ rowData }) =>
      h(
        ElSelect,
        {
          modelValue: rowData.direction,
          "onUpdate:modelValue": (value: UserDictionaryDirection) => {
            rowData.direction = value;
            touch();
          },
        },
        () =>
          directions.map((value) =>
            h(ElOption, { key: value, value, label: directionLabel(value) }),
          ),
      ),
  },
  {
    key: "source",
    title: t("dictionary.column.source"),
    width: 220,
    flexGrow: 1,
    cellRenderer: ({ rowData }) =>
      rowData.sourceFile
        ? h("div", { class: "dictionary-source", title: rowData.sourceFile }, [
            h("span", rowData.sourceFile),
            h("small", formatImportedAt(rowData.importedAt)),
          ])
        : h("span", { class: "dictionary-source-manual" }, t("dictionary.manualSource")),
  },
  {
    key: "actions",
    title: t("dictionary.column.actions"),
    width: 72,
    cellRenderer: ({ rowData }) =>
      h(ElButton, { link: true, type: "danger", onClick: () => void removeEntry(rowData) }, () =>
        t("common.delete"),
      ),
  },
]);

async function load() {
  busy.value = true;
  try {
    const result = await core.request<UserDictionaryReadResult>("userDictionary.read", {});
    path.value = result.path;
    loadError.value = result.loadError ?? "";
    replaceRows(toRows(result.entries), false);
    savedRevision.value = revision.value;
  } catch (error) {
    ElMessage.error(formatUnknownError(error));
  } finally {
    busy.value = false;
  }
}

function addEntry() {
  Object.assign(filter, { keyword: "", direction: "", source: "" });
  const row = reactive<Row>({
    key: nextKey++,
    simplified: "",
    traditional: "",
    direction: "both",
  });
  replaceRows([row, ...rows.value], true);
  table.value?.scrollToTop(0);
}

async function confirmRemoval(message: string): Promise<boolean> {
  return confirm(message, {
    title: t("dictionary.removal.title"),
    kind: "warning",
  });
}

async function removeEntry(row: Row) {
  const label =
    [row.simplified, row.traditional].filter(Boolean).join("／") ||
    t("dictionary.removal.emptyEntry");
  if (!(await confirmRemoval(t("dictionary.removal.confirmOne", { label })))) return;
  selected.delete(row.key);
  replaceRows(
    rows.value.filter((candidate) => candidate.key !== row.key),
    true,
  );
}

async function removeSelected() {
  const count = selected.size;
  if (!count) return;
  const message = tn("dictionary.removal.confirmSelected", count, {
    count: count.toLocaleString(),
  });
  if (!(await confirmRemoval(message))) return;
  const keys = new Set(selected);
  selected.clear();
  replaceRows(
    rows.value.filter((row) => !keys.has(row.key)),
    true,
  );
  ElMessage.success(tn("dictionary.removal.removed", count));
}

function conflictMessage(values: UserDictionaryEntry[]): string | undefined {
  const conflicts = findConflicts(values);
  if (!conflicts.length) return undefined;
  return t("dictionary.conflict", {
    list: conflicts.slice(0, 5).join(t("dictionary.listSeparator")),
  });
}

/** 寫入 user-dictionary.txt 並重新載入轉換引擎；呼叫前須已取得使用者確認。 */
async function persist(values: UserDictionaryEntry[]): Promise<boolean> {
  const savingRevision = revision.value;
  busy.value = true;
  try {
    await core.request("userDictionary.save", { entries: values });
    savedRevision.value = savingRevision;
    loadError.value = "";
    return true;
  } catch (error) {
    ElMessage.error(formatUnknownError(error));
    return false;
  } finally {
    busy.value = false;
  }
}

async function save() {
  const values = entries();
  const conflict = conflictMessage(values);
  if (conflict) {
    ElMessage.error(conflict);
    return;
  }
  if (
    !(await confirm(tn("dictionary.save.confirm", values.length), {
      title: t("dictionary.save.confirmTitle"),
      kind: "warning",
    }))
  )
    return;
  if (await persist(values)) ElMessage.success(tn("dictionary.save.saved", values.length));
}

async function chooseImportFile() {
  const selected = await open({ multiple: false, filters: fileFilters() });
  if (!selected || Array.isArray(selected)) return;
  busy.value = true;
  let preview: DictionaryImportPreview;
  try {
    preview = await core.request<DictionaryImportPreview>("userDictionary.previewImport", {
      path: selected,
    });
  } catch (error) {
    ElMessage.error(formatUnknownError(error));
    return;
  } finally {
    busy.value = false;
  }
  if (
    preview.format === "legacy" &&
    !(await confirm(
      t("dictionary.importDialog.legacyConfirm", { sourceFile: preview.sourceFile }),
      { title: t("dictionary.importDialog.legacyTitle"), kind: "warning" },
    ))
  )
    return;
  importPreview.value = preview;
  importVisible.value = true;
}

async function applyImport() {
  const preview = importPreview.value;
  if (!preview) return;
  const result = mergeImport(entries(), preview);
  const conflict = conflictMessage(result.entries);
  if (conflict) {
    ElMessage.error(t("dictionary.importDialog.conflict", { conflict }));
    return;
  }
  const savingEdits = dirty.value;
  replaceRows(toRows(result.entries), true);
  importVisible.value = false;
  if (!(await persist(result.entries))) {
    ElMessage.warning(t("dictionary.importDialog.notSaved"));
    return;
  }
  const notes = [tn("dictionary.importDialog.added", result.added)];
  if (result.existing) notes.push(tn("dictionary.importDialog.existing", result.existing));
  if (result.conflicts) notes.push(tn("dictionary.importDialog.conflicts", result.conflicts));
  if (savingEdits) notes.push(t("dictionary.importDialog.savedEdits"));
  ElMessage.success(
    t("dictionary.importDialog.done", {
      notes: notes.join(t("dictionary.importDialog.noteSeparator")),
    }),
  );
}

async function exportDictionary() {
  const values = entries();
  const conflict = conflictMessage(values);
  if (conflict) {
    ElMessage.error(conflict);
    return;
  }
  const target = await chooseSavePath({
    defaultPath: t("dictionary.defaultExportName"),
    filters: fileFilters(),
  });
  if (!target) return;
  busy.value = true;
  try {
    await core.request("userDictionary.export", { path: target, entries: values });
    ElMessage.success(
      tn(dirty.value ? "dictionary.exportedWithUnsaved" : "dictionary.exported", values.length),
    );
  } catch (error) {
    ElMessage.error(formatUnknownError(error));
  } finally {
    busy.value = false;
  }
}

void load();
</script>

<template>
  <section class="page-stack">
    <header class="page-header">
      <div>
        <p class="eyebrow">USER DICTIONARY</p>
        <h1>{{ t("dictionary.title") }}</h1>
        <p>{{ t("dictionary.subtitle") }}</p>
      </div>
      <div class="header-actions">
        <el-button :loading="busy" @click="chooseImportFile">{{ t("dictionary.import") }}</el-button
        ><el-button :disabled="busy" @click="exportDictionary">{{
          t("dictionary.export")
        }}</el-button
        ><el-button @click="addEntry">{{ t("dictionary.add") }}</el-button
        ><el-button type="danger" plain :disabled="!selectedCount" @click="removeSelected">{{
          selectedCount
            ? t("dictionary.deleteSelectedCount", { count: selectedCount.toLocaleString() })
            : t("dictionary.deleteSelected")
        }}</el-button
        ><el-button :disabled="!dirty" @click="load">{{ t("dictionary.resetChanges") }}</el-button
        ><el-button type="primary" :disabled="!dirty" :loading="busy" @click="save">{{
          t("common.save")
        }}</el-button>
      </div>
    </header>
    <el-alert :title="t('dictionary.info')" type="info" :closable="false" />
    <el-alert
      v-if="loadError"
      :title="t('dictionary.loadError', { error: loadError })"
      type="error"
      :closable="false"
    />
    <el-card shadow="never"
      ><div class="dictionary-toolbar">
        <el-input
          v-model="filter.keyword"
          clearable
          :placeholder="t('dictionary.filter.search')"
        /><el-select
          v-model="filter.direction"
          :aria-label="t('dictionary.filter.directionLabel')"
          :placeholder="t('dictionary.filter.allDirections')"
          ><el-option :label="t('dictionary.filter.allDirections')" value="" /><el-option
            v-for="value in directions"
            :key="value"
            :label="directionLabel(value)"
            :value="value" /></el-select
        ><el-select
          v-model="filter.source"
          :aria-label="t('dictionary.filter.sourceLabel')"
          :placeholder="t('dictionary.filter.allSources')"
          filterable
          ><el-option :label="t('dictionary.filter.allSources')" value="" /><el-option
            :label="t('dictionary.manualSource')"
            :value="MANUAL_SOURCE" /><el-option
            v-for="source in sourceOptions"
            :key="source"
            :label="source"
            :value="source" /></el-select
        ><span>{{
          t("dictionary.filter.rowCount", {
            visible: visibleRows.length.toLocaleString(),
            total: rows.length.toLocaleString(),
          })
        }}</span>
      </div>
      <div class="path-label">{{ path }}</div></el-card
    >
    <el-card shadow="never" class="page-fill-main dictionary-table-card"
      ><div v-loading="busy" class="dictionary-table-host">
        <el-auto-resizer>
          <template #default="{ height, width }">
            <el-table-v2
              ref="table"
              :columns="columns"
              :data="visibleRows"
              :width="width"
              :height="height"
              :row-height="48"
              :header-height="40"
              row-key="key"
            />
          </template>
        </el-auto-resizer></div
    ></el-card>
    <el-dialog v-model="importVisible" :title="t('dictionary.importDialog.title')" width="720px">
      <template v-if="importPreview">
        <p class="path-label">{{ importPreview.path }}</p>
        <p>
          {{
            t("dictionary.importDialog.summary", {
              format:
                importPreview.format === "legacy"
                  ? t("dictionary.importDialog.formatLegacy")
                  : t("dictionary.importDialog.formatUser"),
              total: importPreview.total,
              importable: importPreview.importable,
              disabled: importPreview.disabled,
              invalid: importPreview.invalid,
              duplicate: importPreview.duplicate,
            })
          }}
        </p>
        <p>
          {{ t("dictionary.importDialog.note", { sourceFile: importPreview.sourceFile }) }}
        </p>
        <el-alert
          v-if="dirty"
          :title="t('dictionary.importDialog.unsavedWarning')"
          type="warning"
          :closable="false"
        />
        <el-table v-if="skippedImportRows.length" :data="skippedImportRows" max-height="320"
          ><el-table-column
            prop="line"
            :label="t('dictionary.column.line')"
            width="70" /><el-table-column
            prop="simplified"
            :label="t('dictionary.column.simplified')" /><el-table-column
            prop="traditional"
            :label="t('dictionary.column.traditional')" /><el-table-column
            :label="t('dictionary.column.status')"
            width="100"
            ><template #default="scope">{{
              statusLabel(scope.row.status as DictionaryImportRow["status"])
            }}</template></el-table-column
          ><el-table-column prop="reason" :label="t('dictionary.column.reason')" min-width="200"
        /></el-table>
      </template>
      <template #footer
        ><el-button @click="importVisible = false">{{ t("common.cancel") }}</el-button
        ><el-button
          type="primary"
          :loading="busy"
          :disabled="!importPreview?.importable"
          @click="applyImport"
          >{{ t("dictionary.importDialog.confirm") }}</el-button
        ></template
      >
    </el-dialog>
  </section>
</template>
