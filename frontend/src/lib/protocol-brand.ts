const EXPLICIT_LOGO_IDS: Record<string, string> = {
	orca_whirlpools: 'orca',
	kamino_lend: 'kamino',
	drift: 'drift',
	marginfi: 'marginfi',
	jupiter_lend: 'jupiter-exchange',
	raydium: 'raydium',
	meteora_dlmm: 'meteora',
	meteora: 'meteora',
	exponent: 'exponent',
	carrot: 'carrot',
	save: 'save',
	openbook: 'openbook',
	manifest: 'manifest',
	glam: 'glam',
	adrena: 'adrena',
	loopscale: 'loopscale',
	neutral: 'neutral',
	fragmetric: 'fragmetric',
	marinade: 'marinade',
	jito: 'jito',
	kamino: 'kamino',
	parcl: 'parcl',
	perena: 'perena',
	sanctum: 'sanctum',
	solayer: 'solayer',
	huma: 'huma',
	maple: 'maple',
	flashtrade: 'flash',
	wasabi: 'wasabi'
};

const EXPLICIT_LOGO_URLS: Record<string, string> = {
	bonk_dao:
		'https://www.bonkcoin.com/_next/image?url=%2F_next%2Fstatic%2Fmedia%2Fbonk-logo.6e917566.png&w=256&q=75'
};

export function protocolLogoId(slug: string | null) {
	if (!slug) return null;
	if (slug === 'ferno') return null;
	return EXPLICIT_LOGO_IDS[slug] ?? slug.replace(/_/g, '-');
}

export function protocolLogoUrl(slug: string | null, failed: Record<string, true> = {}) {
	if (slug && failed[slug]) return null;
	if (slug && EXPLICIT_LOGO_URLS[slug]) return EXPLICIT_LOGO_URLS[slug];

	const logoId = protocolLogoId(slug);
	if (!logoId || failed[logoId]) return null;
	return `https://raw.githubusercontent.com/jup-ag/platform-list/main/img/${logoId}.webp`;
}

export function protocolMonogram(slug: string | null, label: string | null) {
	if (!label) return 'F';
	if (slug === 'meteora_dlmm') return 'DL';
	if (slug === 'jupiter_lend') return 'JL';
	if (slug === 'orca_whirlpools') return 'OR';
	if (slug === 'raydium') return 'RY';
	if (slug === 'kamino_lend') return 'KA';
	if (slug === 'drift') return 'DR';
	if (slug === 'marginfi') return 'MF';

	return label
		.split(/\s+/)
		.map((part) => part[0])
		.join('')
		.slice(0, 2)
		.toUpperCase();
}

export function protocolTone(slug: string | null) {
	const tones = ['amber', 'teal', 'ocean', 'red', 'forest', 'indigo'];
	if (!slug) return tones[0];
	const sum = [...slug].reduce((acc, char) => acc + char.charCodeAt(0), 0);
	return tones[sum % tones.length];
}
