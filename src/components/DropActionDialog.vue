<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import type { DropActionChoice, DropActionKind } from "../lib/dropActions";
import { normalizeDropActionChoice, summarizeDropPaths } from "../lib/dropActions";

const props = defineProps<{
  modelValue: boolean;
  paths: string[];
  lastChoice: DropActionChoice;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
  confirm: [choice: DropActionChoice];
}>();

const form = reactive<{
  kind: DropActionKind;
  operation: DropActionChoice["operation"];
  direction: DropActionChoice["direction"];
}>({ ...normalizeDropActionChoice(props.lastChoice) });

watch(
  () => props.modelValue,
  (open) => {
    if (!open) return;
    Object.assign(form, normalizeDropActionChoice(props.lastChoice));
  },
);

const summary = computed(() => summarizeDropPaths(props.paths));

function setVisible(value: boolean): void {
  emit("update:modelValue", value);
}

function cancel(): void {
  setVisible(false);
}

function continueAction(): void {
  emit("confirm", {
    kind: form.kind,
    operation: form.operation,
    direction: form.direction,
  });
  setVisible(false);
}
</script>

<template>
  <el-dialog
    :model-value="modelValue"
    title="選擇轉換動作"
    width="460px"
    align-center
    destroy-on-close
    :close-on-click-modal="false"
    @update:model-value="setVisible"
  >
    <p class="drop-dialog-summary">已拖入 {{ paths.length }} 項：{{ summary }}</p>
    <el-form label-position="top">
      <el-form-item label="動作">
        <el-radio-group v-model="form.kind">
          <el-radio value="file">檔案與檔名轉換</el-radio>
          <el-radio value="audio">音訊標籤</el-radio>
        </el-radio-group>
      </el-form-item>
      <el-form-item v-if="form.kind === 'file'" label="作業">
        <el-radio-group v-model="form.operation">
          <el-radio value="content">轉換內容</el-radio>
          <el-radio value="filename">轉換檔名</el-radio>
          <el-radio value="both">內容與檔名</el-radio>
        </el-radio-group>
      </el-form-item>
      <el-form-item label="方向">
        <el-radio-group v-model="form.direction">
          <el-radio value="s2t">簡轉繁</el-radio>
          <el-radio value="t2s">繁轉簡</el-radio>
          <el-radio value="none">不轉換</el-radio>
        </el-radio-group>
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="cancel">取消</el-button>
      <el-button type="primary" :disabled="!paths.length" @click="continueAction">繼續</el-button>
    </template>
  </el-dialog>
</template>
