<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ElMessage } from "element-plus";
import { inject, onMounted, ref } from "vue";
import type { PlatformCapabilities } from "@shared/contracts";
import BrandMark from "../BrandMark.vue";
import { isDialogCancelled, promptForAppUpdate } from "../lib/appUpdate";

const capabilities = ref<PlatformCapabilities>();
const checkingUpdate = ref(false);
const replayOnboarding = inject<() => void>("replayOnboarding");
onMounted(async () => {
  capabilities.value = await invoke("platform_capabilities");
});

async function checkForUpdates(): Promise<void> {
  checkingUpdate.value = true;
  try {
    await promptForAppUpdate();
  } catch (error) {
    if (!isDialogCancelled(error)) {
      ElMessage.error(error instanceof Error ? error.message : String(error));
    }
  } finally {
    checkingUpdate.value = false;
  }
}

const differences = [
  { feature: "文字、檔案與檔名轉換", windows: "完整", x11: "完整", wayland: "完整" },
  { feature: "ID3、APEv2、OGG、Opus 標籤", windows: "完整", x11: "完整", wayland: "完整" },
  { feature: "全域快捷鍵", windows: "完整", x11: "完整", wayland: "本版停用" },
  { feature: "自動複製與貼上", windows: "完整", x11: "完整", wayland: "停用" },
  { feature: "浮動球置頂", windows: "完整", x11: "完整", wayland: "依合成器" },
  { feature: "SendTo 捷徑", windows: "完整", x11: "不適用", wayland: "不適用" },
  {
    feature: "系統托盤",
    windows: "左鍵與選單完整",
    x11: "需 AppIndicator；使用選單開啟",
    wayland: "需 AppIndicator；使用選單開啟",
  },
  {
    feature: "憑證庫",
    windows: "Windows Credential Manager",
    x11: "Secret Service",
    wayland: "Secret Service",
  },
  {
    feature: "自動更新",
    windows: "安裝程式可下載安裝",
    x11: "AppImage 可下載安裝；DEB／RPM 開啟下載頁",
    wayland: "AppImage 可下載安裝；DEB／RPM 開啟下載頁",
  },
];
</script>

<template>
  <section class="page-stack">
    <header class="page-header">
      <div>
        <p class="eyebrow">ABOUT</p>
        <h1>ConvertZZ 2.0</h1>
        <p>GPL-3.0 跨平台中文轉換工具。</p>
      </div>
      <div class="header-actions">
        <el-button @click="replayOnboarding?.()">重看系統導覽</el-button
        ><el-button :loading="checkingUpdate" @click="checkForUpdates">檢查更新</el-button
        ><el-button type="primary" @click="openUrl('https://github.com/flier268/ConvertZZ/issues')"
          >回報問題</el-button
        >
      </div>
    </header>
    <el-card shadow="never" class="about-hero"
      ><div class="about-mark"><BrandMark /></div>
      <div>
        <h2>新的核心。</h2>
        <p>Vue 與 Element Plus 提供介面。</p>
        <p>Node.js sidecar 提供相同的 Windows 與 Linux 轉換結果。</p>
      </div></el-card
    >
    <el-card shadow="never"
      ><template #header><div class="section-title">平台差異</div></template
      ><el-table :data="differences"
        ><el-table-column prop="feature" label="功能" min-width="240" /><el-table-column
          prop="windows"
          label="Windows" /><el-table-column prop="x11" label="Linux X11" /><el-table-column
          prop="wayland"
          label="Linux Wayland" /></el-table
      ><el-alert
        v-if="capabilities?.limitations.length"
        class="capability-alert"
        :title="`目前環境：${capabilities.platform} / ${capabilities.displayServer}`"
        :description="capabilities.limitations.join(' ')"
        type="warning"
        :closable="false"
    /></el-card>
    <el-card shadow="never"
      ><template #header><div class="section-title">轉換差異</div></template>
      <p>舊版字典的優先權、長詞與保護詞規則保持不變。</p>
      <p>未命中字元改由 cjk-conv 處理。</p>
      <p>少數字形可能與 Windows LCMapStringEx 不同。</p>
      <p>舊版 Encoding.Default 會依 Windows 系統碼頁改變。新版編碼工具改用明確指定的編碼。</p>
      <p>ZhConvert 是選用的網路服務。</p></el-card
    >
  </section>
</template>
