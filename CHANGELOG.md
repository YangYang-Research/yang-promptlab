# Changelog

All notable changes to Prompt Lab are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.3] - 2026-10-25

### Added

- Agents screen lists each agent with its prompts, tools, attempt budget, and temperature.
- Agent Trace records every agent run. Reflection, retries, and later model calls are spans on that same trace.
- Jailbreak follow-up output is reviewed by ReflectionAgent.
- Shared output reflection and a per-agent attempt budget (1 to 10, default 3).
- Per-agent temperature. An agent override wins over the model temperature.
- LLM request and response logging for agent calls.
- One catalog for agents, prompts, and tools (`promptlab-registry`).
- Prompt Lab Jailbreak agent for goal-chase planning and follow-up probes.
- Live logs show the agent name and a local timestamp, and follow the selected log file.

### Changed

- Scenario agent is now Jailbreak. History stays Scenario history.
- Agent ids that name an agent end with `_agent`.
- Usage heading is Detail. The agent-count badge is gone.
- Live log messages show at most three lines. The time column shows the local date and time.
- Agent detail shows the display name above the id.
- Agent cards show attempt and temperature, and wrap long names inside the card.
- Prompt Lab, Agents, and Agent detail have a refresh action.
- Scan and studio features call the specialist agent directly.

## [0.1.2] - 2026-10-14

### Added

- Prompt Lab keeps generate state when switching tabs.

### Changed

- Updater download flow and the toast after a successful update.

### Fixed

- Open Dependabot advisories.
- Release workflow: platform jobs, update manifest arch names, release-note range, and empty Apple signing env.

## [0.1.1] - 2026-10-11

### Added

- Prompt Lab history and an R2 backup of update artifacts.
- YangYang provider support and the application logo.
- Additional attack mutators, including LLM and obfuscation types.
- `status_comment` on findings.

### Changed

- LLM response parser handles JSON and verdict text more reliably.
- Application icons and favicons.

## [0.1.0] - 2026-08-27

### Added

- Initial desktop release.

[Unreleased]: https://github.com/YangYang-Research/yang-promptlab/compare/v0.1.3...HEAD
[0.1.3]: https://github.com/YangYang-Research/yang-promptlab/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/YangYang-Research/yang-promptlab/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/YangYang-Research/yang-promptlab/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/YangYang-Research/yang-promptlab/releases/tag/v0.1.0
