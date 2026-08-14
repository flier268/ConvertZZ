import { createApp } from "vue";
import ElementPlus from "element-plus";
import zhTw from "element-plus/es/locale/lang/zh-tw";
import "element-plus/dist/index.css";
import "./styles.css";
import App from "./App.vue";
import FloatingBall from "./FloatingBall.vue";

const floating = new URLSearchParams(window.location.search).get("window") === "floating";
document.documentElement.classList.toggle("floating-window", floating);
createApp(floating ? FloatingBall : App).use(ElementPlus, { locale: zhTw }).mount("#app");
