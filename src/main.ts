import { computed, createApp, h } from "vue";
import ElementPlus, { ElConfigProvider } from "element-plus";
import zhTw from "element-plus/es/locale/lang/zh-tw";
import elementEn from "element-plus/es/locale/lang/en";
import "element-plus/dist/index.css";
import "./styles.css";
import App from "./App.vue";
import FloatingBall from "./FloatingBall.vue";
import ToastOverlay from "./ToastOverlay.vue";
import { locale, resolveLocale, setLocale } from "./i18n";
import { loadSettings } from "./lib/settings";
import { applyUiLanguage, listenUiLanguage } from "./lib/uiLanguage";

const windowKind = new URLSearchParams(window.location.search).get("window");
document.documentElement.classList.toggle("floating-window", windowKind === "floating");
document.documentElement.classList.toggle("toast-window", windowKind === "toast");
const root = windowKind === "floating" ? FloatingBall : windowKind === "toast" ? ToastOverlay : App;
const elementLocale = computed(() => (locale.value === "en" ? elementEn : zhTw));

async function initialLanguage(): Promise<void> {
  void listenUiLanguage().catch(() => undefined);
  if (windowKind === "toast") return;
  const settings = await loadSettings().catch(() => undefined);
  if (windowKind === "floating") setLocale(resolveLocale(settings?.language));
  else await applyUiLanguage(settings?.language);
}

void initialLanguage().finally(() => {
  createApp({
    render: () => h(ElConfigProvider, { locale: elementLocale.value }, { default: () => h(root) }),
  })
    .use(ElementPlus)
    .mount("#app");
});
