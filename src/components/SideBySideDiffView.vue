<script lang="ts">
/** 工作階段內共用左右比例，切換頁或重掛載差異檢視時仍保留。 */
const sessionPaneSizes = {
  left: "50%" as string | number,
  right: "50%" as string | number,
};
</script>

<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { ArrowDown, ArrowUp } from "@element-plus/icons-vue";
import {
  DEFAULT_DIFF_PAGE_SIZE,
  buildPagedSideBySideDiff,
  buildSideBySideDiff,
  escapeHtml,
  sideBySideToHtml,
  type DiffPage,
} from "../lib/textDiff";

const props = withDefaults(
  defineProps<{
    source: string;
    output: string;
    sourceLabel?: string;
    outputLabel?: string;
    compact?: boolean;
    editable?: boolean;
    /** 長文依碼點分頁；編輯模式會忽略。 */
    paginated?: boolean;
    pageSize?: number;
    /** 顯示上／下一個差異與分頁工具列。 */
    showNav?: boolean;
    /** 讓內容區吃滿外層高度（全視窗預覽用）。 */
    fillHeight?: boolean;
    sourcePlaceholder?: string;
    outputPlaceholder?: string;
  }>(),
  {
    sourceLabel: "來源",
    outputLabel: "輸出",
    compact: false,
    editable: false,
    paginated: false,
    pageSize: DEFAULT_DIFF_PAGE_SIZE,
    showNav: false,
    fillHeight: false,
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
const currentPage = ref(1);
const activeChangeKey = ref<string>();
let syncing = false;

const pagingEnabled = computed(() => props.paginated && !props.editable);

const pages = computed<DiffPage[]>(() => {
  if (!pagingEnabled.value) {
    const sides = buildSideBySideDiff(props.source ?? "", props.output ?? "");
    return [
      {
        left: sides.left,
        right: sides.right,
        hasChanges:
          sides.left.some((span) => span.kind === "change") ||
          sides.right.some((span) => span.kind === "change"),
      },
    ];
  }
  return buildPagedSideBySideDiff(props.source ?? "", props.output ?? "", props.pageSize);
});

const pageCount = computed(() => Math.max(1, pages.value.length));
const activePage = computed(() => pages.value[currentPage.value - 1] ?? pages.value[0]!);
const leftHtml = computed(() => sideBySideToHtml(activePage.value.left, "diff-remove"));
const rightHtml = computed(() =>
  props.output || pagingEnabled.value
    ? sideBySideToHtml(activePage.value.right, "diff-add")
    : `<span class="preview-diff-placeholder">${escapeHtml(props.outputPlaceholder)}</span>`,
);
const sourceCount = computed(() => Array.from(props.source ?? "").length);
const outputCount = computed(() => Array.from(props.output ?? "").length);
const empty = computed(() => !props.editable && !props.source && !props.output);
const hasChanges = computed(() => pages.value.some((page) => page.hasChanges));
const showToolbar = computed(
  () =>
    props.showNav && !props.editable && !empty.value && (hasChanges.value || pageCount.value > 1),
);

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

type ChangeTarget = {
  key: string;
  top: number;
  left?: HTMLElement;
  right?: HTMLElement;
};

function clearActiveMarks() {
  for (const pane of [leftPane.value, rightPane.value]) {
    pane?.querySelectorAll("mark.diff-change.is-active").forEach((node) => {
      node.classList.remove("is-active");
    });
  }
}

function collectChangeTargets(): ChangeTarget[] {
  const leftMarks = Array.from(
    leftPane.value?.querySelectorAll<HTMLElement>("mark.diff-change") ?? [],
  );
  const rightMarks = Array.from(
    rightPane.value?.querySelectorAll<HTMLElement>("mark.diff-change") ?? [],
  );
  const targets: ChangeTarget[] = [];
  const threshold = 8;
  let leftIndex = 0;
  let rightIndex = 0;

  while (leftIndex < leftMarks.length || rightIndex < rightMarks.length) {
    const leftMark = leftMarks[leftIndex];
    const rightMark = rightMarks[rightIndex];
    if (leftMark && rightMark && Math.abs(leftMark.offsetTop - rightMark.offsetTop) <= threshold) {
      targets.push({
        key: `l${leftIndex}-r${rightIndex}`,
        top: Math.min(leftMark.offsetTop, rightMark.offsetTop),
        left: leftMark,
        right: rightMark,
      });
      leftIndex += 1;
      rightIndex += 1;
      continue;
    }
    if (leftMark && (!rightMark || leftMark.offsetTop < rightMark.offsetTop - threshold)) {
      targets.push({ key: `l${leftIndex}`, top: leftMark.offsetTop, left: leftMark });
      leftIndex += 1;
      continue;
    }
    if (rightMark) {
      targets.push({ key: `r${rightIndex}`, top: rightMark.offsetTop, right: rightMark });
      rightIndex += 1;
    }
  }
  return targets;
}

function scrollPaneToMark(pane: HTMLElement, mark: HTMLElement) {
  pane.scrollTop = Math.max(0, mark.offsetTop - Math.round(pane.clientHeight / 3));
}

function syncPaneFrom(from: "left" | "right") {
  const source = from === "left" ? leftPane.value : rightPane.value;
  const target = from === "left" ? rightPane.value : leftPane.value;
  if (!source || !target) return;
  const maxSourceY = source.scrollHeight - source.clientHeight;
  const maxTargetY = target.scrollHeight - target.clientHeight;
  target.scrollTop =
    maxSourceY > 0 && maxTargetY > 0
      ? (source.scrollTop / maxSourceY) * maxTargetY
      : source.scrollTop;
}

function focusChangeTarget(target: ChangeTarget) {
  clearActiveMarks();
  target.left?.classList.add("is-active");
  target.right?.classList.add("is-active");
  activeChangeKey.value = target.key;

  const left = leftPane.value;
  const right = rightPane.value;
  if (!left || !right) return;

  // 直接對齊兩側，避免先設 syncing 導致 syncScroll 被略過。
  syncing = true;
  if (target.left && target.right) {
    scrollPaneToMark(left, target.left);
    scrollPaneToMark(right, target.right);
  } else if (target.left) {
    scrollPaneToMark(left, target.left);
    syncPaneFrom("left");
  } else if (target.right) {
    scrollPaneToMark(right, target.right);
    syncPaneFrom("right");
  }
  requestAnimationFrame(() => {
    syncing = false;
  });
}

async function goToPage(page: number, options?: { focus?: "first" | "last" | "none" }) {
  const next = Math.min(pageCount.value, Math.max(1, page));
  currentPage.value = next;
  activeChangeKey.value = undefined;
  await nextTick();
  if (leftPane.value) leftPane.value.scrollTop = 0;
  if (rightPane.value) rightPane.value.scrollTop = 0;
  clearActiveMarks();
  const focus = options?.focus ?? "none";
  if (focus === "none") return;
  const targets = collectChangeTargets();
  if (!targets.length) return;
  focusChangeTarget(focus === "last" ? targets[targets.length - 1]! : targets[0]!);
}

async function goToChange(direction: 1 | -1) {
  if (!hasChanges.value) return;
  const targets = collectChangeTargets();
  const currentIndex = targets.findIndex((target) => target.key === activeChangeKey.value);

  if (direction === 1) {
    if (currentIndex >= 0 && currentIndex < targets.length - 1) {
      focusChangeTarget(targets[currentIndex + 1]!);
      return;
    }
    if (currentIndex < 0 && targets.length) {
      const viewport = (rightPane.value ?? leftPane.value)?.scrollTop ?? 0;
      const next = targets.find((target) => target.top > viewport + 4) ?? targets[0];
      if (next) {
        focusChangeTarget(next);
        return;
      }
    }
    for (let page = currentPage.value + 1; page <= pageCount.value; page += 1) {
      if (pages.value[page - 1]?.hasChanges) {
        await goToPage(page, { focus: "first" });
        return;
      }
    }
    for (let page = 1; page <= pageCount.value; page += 1) {
      if (pages.value[page - 1]?.hasChanges) {
        await goToPage(page, { focus: "first" });
        return;
      }
    }
    return;
  }

  if (currentIndex > 0) {
    focusChangeTarget(targets[currentIndex - 1]!);
    return;
  }
  if (currentIndex < 0 && targets.length) {
    const viewport = (rightPane.value ?? leftPane.value)?.scrollTop ?? 0;
    const previous =
      [...targets].reverse().find((target) => target.top < viewport - 4) ??
      targets[targets.length - 1];
    if (previous) {
      focusChangeTarget(previous);
      return;
    }
  }
  for (let page = currentPage.value - 1; page >= 1; page -= 1) {
    if (pages.value[page - 1]?.hasChanges) {
      await goToPage(page, { focus: "last" });
      return;
    }
  }
  for (let page = pageCount.value; page >= 1; page -= 1) {
    if (pages.value[page - 1]?.hasChanges) {
      await goToPage(page, { focus: "last" });
      return;
    }
  }
}

watch(
  () => [props.source, props.output, props.paginated, props.pageSize, props.editable],
  async () => {
    currentPage.value = 1;
    activeChangeKey.value = undefined;
    await nextTick();
    if (leftPane.value) leftPane.value.scrollTop = 0;
    if (rightPane.value) rightPane.value.scrollTop = 0;
    clearActiveMarks();
  },
);

watch(pageCount, (count) => {
  if (currentPage.value > count) currentPage.value = count;
});
</script>

<template>
  <el-empty v-if="empty" description="沒有可顯示的差異內容" />
  <div
    v-else
    class="preview-diff-root"
    :class="{ compact, 'fill-height': fillHeight, 'has-toolbar': showToolbar }"
  >
    <div v-if="showToolbar" class="preview-diff-toolbar">
      <div class="preview-diff-toolbar-group">
        <el-button size="small" :disabled="!hasChanges" :icon="ArrowUp" @click="goToChange(-1)">
          上一個差異
        </el-button>
        <el-button size="small" :disabled="!hasChanges" :icon="ArrowDown" @click="goToChange(1)">
          下一個差異
        </el-button>
      </div>
      <div v-if="pagingEnabled && pageCount > 1" class="preview-diff-toolbar-group">
        <el-pagination
          size="small"
          layout="prev, pager, next"
          :total="pageCount"
          :page-size="1"
          :current-page="currentPage"
          :pager-count="5"
          @current-change="(page: number) => goToPage(page)"
        />
        <span class="preview-diff-page-label">{{ currentPage }} / {{ pageCount }} 頁</span>
      </div>
    </div>
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
.preview-diff-root.fill-height {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
}
.preview-diff-root.has-toolbar {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.preview-diff-toolbar {
  flex: 0 0 auto;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.preview-diff-toolbar-group {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}
.preview-diff-page-label {
  color: var(--el-text-color-secondary);
  font-size: 12px;
}
.preview-diff-grid {
  height: 100%;
  min-height: inherit;
}
.preview-diff-root.has-toolbar .preview-diff-grid,
.preview-diff-root.fill-height .preview-diff-grid {
  flex: 1 1 auto;
  min-height: 0;
}
.preview-diff-root:not(.compact):not(.fill-height) .preview-diff-grid {
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
.preview-diff-root.fill-height .preview-diff-body {
  height: auto;
  max-height: none;
  min-height: 0;
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
.preview-diff-body :deep(.diff-change.is-active) {
  outline: 2px solid var(--el-color-primary);
  outline-offset: 1px;
  background: color-mix(in srgb, var(--el-color-primary) 22%, transparent);
}

@media (max-width: 800px) {
  .preview-diff-root:not(.compact):not(.fill-height) {
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
