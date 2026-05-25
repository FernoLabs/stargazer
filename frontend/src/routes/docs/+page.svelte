<script lang="ts">
	import SectionHeader from '$lib/components/SectionHeader.svelte';
</script>

<svelte:head>
	<title>FireWatch Reference</title>
</svelte:head>

<div class="page-shell">
	<section class="panel">
		<SectionHeader
			title="Telegram setup and watch model"
			description="The app stays lean. This page keeps command, setup, and access detail in one place."
			level="1"
			href="/pricing"
			hrefLabel="Pricing"
		/>
	</section>

	<section class="grid">
		<article class="panel">
			<h2>Telegram setup</h2>
			<ol>
				<li>Add the configured FireWatch bot to your private chat, group, or channel.</li>
				<li>Send <code>/help</code> once.</li>
				<li>Copy commands from Home, a protocol page, or a program page.</li>
			</ol>
		</article>

		<article class="panel">
			<h2>Access model</h2>
			<ul>
				<li>Free users get only the shared, unfiltered <code>/watch defi</code> alert feed.</li>
				<li>Custom Plan access unlocks program, protocol, wallet, and threshold watches.</li>
				<li>Ferno members get Custom Plan access for free.</li>
				<li>Superteam members can use a promo code for 12 months of Custom Plan access.</li>
				<li>Pro Response adds reviewed signal access and an authorized Smart Account flow.</li>
			</ul>
		</article>

		<article class="panel" id="commands">
			<h2>Commands</h2>
			<ul>
				<li><code>/help</code> shows the command list.</li>
				<li><code>/info</code> shows live FireWatch status for the current bot.</li>
				<li><code>/list</code> shows the active FireWatch watches in the current chat.</li>
				<li><code>/watch</code> opens the quick subscription menu.</li>
				<li><code>/watch defi</code> tracks the free, unfiltered DeFi alert feed.</li>
				<li><code>/watch protocol &lt;slug&gt;</code> is configured through Custom Plan setup.</li>
				<li><code>/watch program &lt;address&gt;</code> is configured through Custom Plan setup.</li>
				<li><code>/watch multisig &lt;address&gt;</code> is configured through Custom Plan setup.</li>
				<li><code>/unwatch</code> opens the remove-actions menu.</li>
				<li><code>/cancel</code> closes the menu without removing live rules.</li>
			</ul>
		</article>

		<article class="panel">
			<h2>What DeFi watch means</h2>
			<ul>
				<li>Known protocol upgrades and canonical IDL changes.</li>
				<li>High-impact admin actions when the catalog knows them.</li>
				<li>Unknown protocol instructions when supported.</li>
				<li>Optional metric anomaly rules for a few noisy instructions.</li>
			</ul>
		</article>

		<article class="panel">
			<h2>Pages</h2>
			<p>
				Home is for setup and catalog browsing. Activity is the global event stream. Protocol pages
				group one platform. Program pages focus on one exact watched address.
			</p>
		</article>
	</section>
</div>

<style>
	.page-shell {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem 0 2.4rem;
		display: grid;
		gap: 0.8rem;
	}

	.grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 1rem;
	}

	.panel {
		padding: 1.05rem;
		background: #fff;
		border: 1px solid rgba(23, 32, 42, 0.08);
		border-radius: 10px;
	}

	h2 {
		margin: 0;
	}

	li,
	p {
		color: #5f6c79;
		line-height: 1.65;
	}

	code {
		font-family: 'IBM Plex Mono', 'SFMono-Regular', monospace;
		overflow-wrap: anywhere;
	}

	@media (max-width: 900px) {
		.grid {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 720px) {
		.page-shell {
			width: min(100vw - 1rem, var(--fw-page-max));
			padding-top: 0.8rem;
		}
	}
</style>
