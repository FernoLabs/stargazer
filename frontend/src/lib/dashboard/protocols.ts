export type ProtocolLogoSurface = 'light' | 'dark';

export interface ProtocolBrand {
	readonly name: string;
	readonly logoPath?: string;
	readonly logoSurface: ProtocolLogoSurface;
}

const PROTOCOL_BRANDS: Readonly<Record<string, ProtocolBrand>> = {
	aquarius: {
		name: 'Aquarius',
		logoSurface: 'light'
	},
	'blend-v2': {
		name: 'Blend v2',
		logoSurface: 'light'
	},
	defindex: {
		name: 'DeFindex',
		logoSurface: 'light'
	},
	fxdao: {
		name: 'FxDAO',
		logoSurface: 'light'
	},
	phoenix: {
		name: 'Phoenix',
		logoSurface: 'light'
	},
	reflector: {
		name: 'Reflector',
		logoSurface: 'light'
	},
	soroswap: {
		name: 'Soroswap',
		logoSurface: 'light'
	},
	yieldblox: {
		name: 'YieldBlox',
		logoSurface: 'light'
	},
	'yieldblox-blend-v2-pool': {
		name: 'YieldBlox pool',
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
