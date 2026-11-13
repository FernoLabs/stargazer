import mermaid from 'mermaid';

let initialized = false;
let sequence = 0;

/**
 * Renders each `figure[data-mermaid]` under `root` into an SVG diagram.
 * The page loads this module only in the browser. If a diagram fails, its source stays visible.
 */
export async function renderMermaidDiagrams(root: HTMLElement): Promise<void> {
	const figures = Array.from(root.querySelectorAll<HTMLElement>('figure[data-mermaid]'));
	if (figures.length === 0) return;

	if (!initialized) {
		mermaid.initialize({
			startOnLoad: false,
			theme: 'dark',
			securityLevel: 'strict',
			fontFamily: 'inherit'
		});
		initialized = true;
	}

	for (const figure of figures) {
		if (figure.dataset.rendered === 'true') continue;
		const source = figure.querySelector('code')?.textContent;
		if (!source) continue;

		try {
			sequence += 1;
			const { svg } = await mermaid.render(`stargazer-diagram-${sequence}`, source);
			const diagram = document.createElement('div');
			diagram.className = 'doc-diagram-render';
			diagram.innerHTML = svg;
			figure.replaceChildren(diagram);
			figure.dataset.rendered = 'true';
		} catch (error) {
			console.error('Stargazer docs: a diagram could not be rendered; its source stays visible.', error);
		}
	}
}
