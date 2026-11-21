import React from "react";
import ReactDOM from "react-dom/client";
// The design tokens (generated): before App.css, which uses them.
import "./tokens.css";
import App from "./App";
import { LibraryProvider } from "./lib/library";
import { DownloadsProvider } from "./lib/downloads";
import { StationsProvider } from "./lib/stationsContext";
import { applyTheme, savedTheme } from "./lib/theme";

// Before the first paint, so the app doesn't flash in the other theme.
applyTheme(savedTheme());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <LibraryProvider>
      <DownloadsProvider>
        <StationsProvider>
          <App />
        </StationsProvider>
      </DownloadsProvider>
    </LibraryProvider>
  </React.StrictMode>,
);
