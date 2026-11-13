import { Marked, type Tokens } from 'marked';

/**
 * Build-time Markdown renderer for repository documents shown under /docs.
 * The Vite plugin in vite.config.ts calls it, so the browser receives HTML only.
 * The source is a reviewed file in this repository, not user input.
 */

const HTML_ESCAPES: Record<string, string> = {
	'&': '&amp;',
	'<': '&lt;',
	'>': '&gt;',
	'"': '&quot;',
	"'": '&#39;'
};

function escapeHtml(value: string): string {
	return value.replace(/[&<>"']/g, (character) => HTML_ESCAPES[character]);
}

/** Heading anchor: "4. Overview" gives "overview". */
export function headingId(text: string): string {
	return text
		.toLowerCase()
		.replace(/^\d+\.\s*/, '')
		.replace(/[^a-z0-9]+/g, '-')
		.replace(/^-+|-+$/g, '');
}

export function renderMarkdown(source: string): string {
	const usedIds = new Map<string, number>();

	const marked = new Marked({
		gfm: true,
		renderer: {
			heading({ tokens, depth, text }: Tokens.Heading): string {
				const base = headingId(text) || 'section';
				const count = usedIds.get(base) ?? 0;
				usedIds.set(base, count + 1);
				const id = count === 0 ? base : `${base}-${count + 1}`;
				return `<h${depth} id="${id}">${this.parser.parseInline(tokens)}</h${depth}>\n`;
			},
			code({ text, lang }: Tokens.Code): string | false {
				if (lang?.trim() !== 'mermaid') return false;
				// The browser renders this source into an SVG diagram; the source stays visible without JavaScript.
				return `<figure class="doc-diagram" data-mermaid><pre class="doc-diagram-source"><code>${escapeHtml(text)}</code></pre></figure>\n`;
			}
		},
		hooks: {
			postprocess(html: string): string {
				return html
					.replaceAll('<table>', '<div class="table-scroll"><table>')
					.replaceAll('</table>', '</table></div>');
			}
		}
	});

	return marked.parse(source, { async: false });
}
