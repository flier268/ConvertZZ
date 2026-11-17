<script setup lang="ts">
import { computed } from "vue";
import type { DiffSection } from "../lib/fileDiff";
import SideBySideDiffView from "./SideBySideDiffView.vue";

const props = withDefaults(
  defineProps<{
    modelValue: boolean;
    title?: string;
    meta?: Array<{ label: string; value: string }>;
    sections?: DiffSection[];
    fullscreen?: boolean;
    /** 長文分頁與上下差異導航（全視窗檔案預覽用）。 */
    enableNav?: boolean;
  }>(),
  {
    title: "差異",
    meta: () => [],
    sections: () => [],
    fullscreen: false,
    enableNav: false,
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
}>();

const visible = computed({
  get: () => props.modelValue,
  set: (value: boolean) => emit("update:modelValue", value),
});

const hasContent = computed(() =>
  props.sections.some((section) => section.source || section.output),
);
const compact = computed(() => !props.fullscreen && props.sections.length > 1);
</script>

<template>
  <el-dialog
    v-model="visible"
    :title="title"
    :width="fullscreen ? '100%' : '920px'"
    :fullscreen="fullscreen"
    class="preview-diff-dialog"
    :class="{ 'is-fullscreen': fullscreen }"
    destroy-on-close
    align-center
  >
    <div v-if="meta.length" class="preview-diff-meta">
      <div v-for="entry in meta" :key="`${entry.label}:${entry.value}`">
        <span class="meta-label">{{ entry.label }}</span>
        <code>{{ entry.value }}</code>
      </div>
    </div>
    <el-empty v-if="!hasContent" description="此項目沒有可顯示的預覽內容" />
    <div v-else class="preview-diff-sections" :class="{ fullscreen }">
      <section
        v-for="(section, index) in sections"
        :key="`${section.title}-${index}`"
        class="preview-diff-section"
      >
        <h3 v-if="sections.length > 1">{{ section.title }}</h3>
        <SideBySideDiffView
          :source="section.source"
          :output="section.output"
          :source-label="section.sourceLabel"
          :output-label="section.outputLabel"
          :compact="compact"
          :paginated="enableNav"
          :show-nav="enableNav"
          :fill-height="fullscreen"
        />
      </section>
    </div>
  </el-dialog>
</template>

<style scoped>
.preview-diff-meta {
  display: grid;
  gap: 8px;
  margin-bottom: 14px;
  color: var(--el-text-color-regular);
  font-size: 13px;
}
.preview-diff-meta > div {
  display: grid;
  grid-template-columns: 48px 1fr;
  gap: 10px;
  align-items: start;
}
.meta-label {
  color: var(--el-text-color-secondary);
}
.preview-diff-meta code {
  word-break: break-all;
  white-space: pre-wrap;
  font-family: inherit;
}
.preview-diff-sections {
  display: grid;
  gap: 18px;
}
.preview-diff-sections.fullscreen {
  display: flex;
  flex-direction: column;
  gap: 12px;
  min-height: 0;
  height: calc(100vh - 160px);
}
.preview-diff-sections.fullscreen .preview-diff-section {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
}
.preview-diff-sections.fullscreen .preview-diff-section:last-child {
  flex: 1 1 auto;
}
.preview-diff-sections.fullscreen .preview-diff-section :deep(.preview-diff-root) {
  flex: 1 1 auto;
  min-height: 0;
}
.preview-diff-sections h3 {
  margin: 0 0 8px;
  color: var(--el-text-color-primary);
  font-size: 14px;
}
.preview-diff-sections.fullscreen h3 {
  margin: 0;
  flex: 0 0 auto;
}
</style>
