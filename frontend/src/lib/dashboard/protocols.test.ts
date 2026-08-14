import assert from 'node:assert/strict';
import test from 'node:test';
import { protocolBrand, protocolDisplayName, protocolInitials } from './protocols';

test('known protocols receive stable text identities', () => {
	assert.equal(protocolDisplayName('aquarius'), 'Aquarius');
	assert.equal(protocolInitials('aquarius'), 'AQ');
	assert.equal(protocolDisplayName('blend-v2'), 'Blend v2');
	assert.equal(protocolInitials('blend-v2'), 'BV');
});

test('uses one YieldBlox identity for incident and protocol IDs', () => {
	assert.equal(protocolBrand('yieldblox').name, 'YieldBlox');
	assert.equal(protocolDisplayName('yieldblox-blend-v2-pool'), 'YieldBlox pool');
	assert.equal(protocolInitials('yieldblox'), 'YI');
	assert.equal(protocolInitials('yieldblox-blend-v2-pool'), 'YP');
});

test('unknown protocol IDs receive a readable identity fallback', () => {
	assert.equal(protocolDisplayName('new_protocol-adapter'), 'New protocol adapter');
	assert.equal(protocolInitials('new_protocol-adapter'), 'NP');
	assert.equal(protocolInitials(''), '?');
});
