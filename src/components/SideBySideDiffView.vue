<script lang="ts">
/** 工作階段內共用左右比例，切換頁或重掛載差異檢視時仍保留。 */
const sessionPaneSizes = {
  left: "50%" as string | number,
  right: "50%" as string | number,
};
</script>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { buildSideBySideDiff, escapeHtml, sideBySideToHtml } from "../lib/textDiff";

const props = withDefaults(
  defineProps<{
    source: string;
    output: string;
    sourceLabel?: string;
    outputLabel?: string;
    compact?: boolean;
    editable?: boolean;
    sourcePlaceholder?: string;
    outputPlaceholder?: string;
  }>(),
  {
    sourceLabel: "來源",
    outputLabel: "輸出",
    compact: false,
    editable: false,
    sourcePlaceholder: "在此輸入或貼上文字",
    outputPlaceholder: "結果會顯示於此",
  },
);

const emit = defineEmits<{
  "update:source": [value: string];
}>();

const leftSize = ref<string | number>(sessionPaneSizes.left);
const rightSize = ref<string | number>(sessionPaneSizes.right);
const leftPane = ref<HTMLElement>();
const rightPane = ref<HTMLElement>();
let syncing = false;

const sides = computed(() => buildSideBySideDiff(props.source ?? "", props.output ?? ""));
const leftHtml = computed(() => sideBySideToHtml(sides.value.left, "diff-remove"));
const rightHtml = computed(() =>
  props.output
    ? sideBySideToHtml(sides.value.right, "diff-add")
    : `<span class="preview-diff-placeholder">${escapeHtml(props.outputPlaceholder)}</span>`,
);
const sourceCount = computed(() => Array.from(props.source ?? "").length);
const outputCount = computed(() => Array.from(props.output ?? "").length);
const empty = computed(() => !props.editable && !props.source && !props.output);

function rememberPaneSizes() {
  sessionPaneSizes.left = leftSize.value;
  sessionPaneSizes.right = rightSize.value;
}

function syncScroll(from: "left" | "right") {
  const source = from === "left" ? leftPane.value : rightPane.value;
  const target = from === "left" ? rightPane.value : leftPane.value;
  if (syncing || !source || !target) return;
  syncing = true;
  const maxSourceY = source.scrollHeight - source.clientHeight;
  const maxTargetY = target.scrollHeight - target.clientHeight;
  target.scrollTop =
    maxSourceY > 0 && maxTargetY > 0
      ? (source.scrollTop / maxSourceY) * maxTargetY
      : source.scrollTop;
  const maxSourceX = source.scrollWidth - source.clientWidth;
  const maxTargetX = target.scrollWidth - target.clientWidth;
  target.scrollLeft =
    maxSourceX > 0 && maxTargetX > 0
      ? (source.scrollLeft / maxSourceX) * maxTargetX
      : source.scrollLeft;
  requestAnimationFrame(() => {
    syncing = false;
  });
}

watch(
  () => [props.source, props.output],
  async () => {
    await nextTick();
    if (leftPane.value) leftPane.value.scrollTop = 0;
    if (rightPane.value) rightPane.value.scrollTop = 0;
  },
);
</script>

<template>
  <el-empty v-if="empty" description="沒有可顯示的差異內容" />
  <div v-else class="preview-diff-root" :class="{ compact }">
    <el-splitter class="preview-diff-grid" @resize-end="rememberPaneSizes">
      <el-splitter-panel v-model:size="leftSize" min="20%" max="80%">
        <section class="preview-diff-pane">
          <header>
            <span>{{ sourceLabel }}</span>
            <small>{{ sourceCount }} 字</small>
          </header>
          <textarea
            v-if="editable"
            ref="leftPane"
            class="preview-diff-body preview-diff-input"
            :value="source"
            :placeholder="sourcePlaceholder"
            spellcheck="false"
            @input="emit('update:source', ($event.target as HTMLTextAreaElement).value)"
            @scroll="syncScroll('left')"
          />
          <pre
            v-else
            ref="leftPane"
            class="preview-diff-body"
            @scroll="syncScroll('left')"
            v-html="leftHtml"
          />
        </section>
      </el-splitter-panel>
      <el-splitter-panel v-model:size="rightSize" min="20%" max="80%">
        <section class="preview-diff-pane">
          <header>
            <span>{{ outputLabel }}</span>
            <small>{{ outputCount }} 字</small>
          </header>
          <pre
            ref="rightPane"
            class="preview-diff-body"
            @scroll="syncScroll('right')"
            v-html="rightHtml"
          />
        </section>
      </el-splitter-panel>
    </el-splitter>
  </div>
</template>

<style scoped>
.preview-diff-root {
  min-height: 420px;
}
.preview-diff-root.compact {
  display: flex;
  flex-direction: column;
  min-height: 112px;
  height: 100%;
}
.preview-diff-grid {
  height: 100%;
  min-height: inherit;
}
.preview-diff-root:not(.compact) .preview-diff-grid {
  min-height: 420px;
}
.preview-diff-root.compact .preview-diff-grid {
  /* el-splitter 需明確高度；由外層 flex 分配剩餘空間 */
  flex: 1 1 auto;
  height: 100%;
  min-height: 112px;
}
.preview-diff-grid :deep(.el-splitter-bar) {
  width: 10px;
}
.preview-diff-grid :deep(.el-splitter-bar__dragger-horizontal) {
  width: 10px;
  height: 100%;
}
.preview-diff-grid :deep(.el-splitter-bar__dragger-horizontal::before) {
  width: 3px;
  height: 40px;
  border-radius: 999px;
  background-color: #9bb8b1;
}
.preview-diff-pane {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}
.preview-diff-root:not(.compact) .preview-diff-pane {
  margin: 0 4px;
  background: #f7faf9;
  border-radius: 8px;
  padding: 0 12px;
}
.preview-diff-root.compact .preview-diff-pane {
  margin: 0 4px;
}
.preview-diff-pane header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 10px 0 8px;
  border-bottom: 1px solid #e3eeea;
  font-size: 13px;
  font-weight: 600;
}
.preview-diff-pane header small {
  color: var(--el-text-color-secondary);
  font-weight: 400;
}
.preview-diff-body {
  margin: 0;
  padding: 12px 0;
  flex: 1 1 auto;
  height: 52vh;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.75;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 13px;
}
.preview-diff-input {
  display: block;
  width: 100%;
  border: 0;
  resize: none;
  background: transparent;
  color: inherit;
  outline: none;
}
.preview-diff-body :deep(.preview-diff-placeholder) {
  color: var(--el-text-color-placeholder);
}
.compact .preview-diff-pane {
  height: 100%;
  min-height: 0;
}
.compact .preview-diff-body {
  flex: 1 1 auto;
  height: auto;
  max-height: none;
  min-height: 0;
  padding: 8px 0 10px;
}
.preview-diff-body :deep(.diff-change) {
  border-radius: 3px;
  padding: 0 1px;
}
.preview-diff-body :deep(.diff-remove) {
  background: color-mix(in srgb, var(--el-color-danger) 18%, transparent);
  color: var(--el-color-danger-dark-2);
}
.preview-diff-body :deep(.diff-add) {
  background: color-mix(in srgb, var(--el-color-success) 18%, transparent);
  color: var(--el-color-success-dark-2);
}

@media (max-width: 800px) {
  .preview-diff-root:not(.compact) {
    min-height: 0;
  }
  .preview-diff-body {
    height: 28vh;
  }
  .compact .preview-diff-body {
    max-height: none;
  }
}
</style>
