import assert from 'node:assert/strict';
import { accessSync } from 'node:fs';
import { join } from 'node:path';
import test from 'node:test';
import { protocolBrand, protocolDisplayName, protocolInitials } from './protocols';

const CATALOG_PROTOCOLS = [
	'aquarius',
	'blend-v2',
	'defindex',
	'fxdao',
	'phoenix',
	'reflector',
	'soroswap'
] as const;

test('every live catalog protocol has a local logo asset', () => {
	for (const protocolId of CATALOG_PROTOCOLS) {
		const brand = protocolBrand(protocolId);
		assert.ok(brand.logoPath, `${protocolId} is missing a logo path`);
		assert.doesNotThrow(() =>
			accessSync(join(process.cwd(), 'static', brand.logoPath?.replace(/^\//, '') ?? ''))
		);
	}
});

test('uses one YieldBlox identity for incident and protocol IDs', () => {
	assert.equal(protocolBrand('yieldblox').logoPath, '/protocols/yieldblox.svg');
	assert.equal(protocolBrand('yieldblox-blend-v2-pool').logoPath, '/protocols/yieldblox.svg');
	assert.equal(protocolDisplayName('yieldblox-blend-v2-pool'), 'YieldBlox pool');
});

test('unknown protocol IDs receive a readable identity fallback', () => {
	assert.equal(protocolDisplayName('new_protocol-adapter'), 'New protocol adapter');
	assert.equal(protocolInitials('new_protocol-adapter'), 'NP');
	assert.equal(protocolInitials(''), '?');
});
