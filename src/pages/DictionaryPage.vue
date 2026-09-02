<script setup lang="ts">
import { computed, reactive, ref, watch } from "vue";
import { confirm, open } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import { core } from "../lib/coreClient";
import { loadSettings, saveSettings } from "../lib/settings";

defineOptions({ name: "DictionaryPage" });

interface Entry {
  index: number;
  enabled: boolean;
  type: string;
  simplified: string;
  simplifiedPriority: number;
  traditional: string;
  traditionalPriority: number;
}

const path = ref<string>();
const query = ref("");
const total = ref(0);
const page = ref(1);
const pageSize = 100;
const entries = ref<Entry[]>([]);
const dirty = reactive(new Map<number, Entry>());
const deleted = reactive(new Set<number>());
const busy = ref(false);
const sort = ref<"source" | "s2t" | "t2s">("source");
const previewSource = ref("");
const previewDirection = ref<"s2t" | "t2s">("s2t");
const previewResult = ref("");
let temporaryIndex = -1;
let previewTimer: ReturnType<typeof setTimeout> | undefined;
const changeCount = computed(() => dirty.size + deleted.size);

async function choose() {
  const selected = await open({
    multiple: false,
    filters: [{ name: "ConvertZZ 字典", extensions: ["csv"] }],
  });
  if (selected) {
    path.value = selected as string;
    const settings = await loadSettings();
    settings.dictionaryPath = path.value;
    await saveSettings();
    page.value = 1;
    await load();
  }
}

async function load() {
  busy.value = true;
  try {
    const result = await core.request<{ path: string; total: number; entries: Entry[] }>(
      "dictionary.read",
      {
        path: path.value,
        query: query.value,
        sort: sort.value,
        offset: (page.value - 1) * pageSize,
        limit: pageSize,
      },
    );
    path.value = result.path;
    total.value = result.total;
    entries.value = result.entries;
    dirty.clear();
    deleted.clear();
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    busy.value = false;
  }
}

function mark(entry: Entry) {
  dirty.set(entry.index, JSON.parse(JSON.stringify(entry)));
}

function addEntry() {
  const entry: Entry = {
    index: temporaryIndex--,
    enabled: true,
    type: "自訂",
    simplified: "",
    simplifiedPriority: 0,
    traditional: "",
    traditionalPriority: 0,
  };
  entries.value.unshift(entry);
  mark(entry);
}

function removeEntry(entry: Entry) {
  if (entry.index >= 0) deleted.add(entry.index);
  dirty.delete(entry.index);
  entries.value = entries.value.filter((candidate) => candidate !== entry);
}

function changes() {
  const values = Array.from(dirty.values());
  return {
    updates: values
      .filter((entry) => entry.index >= 0)
      .map(({ index, ...entry }) => ({ index, entry })),
    inserts: values.filter((entry) => entry.index < 0).map(({ index: _index, ...entry }) => entry),
    deletes: Array.from(deleted),
  };
}

async function save() {
  if (!path.value || !changeCount.value) return;
  if (
    !(await confirm(`將先備份字典，再寫入 ${changeCount.value} 筆變更。是否繼續？`, {
      title: "確認字典備份",
      kind: "warning",
    }))
  )
    return;
  busy.value = true;
  try {
    const result = await core.request<{ updated: number; backupPath: string }>(
      "dictionary.update",
      { path: path.value, ...changes() },
    );
    ElMessage.success(`已更新 ${result.updated} 筆資料。備份位於 ${result.backupPath}`);
    await load();
  } catch (error) {
    ElMessage.error(error instanceof Error ? error.message : String(error));
  } finally {
    busy.value = false;
  }
}

async function preview() {
  if (!previewSource.value) {
    previewResult.value = "";
    return;
  }
  previewResult.value = (
    await core.request<{ text: string }>("dictionary.preview", {
      path: path.value,
      text: previewSource.value,
      direction: previewDirection.value,
      ...changes(),
    })
  ).text;
}

watch(
  [previewSource, previewDirection, entries],
  () => {
    if (previewTimer) clearTimeout(previewTimer);
    previewTimer = setTimeout(() => void preview().catch(() => undefined), 180);
  },
  { deep: true },
);

loadSettings().then((settings) => {
  path.value = settings.dictionaryPath;
  return load();
});
</script>

<template>
  <section class="page-stack">
    <header class="page-header">
      <div>
        <p class="eyebrow">LEGACY DICTIONARY</p>
        <h1>舊版字典</h1>
        <p>此頁選取的外部字典會供舊版引擎使用。</p>
      </div>
      <div class="header-actions">
        <el-button @click="choose">選取可寫字典</el-button
        ><el-button @click="addEntry">新增</el-button
        ><el-button :disabled="!changeCount" @click="load">重設未儲存變更</el-button
        ><el-button type="primary" :disabled="!changeCount || !path" :loading="busy" @click="save"
          >儲存變更</el-button
        >
      </div>
    </header>
    <el-alert
      title="內建 Dictionary.csv 隨安裝包提供。選取外部副本後會切換舊版引擎的作用中字典。"
      type="info"
      :closable="false"
    />
    <el-card shadow="never"
      ><div class="dictionary-toolbar">
        <el-input
          v-model="query"
          clearable
          placeholder="搜尋簡體、繁體或分類"
          @keyup.enter="
            page = 1;
            load();
          "
        /><el-select
          v-model="sort"
          style="width: 150px"
          @change="
            page = 1;
            load();
          "
          ><el-option label="原始順序" value="source" /><el-option
            label="簡轉繁優先"
            value="s2t" /><el-option label="繁轉簡優先" value="t2s" /></el-select
        ><el-button
          @click="
            page = 1;
            load();
          "
          >搜尋</el-button
        ><span>{{ total.toLocaleString() }} 筆</span>
      </div>
      <div class="path-label">{{ path }}</div>
      <div class="control-row">
        <el-select v-model="previewDirection" style="width: 130px"
          ><el-option label="簡轉繁" value="s2t" /><el-option
            label="繁轉簡"
            value="t2s" /></el-select
        ><el-input v-model="previewSource" placeholder="輸入文字以即時預覽目前變更" /><el-input
          v-model="previewResult"
          readonly
          placeholder="預覽結果"
        /></div
    ></el-card>
    <el-card shadow="never" class="page-fill-main dictionary-table-card"
      ><div class="dictionary-table-host">
        <el-table v-loading="busy" :data="entries" height="100%"
          ><el-table-column label="啟用" width="68"
            ><template #default="scope"
              ><el-checkbox
                v-model="scope.row.enabled"
                @change="mark(scope.row)" /></template></el-table-column
          ><el-table-column label="分類" width="120"
            ><template #default="scope"
              ><el-input
                v-model="scope.row.type"
                @input="mark(scope.row)" /></template></el-table-column
          ><el-table-column label="簡體" min-width="180"
            ><template #default="scope"
              ><el-input
                v-model="scope.row.simplified"
                @input="mark(scope.row)" /></template></el-table-column
          ><el-table-column label="優先" width="100"
            ><template #default="scope"
              ><el-input-number
                v-model="scope.row.simplifiedPriority"
                :controls="false"
                @change="mark(scope.row)" /></template></el-table-column
          ><el-table-column label="繁體" min-width="180"
            ><template #default="scope"
              ><el-input
                v-model="scope.row.traditional"
                @input="mark(scope.row)" /></template></el-table-column
          ><el-table-column label="優先" width="100"
            ><template #default="scope"
              ><el-input-number
                v-model="scope.row.traditionalPriority"
                :controls="false"
                @change="mark(scope.row)" /></template></el-table-column
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
        :total="total"
        layout="prev, pager, next"
        @current-change="load"
    /></el-card>
  </section>
</template>
