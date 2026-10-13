<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";

const props = withDefaults(
  defineProps<{
    count: number;
    itemHeight?: number;
    overscan?: number;
    /** 測試或不依賴版面計算時可直接指定可視高度。 */
    viewportHeight?: number;
    /** 列索引 → 所屬區段標題列。有提供時，標題捲出視窗後會黏在上方。 */
    sectionOf?: (index: number) => number | null;
  }>(),
  { itemHeight: 40, overscan: 6 },
);

const root = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const measured = ref(0);

const viewport = computed(() => props.viewportHeight ?? measured.value);

function maxScrollTop(): number {
  const view = root.value?.clientHeight || props.viewportHeight || viewport.value || 0;
  return Math.max(0, props.count * props.itemHeight - view);
}

function clampScroll(value: number): number {
  return Math.min(Math.max(0, value), maxScrollTop());
}

const start = computed(() =>
  Math.max(0, Math.floor(scrollTop.value / props.itemHeight) - props.overscan),
);
const end = computed(() =>
  Math.min(
    props.count,
    start.value +
      Math.ceil((viewport.value || props.itemHeight) / props.itemHeight) +
      props.overscan * 2,
  ),
);
const indexes = computed(() => {
  const list: number[] = [];
  for (let index = start.value; index < end.value; index += 1) list.push(index);
  return list;
});
const stickyIndex = computed(() => {
  if (!props.sectionOf || props.count <= 0) return null;
  const first = Math.min(props.count - 1, Math.floor(scrollTop.value / props.itemHeight));
  const section = props.sectionOf(first);
  if (section == null || section < 0) return null;
  if (section * props.itemHeight >= scrollTop.value) return null;
  return section;
});

function applyClampedScroll(value: number) {
  const next = clampScroll(value);
  scrollTop.value = next;
  if (root.value && root.value.scrollTop !== next) root.value.scrollTop = next;
}

function onScroll(event: Event) {
  applyClampedScroll((event.target as HTMLElement).scrollTop);
}

watch(
  () => [props.count, props.itemHeight, viewport.value] as const,
  () => applyClampedScroll(scrollTop.value),
);

function measure() {
  measured.value = root.value?.clientHeight ?? 0;
}

let observer: ResizeObserver | undefined;
onMounted(() => {
  measure();
  if (typeof ResizeObserver !== "undefined" && root.value) {
    observer = new ResizeObserver(measure);
    observer.observe(root.value);
  }
});
onBeforeUnmount(() => observer?.disconnect());

function scrollToIndex(index: number) {
  const host = root.value;
  if (!host) return;
  const top = index * props.itemHeight;
  const view = host.clientHeight || props.viewportHeight || 0;
  if (top < host.scrollTop) host.scrollTop = top;
  else if (top + props.itemHeight > host.scrollTop + view) {
    host.scrollTop = top + props.itemHeight - view;
  }
  applyClampedScroll(host.scrollTop);
}

defineExpose({ scrollToIndex, start, end });
</script>

<template>
  <div ref="root" class="virtual-list" @scroll="onScroll">
    <div
      v-if="stickyIndex != null"
      class="virtual-list-sticky"
      :style="{ height: `${itemHeight}px`, marginBottom: `-${itemHeight}px` }"
    >
      <slot name="sticky" :index="stickyIndex" />
    </div>
    <div class="virtual-list-spacer" :style="{ height: `${count * itemHeight}px` }">
      <div
        v-for="index in indexes"
        :key="index"
        class="virtual-list-row"
        :style="{
          transform: `translateY(${index * itemHeight}px)`,
          height: `${itemHeight}px`,
        }"
      >
        <slot :index="index" />
      </div>
    </div>
  </div>
</template>
