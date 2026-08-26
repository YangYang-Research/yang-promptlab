# Security

PromptLab is an **authorized AI security testing** tool. This document covers (1) how to report vulnerabilities **in PromptLab itself**, and (2) how the product is meant to be used safely.

## Reporting a vulnerability in PromptLab

If you find a security issue in this repository or a released build (IPC exposure, secret handling, path traversal, unsafe deserialization, dependency RCE, etc.):

1. **Do not** open a public issue with exploit details.
2. Prefer a **private** report:
   - [GitHub Security Advisories](https://github.com/YangYang-Research/yang-promptlab/security/advisories/new) (if enabled for this repo), or
   - Contact the maintainers via the [YangYang-Research](https://github.com/YangYang-Research) org / private channel.
3. Include: affected version or commit, OS, steps to reproduce, impact, and a minimal PoC when possible.

We aim to acknowledge reports promptly and coordinate a fix before public disclosure.

### In scope (product)

- PromptLab desktop app (`src/`, `src-tauri/`, `crates/`)
- Local data under `~/.promptlab/` (SQLite, logs, AgentTrace, model registry)
- Secret handling (OS keychain, credential refs, `allow_insecure_tls` / proxy misuse that leaks secrets)
- Tauri IPC / command surface that can escalate beyond the operator’s intent on the same machine
- Supply-chain issues in **direct** dependencies with a clear exploit path into PromptLab

### Out of scope

- Using PromptLab against systems **without** written permission (that is misuse, not a product bug)
- False positives / false negatives from judges, Yazg, or attack catalog techniques
- “A clean scan means secure” — it does not; see the README Disclaimer
- Issues only in leftover / unused code paths that are not reachable from the shipping product (plugins host, crawl-era discovery) unless they are still linked into the desktop binary in a dangerous way
- Social engineering, physical access, or compromising the operator’s OS outside PromptLab

## Authorized use

Use PromptLab **only** against systems you own or have **written, explicit permission** to test (engagement, bug bounty, internal lab).

The attack catalog and harness probes are for **assessment**. You are responsible for targets, scope, and how results are used. Unauthorized scanning or exploitation may be illegal.

## Local data and secrets

- Workspace root: `~/.promptlab/` (not Tauri `app_data_dir`)
- Target credentials and API keys should live in the **OS keychain** (see [docs/AUTH.md](docs/AUTH.md))
- Do not commit `.env`, keychain dumps, or workspace DB/report files into PRs ([CONTRIBUTING.md](CONTRIBUTING.md))

## Related

- Product disclaimer: [README.md](README.md#disclaimer)
- Contributing: [CONTRIBUTING.md](CONTRIBUTING.md)
- License: [LICENSE](LICENSE) (MIT, as-is, no warranty)
