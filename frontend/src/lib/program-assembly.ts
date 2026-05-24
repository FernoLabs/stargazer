export const DISASSEMBLY_VIEW_CACHE_VERSION = 'handler-labels-v3';

export function isAssemblyAnnotationLine(line: string) {
	return line.startsWith('; ');
}

export function normalizeAssemblyRelocations(body: string) {
	return body
		.replace(/\bcall\s+0x[0-9a-f]+\b/g, 'call <target>')
		.replace(/\bgoto\s+[+-]0x[0-9a-f]+\b/g, 'goto <offset>')
		.replace(/\b0x[0-9a-f]{5,}\s+ll\b/g, '<addr> ll');
}

export function prepareAssemblyText(
	text: string,
	options: { showAnnotations: boolean; ignoreRelocationNoise: boolean }
) {
	return text
		.split('\n')
		.filter((line) => options.showAnnotations || !isAssemblyAnnotationLine(line))
		.map((line) => (options.ignoreRelocationNoise ? normalizeAssemblyLine(line) : line))
		.join('\n');
}

function normalizeAssemblyLine(line: string) {
	const match = /^(\d+:\s?)(.*)$/.exec(line);
	if (!match) return normalizeAssemblyRelocations(line);
	return `${match[1]}${normalizeAssemblyRelocations(match[2])}`;
}
