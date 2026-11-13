declare module '*.md?html' {
	/** HTML rendered at build time by the stargazer-markdown-html Vite plugin. */
	const html: string;
	export default html;
}
