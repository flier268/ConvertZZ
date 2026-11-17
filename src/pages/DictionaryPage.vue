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

defineOptions({ name: "DictionaryPage" });

interface Row extends UserDictionaryEntry {
  key: number;
}

const directionLabels: Record<UserDictionaryDirection, string> = {
  both: "雙向",
  s2t: "只用於簡轉繁",
  t2s: "只用於繁轉簡",
};

const statusLabels: Record<DictionaryImportRow["status"], string> = {
  importable: "可匯入",
  disabled: "已停用",
  invalid: "無法匯入",
  duplicate: "重複",
};

const fileFilters = [{ name: "字典檔", extensions: DICTIONARY_FILE_EXTENSIONS }];

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
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString("zh-TW", { hour12: false });
}

function setAllVisibleSelected(value: boolean) {
  selected.clear();
  if (value) for (const row of visibleRows.value) selected.add(row.key);
}

function setSelected(row: Row, value: boolean) {
  if (value) selected.add(row.key);
  else selected.delete(row.key);
}

const columns: Column<Row>[] = [
  {
    key: "selected",
    title: "選取",
    width: 48,
    align: "center",
    headerCellRenderer: () =>
      h(ElCheckbox, {
        modelValue: allVisibleSelected.value,
        indeterminate: selected.size > 0 && !allVisibleSelected.value,
        disabled: visibleRows.value.length === 0,
        "aria-label": "全選篩選結果",
        "onUpdate:modelValue": (value: CheckboxValueType) => setAllVisibleSelected(Boolean(value)),
      }),
    cellRenderer: ({ rowData }) =>
      h(ElCheckbox, {
        modelValue: selected.has(rowData.key),
        "aria-label": "選取此列",
        "onUpdate:modelValue": (value: CheckboxValueType) => setSelected(rowData, Boolean(value)),
      }),
  },
  {
    key: "simplified",
    title: "簡體",
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
    title: "繁體",
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
    title: "方向",
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
          Object.entries(directionLabels).map(([value, label]) =>
            h(ElOption, { key: value, value, label }),
          ),
      ),
  },
  {
    key: "source",
    title: "匯入來源",
    width: 220,
    flexGrow: 1,
    cellRenderer: ({ rowData }) =>
      rowData.sourceFile
        ? h("div", { class: "dictionary-source", title: rowData.sourceFile }, [
            h("span", rowData.sourceFile),
            h("small", formatImportedAt(rowData.importedAt)),
          ])
        : h("span", { class: "dictionary-source-manual" }, "手動加入"),
  },
  {
    key: "actions",
    title: "操作",
    width: 72,
    cellRenderer: ({ rowData }) =>
      h(
        ElButton,
        { link: true, type: "danger", onClick: () => void removeEntry(rowData) },
        () => "刪除",
      ),
  },
];

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
  return confirm(`${message}按「儲存」後才會寫入詞典；儲存前可按「重設未儲存變更」復原。`, {
    title: "確認刪除詞條",
    kind: "warning",
  });
}

async function removeEntry(row: Row) {
  const label = [row.simplified, row.traditional].filter(Boolean).join("／") || "空白詞條";
  if (!(await confirmRemoval(`要從表格刪除「${label}」嗎？`))) return;
  selected.delete(row.key);
  replaceRows(
    rows.value.filter((candidate) => candidate.key !== row.key),
    true,
  );
}

async function removeSelected() {
  const count = selected.size;
  if (!count) return;
  if (!(await confirmRemoval(`要從表格刪除選取的 ${count.toLocaleString()} 筆詞條嗎？`))) return;
  const keys = new Set(selected);
  selected.clear();
  replaceRows(
    rows.value.filter((row) => !keys.has(row.key)),
    true,
  );
  ElMessage.success(`已從表格刪除 ${count} 筆，按「儲存」後才會生效。`);
}

function conflictMessage(values: UserDictionaryEntry[]): string | undefined {
  const conflicts = findConflicts(values);
  if (!conflicts.length) return undefined;
  return `同一方向的來源詞對應到不同詞：${conflicts.slice(0, 5).join("、")}`;
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
    !(await confirm(`將寫入 ${values.length} 筆詞彙並重新載入轉換引擎。是否繼續？`, {
      title: "確認儲存使用者詞典",
      kind: "warning",
    }))
  )
    return;
  if (await persist(values)) ElMessage.success(`已儲存 ${values.length} 筆詞彙。`);
}

async function chooseImportFile() {
  const selected = await open({ multiple: false, filters: fileFilters });
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
      `「${preview.sourceFile}」是 1.x 的舊版字典，不建議直接匯入：舊版內建詞條品質參差，部分詞 2.0 的轉法已經比較好，直接匯入反而會蓋掉。建議只挑自己加過的詞手動新增。仍要匯入嗎？`,
      { title: "不建議直接匯入舊版字典", kind: "warning" },
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
    ElMessage.error(`${conflict}。請先修正表格再匯入。`);
    return;
  }
  const savingEdits = dirty.value;
  replaceRows(toRows(result.entries), true);
  importVisible.value = false;
  if (!(await persist(result.entries))) {
    ElMessage.warning("匯入的詞條已加入表格，但尚未儲存。");
    return;
  }
  const notes = [`加入 ${result.added} 筆`];
  if (result.existing) notes.push(`${result.existing} 筆已在表格中，改標為這次匯入`);
  if (result.conflicts) notes.push(`${result.conflicts} 筆與表格既有詞條衝突，保留既有詞條`);
  if (savingEdits) notes.push("表格中原本未儲存的變更也一併儲存");
  ElMessage.success(`${notes.join("，")}。已轉存為使用者詞典。`);
}

async function exportDictionary() {
  const values = entries();
  const conflict = conflictMessage(values);
  if (conflict) {
    ElMessage.error(conflict);
    return;
  }
  const target = await chooseSavePath({
    defaultPath: "ConvertZZ-使用者詞典.txt",
    filters: fileFilters,
  });
  if (!target) return;
  busy.value = true;
  try {
    await core.request("userDictionary.export", { path: target, entries: values });
    ElMessage.success(
      `已匯出 ${values.length} 筆詞彙${dirty.value ? "（含尚未儲存的變更）" : ""}。`,
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
        <h1>使用者詞典</h1>
        <p>自訂整詞對應。詞彙會加入分詞，轉換時整詞套用，不做全文直接取代。</p>
      </div>
      <div class="header-actions">
        <el-button :loading="busy" @click="chooseImportFile">匯入</el-button
        ><el-button :disabled="busy" @click="exportDictionary">匯出</el-button
        ><el-button @click="addEntry">新增</el-button
        ><el-button type="danger" plain :disabled="!selectedCount" @click="removeSelected"
          >刪除選取{{ selectedCount ? `（${selectedCount.toLocaleString()}）` : "" }}</el-button
        ><el-button :disabled="!dirty" @click="load">重設未儲存變更</el-button
        ><el-button type="primary" :disabled="!dirty" :loading="busy" @click="save">儲存</el-button>
      </div>
    </header>
    <el-alert
      title="簡轉繁時以簡體為來源詞、繁轉簡時以繁體為來源詞；來源詞只能是中日韓文字。匯入接受 .txt、.tsv、.csv，會自動判斷是本程式匯出的詞典或 1.x 舊版字典；匯入後轉存成使用者詞典，不會修改或再讀取原檔。"
      type="info"
      :closable="false"
    />
    <el-alert
      v-if="loadError"
      :title="`無法讀取使用者詞典，目前以空詞典轉換：${loadError}`"
      type="error"
      :closable="false"
    />
    <el-card shadow="never"
      ><div class="dictionary-toolbar">
        <el-input v-model="filter.keyword" clearable placeholder="搜尋簡體或繁體" /><el-select
          v-model="filter.direction"
          aria-label="方向篩選"
          placeholder="全部方向"
          ><el-option label="全部方向" value="" /><el-option
            v-for="(label, value) in directionLabels"
            :key="value"
            :label="label"
            :value="value" /></el-select
        ><el-select v-model="filter.source" aria-label="來源篩選" placeholder="全部來源" filterable
          ><el-option label="全部來源" value="" /><el-option
            label="手動加入"
            :value="MANUAL_SOURCE" /><el-option
            v-for="source in sourceOptions"
            :key="source"
            :label="source"
            :value="source" /></el-select
        ><span
          >{{ visibleRows.length.toLocaleString() }} / {{ rows.length.toLocaleString() }} 筆</span
        >
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
    <el-dialog v-model="importVisible" title="匯入詞典" width="720px">
      <template v-if="importPreview">
        <p class="path-label">{{ importPreview.path }}</p>
        <p>
          {{ importPreview.format === "legacy" ? "1.x 舊版字典" : "使用者詞典" }}，共
          {{ importPreview.total }} 列：可匯入 {{ importPreview.importable }}、已停用
          {{ importPreview.disabled }}、無法匯入 {{ importPreview.invalid }}、重複
          {{ importPreview.duplicate }}。
        </p>
        <p>
          匯入的詞條會標記來源「{{ importPreview.sourceFile }}」與匯入時間，併入目前表格後立即儲存。
        </p>
        <el-alert
          v-if="dirty"
          title="表格中尚未儲存的變更也會一併儲存。"
          type="warning"
          :closable="false"
        />
        <el-table v-if="skippedImportRows.length" :data="skippedImportRows" max-height="320"
          ><el-table-column prop="line" label="列" width="70" /><el-table-column
            prop="simplified"
            label="簡體" /><el-table-column prop="traditional" label="繁體" /><el-table-column
            label="狀態"
            width="100"
            ><template #default="scope">{{
              statusLabels[scope.row.status as DictionaryImportRow["status"]]
            }}</template></el-table-column
          ><el-table-column prop="reason" label="原因" min-width="200"
        /></el-table>
      </template>
      <template #footer
        ><el-button @click="importVisible = false">取消</el-button
        ><el-button
          type="primary"
          :loading="busy"
          :disabled="!importPreview?.importable"
          @click="applyImport"
          >匯入並儲存</el-button
        ></template
      >
    </el-dialog>
  </section>
</template>
