import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { OsdHost } from "./components/OsdHost";
import "./styles/tokens.css";
import "./styles/base.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {new URLSearchParams(window.location.search).get("surface") === "osd" ? <OsdHost /> : <App />}
  </React.StrictMode>,
);
