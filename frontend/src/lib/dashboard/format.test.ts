import assert from 'node:assert/strict';
import test from 'node:test';
import { formatDatasetPeriod } from './format';

test('formats the exact indexed archive window and rounded duration', () => {
	assert.deepEqual(
		formatDatasetPeriod('2025-05-28T09:28:29.000Z', '2026-07-23T21:01:16.000Z'),
		{
			start: '28 May 2025',
			end: '23 Jul 2026',
			range: '28 May 2025–23 Jul 2026',
			duration: '≈14 months'
		}
	);
});

test('does not render a missing or inverted archive window', () => {
	assert.equal(formatDatasetPeriod(undefined, '2026-07-23T21:01:16.000Z'), null);
	assert.equal(
		formatDatasetPeriod('2026-07-24T00:00:00.000Z', '2026-07-23T21:01:16.000Z'),
		null
	);
});
