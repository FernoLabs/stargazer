import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [sveltekit()],
	ssr: {
		noExternal: ['@fernolab/wallet-adapter-core', '@fernolab/wallet-adapter-svelte']
	}
});
