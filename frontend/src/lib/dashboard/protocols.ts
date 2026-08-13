export type ProtocolLogoSurface = 'light' | 'dark';

export interface ProtocolBrand {
	readonly name: string;
	readonly logoPath?: string;
	readonly logoSurface: ProtocolLogoSurface;
}

const PROTOCOL_BRANDS: Readonly<Record<string, ProtocolBrand>> = {
	aquarius: {
		name: 'Aquarius',
		logoPath: '/protocols/aquarius.svg',
		logoSurface: 'light'
	},
	'blend-v2': {
		name: 'Blend v2',
		logoPath: '/protocols/blend-v2.svg',
		logoSurface: 'light'
	},
	defindex: {
		name: 'DeFindex',
		logoPath: '/protocols/defindex.svg',
		logoSurface: 'dark'
	},
	fxdao: {
		name: 'FxDAO',
		logoPath: '/protocols/fxdao.svg',
		logoSurface: 'light'
	},
	phoenix: {
		name: 'Phoenix',
		logoPath: '/protocols/phoenix.svg',
		logoSurface: 'dark'
	},
	reflector: {
		name: 'Reflector',
		logoPath: '/protocols/reflector.svg',
		logoSurface: 'light'
	},
	soroswap: {
		name: 'Soroswap',
		logoPath: '/protocols/soroswap.svg',
		logoSurface: 'dark'
	},
	yieldblox: {
		name: 'YieldBlox',
		logoPath: '/protocols/yieldblox.svg',
		logoSurface: 'light'
	},
	'yieldblox-blend-v2-pool': {
		name: 'YieldBlox pool',
		logoPath: '/protocols/yieldblox.svg',
		logoSurface: 'light'
	}
};

export function protocolBrand(protocolId: string): ProtocolBrand {
	return (
		PROTOCOL_BRANDS[protocolId] ?? {
			name: humanizeProtocolId(protocolId),
			logoSurface: 'light'
		}
	);
}

export function protocolDisplayName(protocolId: string): string {
	return protocolBrand(protocolId).name;
}

export function protocolInitials(protocolId: string): string {
	if (protocolId.trim().length === 0) return '?';
	const words = protocolDisplayName(protocolId)
		.split(/[^A-Za-z0-9]+/)
		.filter(Boolean);
	if (words.length === 0) return '?';
	if (words.length === 1) return words[0]?.slice(0, 2).toUpperCase() ?? '?';
	return words
		.slice(0, 2)
		.map((word) => word[0])
		.join('')
		.toUpperCase();
}

function humanizeProtocolId(protocolId: string): string {
	const value = protocolId.trim().replaceAll(/[-_]+/g, ' ');
	if (value.length === 0) return 'Unknown protocol';
	return value[0]?.toUpperCase() + value.slice(1);
}
