<script lang="ts">
	import BrandLockup from '$lib/components/BrandLockup.svelte';
</script>

<footer class="site-footer">
	<div class="footer-shell">
		<div class="footer-brand">
			<BrandLockup compact href="/" />
			<p>FireWatch is protocol safety tooling for Solana operators.</p>
		</div>

		<nav class="footer-links" aria-label="FireWatch footer">
			<a href="/docs">Reference</a>
			<span class="muted-text">GitHub soon</span>
		</nav>
	</div>
</footer>

<style>
	.site-footer {
		margin-top: 1.2rem;
		border-top: 1px solid rgba(23, 32, 42, 0.08);
		background: #fff;
	}

	.footer-shell {
		width: min(var(--fw-page-max), calc(100vw - 2rem));
		margin: 0 auto;
		padding: 1rem 0 1.6rem;
		display: flex;
		align-items: end;
		justify-content: space-between;
		gap: 1rem;
	}

	.footer-brand {
		display: grid;
		gap: 0.35rem;
	}

	p {
		margin: 0;
		max-width: 34rem;
		font-size: 0.83rem;
		line-height: 1.5;
		color: #5f6c79;
	}

	.footer-links {
		display: flex;
		flex-wrap: wrap;
		gap: 0.8rem;
		align-items: center;
		justify-content: flex-end;
	}

	.footer-links a,
	.muted-text {
		font-size: 0.78rem;
		font-weight: 600;
		color: #556473;
		text-decoration: none;
	}

	.footer-links a {
		text-decoration: underline;
		text-underline-offset: 0.12em;
	}

	.muted-text {
		color: #8a6d55;
	}

	@media (max-width: 820px) {
		.footer-shell {
			flex-direction: column;
			align-items: start;
		}

		.footer-links {
			justify-content: start;
		}
	}

	@media (max-width: 720px) {
		.footer-shell {
			width: min(100vw - 1rem, var(--fw-page-max));
		}
	}
</style>
