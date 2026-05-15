import React from "react";
import ReactDOM from "react-dom/client";
import { Provider } from "jotai";
import App from "./App";
import "./app.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Provider>
      <App />
    </Provider>
  </React.StrictMode>,
);
