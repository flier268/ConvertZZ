<script setup lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ElMessage } from "element-plus";
import { computed, inject, onMounted, ref } from "vue";
import type { PlatformCapabilities } from "@shared/contracts";
import BrandMark from "../BrandMark.vue";
import { isDialogCancelled, promptForAppUpdate } from "../lib/appUpdate";
import { loadSettings } from "../lib/settings";

defineOptions({ name: "AboutPage" });

const capabilities = ref<PlatformCapabilities>();
const checkingUpdate = ref(false);
const replayOnboarding = inject<() => void>("replayOnboarding");
onMounted(async () => {
  capabilities.value = await invoke("platform_capabilities");
});

async function checkForUpdates(): Promise<void> {
  checkingUpdate.value = true;
  try {
    const settings = await loadSettings();
    await promptForAppUpdate({ includePreRelease: settings.checkPreReleaseUpdates });
  } catch (error) {
    if (!isDialogCancelled(error)) {
      ElMessage.error(error instanceof Error ? error.message : String(error));
    }
  } finally {
    checkingUpdate.value = false;
  }
}

type PlatformColumn = "windows" | "x11" | "wayland";

const platformColumns: Array<{ key: PlatformColumn; label: string }> = [
  { key: "windows", label: "Windows" },
  { key: "x11", label: "Linux X11" },
  { key: "wayland", label: "Linux Wayland" },
];

const differences: Array<{ feature: string } & Record<PlatformColumn, string>> = [
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

const conversionDifferences = [
  {
    title: "舊版字典",
    body: "舊版字典的優先權、長詞與保護詞規則保持不變。",
  },
  {
    title: "未命中字元",
    body: "未命中字元改由 cjk-convert-rs 處理。",
  },
  {
    title: "字形",
    body: "少數字形可能與 Windows LCMapStringEx 不同。",
  },
  {
    title: "編碼",
    body: "舊版 Encoding.Default 會依 Windows 系統碼頁改變。新版編碼工具改用明確指定的編碼。",
  },
  {
    title: "ZhConvert",
    body: "ZhConvert 是選用的網路服務。",
  },
];

const currentColumn = computed<PlatformColumn | null>(() => {
  const value = capabilities.value;
  if (!value) return null;
  if (value.platform === "windows") return "windows";
  if (value.displayServer === "x11") return "x11";
  if (value.displayServer === "wayland") return "wayland";
  return null;
});

function capabilityTone(value: string): "success" | "warning" | "info" | undefined {
  if (value === "完整") return "success";
  if (value === "停用" || value === "本版停用" || value === "依合成器") return "warning";
  if (value === "不適用") return "info";
  return undefined;
}
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
        <el-button @click="replayOnboarding?.()">重看系統導覽</el-button>
        <el-button :loading="checkingUpdate" @click="checkForUpdates">檢查更新</el-button>
        <el-button type="primary" @click="openUrl('https://github.com/flier268/ConvertZZ/issues')"
          >回報問題</el-button
        >
      </div>
    </header>
    <div class="page-fill-main about-stack">
      <el-card shadow="never" class="about-hero">
        <div class="about-mark"><BrandMark /></div>
        <div>
          <h2>新的核心。</h2>
          <p>
            Vue 與 Element Plus 提供介面。Rust 轉換核心在同程序提供相同的 Windows 與 Linux 結果。
          </p>
        </div>
      </el-card>
      <div class="about-diff-grid">
        <el-card shadow="never" class="about-panel">
          <template #header>
            <div class="about-panel-header">
              <div class="section-title">平台差異</div>
              <p class="muted">同一套轉換核心，桌面能力依作業系統與顯示伺服器而異。</p>
            </div>
          </template>
          <el-alert
            v-if="capabilities"
            class="capability-alert"
            :title="`目前環境：${capabilities.platform} / ${capabilities.displayServer}`"
            :description="
              capabilities?.limitations.length
                ? capabilities.limitations.join(' ')
                : '此環境沒有額外桌面限制。'
            "
            :type="capabilities?.limitations.length ? 'warning' : 'info'"
            :closable="false"
            show-icon
          />
          <table class="capability-table">
            <thead>
              <tr>
                <th scope="col">功能</th>
                <th
                  v-for="column in platformColumns"
                  :key="column.key"
                  scope="col"
                  :class="{ 'is-current': currentColumn === column.key }"
                >
                  {{ column.label }}<small v-if="currentColumn === column.key">目前</small>
                </th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="row in differences" :key="row.feature">
                <th scope="row">{{ row.feature }}</th>
                <td
                  v-for="column in platformColumns"
                  :key="column.key"
                  :class="{ 'is-current': currentColumn === column.key }"
                >
                  <el-tag
                    v-if="capabilityTone(row[column.key])"
                    :type="capabilityTone(row[column.key])"
                    effect="light"
                    size="small"
                  >
                    {{ row[column.key] }}
                  </el-tag>
                  <span v-else class="capability-detail">{{ row[column.key] }}</span>
                </td>
              </tr>
            </tbody>
          </table>
        </el-card>
        <el-card shadow="never" class="about-panel">
          <template #header>
            <div class="about-panel-header">
              <div class="section-title">轉換差異</div>
              <p class="muted">與 1.x 相比，規則與未命中字元的處理如下。</p>
            </div>
          </template>
          <ul class="conversion-diff-list">
            <li v-for="item in conversionDifferences" :key="item.title">
              <strong>{{ item.title }}</strong>
              <p>{{ item.body }}</p>
            </li>
          </ul>
        </el-card>
      </div>
    </div>
  </section>
</template>
