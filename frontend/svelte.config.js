import adapter from '@sveltejs/adapter-cloudflare';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	compilerOptions: {
		// Force runes mode for the project, except for libraries. Can be removed in svelte 6.
		runes: ({ filename }) => filename.split(/[/\\]/).includes('node_modules') ? undefined : true
	},
	kit: {
		adapter: adapter(),
		alias: {
			// Repository-level documents, such as ARCHITECTURE.md, rendered by the /docs pages.
			'$repo-docs': '../docs'
		}
	}
};

export default config;
