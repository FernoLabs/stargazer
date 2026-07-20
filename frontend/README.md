# Stargazer web

SvelteKit website for Stargazer by Ferno, a Stellar protocol monitoring and intelligence layer.

The public product page is served at `/`. The grant working page is served at
`/scf/application` with `noindex`, `nofollow`, and `noarchive` headers.

Production is published at [`stargazer.ferno.ag`](https://stargazer.ferno.ag/).
`stellarwatch.ferno.ag` remains available as a legacy alias. The Cloudflare Pages project
retains its existing `ferno-pulsar` technical identifier.

## Developing

```bash
npm ci
npm run dev
```

## Verification

```bash
npm run check
npm run build
```
