import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig, searchForWorkspaceRoot, type Plugin } from 'vite';
import { renderMarkdown } from './src/lib/docs/markdown';

const repositoryDocs = fileURLToPath(new URL('../docs', import.meta.url));

/** Turns `import html from '<file>.md?html'` into a build-time HTML string. */
function markdownHtml(): Plugin {
	const suffix = '.md?html';
	return {
		name: 'stargazer-markdown-html',
		enforce: 'pre',
		async load(id) {
			if (!id.endsWith(suffix)) return null;
			const file = id.slice(0, -'?html'.length);
			this.addWatchFile(file);
			const html = renderMarkdown(await readFile(file, 'utf8'));
			return `export default ${JSON.stringify(html)};`;
		}
	};
}

export default defineConfig({
	plugins: [markdownHtml(), sveltekit()],
	server: {
		fs: {
			// /docs/architecture renders ../docs/ARCHITECTURE.md from the repository root.
			allow: [searchForWorkspaceRoot(process.cwd()), repositoryDocs]
		}
	}
});
