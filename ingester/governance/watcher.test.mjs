import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';

import {
  canonicalizeAccount,
  classifyPolicy,
  runCycle,
  validatePublicKey,
  validateRegistry
} from './watcher.mjs';
import registry from './controllers.json' with { type: 'json' };

const AQUARIUS = 'GAV5FBMKD2ZF4X2MGWDNQYUP7KFL7MRM6HZBY7HKQLB4BRHSCCX5J6VS';
const AQUARIUS_SIGNERS = [
  'GA2YKNLNRAMQ6WXF6D2PD6FKTHFADDYFLJ5N3QGSYG752YFSCGV4I5N7',
  AQUARIUS,
  'GDDILLHDLKEB4A7NASCSAJ65V3XAIIEZQK5I6MAJQ3FTVYQYFQGBHXJH'
];

test('all curated controller IDs and source metadata validate', () => {
  const validated = validateRegistry(registry);
  assert.equal(validated.controllers.length, 8);
  for (const controller of validated.controllers) {
    assert.equal(validatePublicKey(controller.accountId), controller.accountId);
  }
  assert.throws(() => validatePublicKey(`${AQUARIUS.slice(0, -1)}A`), /checksum/);
});

test('classification preserves strict and weighted policy semantics', () => {
  assert.equal(
    classifyPolicy({
      thresholds: { low: 2, medium: 2, high: 2 },
      signers: AQUARIUS_SIGNERS.map((key) => ({ key, type: 'ed25519_public_key', weight: 1 }))
    }),
    'strict'
  );
  assert.equal(
    classifyPolicy({
      thresholds: { low: 20, medium: 20, high: 20 },
      signers: [20, 10, 10].map((weight, index) => ({
        key: AQUARIUS_SIGNERS[index],
        type: 'ed25519_public_key',
        weight
      }))
    }),
    'weighted'
  );
  assert.equal(
    classifyPolicy({
      thresholds: { low: 0, medium: 0, high: 0 },
      signers: [{ key: AQUARIUS, type: 'ed25519_public_key', weight: 1 }]
    }),
    'single-key'
  );
  assert.equal(
    classifyPolicy({
      thresholds: { low: 0, medium: 0, high: 0 },
      signers: [{ key: AQUARIUS, type: 'ed25519_public_key', weight: 0 }]
    }),
    'locked'
  );
});

test('canonicalization sorts the full signer set and derives master weight', () => {
  const controller = validateRegistry(registry).controllers.find(
    (candidate) => candidate.accountId === AQUARIUS
  );
  const result = canonicalizeAccount(accountResponse(controller, 2), controller);
  assert.equal(result.policy.classification, 'strict');
  assert.equal(result.policy.masterWeight, 1);
  assert.deepEqual(
    result.policy.signers.map((signer) => signer.key),
    [...AQUARIUS_SIGNERS].sort()
  );
});

test('baseline and no-change cycles are alert-idempotent', async () => {
  const outputDir = await mkdtemp(join(tmpdir(), 'stargazer-governance-'));
  const controllers = validateRegistry(registry).controllers;
  const fetchImpl = horizonFixture(controllers, () => 2);

  const baseline = await runCycle({
    registry,
    outputDir,
    fetchImpl,
    now: () => new Date('2026-07-27T20:00:00Z')
  });
  assert.deepEqual(baseline, { status: 'baseline', checked: 8, alertsCreated: 0 });

  const unchanged = await runCycle({
    registry,
    outputDir,
    fetchImpl,
    now: () => new Date('2026-07-27T20:01:00Z')
  });
  assert.deepEqual(unchanged, { status: 'unchanged', checked: 8, alertsCreated: 0 });
  assert.deepEqual(await readJson(join(outputDir, 'alerts.json')), {
    schemaVersion: 1,
    alerts: []
  });
});

test('a semantic signer policy change emits one critical alert', async () => {
  const outputDir = await mkdtemp(join(tmpdir(), 'stargazer-governance-'));
  const controllers = validateRegistry(registry).controllers;
  let changed = false;
  const fetchImpl = horizonFixture(controllers, (controller) =>
    changed && controller.accountId === AQUARIUS ? 3 : 2
  );
  await runCycle({
    registry,
    outputDir,
    fetchImpl,
    now: () => new Date('2026-07-27T20:00:00Z')
  });

  changed = true;
  const result = await runCycle({
    registry,
    outputDir,
    fetchImpl,
    now: () => new Date('2026-07-27T20:02:00Z')
  });
  assert.deepEqual(result, { status: 'changed', checked: 8, alertsCreated: 1 });
  const alerts = await readJson(join(outputDir, 'alerts.json'));
  assert.equal(alerts.alerts.length, 1);
  assert.equal(alerts.alerts[0].severity, 'critical');
  assert.equal(alerts.alerts[0].accountId, AQUARIUS);
  assert.equal(alerts.alerts[0].before.thresholds.high, 2);
  assert.equal(alerts.alerts[0].after.thresholds.high, 3);

  const repeated = await runCycle({
    registry,
    outputDir,
    fetchImpl,
    now: () => new Date('2026-07-27T20:03:00Z')
  });
  assert.deepEqual(repeated, { status: 'unchanged', checked: 8, alertsCreated: 0 });
  assert.equal((await readJson(join(outputDir, 'alerts.json'))).alerts.length, 1);
});

test('a successful multisig controller operation emits its bounded payload after baseline', async () => {
  const outputDir = await mkdtemp(join(tmpdir(), 'stargazer-governance-'));
  const controllers = validateRegistry(registry).controllers;
  const operations = new Map(
    controllers.map((controller) => [controller.accountId, []])
  );
  operations.get(AQUARIUS).push(
    operationResponse({
      id: '100',
      sourceAccount: AQUARIUS_SIGNERS[0],
      transactionHash: '11'.repeat(32),
      name: 'incoming-before-baseline'
    })
  );
  const fetchImpl = horizonFixture(controllers, () => 2, (controller) =>
    operations.get(controller.accountId)
  );

  await runCycle({
    registry,
    outputDir,
    fetchImpl,
    now: () => new Date('2026-07-27T20:00:00Z')
  });
  assert.equal((await readJson(join(outputDir, 'alerts.json'))).alerts.length, 0);

  operations.get(AQUARIUS).push(
    operationResponse({
      id: '101',
      sourceAccount: AQUARIUS_SIGNERS[0],
      transactionHash: '22'.repeat(32),
      name: 'incoming-after-baseline'
    }),
    operationResponse({
      id: '102',
      sourceAccount: AQUARIUS,
      transactionHash: '33'.repeat(32),
      name: 'emergency-mode',
      value: 'enabled'
    })
  );
  const result = await runCycle({
    registry,
    outputDir,
    fetchImpl,
    now: () => new Date('2026-07-27T20:01:00Z')
  });

  assert.deepEqual(result, { status: 'changed', checked: 8, alertsCreated: 1 });
  const document = await readJson(join(outputDir, 'alerts.json'));
  assert.equal(document.alerts.length, 1);
  assert.deepEqual(document.alerts[0], {
    id: document.alerts[0].id,
    type: 'classic-controller-transaction-executed',
    severity: 'high',
    detectedAt: '2026-07-27T20:01:00.000Z',
    occurredAt: '2026-07-27T20:00:30.000Z',
    accountId: AQUARIUS,
    protocol: 'Aquarius',
    role: 'root-admin',
    transactionHash: '33'.repeat(32),
    operations: [
      {
        id: '102',
        type: 'manage_data',
        typeCode: 10,
        sourceAccount: AQUARIUS,
        payload: { name: 'emergency-mode', value: 'enabled' }
      }
    ]
  });
  const current = await readJson(join(outputDir, 'current.json'));
  assert.equal(
    current.accounts.find((account) => account.accountId === AQUARIUS).operationCursor,
    '102'
  );
});

test('routine Reflector set_price is suppressed while another contract invocation emits', async () => {
  const outputDir = await mkdtemp(join(tmpdir(), 'stargazer-governance-'));
  const controllers = validateRegistry(registry).controllers;
  const controller = controllers.find(
    (candidate) => candidate.protocol === 'Reflector' && candidate.role === 'crypto-usd-oracle-admin'
  );
  const operations = new Map(controllers.map((candidate) => [candidate.accountId, []]));
  const fetchImpl = horizonFixture(
    controllers,
    () => 2,
    (candidate) => operations.get(candidate.accountId)
  );
  await runCycle({ registry, outputDir, fetchImpl });

  operations.get(controller.accountId).push(
    invokeOperationResponse({
      id: '301',
      sourceAccount: controller.accountId,
      transactionHash: '71'.repeat(32),
      functionSymbol: 'set_price'
    })
  );
  const routine = await runCycle({ registry, outputDir, fetchImpl });
  assert.deepEqual(routine, { status: 'unchanged', checked: 8, alertsCreated: 0 });
  assert.equal((await readJson(join(outputDir, 'alerts.json'))).alerts.length, 0);
  const afterRoutine = await readJson(join(outputDir, 'current.json'));
  assert.equal(
    afterRoutine.accounts.find((account) => account.accountId === controller.accountId)
      .operationCursor,
    '301'
  );

  operations.get(controller.accountId).push(
    invokeOperationResponse({
      id: '302',
      sourceAccount: controller.accountId,
      transactionHash: '72'.repeat(32),
      functionSymbol: 'set_emergency_mode'
    })
  );
  const meaningful = await runCycle({ registry, outputDir, fetchImpl });
  assert.deepEqual(meaningful, { status: 'changed', checked: 8, alertsCreated: 1 });
  const alerts = (await readJson(join(outputDir, 'alerts.json'))).alerts;
  assert.equal(alerts.length, 1);
  assert.equal(alerts[0].transactionHash, '72'.repeat(32));
  assert.equal(alerts[0].operations[0].payload.parameters[1].value, scSymbol('set_emergency_mode'));
});

test('a cycle removes retained routine oracle alerts without removing meaningful activity', async () => {
  const outputDir = await mkdtemp(join(tmpdir(), 'stargazer-governance-'));
  const controllers = validateRegistry(registry).controllers;
  const controller = controllers.find(
    (candidate) => candidate.protocol === 'Reflector' && candidate.role === 'forex-oracle-admin'
  );
  const fetchImpl = horizonFixture(controllers, () => 2);
  await runCycle({ registry, outputDir, fetchImpl });

  const routine = activityAlert({
    id: '81'.repeat(32),
    controller,
    transactionHash: '82'.repeat(32),
    operationId: '401',
    functionSymbol: 'set_price'
  });
  const meaningful = activityAlert({
    id: '83'.repeat(32),
    controller,
    transactionHash: '84'.repeat(32),
    operationId: '402',
    functionSymbol: 'set_emergency_mode'
  });
  await writeFile(
    join(outputDir, 'alerts.json'),
    `${JSON.stringify({ schemaVersion: 1, alerts: [routine, meaningful] }, null, 2)}\n`
  );

  const result = await runCycle({ registry, outputDir, fetchImpl });
  assert.deepEqual(result, {
    status: 'changed',
    checked: 8,
    alertsCreated: 0,
    alertsRemoved: 1
  });
  const retained = (await readJson(join(outputDir, 'alerts.json'))).alerts;
  assert.deepEqual(retained.map((alert) => alert.id), [meaningful.id]);

  const repeated = await runCycle({ registry, outputDir, fetchImpl });
  assert.deepEqual(repeated, { status: 'unchanged', checked: 8, alertsCreated: 0 });
});

test('a partial fetch cycle writes neither snapshot nor alert state', async () => {
  const outputDir = await mkdtemp(join(tmpdir(), 'stargazer-governance-'));
  const controllers = validateRegistry(registry).controllers;
  await runCycle({
    registry,
    outputDir,
    fetchImpl: horizonFixture(controllers, () => 2),
    now: () => new Date('2026-07-27T20:00:00Z')
  });
  const currentBefore = await readFile(join(outputDir, 'current.json'), 'utf8');
  const alertsBefore = await readFile(join(outputDir, 'alerts.json'), 'utf8');

  const failingFetch = async (url, options) => {
    if (url.includes(encodeURIComponent(AQUARIUS))) {
      return { ok: false, status: 503, json: async () => ({}) };
    }
    return horizonFixture(controllers, () => 2)(url, options);
  };
  const result = await runCycle({
    registry,
    outputDir,
    fetchImpl: failingFetch,
    now: () => new Date('2026-07-27T20:04:00Z')
  });
  assert.equal(result.status, 'failed');
  assert.equal(result.failures.length, 1);
  assert.equal(await readFile(join(outputDir, 'current.json'), 'utf8'), currentBefore);
  assert.equal(await readFile(join(outputDir, 'alerts.json'), 'utf8'), alertsBefore);
});

test('an unexpected baseline classification fails closed without writing state', async () => {
  const outputDir = await mkdtemp(join(tmpdir(), 'stargazer-governance-'));
  const controllers = validateRegistry(registry).controllers;
  const result = await runCycle({
    registry,
    outputDir,
    fetchImpl: horizonFixture(controllers, (controller) =>
      controller.accountId === AQUARIUS ? 1 : 2
    )
  });

  assert.equal(result.status, 'failed');
  assert.equal(result.failures.length, 1);
  assert.match(result.failures[0].error, /expected strict policy, observed weighted/);
  await assert.rejects(readFile(join(outputDir, 'current.json')), { code: 'ENOENT' });
  await assert.rejects(readFile(join(outputDir, 'alerts.json')), { code: 'ENOENT' });
});

test('a post-baseline classification change is preserved as a critical policy alert', async () => {
  const outputDir = await mkdtemp(join(tmpdir(), 'stargazer-governance-'));
  const controllers = validateRegistry(registry).controllers;
  let threshold = 2;
  const operations = new Map(controllers.map((controller) => [controller.accountId, []]));
  const fetchImpl = horizonFixture(
    controllers,
    (controller) => controller.accountId === AQUARIUS ? threshold : 2,
    (controller) => operations.get(controller.accountId)
  );
  await runCycle({ registry, outputDir, fetchImpl });

  threshold = 1;
  operations.get(AQUARIUS).push(
    operationResponse({
      id: '201',
      sourceAccount: AQUARIUS,
      transactionHash: '66'.repeat(32),
      name: 'lower-threshold',
      value: '1'
    })
  );
  const result = await runCycle({ registry, outputDir, fetchImpl });
  assert.deepEqual(result, { status: 'changed', checked: 8, alertsCreated: 2 });
  const alerts = (await readJson(join(outputDir, 'alerts.json'))).alerts;
  const policyAlert = alerts.find((alert) => alert.type === 'classic-controller-policy-changed');
  assert.equal(policyAlert.before.classification, 'strict');
  assert.equal(policyAlert.after.classification, 'weighted');
  assert.ok(alerts.some((alert) => alert.type === 'classic-controller-transaction-executed'));
});

test('retention is bounded across repeated real transitions', async () => {
  const outputDir = await mkdtemp(join(tmpdir(), 'stargazer-governance-'));
  const controllers = validateRegistry(registry).controllers;
  let threshold = 2;
  const fetchImpl = horizonFixture(controllers, (controller) =>
    controller.accountId === AQUARIUS ? threshold : 2
  );
  await runCycle({ registry, outputDir, fetchImpl });
  threshold = 3;
  await runCycle({ registry, outputDir, fetchImpl, maxAlerts: 2 });
  threshold = 2;
  await runCycle({ registry, outputDir, fetchImpl, maxAlerts: 2 });
  threshold = 3;
  await runCycle({ registry, outputDir, fetchImpl, maxAlerts: 2 });
  assert.equal((await readJson(join(outputDir, 'alerts.json'))).alerts.length, 2);
});

function horizonFixture(controllers, thresholdFor, operationsFor = () => []) {
  const byAccountId = new Map(controllers.map((controller) => [controller.accountId, controller]));
  return async (url) => {
    const parsed = new URL(url);
    const segments = parsed.pathname.split('/').filter(Boolean);
    const isOperationsRequest = segments.at(-1) === 'operations';
    const accountId = decodeURIComponent(segments.at(isOperationsRequest ? -2 : -1));
    const controller = byAccountId.get(accountId);
    assert.ok(controller, `unexpected account request: ${accountId}`);
    if (isOperationsRequest) {
      const order = parsed.searchParams.get('order');
      const limit = Number(parsed.searchParams.get('limit'));
      const cursor = parsed.searchParams.get('cursor');
      const operations = [...operationsFor(controller)].sort((left, right) =>
        BigInt(left.paging_token) < BigInt(right.paging_token) ? -1 : 1
      );
      const records = order === 'desc'
        ? operations.slice(-limit).reverse()
        : operations.filter((operation) => BigInt(operation.paging_token) > BigInt(cursor)).slice(0, limit);
      return {
        ok: true,
        status: 200,
        json: async () => ({ _embedded: { records } })
      };
    }
    return {
      ok: true,
      status: 200,
      json: async () => accountResponse(controller, thresholdFor(controller))
    };
  };
}

function operationResponse({ id, sourceAccount, transactionHash, name, value = null }) {
  return {
    _links: { self: { href: `https://example.test/operations/${id}` } },
    id,
    paging_token: id,
    transaction_successful: true,
    source_account: sourceAccount,
    transaction_hash: transactionHash,
    created_at: '2026-07-27T20:00:30Z',
    type: 'manage_data',
    type_i: 10,
    name,
    value
  };
}

function invokeOperationResponse({ id, sourceAccount, transactionHash, functionSymbol }) {
  return {
    _links: { self: { href: `https://example.test/operations/${id}` } },
    id,
    paging_token: id,
    transaction_successful: true,
    source_account: sourceAccount,
    transaction_hash: transactionHash,
    created_at: '2026-07-27T20:00:30Z',
    type: 'invoke_host_function',
    type_i: 24,
    address: '',
    asset_balance_changes: null,
    function: 'HostFunctionTypeHostFunctionTypeInvokeContract',
    parameters: [
      { type: 'Address', value: 'AAAAEgAAAAEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA==' },
      { type: 'Sym', value: scSymbol(functionSymbol) }
    ],
    salt: ''
  };
}

function activityAlert({ id, controller, transactionHash, operationId, functionSymbol }) {
  const operation = invokeOperationResponse({
    id: operationId,
    sourceAccount: controller.accountId,
    transactionHash,
    functionSymbol
  });
  return {
    id,
    type: 'classic-controller-transaction-executed',
    severity: 'high',
    detectedAt: '2026-07-27T20:01:00.000Z',
    occurredAt: operation.created_at,
    accountId: controller.accountId,
    protocol: controller.protocol,
    role: controller.role,
    transactionHash,
    operations: [
      {
        id: operation.id,
        type: operation.type,
        typeCode: operation.type_i,
        sourceAccount: operation.source_account,
        payload: {
          address: operation.address,
          asset_balance_changes: operation.asset_balance_changes,
          function: operation.function,
          parameters: operation.parameters,
          salt: operation.salt
        }
      }
    ]
  };
}

function scSymbol(value) {
  const symbol = Buffer.from(value, 'utf8');
  const paddedLength = Math.ceil(symbol.length / 4) * 4;
  const encoded = Buffer.alloc(8 + paddedLength);
  encoded.writeUInt32BE(15, 0);
  encoded.writeUInt32BE(symbol.length, 4);
  symbol.copy(encoded, 8);
  return encoded.toString('base64');
}

function accountResponse(controller, threshold) {
  const extraKeys = AQUARIUS_SIGNERS.filter((key) => key !== controller.accountId);
  const weights = controller.expectedClassification === 'weighted' ? [2, 1, 1] : [1, 1, 1];
  const signers = [
    { key: extraKeys[1], type: 'ed25519_public_key', weight: weights[0] },
    { key: controller.accountId, type: 'ed25519_public_key', weight: weights[1] },
    { key: extraKeys[0], type: 'ed25519_public_key', weight: weights[2] }
  ];
  return {
    account_id: controller.accountId,
    last_modified_ledger: 63_676_623 + threshold,
    thresholds: {
      low_threshold: threshold,
      med_threshold: threshold,
      high_threshold: threshold
    },
    signers
  };
}

async function readJson(path) {
  return JSON.parse(await readFile(path, 'utf8'));
}
