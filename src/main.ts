import { createApp } from "vue";
import "ant-design-vue/dist/reset.css";
import "./styles.css";
import App from "./App.vue";

const app = createApp(App);

app.config.errorHandler = (err, _instance, info) => {
  console.error("[Global error handler]", err, info);
  // Show a minimal fallback to the user instead of a blank screen
  const root = document.getElementById("app");
  if (root && !root.querySelector(".global-error-fallback")) {
    const fallback = document.createElement("div");
    fallback.className = "global-error-fallback";
    fallback.style.cssText =
      "display:flex;align-items:center;justify-content:center;min-height:100vh;font-family:sans-serif;color:#a61d24;";
    fallback.textContent =
      "An unexpected error occurred. Please restart the application.";
    root.appendChild(fallback);
  }
};

app.mount("#app");
