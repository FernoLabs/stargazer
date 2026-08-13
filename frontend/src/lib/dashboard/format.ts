export interface AddressFormatOptions {
	readonly start?: number;
	readonly end?: number;
}

/** Shortens a StrKey or transaction hash without changing the underlying value. */
export function formatAddress(
	value: string,
	{ start = 7, end = 6 }: AddressFormatOptions = {}
): string {
	const safeStart = Math.max(1, Math.trunc(start));
	const safeEnd = Math.max(1, Math.trunc(end));
	if (value.length <= safeStart + safeEnd + 1) return value;
	return `${value.slice(0, safeStart)}…${value.slice(-safeEnd)}`;
}

export function formatCount(value: number): string {
	if (!Number.isFinite(value)) return '—';
	return new Intl.NumberFormat('en-US', {
		notation: Math.abs(value) >= 1_000 ? 'compact' : 'standard',
		maximumFractionDigits: Math.abs(value) >= 10_000 ? 0 : 1
	}).format(value);
}

export function formatExactCount(value: number): string {
	if (!Number.isFinite(value)) return '—';
	return new Intl.NumberFormat('en-US', { maximumFractionDigits: 0 }).format(value);
}

const datasetDateFormatter = new Intl.DateTimeFormat('en-GB', {
	day: '2-digit',
	month: 'short',
	year: 'numeric',
	timeZone: 'UTC'
});

export interface FormattedDatasetPeriod {
	readonly start: string;
	readonly end: string;
	readonly range: string;
	readonly duration: string;
}

/** Formats an archive window without implying more precision than calendar months. */
export function formatDatasetPeriod(
	indexedFrom: string | undefined,
	indexedThrough: string
): FormattedDatasetPeriod | null {
	if (!indexedFrom) return null;

	const startDate = new Date(indexedFrom);
	const endDate = new Date(indexedThrough);
	if (
		Number.isNaN(startDate.valueOf()) ||
		Number.isNaN(endDate.valueOf()) ||
		endDate < startDate
	) {
		return null;
	}

	const elapsedDays = (endDate.valueOf() - startDate.valueOf()) / 86_400_000;
	const roundedMonths = Math.max(1, Math.round(elapsedDays / 30.4375));
	const start = datasetDateFormatter.format(startDate);
	const end = datasetDateFormatter.format(endDate);

	return {
		start,
		end,
		range: `${start}–${end}`,
		duration: `≈${roundedMonths} ${roundedMonths === 1 ? 'month' : 'months'}`
	};
}
