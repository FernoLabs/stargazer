# Ferno Pulsar — SCF $100k grant-readiness plan

Research and repository review snapshot: **2026-07-18**

## Decision

Ferno Pulsar is **not ready for a full $100k SCF Build submission today**, but it has a credible foundation. The inherited FireWatch product is tested and feature-rich, and the Stellar architecture is unusually well thought through. The blocking issue is evidence: the repository does not yet run any Stellar code, has no Stellar demo or usage, and does not document team capacity, customer validation, differentiation, or grant economics.

The right immediate move is to **submit the SCF interest form now**, while building one narrow Stellar end-to-end proof and collecting protocol validation before the full application.

Use this funding language:

> **Request: $100,000 worth of XLM through the SCF Build Award.**

Do not call it a “100k USDC grant.” SCF Build budgets are expressed in USD value but paid in XLM at the applicable XLM/USD settlement price on each payment date.

## Current SCF facts

- The current program is **Stellar Community Fund v7 Build**.
- The maximum Build Award is **$150,000 worth of XLM**.
- The SCF site currently lists **SCF #45** with a full-submission deadline of **August 16, 2026**.
- The interest form is reviewed first; eligible teams are then invited to submit to a round. It should be sent as soon as possible.
- Projects should normally ship in 3–6 months and must not exceed six months.
- Funding is released in four payments: 10% on award, 20% after MVP, 30% after testnet/equivalent readiness, and 40% after a usable mainnet launch.
- A $100k award therefore pays nominally **$10k / $20k / $30k / $40k worth of XLM**.
- The application must contain three verifiable delivery tranches, with the final tranche proving a usable, discoverable mainnet product—not deployment alone.
- Funding is for future Stellar development. It cannot reimburse past work.
- Audit fees, marketing/user acquisition, token giveaways, legal/entity costs, unrelated work, and general overhead are excluded. Eligible work includes indexers, backend and frontend development, testing, QA, monitoring, deployment engineering, APIs/SDKs, and documentation.
- A referral is optional, but SCF says referral signals materially help its review.
- The application must be self-contained. Reviewers say they evaluate the information in the submission, so essential facts must not exist only behind external links.

Official sources:

- [SCF awards page and interest form](https://communityfund.stellar.org/awards)
- [SCF Build handbook](https://stellar.gitbook.io/scf-handbook/scf-awards/build-award)
- [Submission criteria](https://stellar.gitbook.io/scf-handbook/scf-awards/build-award/submission-criteria)
- [Budget and deliverable guidelines](https://stellar.gitbook.io/scf-handbook/scf-awards/build-award/budget-and-deliverable-guidelines)
- [Official submission rules](https://stellar.gitbook.io/scf-handbook/scf-awards/official-rules-for-submissions)
- [SCF v7 overview](https://stellar.org/blog/ecosystem/introducing-scf-v7)

## Repository assessment

| Area | Current evidence | Readiness |
| --- | --- | --- |
| Existing product | FireWatch has a Rust indexer/API, SQLite persistence, alert rules, Telegram delivery, Svelte dashboard, deployment and backup workflows | Strong inherited base |
| Tests/build | 152 backend tests pass; Svelte check reports no errors or warnings; production frontend build succeeds | Strong, but tests are predominantly Solana-specific |
| Stellar architecture | Detailed ingestion, replay, data-model, upgrade, registry, and multisig decisions exist | Strong design |
| Stellar implementation | The README says it does not index Stellar; `ingester/` has no executable, fixture, or Go module | Blocking |
| Public proof | No Stellar contract indexed, no Wasm/version history, no upgrade alert, no demo URL | Blocking |
| Validation | No Stellar interviews, pilot letters, testimonials, or usage data are recorded | Blocking |
| Team | No bios, roles, FTE allocation, relevant credentials, or availability are recorded | Blocking |
| Differentiation | No application-ready comparison with already funded Stellar security and data products | Blocking |
| Grant plan | No SCF-specific milestones, budget basis, KPIs, adoption plan, or sustainability case | Blocking |
| Repository presentation | Stellar work is uncommitted, `main` still points to the FireWatch baseline, and there is no writable Ferno Pulsar `origin` | Blocking for reviewer confidence |
| Security hygiene | MIT license and CI exist; two inherited authorization/origin-control issues were found, the frontend audit reports 1 low and 5 high findings, and strict Clippy currently fails | Blocking remediation |

Key local evidence:

- [README](../README.md) honestly describes the migration status and first vertical slice.
- [Stellar fork plan](stellar-fork-plan.md) defines the architecture, event identity, data model, and phased exit criteria.
- [Archive/live indexing plan](stellar-archive-and-live-indexing.md) defines continuity, replay, idempotency, and operational invariants.
- [Registry research](stellar-contract-registry-research.md) defines evidence-backed protocol attribution.
- [Migration inventory](stellar-migration-inventory.md) separates reusable product capability from Solana-specific code.
- [Ingestion boundary](../ingester/README.md) is currently design-only.

## Recommended positioning

### One sentence

> Ferno Pulsar is a Stellar mainnet security control-plane monitor that builds a replayable history of contract and Wasm deployments, upgrades, authorization changes, account signer/threshold changes, and protocol administration events, then turns those facts into evidence-backed API, dashboard, and Telegram alerts.

### The wedge

Ferno Pulsar should not be pitched as another explorer, generic indexer, audit tool, or AI vulnerability scanner. Its defensible wedge is **post-deployment operational security**:

- independently confirm upgrades using both the `executable_update` system event and the contract-instance ledger diff;
- retain immutable old/new Wasm, spec, and metadata history;
- surface successful invocation authorization context instead of guessing a universal upgrade authority;
- monitor classic-account signer and threshold changes linked to protocol administration;
- attach source and confidence to every protocol/contract attribution;
- provide deterministic replay and incident-ready evidence through a dashboard, API, and alerts.

### Competitive risk that must be addressed

SCF has already funded adjacent products:

- [Bevor — $102k, awarded](https://communityfund.stellar.org/submissions/rec0TflVXwHWBEZiq): AI/static analysis plus chain-scale monitoring;
- [Solarkraft — $50k, awarded](https://communityfund.stellar.org/submissions/rec05ielYqgWQu4OH): specification-based runtime monitoring;
- [Cyvers — $50k, awarded](https://communityfund.stellar.org/submissions/rec06I0eNKQ4gWeOq): ML-based real-time threat detection and response;
- [Obsrvr Flow — $120k, awarded](https://communityfund.stellar.org/submissions/recC5hC2vie4iOctB): generic Stellar/Soroban streaming and data pipelines;
- [Inference — $149.7k, awarded](https://communityfund.stellar.org/submissions/recXo2i8gaESnXd2Y): formal specification tooling.

The application must explain why protocol teams still need deterministic change-control evidence and actionable administration alerts after using those products. Written validation from protocol operators is much stronger than a theoretical comparison.

## Track decision

**Provisional choice: Open Track, subject to written confirmation from SCF.**

Why:

- Integration Track expects an approved ecosystem building block to be central to the funded scope; merely consuming Stellar RPC or Galexie is unlikely to qualify.
- RFP Track only accepts work answering an active published RFP. The relevant prior RFPs for a Soroban-first explorer and specialized reverse engineering are currently marked closed, and the currently visible open RFPs do not match Ferno Pulsar.
- Open Track is the plausible route if Ferno Pulsar is presented as a differentiated security product for protocol operators rather than general-purpose developer infrastructure.

This classification is uncertain because the Open Track also directs developer tooling toward RFPs. Resolve it before investing in a full application.

- [ ] Submit the interest form with the product sentence above and provisional Open Track selection.
- [ ] Ask SCF directly: “Should a post-deployment protocol security monitoring product apply through Open, or wait for a security/indexing RFP?”
- [ ] Join the Stellar Developer Discord, share the architecture and ask in the SCF channel.
- [ ] Seek a referrer code from an ecosystem participant who has reviewed the project; do not solicit a blind referral.
- [ ] If SCF says the project is RFP-only, do not force an Open application. Propose the gap through the quarterly RFP/ideas process and target the next round.

## Critical path to the application

### P0 — inherited security blockers

These findings are in the FireWatch baseline. They should be fixed and regression-tested before exposing or reusing the admin/API surface for a Ferno Pulsar demo.

1. **Backend admin authorization is not enforced as route middleware.** The backend registers destructive `/api/admin/*` mutation routes in `backend/src/http.rs`, but Basic Auth is checked only by the separate `/api/admin/auth/check` handler. The frontend proxy protects a hard-coded subset of routes and currently omits cleanup, sync, and backfill paths. A caller reaching those proxy paths can receive the trusted origin header without first passing the frontend admin-password check.
2. **Origin protection fails open.** `require_origin_token` allows every API request when `ORIGIN_API_TOKEN` is unset, while the environment example describes that token as optional. An exposed or misconfigured backend therefore loses its origin boundary silently.

- [ ] Move admin authentication/authorization into backend middleware covering **all** `/api/admin/*` routes; default-deny the namespace.
- [ ] Keep the frontend check only as UX/defense in depth, not as the security boundary.
- [ ] Add negative integration tests for every admin route: no credentials, bad credentials, valid credentials, and missing backend admin configuration.
- [ ] Require `ORIGIN_API_TOKEN` in production/exposed modes or refuse startup; make any local-development bypass explicit and loopback-only.
- [ ] Add tests proving missing, malformed, and incorrect origin tokens fail closed.
- [ ] Rotate the origin token after deploying the fix and review logs for unexpected calls if the inherited service has been publicly reachable.
- [ ] Perform a focused authorization review before presenting Ferno Pulsar as a security product.

Evidence: `backend/src/http.rs` routes around lines 193–222, `admin_auth_check` around lines 692–715, and `require_origin_token` around lines 3677–3698; `frontend/src/routes/api/backend/[...path]/+server.ts` allowlist around lines 139–153; `backend/.env.example` around lines 14–20.

### P0 — within 72 hours

- [ ] Submit the [SCF interest form](https://communityfund.stellar.org/awards); do not wait for the code to be complete.
- [ ] Confirm the applicant: individual/team/legal entity, authorized representative, country, tax status, and XLM-receiving Stellar address plan.
- [ ] Confirm every participant is eligible under the official rules and can complete KYC/KYB.
- [ ] Confirm there is no overlapping active SDF grant, Matching Fund/Enterprise Fund investment, or outstanding SCF tranche.
- [ ] If applying solo, ask SCF to clarify the apparent handbook inconsistency between individual eligibility and language requiring two available team members.
- [ ] Create a writable Ferno Pulsar repository and `origin`; preserve FireWatch provenance and ownership/licensing evidence.
- [ ] Commit the current Stellar plans so reviewers see a coherent project rather than an uncommitted Solana fork.
- [ ] Name the project lead, backend/ingestion owner, product/frontend owner, and protocol-security/adoption owner. Record weekly availability for the grant period.
- [ ] Create a private application data sheet with legal names, emails, locations, entity details, prior SDF funding, GitHub/LinkedIn profiles, and referrer details.

### P0 — proof before the full submission

Build only enough pre-award code to prove capability. Treat it as prior work and exclude it from the grant budget.

- [ ] Add a captured Mainnet `LedgerCloseMeta` fixture containing a Wasm upload, contract creation, or upgrade.
- [ ] Add `go.mod`, pin the official Stellar Go packages, and document supported protocol/XDR versions.
- [ ] Implement one versioned Go-to-Rust ledger envelope.
- [ ] Add the private Rust ingestion endpoint and an atomic ledger/checkpoint database transaction.
- [ ] Persist one contract, one Wasm hash/blob, and one contract-version relationship.
- [ ] Detect one `executable_update` event and confirm it against the contract-instance ledger diff.
- [ ] Render the same upgrade in the API, dashboard, and Telegram.
- [ ] Replay the ledger/range and prove zero duplicate facts and zero duplicate alerts.
- [ ] Demonstrate behavior for a deliberate gap, conflicting ledger hash, and parser failure.
- [ ] Publish a 2–3 minute demo and a reproducible `make demo` or equivalent command.
- [ ] Clearly label the demo as prototype/test evidence, not a production mainnet service.

Minimum proof gate:

> A reviewer can clone the repository, run one command, replay a real Stellar fixture twice, and observe a single verified upgrade alert with old/new Wasm evidence.

### P0 — validate the need

- [ ] Interview at least five Stellar protocol operators, wallet/infrastructure teams, or security reviewers.
- [ ] Ask what they monitor today, how they learn about upgrades/admin changes, their incident-response channel, required alert latency, false-positive tolerance, and missing evidence.
- [ ] Obtain at least three permissioned quotes and two pilot/letter-of-intent statements.
- [ ] Ask pilots for actual contract IDs, admin/treasury accounts, alert rules, and preferred delivery channels.
- [ ] Quantify inherited FireWatch traction: ledgers/transactions processed, protocols monitored, alerts delivered, subscribers/users, uptime, revenue, and any incident caught. Use only verifiable figures.
- [ ] Produce one FireWatch case study showing the team's ability to turn chain data into a useful operational security alert.
- [ ] Do not invent traction or count internal test accounts as users.

### P0 — application narrative

- [ ] Define the primary user narrowly: protocol security/operations teams, not “everyone on Stellar.”
- [ ] Write the problem with evidence from interviews and concrete failure modes.
- [ ] Add a comparison table covering Bevor, Solarkraft, Cyvers, Obsrvr Flow, StellarExpert/Lab, and Ferno Pulsar.
- [ ] Explain the complementary boundary: observable control-plane changes and evidence, not source-code audit, formal verification, generic data plumbing, or AML scoring.
- [ ] Explain why Stellar is core: LedgerCloseMeta, Wasm/contract-instance history, system events, auth trees, and classic account signer/threshold state are the product's source of truth.
- [ ] Separate completed FireWatch/research work from future Stellar work requested from SCF.
- [ ] Define what remains free/open and what can support long-term revenue.
- [ ] Explain post-grant ownership, hosting, incident support, protocol adapter maintenance, and upgrade compatibility.
- [ ] Show how security confidence can support onchain growth: safer launches, faster detection, protected TVL, and lower operational burden. Do not claim that indexed-ledger counts alone equal ecosystem growth.

## Draft $100k grant scope

The pre-application prototype is not part of this budget. The following is a starting point and must be converted into named people, hours, rates, dates, and cost per deliverable.

### Tranche 1 — production MVP

Cash received by completion: initial 10% plus the 20% MVP payment, or **$30k worth of XLM**.

- production recent-ledger ingestion with restart-safe checkpoints and automatic short-gap recovery;
- contract creation, Wasm upload, interface extraction, and version history;
- verified upgrade detection with old/new Wasm and spec/meta diff;
- public API, dashboard detail, and Telegram delivery;
- fixture, replay, continuity, and failure-path test corpus;
- documented deployment and operator health metrics.

Verification:

- [ ] Real Mainnet range is processed end to end.
- [ ] Replaying it creates zero duplicate facts or alerts.
- [ ] Every upgrade in the published corpus is detected by event and ledger diff.
- [ ] Parser failures retain raw evidence and do not silently skip the ledger.

### Tranche 2 — testnet/equivalent product readiness

Payment: **$30k worth of XLM** after acceptance.

- historical Galexie replay and reconciliation with the live tail;
- classic account signer/threshold history for linked protocol accounts;
- evidence-backed protocol registry and admin/guardian/treasury role mapping;
- generic administration, pause, oracle, dormant-contract, and unusual-outflow rules;
- onboarding/configuration UX for pilot protocols;
- performance, load, recovery, and alert-quality evaluation.

Verification:

- [ ] Historical and live paths converge on the same state.
- [ ] At least 10 protocols are attributed with a source and confidence value.
- [ ] At least three pilot teams configure real contracts/accounts and review alerts.
- [ ] Pilot feedback and false-positive adjudication are documented.

### Tranche 3 — usable Mainnet launch

Payment: **$40k worth of XLM** after acceptance.

- production Mainnet service with complete smart-contract-era coverage or a clearly disclosed, measured coverage boundary;
- hardened health, lag, continuity, backup, restore, and incident runbooks;
- stable public API/documentation and a usable, discoverable dashboard;
- five live protocol integrations or pilots, subject to partner willingness;
- published security model, data-quality report, limitations, and sustainability plan;
- final demo and usability validation.

Verification:

- [ ] At least 30 continuous days at 99.5% or better ingestion availability.
- [ ] p95 ledger-close-to-alert latency is below 60 seconds.
- [ ] Supported-XDR parse success is at least 99.99%, with all failures observable.
- [ ] Upgrade recall is 100% against the public regression corpus.
- [ ] At least 80% of sampled high-priority alerts are rated actionable by pilot reviewers.
- [ ] A new protocol can self-onboard or follow a documented assisted-onboarding flow.

### Illustrative engineering allocation

Do not submit this table until it is reconciled with actual team rates and the fixed SCF payment schedule.

| Workstream | Draft allocation |
| --- | ---: |
| Go ingestion, historical replay, and continuity | $30,000 |
| Rust facts, rules, persistence, and API | $28,000 |
| Protocol registry, attribution, and security decoders | $14,000 |
| Dashboard, configuration, and alert-delivery UX | $10,000 |
| QA, fixtures, reliability, performance, and security hardening | $10,000 |
| Public API docs, onboarding, launch, and maintainer documentation | $8,000 |
| **Total** | **$100,000** |

Do not include an external audit fee in this budget; SCF directs eligible audit work to the separate Audit Bank.

## Engineering and credibility backlog

### Before submission

- [ ] Rename packages, deploy services, environment variables, routes, assets, bot identity, and documentation from FireWatch/Solana to Ferno Pulsar/Stellar where they are part of the demo.
- [ ] Make the backend start without Solana/Yellowstone configuration.
- [ ] Remove Solana payment defaults, secrets, active snapshots, and provider assumptions from the Ferno runtime.
- [ ] Add Go tests to CI and run Rust formatting, Clippy, locked tests, Svelte checks, and production build. `cargo fmt --check` currently passes, but `cargo clippy --locked --all-targets -- -D warnings` currently fails across the inherited codebase and needs either remediation or a documented, ratcheted lint baseline.
- [ ] Remediate or explicitly disposition the current npm audit findings: 1 low and 5 high.
- [ ] Add dependency-update automation, secret scanning, and a dependency/SBOM review.
- [ ] Add `SECURITY.md`, vulnerability reporting instructions, a threat model, and a data-integrity model.
- [ ] Document trust boundaries between acquisition, Go normalization, Rust commit/rules, delivery, and external registries.
- [ ] Add restore/replay tests, not only backup scripts.
- [ ] Publish API schema/versioning and raw-XDR retention policy.
- [ ] Define privacy and retention behavior for Telegram/user configuration data.

### Before mainnet tranche

- [ ] Load-test ingestion, backfill, API, delivery, and database growth.
- [ ] Decide and document the SQLite-to-PostgreSQL migration gate.
- [ ] Validate Captive Core/RPC/data-lake failover and maximum recoverable outage.
- [ ] Test network/protocol upgrade compatibility and unsupported-XDR behavior.
- [ ] Add service-level objectives, dashboards, paging thresholds, and an incident response runbook.
- [ ] Test database and blob restoration into a clean environment.
- [ ] Publish status and coverage pages so users can tell what is and is not monitored.

## Application packet checklist

- [ ] Project name, one-line pitch, logo, website, contact email, public repository.
- [ ] Applicant/entity/KYC readiness and prior-funding disclosure.
- [ ] Problem, primary users, interview evidence, and market size/impact.
- [ ] Product workflow and screenshots.
- [ ] Complete architecture embedded in the application.
- [ ] Meaningful Stellar integration and why another chain/data source cannot substitute.
- [ ] Honest current status and a clear boundary around prior work.
- [ ] Competitor/differentiation matrix.
- [ ] Verifiable FireWatch and Stellar prototype traction.
- [ ] Team bios, roles, relevant work, availability, and delivery ownership.
- [ ] Three tranches with dates, costs, verification links, and acceptance criteria.
- [ ] Line-item budget based on people, time, and rates.
- [ ] Technical, product, adoption, regulatory, dependency, and delivery risks with mitigations.
- [ ] Mainnet launch, UX readiness, onboarding, documentation, and support plan.
- [ ] KPI table with baselines, targets, measurement source, owner, and reporting cadence.
- [ ] Sustainability and maintenance plan after SCF funding.
- [ ] Two-minute and five-minute pitch scripts plus demo video.
- [ ] Red-team review for unsupported claims, vague milestones, ineligible costs, and missing verification.
- [ ] Submit by **August 13, 2026** internally, keeping three days of buffer before the listed August 16 deadline.

## Suggested working calendar

| Date | Outcome |
| --- | --- |
| Jul 18–21 | Interest form, applicant/KYC check, track-fit question, repository origin, named team |
| Jul 21–31 | Real Stellar fixture and end-to-end prototype; first interviews |
| Aug 1–5 | Public demo, replay proof, pilot letters, differentiation and traction evidence |
| Aug 5–9 | Full application, architecture, milestones, budget, KPIs, sustainability |
| Aug 10–12 | Technical and narrative red-team; partner fact-checking |
| Aug 13 | Internal submission deadline |
| Aug 16 | Current SCF #45 deadline shown by SCF |

## Go/no-go gate for a $100k request

Submit the full $100k application only when all are true:

- [ ] SCF has confirmed the appropriate track and invited the team to submit.
- [ ] The real-fixture vertical slice works and is reproducible.
- [ ] The repository is public/presentable and CI is green for Stellar code.
- [ ] At least five discovery calls and two pilot commitments validate the need.
- [ ] The competitor section makes the funded overlap explicit and defensibly explains the remaining gap.
- [ ] Named contributors, availability, rates, and responsibilities justify $100k.
- [ ] All three tranches fund future work, end in a usable Mainnet product, and have reviewer-verifiable acceptance tests.
- [ ] Sustainability after the final payment is credible.

If these gates are not met in time, keep the interest form active, ask to move to the next round, and use the additional six weeks to ship stronger evidence. A later, stronger application is preferable to asking reviewers to fund a design-only Stellar migration.
