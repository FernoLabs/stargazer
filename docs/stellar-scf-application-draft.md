# Ferno Pulsar — SCF Build application draft

Working draft: **2026-07-18**  
Target request: **$100,000 worth of XLM**  
Target round: **SCF #45**, subject to invitation  
Public submission deadline currently shown by SCF: **August 16, 2026**  
Internal deadline: **August 13, 2026**

This file converts the research and grant-readiness work into application-shaped answers. Text marked `TODO` is a submission blocker, not copy to leave in the final application.

Official references:

- [SCF awards and current round](https://communityfund.stellar.org/awards)
- [SCF Build Award](https://stellar.gitbook.io/scf-handbook/scf-awards/build-award)
- [Submission criteria](https://stellar.gitbook.io/scf-handbook/scf-awards/build-award/submission-criteria)
- [Budget and deliverable guidelines](https://stellar.gitbook.io/scf-handbook/scf-awards/build-award/budget-and-deliverable-guidelines)
- [Official rules](https://stellar.gitbook.io/scf-handbook/scf-awards/official-rules-for-submissions)

## Submission control

| Field | Current answer | Status |
| --- | --- | --- |
| Project | Ferno Pulsar | Ready |
| Submission title | Stellar contract security history | Draft |
| Request | $100,000 worth of XLM | Draft; cost basis incomplete |
| Duration | 6 months | Draft |
| Track | Open Track | Provisional; obtain written SCF confirmation |
| Applicant | `TODO: individual, team, or legal entity` | Blocking |
| Public repository | `TODO: writable Ferno Pulsar origin` | Blocking |
| Product URL | `TODO: deployed landing/product URL` | Blocking |
| Architecture URL | `TODO: public architecture brief` | Blocking |
| Demo URL | `TODO: 2–3 minute Stellar POC video` | Blocking |
| Reproducible POC | `TODO: clone-and-run command and fixture` | Blocking |
| Operator evidence | `TODO: interview notes, three quotes, two pilot/LOI statements` | Blocking |
| Team and rates | `TODO: names, availability, deliverable ownership, days, rates` | Blocking |

## Canonical application copy

### One sentence

Ferno Pulsar builds a replayable Stellar security history of contract code, administrative authority, signer changes, and DeFi incidents, then delivers evidence-backed findings through an API, dashboard, and Telegram.

### Short description

Every Stellar contract change, turned into a security signal.

### Project description

Stellar protocols can be audited before deployment and still change afterward. Contract Wasm can be upgraded, administrative roles can rotate, account signer thresholds can weaken, and an authorized oracle update can carry an unsafe price. The evidence exists across ledger-entry changes, system events, Soroban authorization trees, classic account state, and protocol-specific storage, but operators do not have a joined-up, replayable security history.

Ferno Pulsar is a post-deployment security monitor for Stellar protocol operations teams. It ingests live and historical `LedgerCloseMeta`, verifies ledger continuity, versions contracts and Wasm, reconstructs account and contract control surfaces where the evidence supports it, and correlates protocol changes with runtime events. Each finding retains chain coordinates, raw evidence provenance, parser version, applicable Wasm version, and confidence. Findings are exposed through a public API, dashboard, and Telegram after they are committed; historical replay never sends live alerts.

The product complements audits, formal verification, explorers, and generic data pipelines. It does not claim to prove arbitrary contract governance or automatically identify every administrator. Its wedge is deterministic change-control evidence and actionable post-deployment monitoring.

### Problem and primary user

The primary user is a Stellar protocol security or operations team responsible for upgrades, privileged roles, treasury controls, oracle configuration, and incident response. Today those teams must join several evidence surfaces manually and may learn about a dangerous change only after capital has moved. Ferno Pulsar reduces that blind spot by preserving what changed, who or what authorized it, which executable version applied, and the exact ledger evidence behind the finding.

`TODO before submission:` Replace the generalized problem statement with at least three permissioned operator quotes and summarize how each team detects upgrades and administrative changes today.

### Why Stellar is essential

Stellar is not a deployment target added to an existing generic monitor. Its ledger model is the product's source of truth:

- `LedgerCloseMeta` provides one normalization boundary for RPC, archive RPC, Captive Core, and Galexie;
- Soroban system events and contract-instance changes independently evidence executable upgrades;
- Soroban authorization entries and invocation trees retain successful call authorization context;
- classic account entries expose signer weights and operation thresholds used by protocol-linked accounts;
- contract data, events, token movements, and source-backed manifests make time-versioned protocol attribution possible.

Ferno's prior monitoring work informs persistence, rules, delivery, and operations design, but its runtime is not shipped as Pulsar. The active repository is Stellar-focused, and the funded implementation starts from explicit Stellar ingestion and evidence boundaries.

### Ecosystem value

Ferno Pulsar is intended to give Stellar protocols and infrastructure teams:

1. independently verified contract and Wasm upgrade history;
2. observable changes to protocol-linked account signers and thresholds;
3. source-backed, time-versioned admin, guardian, treasury, and oracle attribution;
4. evidence-preserving incident timelines and regression fixtures;
5. reusable API data for wallets, explorers, security teams, and operations tooling;
6. faster detection and investigation of unsafe authorized changes, without labeling every anomaly an exploit.

Final impact claims must use operator validation, measured product telemetry, and public acceptance artifacts. Registry size and indexed-ledger counts are engineering coverage, not adoption.

### Current traction and proof

Ferno Pulsar is in pre-application proof development and is not yet a production Stellar monitor. Current verified work is technical evidence rather than user traction:

- 36 selected Mainnet deployments are source-attributed across seven protocol families;
- 38 seed and incident contract IDs resolve on Mainnet;
- seven YieldBlox incident checkpoint ledgers have been fetched and hash-validated;
- live and archive RPC providers returned matching hashes for tested ledgers;
- ingestion, replay, registry, governance, multisig, incident, and decoder boundaries are documented;
- the deployed Pulsar website, Stellar registry seed, provider probes, and research plans are public-facing reviewer artifacts; no prior-chain runtime is presented as Stellar functionality.

The first pre-award proof will let a reviewer clone the repository, run one command, replay a real Stellar fixture twice, and observe exactly one verified upgrade finding with old/new Wasm evidence.

`TODO before submission:` Make the repository reviewer-accessible and add the POC, demo, architecture brief, prior-execution case study, operator interviews, pilot commitments, and only verifiable historical usage figures.

### Track answer

Provisional selection: **Open Track**. Ferno Pulsar is a differentiated end-user security product for Stellar protocol operators, not a general-purpose data pipeline. It consumes core Stellar data sources but does not yet make an approved Integration Track building block central to the funded scope.

`TODO before submission:` Ask SCF in writing whether this post-deployment monitoring product belongs in Open Track or should wait for a security/indexing RFP. Use SCF's answer as authoritative.

### Security model

Acquisition can fail or change providers without changing committed fact identity. Ferno Pulsar verifies network, ledger sequence, hash, and parent continuity before advancing a checkpoint. Raw evidence is retained when a parser fails. Facts are committed atomically with the checkpoint, alerts are queued only after commit, and replay delivery is disabled by default. Contract upgrade findings require the executable-update system event and contract-instance ledger diff to agree. Protocol attribution always carries a source and confidence value, and arbitrary contract-account governance is not labeled as multisig without a verified decoder.

The public POC must introduce a deny-by-default control plane: ingestion and administrative endpoints require explicit authentication, missing configuration fails closed, and negative authorization paths are covered by integration tests.

### Sustainability

Proposed model to validate with operators:

- public/free: contract and Wasm history, coverage boundaries, selected findings, evidence API, incident fixtures, and documentation;
- commercial: private real-time alert policies, additional delivery channels, longer retention, protocol-specific decoders, onboarding support, and an operational SLA;
- maintenance: `TODO: name the post-grant maintainer, annual infrastructure budget, decoder update process, incident support owner, and revenue assumptions`.

## Team and delivery capacity

Complete this table with real people. One person may own multiple roles, but every funded deliverable needs a named accountable owner and credible availability.

| Role | Name | Relevant proof | Grant availability | Day rate | Deliverables |
| --- | --- | --- | ---: | ---: | --- |
| Project lead and architecture | `TODO` | `TODO: prior product/chain/security work and profile links` | `TODO days/week` | `TODO` | `TODO` |
| Go ingestion and historical replay | `TODO` | `TODO` | `TODO days/week` | `TODO` | D1, D4, D7 |
| Rust facts, rules, persistence, and API | `TODO` | `TODO` | `TODO days/week` | `TODO` | D2, D5, D8 |
| Svelte product and delivery UX | `TODO` | `TODO` | `TODO days/week` | `TODO` | D3, D8 |
| Protocol security and pilot validation | `TODO` | `TODO` | `TODO days/week` | `TODO` | D5, D6, D9 |

`TODO before submission:` Add legal/KYC eligibility, location, team size, prior SDF funding, LinkedIn/GitHub links, and a delivery-capacity calculation showing that the named people can complete the proposed days inside six months.

## Deliverables and acceptance

The nine-deliverable structure below is application-shaped but not yet submit-ready. Replace every `TODO` owner, date, and cost with a bottom-up calculation of role days multiplied by actual rates. The total must equal exactly $100,000 in every summary.

### Tranche 1 — production MVP

| ID | Deliverable | Reviewer-verifiable completion | Owner | Due | Working allocation |
| --- | --- | --- | --- | --- | ---: |
| D1 | Restart-safe recent-ledger ingestion | A published Mainnet range ingests through a documented source adapter; network, sequence, hash, and parent continuity are checked; interruption resumes from the last atomic checkpoint; a deliberate gap and conflicting hash stop progress visibly; raw evidence survives a forced parser failure. | `TODO` | `TODO` | $12,000 |
| D2 | Contract, Wasm, and verified upgrade history | The public corpus includes contract creation, Wasm upload, contract-version relationships, and at least one real executable upgrade. Event and contract-instance diff agree on the new executable. Replaying twice produces identical facts and zero duplicate findings. | `TODO` | `TODO` | $14,000 |
| D3 | Evidence API, dashboard, Telegram, and control-plane hardening | A reviewer can inspect the same committed finding through API and dashboard and deliver it once through Telegram. Ingestion and administration routes default to deny; missing or invalid credentials fail closed in integration tests. | `TODO` | `TODO` | $10,000 |
|  | **Tranche 1 cost basis** | `TODO: attach role/day calculation` |  |  | **$36,000** |

### Tranche 2 — historical and operator readiness

| ID | Deliverable | Reviewer-verifiable completion | Owner | Due | Working allocation |
| --- | --- | --- | --- | --- | ---: |
| D4 | Historical replay and live-tail reconciliation | A documented Galexie/Captive Core or equivalent historical range and the live path converge on the same materialized state. Historical replay cannot send live alerts. Restore and reprocess tests preserve deterministic fact identity. | `TODO` | `TODO` | $12,000 |
| D5 | Control-plane history and protocol attribution | Classic account signer/threshold history and supported contract-governance evidence are queryable for protocol-linked accounts. At least ten protocol families carry time-versioned role attribution with source and confidence. Unsupported governance remains explicitly unknown. | `TODO` | `TODO` | $14,000 |
| D6 | Protocol decoders and pilot calibration | Initial Blend, Reflector, and SDEX sequences run against public fixtures. At least three pilot teams configure real contracts or accounts, review sampled findings, and participate in documented false-positive/actionability adjudication. | `TODO` | `TODO` | $7,000 |
|  | **Tranche 2 cost basis** | `TODO: attach role/day calculation` |  |  | **$33,000** |

### Tranche 3 — usable Mainnet launch

| ID | Deliverable | Reviewer-verifiable completion | Owner | Due | Working allocation |
| --- | --- | --- | --- | --- | ---: |
| D7 | Mainnet operations and recoverability | The public service completes a 30-day measurement window with status and coverage pages, observable lag/continuity metrics, backup and clean-environment restore evidence, source-failover tests, and an incident runbook. The provisional ingestion-availability target is at least 99.5%. | `TODO` | `TODO` | $10,000 |
| D8 | Public API, onboarding, and live protocol use | Versioned API documentation and a discoverable dashboard are public. At least five willing pilot/protocol teams are onboarded or a documented assisted/self-service path is demonstrated. Mainnet accounts and evidence URLs are visible to reviewers. | `TODO` | `TODO` | $12,000 |
| D9 | Data-quality, security, and sustainability report | A public report states supported-XDR parse success, unsupported cases, regression-corpus recall, alert latency, sampled actionability, security boundaries, operating cost, revenue model, and post-grant ownership. Provisional targets are validated or revised from measured baselines with SCF agreement. | `TODO` | `TODO` | $9,000 |
|  | **Tranche 3 cost basis** | `TODO: attach role/day calculation` |  |  | **$31,000** |

### Working budget reconciliation

| Summary | Amount |
| --- | ---: |
| D1–D3 | $36,000 |
| D4–D6 | $33,000 |
| D7–D9 | $31,000 |
| **Total requested** | **$100,000** |

These amounts are provisional workstream allocations, not a valid cost basis. Before submission, rebuild each row from named contributors, role days, and rates. Check that:

1. deliverable totals equal tranche totals;
2. tranche totals equal $100,000;
3. role-day totals fit each person's stated availability;
4. all costs fund future development rather than completed research or the pre-application POC;
5. audit, marketing, legal/entity, token incentive, and general-overhead costs are excluded;
6. working capital accounts for SCF's separate 10% / 20% / 30% / 40% payment schedule.

## KPI and evidence plan

| KPI | Current baseline | Pre-application gate | Tranche 2 gate | Mainnet gate | Evidence source |
| --- | --- | --- | --- | --- | --- |
| Reproducible verified upgrades | 0 produced by the Stellar parser | 1 real fixture, replayed twice with one finding | Public regression corpus | 100% recall on supported public corpus | CI, fixture hashes, API output |
| Source-attributed protocol families | 7 seeded | Seed published with citations | At least 10 time-versioned families | Coverage page and update process | Registry export and source log |
| Pilot teams | 0 recorded | 2 pilot/LOI statements | 3 teams review real configurations | 5 willing pilot/protocol teams | Permissioned letters and review log |
| Ingestion availability | Not measured | Measurement method deployed | Baseline published | Provisional target ≥99.5% over 30 days | Public status data |
| Ledger-close-to-finding latency | Not measured | Instrumented in POC | Baseline and bottlenecks published | Provisional p95 target <60 seconds | Timestamped telemetry |
| Supported-XDR parse success | Not measured | Failures observable and raw evidence retained | Baseline published | Provisional target ≥99.99% | Parser metrics and failure corpus |
| High-priority alert actionability | Not measured | Review rubric agreed with pilots | Sample adjudicated by three teams | Provisional target ≥80% | Permissioned review records |

Targets without a measured baseline remain explicitly provisional. Do not convert them into present-tense performance claims.

## Public application evidence index

| Artifact | Required contents | URL |
| --- | --- | --- |
| Product landing page | Problem, user, architecture, honest status, roadmap, grant scope | `TODO` |
| Public repository | Ferno origin, provenance, license, green CI, security policy | `TODO` |
| Architecture brief | Components, trust boundaries, data model, replay and delivery invariants | `TODO` |
| POC instructions | One clone-and-run command, fixture provenance, expected output | `TODO` |
| Demo video | 2–3 minutes: replay, evidence, duplicate prevention, UI and alert | `TODO` |
| Operator validation | Permissioned quotes, anonymized notes where required, pilot/LOI statements | `TODO` |
| Team proof | Bios, profiles, availability, ownership, prior execution | `TODO` |
| Budget workbook | Deliverable, person, role, days, rate, due date, totals | `TODO` |
| Security evidence | Threat model, trust boundaries, CI and negative authorization tests | `TODO` |
| Sustainability brief | Public/commercial boundary, annual cost, maintenance and support ownership | `TODO` |

## Claim controls

Keep these sentences consistent across the landing page, interest form, full submission, demo, and interviews:

- Ferno Pulsar **is building** a Stellar monitor; it is not live on Stellar today.
- The 36 / 7 / 38 / 7 figures describe research coverage and verified fixtures, not users, customers, production coverage, or adoption.
- The YieldBlox sequence is a regression case the parser must reproduce; the parser has not produced it yet.
- The $100,000 figure is a planned request, not an award.
- The pre-application POC and completed research are excluded from the funded scope.
- Alerts are evidence-backed observations, not proof that every anomaly is malicious.
- Contract-account governance remains unknown without supported evidence or a versioned decoder.

## Go/no-go checklist

Submit the full application only when all boxes are checked:

- [ ] SCF has confirmed the track and invited the applicant to submit.
- [ ] Applicant/entity, participant eligibility, KYC/KYB, tax, and XLM address plans are recorded privately.
- [ ] The Ferno Pulsar repository has a writable public origin and reviewer-presentable history.
- [ ] One real Stellar upgrade fixture runs end to end with deterministic replay.
- [ ] Public architecture, POC, demo, product, and CI URLs work without private access.
- [ ] Five discovery interviews, three permissioned quotes, and two pilot/LOI statements exist.
- [ ] Team names, availability, profiles, rates, and deliverable ownership are complete.
- [ ] All nine deliverable budgets have bottom-up role/day calculations and reconcile to $100,000.
- [ ] Security blockers are fixed and negative authorization tests pass.
- [ ] Sustainability, infrastructure cost, support, and maintenance ownership are credible.
- [ ] Every application claim has an evidence URL or is clearly labeled as a target.
- [ ] A final red-team pass finds no stale totals, unsupported traction, vague acceptance criteria, or hidden essential facts.
