import { createApp } from "vue";
import ElementPlus from "element-plus";
import zhTw from "element-plus/es/locale/lang/zh-tw";
import "element-plus/dist/index.css";
import "./styles.css";
import App from "./App.vue";
import FloatingBall from "./FloatingBall.vue";
import ToastOverlay from "./ToastOverlay.vue";

const windowKind = new URLSearchParams(window.location.search).get("window");
document.documentElement.classList.toggle("floating-window", windowKind === "floating");
document.documentElement.classList.toggle("toast-window", windowKind === "toast");
const root = windowKind === "floating" ? FloatingBall : windowKind === "toast" ? ToastOverlay : App;
createApp(root).use(ElementPlus, { locale: zhTw }).mount("#app");
