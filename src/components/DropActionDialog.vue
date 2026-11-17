<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import type { DropActionChoice, DropActionKind } from "../lib/dropActions";
import { normalizeDropActionChoice, summarizeDropPaths } from "../lib/dropActions";
import { t, tn } from "@/i18n";

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
    :title="t('components.dropAction.title')"
    width="460px"
    align-center
    destroy-on-close
    :close-on-click-modal="false"
    @update:model-value="setVisible"
  >
    <p class="drop-dialog-summary">
      {{ tn("components.dropAction.summary", paths.length, { summary }) }}
    </p>
    <el-form label-position="top">
      <el-form-item :label="t('components.dropAction.action')">
        <el-radio-group v-model="form.kind">
          <el-radio value="file">{{ t("components.dropAction.file") }}</el-radio>
          <el-radio value="audio">{{ t("components.dropAction.audio") }}</el-radio>
        </el-radio-group>
      </el-form-item>
      <el-form-item v-if="form.kind === 'file'" :label="t('components.dropAction.operation')">
        <el-radio-group v-model="form.operation">
          <el-radio value="content">{{ t("components.dropAction.content") }}</el-radio>
          <el-radio value="filename">{{ t("components.dropAction.filename") }}</el-radio>
          <el-radio value="both">{{ t("components.dropAction.both") }}</el-radio>
        </el-radio-group>
      </el-form-item>
      <el-form-item :label="t('components.dropAction.direction')">
        <el-radio-group v-model="form.direction">
          <el-radio value="s2t">{{ t("common.direction.s2t") }}</el-radio>
          <el-radio value="t2s">{{ t("common.direction.t2s") }}</el-radio>
          <el-radio value="none">{{ t("components.dropAction.none") }}</el-radio>
        </el-radio-group>
      </el-form-item>
    </el-form>
    <template #footer>
      <el-button @click="cancel">{{ t("common.cancel") }}</el-button>
      <el-button type="primary" :disabled="!paths.length" @click="continueAction">{{
        t("components.dropAction.continue")
      }}</el-button>
    </template>
  </el-dialog>
</template>
