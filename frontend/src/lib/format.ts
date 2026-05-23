export function shortAddress(address: string) {
	if (address.length < 12) return address;
	return `${address.slice(0, 4)}...${address.slice(-4)}`;
}

export function formatTimestamp(timestamp: number | null | undefined) {
	if (!timestamp) return 'Pending block time';
	return new Intl.DateTimeFormat('en-US', {
		dateStyle: 'medium',
		timeStyle: 'short'
	}).format(new Date(timestamp * 1000));
}

export function readableInstructionName(instruction: string) {
	return instruction
		.split('_')
		.map((part) => (part ? `${part[0].toUpperCase()}${part.slice(1)}` : part))
		.join(' ');
}

export function formatMetricValue(value: number | null | undefined) {
	if (value === null || value === undefined) return 'n/a';
	return new Intl.NumberFormat('en-US', {
		maximumFractionDigits: value >= 1000 ? 0 : 4
	}).format(value);
}
