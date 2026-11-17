<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { confirm, open } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import type {
  LegacyImportPreview,
  LegacyImportRow,
  UserDictionaryDirection,
  UserDictionaryEntry,
  UserDictionaryReadResult,
} from "@shared/contracts";
import { core } from "../lib/coreClient";
import { formatUnknownError } from "../lib/errors";
import { findConflicts, mergeLegacyRows } from "../lib/userDictionary";

defineOptions({ name: "DictionaryPage" });

interface Row extends UserDictionaryEntry {
  key: number;
}

const directionLabels: Record<UserDictionaryDirection, string> = {
  both: "雙向",
  s2t: "只用於簡轉繁",
  t2s: "只用於繁轉簡",
};

const statusLabels: Record<LegacyImportRow["status"], string> = {
  importable: "可匯入",
  builtinDefault: "1.x 內建",
  disabled: "已停用",
  invalid: "無法匯入",
  duplicate: "重複",
};

const path = ref("");
const loadError = ref("");
const rows = ref<Row[]>([]);
const savedSnapshot = ref("[]");
const query = ref("");
const page = ref(1);
const pageSize = 100;
const busy = ref(false);
const importPreview = ref<LegacyImportPreview>();
const importVisible = ref(false);
let nextKey = 0;

const filteredRows = computed(() => {
  const keyword = query.value.trim();
  if (!keyword) return rows.value;
  return rows.value.filter(
    (row) => row.simplified.includes(keyword) || row.traditional.includes(keyword),
  );
});
const pagedRows = computed(() => {
  const lastPage = Math.max(1, Math.ceil(filteredRows.value.length / pageSize));
  const current = Math.min(page.value, lastPage);
  return filteredRows.value.slice((current - 1) * pageSize, current * pageSize);
});
const skippedImportRows = computed(
  () => importPreview.value?.rows.filter((row) => row.status !== "importable") ?? [],
);
const dirty = computed(() => JSON.stringify(entries()) !== savedSnapshot.value);

function toRows(entries: UserDictionaryEntry[]): Row[] {
  return entries.map((entry) => ({ ...entry, key: nextKey++ }));
}

function entries(): UserDictionaryEntry[] {
  return rows.value.map(({ simplified, traditional, direction }) => ({
    simplified,
    traditional,
    direction,
  }));
}

async function load() {
  busy.value = true;
  try {
    const result = await core.request<UserDictionaryReadResult>("userDictionary.read", {});
    path.value = result.path;
    loadError.value = result.loadError ?? "";
    rows.value = toRows(result.entries);
    savedSnapshot.value = JSON.stringify(entries());
  } catch (error) {
    ElMessage.error(formatUnknownError(error));
  } finally {
    busy.value = false;
  }
}

watch(query, () => {
  page.value = 1;
});

function addEntry() {
  query.value = "";
  page.value = 1;
  rows.value.unshift({ key: nextKey++, simplified: "", traditional: "", direction: "both" });
}

function removeEntry(row: Row) {
  rows.value = rows.value.filter((candidate) => candidate.key !== row.key);
}

async function save() {
  const values = entries();
  const conflicts = findConflicts(values);
  if (conflicts.length) {
    ElMessage.error(`同一方向的來源詞對應到不同詞：${conflicts.slice(0, 5).join("、")}`);
    return;
  }
  if (
    !(await confirm(`將寫入 ${values.length} 筆詞彙並重新載入轉換引擎。是否繼續？`, {
      title: "確認儲存使用者詞典",
      kind: "warning",
    }))
  )
    return;
  busy.value = true;
  try {
    await core.request("userDictionary.save", { entries: values });
    savedSnapshot.value = JSON.stringify(values);
    loadError.value = "";
    ElMessage.success(`已儲存 ${values.length} 筆詞彙。`);
  } catch (error) {
    ElMessage.error(formatUnknownError(error));
  } finally {
    busy.value = false;
  }
}

async function chooseLegacyDictionary() {
  const selected = await open({
    multiple: false,
    filters: [{ name: "ConvertZZ 1.x 字典", extensions: ["csv"] }],
  });
  if (!selected || Array.isArray(selected)) return;
  busy.value = true;
  try {
    importPreview.value = await core.request<LegacyImportPreview>(
      "userDictionary.previewLegacyImport",
      { path: selected },
    );
    importVisible.value = true;
  } catch (error) {
    ElMessage.error(formatUnknownError(error));
  } finally {
    busy.value = false;
  }
}

function applyImport() {
  if (!importPreview.value) return;
  const result = mergeLegacyRows(entries(), importPreview.value.rows);
  rows.value = toRows(result.entries);
  query.value = "";
  page.value = 1;
  importVisible.value = false;
  const notes = [`加入 ${result.added} 筆`];
  if (result.existing) notes.push(`${result.existing} 筆已在表格中`);
  if (result.conflicts) notes.push(`${result.conflicts} 筆與表格既有詞條衝突，保留既有詞條`);
  ElMessage.success(`${notes.join("，")}。按「儲存」後才會生效。`);
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
        <el-button :loading="busy" @click="chooseLegacyDictionary"
          >匯入舊版 Dictionary.csv</el-button
        ><el-button @click="addEntry">新增</el-button
        ><el-button :disabled="!dirty" @click="load">重設未儲存變更</el-button
        ><el-button type="primary" :disabled="!dirty" :loading="busy" @click="save">儲存</el-button>
      </div>
    </header>
    <el-alert
      title="簡轉繁時以簡體為來源詞、繁轉簡時以繁體為來源詞；來源詞只能是中日韓文字。舊版 Dictionary.csv 只讀取，不會修改原檔；1.x 內建的預設詞條會自動略過。"
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
        <el-input v-model="query" clearable placeholder="搜尋簡體或繁體" /><span
          >{{ rows.length.toLocaleString() }} 筆</span
        >
      </div>
      <div class="path-label">{{ path }}</div></el-card
    >
    <el-card shadow="never" class="page-fill-main dictionary-table-card"
      ><div class="dictionary-table-host">
        <el-table v-loading="busy" :data="pagedRows" row-key="key" height="100%"
          ><el-table-column label="簡體" min-width="180"
            ><template #default="scope"
              ><el-input v-model="scope.row.simplified" /></template></el-table-column
          ><el-table-column label="繁體" min-width="180"
            ><template #default="scope"
              ><el-input v-model="scope.row.traditional" /></template></el-table-column
          ><el-table-column label="方向" width="160"
            ><template #default="scope"
              ><el-select v-model="scope.row.direction"
                ><el-option
                  v-for="(label, value) in directionLabels"
                  :key="value"
                  :label="label"
                  :value="value" /></el-select></template></el-table-column
          ><el-table-column label="操作" width="72"
            ><template #default="scope"
              ><el-button link type="danger" @click="removeEntry(scope.row)"
                >刪除</el-button
              ></template
            ></el-table-column
          ></el-table
        >
      </div>
      <el-pagination
        v-model:current-page="page"
        :page-size="pageSize"
        :total="filteredRows.length"
        hide-on-single-page
        layout="prev, pager, next"
    /></el-card>
    <el-dialog v-model="importVisible" title="匯入舊版 Dictionary.csv" width="720px">
      <template v-if="importPreview">
        <p class="path-label">{{ importPreview.path }}</p>
        <p>
          共 {{ importPreview.total }} 列：可匯入 {{ importPreview.importable }}、1.x 內建
          {{ importPreview.builtinDefault }}、已停用 {{ importPreview.disabled }}、無法匯入
          {{ importPreview.invalid }}、重複 {{ importPreview.duplicate }}。
        </p>
        <el-table v-if="skippedImportRows.length" :data="skippedImportRows" max-height="320"
          ><el-table-column prop="line" label="列" width="70" /><el-table-column
            prop="simplified"
            label="簡體" /><el-table-column prop="traditional" label="繁體" /><el-table-column
            label="狀態"
            width="100"
            ><template #default="scope">{{
              statusLabels[scope.row.status as LegacyImportRow["status"]]
            }}</template></el-table-column
          ><el-table-column prop="reason" label="原因" min-width="200"
        /></el-table>
      </template>
      <template #footer
        ><el-button @click="importVisible = false">取消</el-button
        ><el-button type="primary" :disabled="!importPreview?.importable" @click="applyImport"
          >加入表格</el-button
        ></template
      >
    </el-dialog>
  </section>
</template>
