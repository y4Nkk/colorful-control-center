import React from "react";
import ReactDOM from "react-dom/client";
import { bundledFontStylesheet } from "./api/client";
import App from "./App";
import { OsdHost } from "./components/OsdHost";
import { TooltipProvider } from "./components/Tooltip";
import "./styles/tokens.css";
import "./styles/base.css";

const fonts = document.createElement("link");
fonts.rel = "stylesheet";
fonts.href = bundledFontStylesheet();
document.head.append(fonts);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {new URLSearchParams(window.location.search).get("surface") === "osd" ? <OsdHost /> : <TooltipProvider><App /></TooltipProvider>}
  </React.StrictMode>,
);
