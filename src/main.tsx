import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import ShortcutSettings from "./ShortcutSettings";

const params = new URLSearchParams(window.location.search);
const windowType = params.get("window");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {windowType === "settings" ? <ShortcutSettings /> : <App />}
  </React.StrictMode>,
);
