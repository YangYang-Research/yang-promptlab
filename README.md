# PromptLab

<img src="src-tauri/icons/128x128.png" alt="PromptLab" width="72" height="72">

**AI security testing — find security vulnerabilities in LLM apps, chatbots, AI agents, MCP servers, and RAG systems. Mapped to OWASP (LLM, Agentic, MCP) and NIST AI RMF.**

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     PromptLab Desktop                       │
│  ┌─────────────┐   ┌──────────────┐   ┌─────────────────┐   │
│  │ Tauri 2     │◄──┤ React UI     │◄──┤ Rust workspace  │   │
│  │ WebView     │   │ Vite + TS    │   │ (crates/)       │   │
│  └─────────────┘   └──────────────┘   └────────┬────────┘   │
│                                                │            │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────▼────────┐   │
│  │ llama.cpp    │  │ Playwright   │  │ SQLite + vault   │   │
│  │ (in-process) │  │ (bundled)    │  │ ~/.promptlab/    │   │
│  └──────────────┘  └──────────────┘  └──────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

---

## Prerequisites

- **Node.js** 18+
- **Rust stable** ≥ 1.85 (`rustup default stable`)
- Platform toolchain for Tauri 2:

**macOS**

```bash
xcode-select --install
```

**Linux (Debian/Ubuntu)**

```bash
sudo apt install -y \
  build-essential libssl-dev libxdo-dev \
  libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev \
  libsoup-3.0-dev libjavascriptcoregtk-4.1-dev
```

**Windows** — Visual Studio C++ build tools, WebView2, and the scripts under `scripts/dev/os/windows/`.

---

## Quick start

```bash
git clone <repo-url> yang-promptlab
cd yang-promptlab
npm install
```

Desktop app (real Rust backend + IPC):

```bash
npm run tauri dev
```

The top-right indicator shows **Connected** when `health` / `app_info` succeed.

UI only (Vite, no Tauri IPC — empty/mock mode):

```bash
npm run dev
```

Opens `http://localhost:5173`.

Production frontend build:

```bash
npm run build
```

Packaged desktop binary:

```bash
npm run tauri build
```

Application data root: **`~/.promptlab/`** (SQLite, models, logs, auth vault). This is not Tauri’s default `app_data_dir`.

---

## Documentation

Engineering docs start at [`docs/README.md`](docs/README.md).

---

## Authorized use

PromptLab is a security testing tool. Use it only against systems you own or have explicit permission to test (pentest engagements, bug bounty programs, internal labs). Unauthorized scanning or exploitation is out of scope.

---

## License

[MIT](LICENSE)
