## Review 0001 - Decision Summary

**Claim Token:** `954F0417-68BB-4FC7-ACE8-321C3DA3339C`
**Patch Present:** no (report-only)
**Integrity:** OK


---

**Context note:** This review was completed `2026-05-02T02:10:25Z` — *before* the transync integration landed (commits `d9f4eed`..`511a575` on 2026-05-04). Many findings are now superseded. Triage applies that lens consistently.

### ACCEPTED — fix in this session (42 issues)

Critical security:
| Issue ID | Title | Severity |
|---|---|---|
| R0001-0101 | Live OpenAI key in `scripts/{ls,ms}-up.env` | Critical |

Real config/runtime bugs (small fixes):
| Issue ID | Title | Severity |
|---|---|---|
| R0001-0102 | `Cargo.lock` not tracked but CI uses `--locked` | Critical |
| R0001-0103 | First-run config bootstrap loads defaults before writing | Critical |
| R0001-0104 | MCP `--config` loads one path but bootstraps another | Critical |
| R0001-0105 | Malformed config files silently fall back to defaults | Critical |
| R0001-0106 | Zero buffer capacity hangs ingest forever | Critical |
| R0001-0111 | Explicit retry can't bypass cooldown | High |
| R0001-0112 | `PROTOCOL_VERSION` stayed at 1 after frame variants added | High |
| R0001-0113 | Listener accepts handshakes without checking version | High |
| R0001-0119 | OpenCode plugin can submit empty messages | High |
| R0001-0120 | Unknown ingest source silently → ClaudeCode | High |
| R0001-0122 | Listener `/api/active-profile` mutation is dead state | High |
| R0001-0127 | Listener pause/resume loses live events | High |
| R0001-0129 | `TranslationOutcome::Failed` lacks profile name | High |
| R0001-0130 | MCP normalizer accepts empty messages | Medium |
| R0001-0131 | Listener ignores `logging.level` | Medium |
| R0001-0133 | `ui.live_push_transport` is an inert config field | Medium |
| R0001-0134 | `/api/settings` hardcodes `~/.config` path | Medium |
| R0001-0135 | Profile listing hides load errors | Medium |
| R0001-0138 | Listener collapses unknown failure codes to `other` | Medium |
| R0001-0139 | Web UI ignores SSE `status` events | Medium |
| R0001-0148 | `extension_bridge.rs` is unused 11-line stub | Medium |
| R0001-0149 | Listener `profiles.rs` re-export comment stale | Medium |
| R0001-0153 | Invalid log level silently → info | Medium |
| R0001-0157 | Dedup capacity zero-multiplier disables filtering | Medium |
| R0001-0161 | MCP example uses literal tilde path | Medium |
| R0001-0162 | MCP relay URL example omits `/ingest` | Medium |
| R0001-0163 | `--openai-api-key` documented but not implemented | Medium |

Doc/comment one-liners:
| Issue ID | Title | Severity |
|---|---|---|
| R0001-0164 | `dualview.html` has LLM-Trans branding | Low |
| R0001-0166 | Empty-profile UI points to wrong dir | Low |
| R0001-0167 | Default profile TOML comment stale | Low |
| R0001-0168 | DCR-002 has unchecked actions but is closed | Low |
| R0001-0170 | README test totals stale | Low |
| R0001-0172 | Quick Start references non-existent issue ID | Low |
| R0001-0173 | Troubleshooting documents nonexistent Listener flag | Low |
| R0001-0174 | Troubleshooting default model mismatch | Low |
| R0001-0176 | `contracts.md` describes nonexistent `project_root_redacted` field | Low |
| R0001-0180 | `config-surface.md` says relay flags are TBD | Low |
| R0001-0185 | `module-map.md` says relay sender is no-op stub | Low |
| R0001-0188 | CHANGELOG mixes old/new translation eras | Low |
| R0001-0189 | CHANGELOG localhost-impersonation note wrong | Low |
| R0001-0191 | `lsw-up.sh` `TUNNEL_LOG` no default | Low |

### REJECTED — superseded by transync integration (4 issues)

| Issue ID | Title | Why rejected |
|---|---|---|
| R0001-0142 | Sentence pairing duplicated | Sentence-pair code path entirely deleted in T12 (commit `c72e88f`); both old impls are gone post-transync. |
| R0001-0150 | MCP `translation.provider` not honored | Fixed: `translator_factory::build_translator(provider_id, ...)` matches on the provider string and errors on unknown ids (commits `2836f14` + `327d30d`). |
| R0001-0151 | Profile `provider_hint` ignored | Fixed: field renamed to `provider` in schema-v2 and now drives translator dispatch (commit `911a5ca`). |
| R0001-0158 | Implicit two-frame replay contract | Already resolved by R0002-0007: contract is now spelled out on `Frame::ListenerHello`'s doc, anchored in `design-baseline.md`, and pinned by `r0002_0007_b_replay_ordering.rs`. |

### REJECTED — Relay deferred to M3 (19 issues)

`bins/resp-translate-relay` was excluded from the workspace in T8 (commit `a602cce`) because its pipeline still uses pre-transync types. It will be rewritten when M3 reactivates. All Relay-only findings below should be re-reviewed at that point.

| Issue ID | Title | Severity |
|---|---|---|
| R0001-0108 | Relay discards Listener retry/translate frames | High |
| R0001-0109 | Relay retry endpoint returns false success | High |
| R0001-0110 | Relay translation config + pipeline are dead code | High |
| R0001-0116 | MCP relay sender silently disables enabled relay | High |
| R0001-0117 | Relay fanout from MCP fire-and-forget | High |
| R0001-0123 | Relay missing-Origin reject vs ADR-0009 | High |
| R0001-0124 | Relay default 0.0.0.0 bind exposes captures | High |
| R0001-0125 | Relay UI loads controls backed by missing APIs | High |
| R0001-0126 | Frontend ignores Relay live-event names | High |
| R0001-0132 | Relay ignores `logging.level` | Medium |
| R0001-0140 | Relay `captured` rendered as pending translation | Medium |
| R0001-0144 | EventBuffer duplicated MCP/Relay | Medium |
| R0001-0145 | Listener registry duplicated MCP/Relay | Medium |
| R0001-0146 | Relay defines stale local profile model | Medium |
| R0001-0147 | Relay event store appends instead of upsert | Medium |
| R0001-0155 | Listener route selection silently omits enabled-but-incomplete Relay | Medium |
| R0001-0156 | Listener outbound queue route-agnostic | Medium |
| R0001-0159 | Relay retry request uses `Option<String>` event_id | Medium |
| R0001-0160 | Relay pipeline tests exercise dead code | Medium |

### DEFERRED (3 issues)

| Issue ID | Title | Research Needed |
|---|---|---|
| R0001-0154 | TUI is still a stub | Is TUI a documented product surface or pruning candidate? |
| R0001-0165 | Web pages load Google Fonts externally | Is offline operation a goal? |
| R0001-0190 | Local up-scripts duplicated and stale | Consolidate now or wait for further topology changes? |

### ESCALATION CANDIDATES (22 issues)

Each is genuinely ACCEPT but the fix is too large or too architectural for this session's scope. Routing decision in Phase 2.

| Issue ID | Title | Severity | Why escalating |
|---|---|---|---|
| R0001-0107 | Extension trusts any localhost process | Critical | Architecture change to extension pairing model. |
| R0001-0114 | Extension reports success on injection failure | High | Cross-cutting extension/UI/listener change. |
| R0001-0115 | Extension nonce no replay cache | High | Adds a new state surface in the extension. |
| R0001-0118 | OpenCode plugin broad event subscriptions | High | Privacy/scope policy change; needs upstream-API review. |
| R0001-0121 | `/api/settings` schema mismatch with UI | High | Either rewrite the API or rip out UI; depends on settings-page direction. |
| R0001-0128 | Listener race-prone dual-task path | High | Refactor — single-sequencer/actor for event-store updates. |
| R0001-0136 | Listener still loads profile state (DCR-002 conflict) | Medium | Either delete listener-side profile loader or amend DCR-002 (the listener-side state is currently used for the read-only Profiles page). |
| R0001-0137 | Pending output represented as `SkippedEvent` | Medium | Type-vocabulary refactor; touches protocol + listener + UI. |
| R0001-0141 | `app.js` is a 1443-line monolith | Medium | Module split (api-client, event-store, render, …). |
| R0001-0143 | `translator-utils.mjs` carries unused functions | Medium | Clean-up after sentence-pair removal in T12; might also delete `dualview.js` + `markdown-render.mjs` entirely. |
| R0001-0152 | Documented config validation rules not implemented | Medium | Adds shared validation crate/layer. |
| R0001-0169 | README architecture summary pre-DCR-002 | Low | Larger doc rewrite; needs pass after transync integration anyway. |
| R0001-0171 | Quick Start tells users to put OpenAI in Listener | Low | Doc rewrite tied to current MCP-owned config path. |
| R0001-0175 | `contracts.md` lacks current frame variants | Low | Cross-doc rewrite. |
| R0001-0177 | `rough-schema.md` assigns translation to Listener/Relay | Low | Cross-doc rewrite. |
| R0001-0178 | `rough-schema.md` omits current variants/profile fields | Low | Cross-doc rewrite. |
| R0001-0179 | `config-surface.md` puts translation under Listener | Low | Cross-doc rewrite. |
| R0001-0181 | `bootstrap-config.md` pre-DCR-002 | Low | Cross-doc rewrite. |
| R0001-0182 | `persistence-and-files.md` cache in Listener | Low | Cross-doc rewrite. |
| R0001-0183 | `source-of-truth-table.md` obsolete | Low | Cross-doc rewrite. |
| R0001-0184 | `module-map.md` stale (protocol + cache key) | Low | Cross-doc rewrite. |
| R0001-0186 | `workspace-topology.md` stale | Low | Cross-doc rewrite. |
| R0001-0187 | `status.md` inconsistent | Low | Cross-doc rewrite. |

(R0001-0187 makes 22 in this table; if you're counting — yes, double-checked.)

---

## Phase 2 — DEFER & Escalation Discussion

### Routes (legend)
| Code   | Route                       | Effect                                                                                  |
|--------|-----------------------------|-----------------------------------------------------------------------------------------|
| fix    | Accept & fix                | Apply in Phase 3, no record. Overrides "too big" flag for escalation candidates.        |
| reject | Reject                      | Drop the finding; may be recorded as ADR in Phase 5 if significant.                     |
| track  | Track in open-issues.md     | Add OI entry; finding remains pending.                                                  |
| adr    | Record as ADR               | Archive rationale to docs/decisions/.                                                   |
| drop   | Drop                        | Close without fix, OI, or ADR.                                                          |

### Items at a glance

| Issue       | Title                                                          | Category    | Gate suggests |
|-------------|----------------------------------------------------------------|-------------|---------------|
| R0001-0107  | Extension trusts any localhost                                 | ESCALATION  | track         |
| R0001-0114  | Extension reports success on injection failure                 | ESCALATION  | track         |
| R0001-0115  | Extension nonce no replay cache                                | ESCALATION  | track         |
| R0001-0118  | OpenCode plugin broad subscriptions                            | ESCALATION  | track         |
| R0001-0121  | `/api/settings` schema mismatch                                | ESCALATION  | fix           |
| R0001-0128  | Listener race-prone dual-task path                             | ESCALATION  | track         |
| R0001-0136  | Listener still loads profile state (DCR-002)                   | ESCALATION  | adr           |
| R0001-0137  | Pending output as `SkippedEvent`                               | ESCALATION  | track         |
| R0001-0141  | `app.js` monolith                                              | ESCALATION  | track         |
| R0001-0143  | Unused frontend utilities                                      | ESCALATION  | fix           |
| R0001-0152  | Documented config validation not implemented                   | ESCALATION  | track         |
| R0001-0154  | TUI is a stub                                                  | DEFER       | track         |
| R0001-0165  | External Google Fonts                                          | DEFER       | drop          |
| R0001-0190  | Up-scripts duplicated/stale                                    | DEFER       | track         |
| R0001-0169  | README architecture pre-DCR-002                                | ESCALATION  | track         |
| R0001-0171  | Quick Start in Listener                                        | ESCALATION  | track         |
| R0001-0175  | `contracts.md` lacks frame variants                            | ESCALATION  | track         |
| R0001-0177  | `rough-schema.md` Listener/Relay                               | ESCALATION  | track         |
| R0001-0178  | `rough-schema.md` omits variants                               | ESCALATION  | track         |
| R0001-0179  | `config-surface.md` translation under Listener                 | ESCALATION  | track         |
| R0001-0181  | `bootstrap-config.md` pre-DCR-002                              | ESCALATION  | track         |
| R0001-0182  | `persistence-and-files.md` cache in Listener                   | ESCALATION  | track         |
| R0001-0183  | `source-of-truth-table.md` obsolete                            | ESCALATION  | track         |
| R0001-0184  | `module-map.md` stale                                          | ESCALATION  | track         |
| R0001-0186  | `workspace-topology.md` stale                                  | ESCALATION  | track         |
| R0001-0187  | `status.md` inconsistent                                       | ESCALATION  | track         |

### Discussion

#### Group A — Extension hardening (R0001-0107, 0114, 0115, 0118)

**R0001-0107 — Extension trusts any localhost process**

Today: `extension/service-worker.js` accepts any `127.0.0.1`/`localhost` origin and fetches the pairing token from the sender origin itself.

Considerations: Real attack class — any local web server can impersonate the Listener. The fix is architectural: pin the expected origin in extension storage/manifest, require an out-of-band pairing secret, and not fetch the trust anchor from the origin being authenticated. This is M2 work and overlaps with R0001-0114, R0001-0115, R0001-0124, and the existing R0001-0006 (Relay UI security). A coherent extension-security pass is its own slice.

Gate suggests: **track** (single OI to bundle the four extension findings, addressed as a focused security slice).

**R0001-0114 — Extension reports success on injection failure**

Today: `service-worker.js` awaits `sendToContentScript` but returns `{ ok: true }` regardless of outcome.

Considerations: Real correctness issue. Fix is small in code (propagate the structured error from the content script) but requires a UI surface to display the error. Bundle with the extension hardening pass.

Gate suggests: **track**.

**R0001-0115 — Extension nonce no replay cache**

Today: `validateMessage` requires `nonce` + `ts` and only checks the time window.

Considerations: Replay window is currently ±60s. Implementing a bounded seen-nonce cache is small but lives in the extension — bundle with R0001-0107.

Gate suggests: **track**.

**R0001-0118 — OpenCode plugin broad subscriptions**

Today: subscribes to `message.part.updated`, `message.updated`, `session.idle`, with a fallback that JSON-stringifies the event when no expected field matches.

Considerations: Privacy + reliability concern. Needs an OpenCode-API audit to determine the canonical assistant-message field. Adjacent to R0001-0119 / R0001-0130 (empty messages) but more architectural — those are quick gates, this is "what's the right subscription set."

Gate suggests: **track**.

#### Group B — Documentation refresh post-transync (R0001-0169, 0171, 0175, 0177, 0178, 0179, 0181, 0182, 0183, 0184, 0186, 0187)

All twelve are doc-staleness findings on architecture/spec/runbook documents. Each is independently valid; none is small. The right move is **one** doc-refresh sweep after the transync integration soaks (the integration just landed; touching all these docs now would be premature since the architecture might still iterate).

Gate suggests **track** for all twelve as a single grouped OI ("docs/architecture and root README/Quick Start refresh post-transync") — or **drop** if you'd rather wait until the next major architectural change forces them anyway.

Per-item suggestion: **track** uniformly. Override individually if some are urgent.

#### Group C — Frontend / Listener internal architecture (R0001-0121, 0128, 0136, 0137, 0141, 0143)

**R0001-0121 — `/api/settings` schema mismatch**

Today: API returns `translation: { executor, note }`; UI expects `provider`, `model`, `request_timeout_ms`, `max_tokens`, `api_key`.

Considerations: The UI never reads `translation.api_key` directly post-DCR-002 (and `max_tokens` is gone post-T7). Likely the easiest fix is to drop the UI fields the API doesn't actually serve and surface what the listener does know (cwds, profile dir, etc.). Small fix.

Gate suggests: **fix** (drop dead UI fields; align to current settings shape).

**R0001-0128 — Listener race-prone dual-task path**

Today: pending and upstream tasks publish independently; `publish_upstream` polls 20× / 25 ms to find existing events.

Considerations: The polling workaround works but the underlying race is real. Refactor to a single sequencer is medium-sized; benefits long-term reliability. Out of scope this session.

Gate suggests: **track**.

**R0001-0136 — Listener still loads profile state (DCR-002 conflict)**

Today: Listener calls `ensure_default_profile_at` + `load_all_profiles` for the read-only Profiles page UI.

Considerations: DCR-002 says Listener doesn't load profiles. In practice the listener uses them as a read-only UI surface; the architectural decision needs to record that exception or be tightened. This is a write-down decision more than a code change.

Gate suggests: **adr** (record the actual boundary).

**R0001-0137 — Pending output as `SkippedEvent`**

Today: `pending_outputs` constructs `SkippedEvent` with `translation_status="awaiting"`.

Considerations: Type-vocabulary refactor across protocol, listener, UI. Real cleanup but no functional bug. Defer to a future refactor.

Gate suggests: **track**.

**R0001-0141 — `app.js` is a 1443-line monolith**

Today: Single file with API access, state, rendering, SSE, bridge, dual view, filters, kbd, copy, settings.

Considerations: Big refactor. After T12's dual-view rewrite the file is now ~1300 lines and several callers were deleted. Worth doing, but a separate frontend slice.

Gate suggests: **track**.

**R0001-0143 — Frontend translator-utils + dualview.js + markdown-render.mjs unused**

Today: `web/js/translator-utils.mjs`, `web/js/dualview.js`, `web/js/markdown-render.mjs` exist but are no longer imported anywhere after T12.

Considerations: Now genuinely dead code post-transync. Easy delete. R0001-0142 (sentence-pair duplication) is already moot for the same reason.

Gate suggests: **fix** (delete the three files + `web/dualview.html` if it references them; verify webui-assets builds).

**R0001-0152 — Documented config validation rules not implemented**

Today: `docs/architecture/config-surface.md` lists validation rules; loaders deserialize without a validation phase.

Considerations: Several Phase 3 fixes I'm doing today (R0001-0106 zero-buffer, R0001-0153 invalid log level, R0001-0157 zero dedup multiplier) are partial implementations of this. Whether to add a shared validation framework or stay with point fixes is a judgment call.

Gate suggests: **track** (consolidate after the point fixes accumulate enough surface).

#### Standalone items

**R0001-0154 — TUI is a stub**

Today: `tui.rs` logs once and sleeps for 1s; `tui/{status,inbox,listeners}.rs` are placeholders.

Considerations: Documented as part of MCP product surface but never implemented. Either build a minimal real TUI or remove from documented scope.

Gate suggests: **track** (decision deferred — minimal status TUI is genuinely useful for ops, but not blocking).

**R0001-0165 — Web pages load Google Fonts from external origin**

Today: `index.html` and `dualview.html` load Inter + JetBrains Mono from fonts.googleapis.com.

Considerations: For a local-first translation tool this is undesirable but not security-critical; offline operation isn't a stated goal. Bundling fonts adds binary weight. Likely intentional for now.

Gate suggests: **drop** (not worth tracking until offline operation is a stated requirement).

**R0001-0190 — Up-scripts duplicated and stale**

Today: `scripts/{ls,ms,dev,lsw}-up.sh` overlap; some still describe pre-DCR-002 behavior.

Considerations: Local helper scripts with stale comments. Harmless drift. The R0001-0101 fix today partially touches these — see Phase 3 plan.

Gate suggests: **track**.

### Decisions

Reply with the same block, edited only where you disagree:

```
# Group A — extension hardening
R0001-0107: track
R0001-0114: track
R0001-0115: track
R0001-0118: track

# Group B — docs refresh post-transync
R0001-0169: track
R0001-0171: track
R0001-0175: track
R0001-0177: track
R0001-0178: track
R0001-0179: track
R0001-0181: track
R0001-0182: track
R0001-0183: track
R0001-0184: track
R0001-0186: track
R0001-0187: track

# Group C — frontend / listener internals
R0001-0121: fix
R0001-0128: track
R0001-0136: adr
R0001-0137: track
R0001-0141: track
R0001-0143: fix
R0001-0152: track

# Standalone
R0001-0154: track
R0001-0165: drop
R0001-0190: track
```
