<script lang="ts">
  type Capability = {
    id: string;
    title: string;
    body: string;
  };

  type Outcome = {
    id: string;
    title: string;
    body: string;
  };

  type PipelineStep = {
    id: string;
    label: string;
    detail: string;
  };

  type RoadmapPhase = {
    id: string;
    phase: string;
    window: string;
    status: "Building" | "Planned";
    body: string;
    outcome: string;
  };

  const outcomes: Outcome[] = [
    {
      id: "detect-sooner",
      title: "Detect sooner",
      body: "Follow confirmed code, authority, configuration, and capital changes as one operational surface.",
    },
    {
      id: "affected-surface",
      title: "See what is affected",
      body: "Connect a change to mapped contracts, control accounts, oracles, pools, wallets, and value flows.",
    },
    {
      id: "verify",
      title: "Verify from the ledger",
      body: "Start the investigation with the transaction, before-and-after state, source evidence, and parser version.",
    },
  ];

  const capabilities: Capability[] = [
    {
      id: "belongs",
      title: "What belongs to the protocol?",
      body: "A source-backed map of known contracts, pools, vaults, oracles, control accounts, signers, treasuries, and upstream dependencies.",
    },
    {
      id: "changed",
      title: "What changed?",
      body: "Confirmed code, authority, signer-policy, configuration, contract-call, and token-movement changes across that mapped surface.",
    },
    {
      id: "affected",
      title: "What else may be affected?",
      body: "The changed component is traced to known downstream dependencies and value flows so operators can see where to investigate next.",
    },
    {
      id: "proof",
      title: "What proves it?",
      body: "Ledger coordinates, before-and-after state, raw evidence provenance, parser version, applicable Wasm version, and explicit coverage limits.",
    },
  ];

  const pipeline: PipelineStep[] = [
    {
      id: "acquire",
      label: "Acquire",
      detail: "RPC, archive RPC, Captive Core, or Galexie",
    },
    {
      id: "verify",
      label: "Verify",
      detail: "Network, sequence, hash, and ledger continuity",
    },
    {
      id: "normalize",
      label: "Normalize",
      detail: "Contracts, Wasm, auth, accounts, events, and token flows",
    },
    {
      id: "attribute",
      label: "Attribute",
      detail: "Time-versioned protocol roles with source and confidence",
    },
    {
      id: "detect",
      label: "Detect",
      detail: "Generic invariants and Wasm-versioned protocol rules",
    },
    {
      id: "deliver",
      label: "Deliver",
      detail: "Evidence through API, dashboard, and alerts",
    },
  ];

  const roadmap: RoadmapPhase[] = [
    {
      id: "proof",
      phase: "First release",
      window: "Verified contract changes",
      status: "Building",
      body: "Code and Wasm history, evidence-backed upgrade findings, and delivery through API, dashboard, and Telegram.",
      outcome:
        "Know exactly what changed and verify it directly from the ledger.",
    },
    {
      id: "context",
      phase: "Broader coverage",
      window: "Dependencies and operational wallets",
      status: "Planned",
      body: "Reconciled historical backfill, signer and control history, mapped dependencies, operational-wallet rules, and first Blend, Reflector, and SDEX decoders.",
      outcome:
        "Monitor the system around a protocol instead of maintaining separate address lists.",
    },
    {
      id: "launch",
      phase: "Operational service",
      window: "Live Mainnet monitoring",
      status: "Planned",
      body: "Protocol onboarding, reconciled backfill and live tail, source failover, recovery testing, and visible coverage and data-quality reporting.",
      outcome: "Rely on a measured monitoring and recovery path.",
    },
  ];

  const faq = [
    {
      id: "live",
      question: "Is Stellar Watch live on Stellar today?",
      answer:
        "No. The first product release—verified contract-change monitoring—is in development. Historical replay, live alert delivery, and broader dependency coverage are not production services yet.",
    },
    {
      id: "protocol-wide",
      question: "What does protocol-wide monitoring include?",
      answer:
        "Stellar Watch is designed to follow the mapped contracts, pools, vaults, oracles, admin and treasury accounts, signers, and token flows a protocol relies on. Every relationship keeps a source and confidence level; unknown or unsupported dependencies remain visible as coverage gaps.",
    },
    {
      id: "wallets",
      question: "Can Stellar Watch monitor a wallet live?",
      answer:
        "That is a planned pilot capability. A public Stellar account or contract address can be watched for confirmed transfers, material balance movements, signer-policy changes, and new protocol interactions. Stellar Watch cannot protect private keys or determine the off-chain cause of an authorized transaction.",
    },
    {
      id: "audit",
      question: "Does it replace an audit or formal verification?",
      answer:
        "No. Audits evaluate code and assumptions at a point in time. Stellar Watch is intended to preserve and alert on observable post-deployment changes and runtime evidence.",
    },
    {
      id: "admins",
      question: "Can every contract administrator be identified automatically?",
      answer:
        "For supported XDR, ledger-visible changes and available authorization context can be recorded. Parser gaps and arbitrary contract-account governance limit stronger claims, which require verified source or a versioned decoder.",
    },
    {
      id: "registry",
      question: "Which protocols will Stellar Watch cover?",
      answer:
        "The first planned protocol-specific decoders cover Blend, Reflector, and SDEX activity. Generic contract, account, and transfer monitoring can extend further, while deeper dependency coverage will be published with sources and explicit limits.",
    },
  ] as const;
</script>

<svelte:head>
  <title>Stellar Watch — Stellar protocol and wallet monitoring</title>
  <meta
    name="description"
    content="Stellar Watch by Ferno is building post-deployment security monitoring for Stellar protocols: mapped dependencies, confirmed changes, and evidence-backed alerts."
  />
  <link rel="canonical" href="https://stellarwatch.ferno.ag/" />
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link
    rel="preconnect"
    href="https://fonts.gstatic.com"
    crossorigin="anonymous"
  />
  <link
    href="https://fonts.googleapis.com/css2?family=Archivo+SemiCondensed:wght@500;600&family=Fragment+Mono&family=Source+Sans+3:wght@400;500;600&display=swap"
    rel="stylesheet"
  />
  <meta
    property="og:title"
    content="Stellar Watch — Stellar protocol and wallet monitoring"
  />
  <meta
    property="og:description"
    content="Know when a Stellar protocol changes, what else may be at risk, and which ledger evidence proves it."
  />
  <meta property="og:type" content="website" />
  <meta property="og:url" content="https://stellarwatch.ferno.ag/" />
  <meta name="theme-color" content="#f4f5f2" />
  <meta name="twitter:card" content="summary" />
  <meta
    name="twitter:title"
    content="Stellar Watch — Stellar protocol and wallet monitoring"
  />
  <meta
    name="twitter:description"
    content="Post-deployment monitoring for the contracts, controls, wallets, and dependencies behind a Stellar protocol."
  />
</svelte:head>

<div class="landing-page">
  <section class="signal-cover" aria-labelledby="hero-title">
    <div class="atlas-width">
      <div class="cover-grid">
        <div class="cover-title">
          <p class="cover-kicker">
            For Stellar protocol security, operations, and treasury teams
          </p>
          <h1 id="hero-title">Know what changed. See what is at risk.</h1>
        </div>
        <div class="cover-copy">
          <p>
            A live protocol can change after its audit. Stellar Watch maps its known
            contracts, control accounts, signers, oracles, pools, treasuries,
            and dependencies; follows confirmed Stellar ledgers; and turns
            important code, authority, configuration, and capital changes into
            one evidence-linked alert.
          </p>
          <div class="cover-actions">
            <a class="primary-link" href="#solution">How Stellar Watch works</a>
            <a href="https://x.com/ferno_ag" rel="noreferrer" target="_blank"
              >Join the pilot ↗</a
            >
          </div>
          <p class="poc-note">
            Private pilot in development. Verified contract-change monitoring
            comes first; dependency mapping, live Mainnet alerts, and
            operational-wallet monitoring are planned—not production services
            today.
          </p>
        </div>
      </div>

      <ol class="outcome-ledger" aria-label="Stellar Watch operator outcomes">
        {#each outcomes as outcome, index (outcome.id)}
          <li>
            <span>{String(index + 1).padStart(2, "0")}</span>
            <h2>{outcome.title}</h2>
            <p>{outcome.body}</p>
          </li>
        {/each}
      </ol>

      <figure class="ledger-map">
        <svg
          viewBox="0 0 1440 310"
          preserveAspectRatio="none"
          aria-hidden="true"
        >
          <g class="map-grid">
            <path
              d="M0 52H1440 M0 104H1440 M0 156H1440 M0 208H1440 M0 260H1440"
            />
            <path
              d="M120 0V310 M360 0V310 M600 0V310 M840 0V310 M1080 0V310 M1320 0V310"
            />
          </g>
          <path
            class="signal-line"
            d="M0 178H118L140 178L158 88L177 250L196 178H408L430 178L448 126L466 211L486 178H734L754 178L775 46L799 278L822 178H1068L1087 178L1104 112L1121 224L1142 178H1440"
          />
          <g class="signal-points">
            <circle cx="158" cy="88" r="5" />
            <circle cx="448" cy="126" r="5" />
            <circle cx="775" cy="46" r="5" />
            <circle cx="1104" cy="112" r="5" />
          </g>
        </svg>
        <ol class="map-annotations">
          <li>
            <span>01</span><strong>Contracts</strong>
            <p>Code, interfaces, pools, and vaults</p>
          </li>
          <li>
            <span>02</span><strong>Control plane</strong>
            <p>Admins, guardians, signers, and thresholds</p>
          </li>
          <li>
            <span>03</span><strong>Wallets and value</strong>
            <p>Treasuries, transfers, and balance-impact moves</p>
          </li>
          <li>
            <span>04</span><strong>Dependencies</strong>
            <p>Oracles, tokens, and upstream contracts</p>
          </li>
        </ol>
        <figcaption>
          <span>One monitored protocol surface</span>
          <span
            >Mapped system → confirmed change → affected surface →
            evidence-backed alert</span
          >
        </figcaption>
      </figure>
    </div>
  </section>

  <section
    id="problem"
    class="atlas-section problem-chapter"
    aria-labelledby="problem-title"
  >
    <div class="atlas-width">
      <div class="chapter-grid">
        <div class="chapter-title">
          <span aria-hidden="true">01</span>
          <h2 id="problem-title">
            Post-deployment risk is fragmented across the ledger.
          </h2>
        </div>
        <div class="chapter-copy">
          <p class="lead-copy">
            An audit can assess code and assumptions at a point in time. It does
            not continuously tell an operator when Wasm changes later, signer
            thresholds weaken, an oracle input creates downstream exposure, or a
            treasury moves funds.
          </p>
          <p>
            The evidence exists, but it is split across contract state, classic
            accounts, Soroban authorization, events, and token flows. Each event
            may look legitimate alone; the risk often appears only when the
            sequence and dependencies are joined.
          </p>
          <ul class="risk-register">
            <li>Audits capture a moment</li>
            <li>Explorers expose separate transactions</li>
            <li>Address lists lose dependency context</li>
            <li>Incident timelines are rebuilt by hand</li>
          </ul>
        </div>
      </div>
      <a class="problem-sequence" href="#evidence">
        <span>Observed sequence</span>
        <strong>Thin market → oracle → lending pool → reserve outflow</strong>
        <p>
          The YieldBlox case shows why one-address monitoring is not enough.
          View the evidence ↓
        </p>
      </a>
    </div>
  </section>

  <section
    id="solution"
    class="atlas-section solution-chapter"
    aria-labelledby="solution-title"
  >
    <div class="atlas-width">
      <div class="chapter-grid">
        <div class="chapter-title">
          <span aria-hidden="true">02</span>
          <h2 id="solution-title">
            One monitored system. Four questions answered.
          </h2>
        </div>
        <div class="chapter-copy">
          <p class="lead-copy">
            Stellar Watch turns scattered ledger facts into a time-versioned security
            history for one monitored protocol surface.
          </p>
          <p>
            Map the system, follow confirmed change, trace the known affected
            surface, and send one finding with the evidence needed to verify it.
            Unknown and unsupported dependencies remain visible as coverage
            gaps.
          </p>
          <p class="first-release">
            <strong>First release</strong> Deterministic Wasm-upgrade monitoring
            through API, dashboard, and Telegram. Dependency and wallet rules expand
            from that verified base.
          </p>
        </div>
      </div>
      <ol class="capability-ledger">
        {#each capabilities as capability, index (capability.id)}
          <li>
            <span>{String(index + 1).padStart(2, "0")}</span>
            <h3>{capability.title}</h3>
            <p>{capability.body}</p>
          </li>
        {/each}
      </ol>

      <div class="wallet-boundary">
        <strong>Planned pilot: operational wallets</strong>
        <p>
          Watch selected public Stellar accounts for confirmed transfers,
          material balance movements, signer-policy changes, and new protocol
          interactions. Stellar Watch observes ledger activity; it does not protect
          private keys or infer an off-chain compromise.
        </p>
      </div>

      <div class="comparison-block">
        <div>
          <p>Built for the gap after deployment</p>
          <h3>Different tools answer different questions.</h3>
        </div>
        <p>
          Audits review code. Explorers expose transactions. Data services make
          ledgers queryable. Stellar Watch is being built to join change, authority,
          dependency, and capital into an operational finding.
        </p>
      </div>
      <div class="comparison-frame">
        <table>
          <thead>
            <tr
              ><th>Tool</th><th>Primary job</th><th
                >Operational context still needed</th
              ></tr
            >
          </thead>
          <tbody>
            <tr
              ><th>Audit</th><td
                >Review code and assumptions at a point in time</td
              ><td>Continuous post-deployment change</td></tr
            >
            <tr
              ><th>Explorer</th><td>Inspect an address or transaction</td><td
                >Joined protocol and dependency impact</td
              ></tr
            >
            <tr
              ><th>Data API</th><td>Query raw or normalized ledger data</td><td
                >Security interpretation and operator delivery</td
              ></tr
            >
            <tr class="watch-row"
              ><th>Stellar Watch</th><td
                >Connect change, authority, dependencies, and capital</td
              ><td>Evidence and coverage limits are part of the finding</td></tr
            >
          </tbody>
        </table>
      </div>
    </div>
  </section>

  <section
    id="architecture"
    class="atlas-section system-chapter"
    aria-labelledby="architecture-title"
  >
    <div class="atlas-width">
      <div class="chapter-grid compact-chapter">
        <div class="chapter-title">
          <span aria-hidden="true">03</span>
          <h2 id="architecture-title">
            From confirmed ledger to useful alert.
          </h2>
        </div>
        <p class="architecture-intro">
          The live tail and historical replay use the same evidence path. Stellar Watch
          verifies the ledger, resolves the monitored protocol or wallet
          context, evaluates the change, and routes the finding only after the
          evidence is committed.
        </p>
      </div>

      <figure class="system-map">
        <ol aria-label="Stellar Watch processing stages">
          {#each pipeline as step, index (step.id)}
            <li>
              <span>{String(index + 1).padStart(2, "0")}</span>
              <strong>{step.label}</strong>
              <p>{step.detail}</p>
            </li>
          {/each}
        </ol>
        <figcaption>
          One monitoring policy can follow a protocol and its mapped
          dependencies. Stellar Watch retains ledger coordinates, raw evidence
          location, source provenance, parser version, and applicable Wasm
          version; historical replay stays separate from live delivery.
        </figcaption>
      </figure>
    </div>
  </section>

  <section
    id="evidence"
    class="atlas-section casebook"
    aria-labelledby="evidence-title"
  >
    <div class="atlas-width">
      <div class="chapter-grid compact-chapter">
        <div class="chapter-title">
          <span aria-hidden="true">04</span>
          <h2 id="evidence-title">One incident. Five connected surfaces.</h2>
        </div>
        <p class="architecture-intro">
          A useful finding connects the first observable change to the capital
          impact without flattening every event into “exploit.” This historical
          YieldBlox sequence shows the cross-protocol context an operator needs
          during an investigation.
        </p>
      </div>

      <article
        id="incident"
        class="incident-record"
        aria-labelledby="incident-title"
      >
        <header>
          <p>The blind spot, in practice</p>
          <h3 id="incident-title">YieldBlox, February 2026</h3>
        </header>
        <div class="incident-summary">
          <p>
            The sequence joins a thin USTRY/USDC market move, a Reflector price
            publication, collateral entering an isolated Blend pool, and
            subsequent USDC and XLM reserve outflows.
          </p>
          <p class="causal-finding">
            Available evidence points to a YieldBlox pool and
            oracle-configuration failure—not a Blend core-contract exploit or an
            unauthorized Reflector update.
          </p>
        </div>
        <ol class="incident-events" aria-label="YieldBlox incident sequence">
          <li>
            <span>01</span><strong>Thin market</strong>
            <p>Small SDEX fill reprices USTRY</p>
          </li>
          <li>
            <span>02</span><strong>Oracle publish</strong>
            <p>Configured input reaches Reflector</p>
          </li>
          <li>
            <span>03</span><strong>Collateral</strong>
            <p>USTRY enters the isolated lending pool</p>
          </li>
          <li>
            <span>04</span><strong>Reserve outflow</strong>
            <p>USDC and XLM leave shortly afterward</p>
          </li>
        </ol>
        <footer>
          <p>
            Stellar Watch did not detect this incident live. It is now a product
            regression case: the timeline Stellar Watch must be able to reconstruct
            and, once live, recognize from confirmed ledger evidence.
          </p>
          <div>
            <a
              href="https://blocksec.com/blog/yieldblox-dao-incident-on-stellar-oracle-misconfiguration-enabled-a-10m-drain"
              rel="noreferrer"
              target="_blank">BlockSec analysis ↗</a
            >
            <a
              href="https://blockaid.io/blog/73-quarantined-how-blockaid-and-stellar-validators-contained-a-10m-price-manipulation-attack"
              rel="noreferrer"
              target="_blank">Blockaid response ↗</a
            >
          </div>
        </footer>
      </article>
    </div>
  </section>

  <section
    id="roadmap"
    class="atlas-section roadmap-chapter"
    aria-labelledby="roadmap-title"
  >
    <div class="atlas-width">
      <div class="chapter-grid compact-chapter">
        <div class="chapter-title">
          <span aria-hidden="true">05</span>
          <h2 id="roadmap-title">
            Start with one signal. Expand to the system.
          </h2>
        </div>
        <p class="architecture-intro">
          Stellar Watch starts with verified contract changes, then adds history,
          dependencies, operational wallets, and protocol-specific context.
          Every release keeps the same evidence-backed finding format and
          explicit coverage limits.
        </p>
      </div>

      <ol class="roadmap-track">
        {#each roadmap as item, index (item.id)}
          <li
            class={["roadmap-phase", { current: item.status === "Building" }]}
          >
            <div class="phase-coordinate">
              <span>{String(index + 1).padStart(2, "0")}</span>
              <p>{item.status}</p>
            </div>
            <h3>{item.phase}</h3>
            <p class="phase-window">{item.window}</p>
            <p>{item.body}</p>
            <div class="phase-gate">
              <strong>User outcome</strong>
              <p>{item.outcome}</p>
            </div>
          </li>
        {/each}
      </ol>
    </div>
  </section>

  <section class="atlas-section faq-chapter" aria-labelledby="faq-title">
    <div class="atlas-width faq-grid">
      <div class="chapter-title">
        <span aria-hidden="true">06</span>
        <h2 id="faq-title">Boundaries.</h2>
      </div>
      <div class="faq-list">
        {#each faq as item (item.id)}
          <details>
            <summary>{item.question}</summary>
            <p>{item.answer}</p>
          </details>
        {/each}
      </div>
    </div>
  </section>

  <section class="closing-chapter" aria-labelledby="closing-title">
    <div class="atlas-width closing-grid">
      <h2 id="closing-title">Bring one protocol surface into the pilot.</h2>
      <div>
        <p>
          We will map the contracts, control accounts, treasury wallets,
          oracles, and alerts your team needs—and publish the coverage limits
          before monitoring begins.
        </p>
        <a href="https://x.com/ferno_ag" rel="noreferrer" target="_blank"
          >Join the private pilot ↗</a
        >
      </div>
    </div>
  </section>
</div>

<style>
  :global(html) {
    scroll-behavior: smooth;
    scroll-padding-top: 4.5rem;
  }

  :global(body) {
    background: #f4f5f2;
  }

  .landing-page {
    --atlas-field: #f4f5f2;
    --atlas-paper: #ffffff;
    --atlas-ink: #151418;
    --atlas-muted: #67636c;
    --atlas-rule: #d6d7d0;
    --atlas-violet: #6436e8;
    --atlas-violet-soft: #eeeaff;
    --atlas-signal: #ddfe55;
    --atlas-anomaly: #d9546b;
    background: var(--atlas-field);
    color: var(--atlas-ink);
    font-family: "Source Sans 3", sans-serif;
  }

  .atlas-width {
    width: min(1320px, calc(100vw - 4rem));
    margin: 0 auto;
  }

  h1,
  h2,
  h3,
  p,
  figure,
  figcaption {
    margin: 0;
  }

  h1,
  h2,
  h3 {
    font-family: "Archivo SemiCondensed", sans-serif;
    font-weight: 600;
  }

  h1 {
    max-width: 10.5ch;
    font-size: clamp(4rem, 7vw, 6.8rem);
    line-height: 0.88;
    letter-spacing: -0.055em;
  }

  h2 {
    max-width: 16ch;
    font-size: clamp(2.5rem, 4.2vw, 4.5rem);
    line-height: 0.96;
    letter-spacing: -0.045em;
  }

  h3 {
    font-size: 1.35rem;
    line-height: 1.15;
    letter-spacing: -0.02em;
  }

  p {
    color: var(--atlas-muted);
    font-size: 1rem;
    line-height: 1.6;
  }

  a {
    color: inherit;
    text-underline-offset: 0.2em;
  }

  .map-annotations span,
  .outcome-ledger > li > span,
  .chapter-title > span,
  .capability-ledger > li > span,
  .system-map li > span,
  .phase-coordinate,
  .incident-events span {
    font-family: "Fragment Mono", monospace;
  }

  .signal-cover {
    padding: clamp(4rem, 7vw, 7rem) 0 0;
    border-bottom: 1px solid var(--atlas-rule);
  }

  .cover-grid {
    display: grid;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    gap: 1.5rem;
    align-items: end;
  }

  .cover-title {
    grid-column: 1 / span 8;
    display: grid;
    gap: 1rem;
  }

  .cover-kicker {
    max-width: 32rem;
    color: var(--atlas-violet);
    font-family: "Fragment Mono", monospace;
    font-size: 0.72rem;
    font-weight: 500;
    letter-spacing: 0.02em;
    line-height: 1.45;
    text-transform: uppercase;
  }

  .cover-copy {
    grid-column: 9 / -1;
    display: grid;
    gap: 1.4rem;
    padding-bottom: 0.3rem;
  }

  .cover-copy > p:first-child {
    color: #37323d;
    font-size: 1.16rem;
    line-height: 1.55;
  }

  .cover-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 1rem 1.4rem;
  }

  .cover-actions a {
    font-size: 0.9rem;
    font-weight: 600;
  }

  .cover-actions .primary-link {
    padding: 0.7rem 0.9rem;
    border: 1px solid var(--atlas-violet);
    border-radius: 3px;
    background: var(--atlas-violet);
    color: #fff;
    text-decoration: none;
  }

  .poc-note {
    font-family: "Fragment Mono", monospace;
    font-size: 0.72rem;
  }

  .outcome-ledger {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    margin: clamp(3rem, 5vw, 4.5rem) 0 0;
    padding: 0;
    border-top: 1px solid var(--atlas-ink);
    border-bottom: 1px solid var(--atlas-rule);
    list-style: none;
  }

  .outcome-ledger li {
    display: grid;
    grid-template-columns: 2rem minmax(0, 1fr);
    align-content: start;
    gap: 0.35rem 0.7rem;
    min-height: 10.5rem;
    padding: 1rem;
    border-right: 1px solid var(--atlas-rule);
  }

  .outcome-ledger li:last-child {
    border-right: 0;
  }

  .outcome-ledger > li > span {
    grid-row: 1 / span 2;
    color: var(--atlas-violet);
    font-size: 0.68rem;
  }

  .outcome-ledger h2 {
    max-width: none;
    font-size: 1.15rem;
    line-height: 1.15;
    letter-spacing: -0.02em;
  }

  .outcome-ledger p {
    font-size: 0.8rem;
    line-height: 1.42;
  }

  .ledger-map {
    margin-top: clamp(2.5rem, 5vw, 4rem);
    border-top: 1px solid var(--atlas-rule);
    background: var(--atlas-paper);
  }

  .ledger-map svg {
    display: block;
    width: 100%;
    height: clamp(220px, 23vw, 310px);
  }

  .map-grid {
    fill: none;
    stroke: #e4e5df;
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }

  .signal-line {
    fill: none;
    stroke: var(--atlas-violet);
    stroke-linecap: square;
    stroke-linejoin: miter;
    stroke-width: 3;
    vector-effect: non-scaling-stroke;
  }

  .signal-points {
    fill: var(--atlas-signal);
    stroke: var(--atlas-ink);
    stroke-width: 2;
    vector-effect: non-scaling-stroke;
  }

  .ledger-map figcaption {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.75rem 1rem;
    border-top: 1px solid var(--atlas-rule);
    border-bottom: 1px solid var(--atlas-rule);
    font-family: "Fragment Mono", monospace;
    font-size: 0.7rem;
  }

  .map-annotations,
  .capability-ledger,
  .system-map ol,
  .roadmap-track,
  .incident-events {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .map-annotations {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }

  .map-annotations li {
    display: grid;
    grid-template-columns: 2.2rem minmax(0, 1fr);
    gap: 0.35rem 0.7rem;
    padding: 1rem;
    border-right: 1px solid var(--atlas-rule);
  }

  .map-annotations li:last-child {
    border-right: 0;
  }

  .map-annotations span {
    grid-row: 1 / span 2;
    color: var(--atlas-violet);
    font-size: 0.7rem;
  }

  .map-annotations strong {
    font-size: 0.88rem;
  }

  .map-annotations p {
    font-size: 0.76rem;
    line-height: 1.35;
  }

  .atlas-section {
    padding: clamp(5rem, 9vw, 8.5rem) 0;
    border-bottom: 1px solid var(--atlas-rule);
  }

  .chapter-grid {
    display: grid;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    gap: 1.5rem;
  }

  .chapter-title {
    grid-column: 1 / span 5;
    display: grid;
    grid-template-columns: 2.5rem minmax(0, 1fr);
    gap: 1rem;
    align-items: start;
  }

  .chapter-title > span {
    padding-top: 0.5rem;
    color: var(--atlas-violet);
    font-size: 0.7rem;
  }

  .chapter-copy,
  .architecture-intro {
    grid-column: 7 / -1;
  }

  .chapter-copy {
    display: grid;
    gap: 1.25rem;
  }

  .lead-copy {
    color: #2f2a34;
    font-size: 1.3rem;
    font-weight: 500;
    line-height: 1.5;
  }

  .risk-register {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    margin: 1rem 0 0;
    padding: 0;
    border-top: 1px solid var(--atlas-rule);
    list-style: none;
  }

  .risk-register li {
    padding: 0.75rem 0.75rem 0.75rem 0;
    border-bottom: 1px solid var(--atlas-rule);
    font-size: 0.86rem;
    font-weight: 600;
  }

  .problem-chapter,
  .system-chapter,
  .faq-chapter {
    background: var(--atlas-paper);
  }

  .problem-sequence {
    display: grid;
    grid-template-columns: 1.4fr 4fr 5fr;
    gap: 1.5rem;
    align-items: center;
    margin-top: clamp(4rem, 7vw, 6rem);
    padding: 1.2rem 0;
    border-top: 2px solid var(--atlas-anomaly);
    border-bottom: 1px solid var(--atlas-rule);
    text-decoration: none;
  }

  .problem-sequence:hover strong {
    color: var(--atlas-violet);
  }

  .problem-sequence span {
    color: var(--atlas-anomaly);
    font-family: "Fragment Mono", monospace;
    font-size: 0.68rem;
    text-transform: uppercase;
  }

  .problem-sequence strong {
    font-family: "Archivo SemiCondensed", sans-serif;
    font-size: 1rem;
    line-height: 1.35;
  }

  .problem-sequence p {
    font-size: 0.82rem;
  }

  .first-release {
    padding: 0.9rem 0;
    border-top: 1px solid var(--atlas-violet);
    border-bottom: 1px solid var(--atlas-rule);
    font-size: 0.88rem;
  }

  .first-release strong {
    margin-right: 0.55rem;
    color: var(--atlas-violet);
    font-family: "Fragment Mono", monospace;
    font-size: 0.68rem;
    text-transform: uppercase;
  }

  .capability-ledger {
    margin-top: clamp(4rem, 7vw, 6rem);
    border-top: 1px solid var(--atlas-ink);
  }

  .capability-ledger li {
    display: grid;
    grid-template-columns: 1fr 3fr 7fr;
    gap: 1.5rem;
    padding: 1.6rem 0;
    border-bottom: 1px solid var(--atlas-rule);
  }

  .capability-ledger > li > span {
    color: var(--atlas-violet);
    font-size: 0.72rem;
  }

  .capability-ledger p {
    max-width: 52rem;
  }

  .wallet-boundary {
    display: grid;
    grid-template-columns: 3fr 7fr;
    gap: 1.5rem;
    margin-top: 2rem;
    padding: 1.2rem 0;
    border-top: 1px solid var(--atlas-violet);
    border-bottom: 1px solid var(--atlas-rule);
  }

  .wallet-boundary strong {
    color: var(--atlas-violet);
    font-family: "Archivo SemiCondensed", sans-serif;
  }

  .wallet-boundary p {
    max-width: 52rem;
    font-size: 0.86rem;
  }

  .comparison-block {
    display: grid;
    grid-template-columns: 4fr 6fr;
    gap: 1.5rem;
    align-items: end;
    margin-top: clamp(5rem, 8vw, 7rem);
  }

  .comparison-block > div > p {
    margin-bottom: 0.45rem;
    color: var(--atlas-violet);
    font-family: "Fragment Mono", monospace;
    font-size: 0.68rem;
    text-transform: uppercase;
  }

  .comparison-block h3 {
    font-size: clamp(1.7rem, 3vw, 2.7rem);
  }

  .comparison-block > p {
    font-size: 1.02rem;
  }

  .comparison-frame {
    margin-top: 2rem;
    overflow-x: auto;
    border-top: 1px solid var(--atlas-ink);
  }

  .comparison-frame table {
    width: 100%;
    min-width: 720px;
    border-collapse: collapse;
    text-align: left;
  }

  .comparison-frame th,
  .comparison-frame td {
    padding: 0.9rem 1rem 0.9rem 0;
    border-bottom: 1px solid var(--atlas-rule);
    font-size: 0.82rem;
    line-height: 1.4;
    vertical-align: top;
  }

  .comparison-frame thead th {
    color: var(--atlas-muted);
    font-family: "Fragment Mono", monospace;
    font-size: 0.65rem;
    font-weight: 400;
    text-transform: uppercase;
  }

  .comparison-frame tbody th {
    width: 15%;
    font-family: "Archivo SemiCondensed", sans-serif;
    font-size: 0.95rem;
  }

  .comparison-frame .watch-row {
    background: var(--atlas-violet-soft);
  }

  .comparison-frame .watch-row > *:first-child {
    padding-left: 0.8rem;
    color: var(--atlas-violet);
  }

  .compact-chapter {
    align-items: end;
  }

  .architecture-intro {
    max-width: 44rem;
    color: #37323d;
    font-size: 1.08rem;
  }

  .system-map {
    margin-top: clamp(4rem, 7vw, 6rem);
    border-top: 1px solid var(--atlas-ink);
    border-bottom: 1px solid var(--atlas-rule);
  }

  .system-map ol {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
  }

  .system-map li {
    min-height: 13rem;
    padding: 1rem;
    border-right: 1px solid var(--atlas-rule);
  }

  .system-map li:last-child {
    border-right: 0;
  }

  .system-map li > span {
    display: block;
    margin-bottom: 3.5rem;
    color: var(--atlas-violet);
    font-size: 0.7rem;
  }

  .system-map strong {
    display: block;
    font-family: "Archivo SemiCondensed", sans-serif;
    font-size: 1.2rem;
  }

  .system-map p {
    margin-top: 0.5rem;
    font-size: 0.78rem;
    line-height: 1.4;
  }

  .system-map figcaption {
    max-width: 54rem;
    padding: 1.2rem 0;
    color: var(--atlas-muted);
    font-size: 0.8rem;
    line-height: 1.5;
  }

  .incident-record {
    display: grid;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    gap: 1.5rem;
    margin-top: clamp(5rem, 8vw, 7rem);
    padding-top: 1.2rem;
    border-top: 3px solid var(--atlas-anomaly);
  }

  .incident-record > header {
    grid-column: 1 / span 3;
  }

  .incident-record > header p {
    margin-bottom: 0.55rem;
    color: var(--atlas-anomaly);
    font-family: "Fragment Mono", monospace;
    font-size: 0.7rem;
  }

  .incident-summary {
    grid-column: 4 / span 4;
    display: grid;
    align-content: start;
    gap: 1.1rem;
  }

  .causal-finding {
    color: #2e2933;
    font-weight: 600;
  }

  .incident-events {
    grid-column: 8 / -1;
    border-top: 1px solid var(--atlas-rule);
  }

  .incident-events li {
    display: grid;
    grid-template-columns: 2rem minmax(0, 1fr);
    gap: 0.2rem 0.7rem;
    padding: 0.7rem 0;
    border-bottom: 1px solid var(--atlas-rule);
  }

  .incident-events span {
    grid-row: 1 / span 2;
    color: var(--atlas-anomaly);
    font-size: 0.68rem;
  }

  .incident-events strong {
    font-size: 0.85rem;
  }

  .incident-events p {
    font-size: 0.76rem;
    line-height: 1.35;
  }

  .incident-record > footer {
    grid-column: 4 / -1;
    display: flex;
    justify-content: space-between;
    gap: 2rem;
    margin-top: 1.5rem;
    padding-top: 1rem;
    border-top: 1px solid var(--atlas-rule);
  }

  .incident-record > footer p {
    max-width: 43rem;
    font-size: 0.78rem;
  }

  .incident-record > footer div {
    display: flex;
    flex-wrap: wrap;
    align-content: start;
    justify-content: flex-end;
    gap: 0.7rem 1rem;
    font-size: 0.76rem;
    font-weight: 600;
  }

  .roadmap-chapter {
    background: var(--atlas-violet-soft);
  }

  .roadmap-track {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    margin-top: clamp(4rem, 7vw, 6rem);
    border-top: 1px solid var(--atlas-ink);
  }

  .roadmap-phase {
    position: relative;
    display: grid;
    align-content: start;
    gap: 0.65rem;
    min-height: 26rem;
    padding: 1rem 1.2rem 1.2rem;
    border-right: 1px solid rgba(100, 54, 232, 0.22);
  }

  .roadmap-phase:last-child {
    border-right: 0;
  }

  .roadmap-phase.current::before {
    position: absolute;
    top: -1px;
    right: 0;
    left: 0;
    height: 5px;
    background: var(--atlas-signal);
    content: "";
  }

  .phase-coordinate {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 3rem;
    color: var(--atlas-violet);
    font-size: 0.68rem;
  }

  .phase-coordinate p {
    color: inherit;
    font: inherit;
  }

  .phase-window {
    min-height: 2.4rem;
    color: var(--atlas-violet);
    font-family: "Fragment Mono", monospace;
    font-size: 0.66rem;
    line-height: 1.35;
  }

  .roadmap-phase > p:not(.phase-window) {
    font-size: 0.84rem;
    line-height: 1.48;
  }

  .phase-gate {
    align-self: end;
    margin-top: auto;
    padding-top: 1rem;
    border-top: 1px solid rgba(100, 54, 232, 0.22);
  }

  .phase-gate strong {
    font-size: 0.76rem;
  }

  .phase-gate p {
    margin-top: 0.35rem;
    font-size: 0.74rem;
    line-height: 1.42;
  }

  .faq-grid {
    display: grid;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    gap: 1.5rem;
  }

  .faq-grid .chapter-title {
    grid-column: 1 / span 5;
  }

  .faq-list {
    grid-column: 7 / -1;
    border-top: 1px solid var(--atlas-ink);
  }

  details {
    padding: 1rem 0;
    border-bottom: 1px solid var(--atlas-rule);
  }

  summary {
    font-family: "Archivo SemiCondensed", sans-serif;
    font-size: 1.1rem;
    font-weight: 600;
    cursor: pointer;
  }

  details p {
    max-width: 42rem;
    padding-top: 0.8rem;
  }

  .closing-chapter {
    padding: clamp(5rem, 9vw, 8rem) 0;
    background: var(--atlas-violet);
    color: #fff;
  }

  .closing-grid {
    display: grid;
    grid-template-columns: repeat(12, minmax(0, 1fr));
    gap: 1.5rem;
    align-items: end;
  }

  .closing-grid h2 {
    grid-column: 1 / span 7;
  }

  .closing-grid > div {
    grid-column: 9 / -1;
    display: grid;
    gap: 1.4rem;
  }

  .closing-grid p {
    color: rgba(255, 255, 255, 0.76);
  }

  .closing-grid a {
    justify-self: start;
    padding-bottom: 0.15rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.7);
    font-weight: 600;
    text-decoration: none;
  }

  @media (max-width: 1020px) {
    .outcome-ledger {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }

    .cover-title {
      grid-column: 1 / span 7;
    }

    .cover-copy {
      grid-column: 8 / -1;
    }

    .chapter-title {
      grid-column: 1 / span 6;
    }

    .chapter-copy,
    .architecture-intro {
      grid-column: 7 / -1;
    }

    .system-map ol {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }

    .system-map li:nth-child(3) {
      border-right: 0;
    }

    .system-map li:nth-child(n + 4) {
      border-top: 1px solid var(--atlas-rule);
    }

    .roadmap-track {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .roadmap-phase {
      min-height: 27rem;
      border-bottom: 1px solid rgba(100, 54, 232, 0.22);
    }

    .roadmap-phase:nth-child(2n) {
      border-right: 0;
    }

    .roadmap-phase:last-child {
      border-right: 1px solid rgba(100, 54, 232, 0.22);
    }
  }

  @media (max-width: 760px) {
    .atlas-width {
      width: min(100vw - 2rem, 1320px);
    }

    h1 {
      font-size: clamp(3.5rem, 18vw, 5.2rem);
    }

    h2 {
      font-size: clamp(2.4rem, 12vw, 3.7rem);
    }

    .signal-cover {
      padding-top: 3.5rem;
    }

    .cover-grid,
    .chapter-grid,
    .faq-grid,
    .closing-grid {
      grid-template-columns: 1fr;
    }

    .cover-title,
    .cover-copy,
    .chapter-title,
    .chapter-copy,
    .architecture-intro,
    .faq-grid .chapter-title,
    .faq-list,
    .closing-grid h2,
    .closing-grid > div {
      grid-column: 1;
    }

    .cover-copy {
      margin-top: 2rem;
    }

    .outcome-ledger {
      grid-template-columns: 1fr;
    }

    .outcome-ledger li:nth-child(n) {
      min-height: 0;
      border-right: 0;
    }

    .outcome-ledger li:nth-child(n + 2) {
      border-top: 1px solid var(--atlas-rule);
    }

    .ledger-map {
      margin-right: 0;
      margin-left: 0;
    }

    .ledger-map svg {
      height: 220px;
    }

    .ledger-map figcaption {
      align-items: flex-start;
      flex-direction: column;
    }

    .map-annotations {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .map-annotations li:nth-child(2) {
      border-right: 0;
    }

    .map-annotations li:nth-child(n + 3) {
      border-top: 1px solid var(--atlas-rule);
    }

    .chapter-title {
      grid-template-columns: 2rem minmax(0, 1fr);
    }

    .chapter-copy,
    .architecture-intro {
      margin-top: 2rem;
    }

    .risk-register {
      grid-template-columns: 1fr;
    }

    .problem-sequence,
    .wallet-boundary,
    .comparison-block {
      grid-template-columns: 1fr;
    }

    .problem-sequence {
      gap: 0.7rem;
    }

    .comparison-block > p {
      margin-top: 1rem;
    }

		.comparison-frame {
			margin-right: 0;
		}

    .capability-ledger li {
      grid-template-columns: 2rem minmax(0, 1fr);
      gap: 0.6rem 0.8rem;
    }

    .capability-ledger p {
      grid-column: 2;
    }

    .system-map ol {
      grid-template-columns: 1fr;
    }

    .system-map li,
    .system-map li:nth-child(3) {
      display: grid;
      grid-template-columns: 2rem 7rem minmax(0, 1fr);
      gap: 0.8rem;
      min-height: 0;
      padding: 1rem 0;
      border-top: 0;
      border-right: 0;
      border-bottom: 1px solid var(--atlas-rule);
    }

    .system-map li > span {
      margin-bottom: 0;
    }

    .system-map p {
      margin-top: 0;
    }

    .incident-record {
      grid-template-columns: 1fr;
    }

    .incident-record > header,
    .incident-summary,
    .incident-events,
    .incident-record > footer {
      grid-column: 1;
    }

    .incident-summary,
    .incident-events {
      margin-top: 1rem;
    }

    .incident-record > footer {
      align-items: flex-start;
      flex-direction: column;
    }

    .incident-record > footer div {
      justify-content: flex-start;
    }

    .roadmap-track {
      grid-template-columns: 1fr;
      border-left: 1px solid var(--atlas-violet);
    }

    .roadmap-phase,
    .roadmap-phase:last-child,
    .roadmap-phase:nth-child(2n) {
      min-height: 0;
      padding: 1.2rem 0 1.8rem 1.2rem;
      border-right: 0;
      border-bottom: 1px solid rgba(100, 54, 232, 0.22);
    }

    .roadmap-phase.current::before {
      top: 0;
      bottom: 0;
      left: -4px;
      width: 7px;
      height: auto;
    }

    .phase-coordinate {
      margin-bottom: 1.5rem;
    }

    .phase-window {
      min-height: 0;
    }

    .phase-gate {
      margin-top: 1rem;
    }

    .closing-grid > div {
      margin-top: 2rem;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    :global(html) {
      scroll-behavior: auto;
    }
  }
</style>
