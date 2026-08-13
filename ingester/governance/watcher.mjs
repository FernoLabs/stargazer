#!/usr/bin/env node

import { createHash, randomUUID } from 'node:crypto';
import { mkdir, open, readFile, rename, unlink } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const DEFAULT_HORIZON_URL = 'https://horizon.stellar.org';
const DEFAULT_INTERVAL_MS = 60_000;
const DEFAULT_TIMEOUT_MS = 10_000;
const DEFAULT_MAX_ALERTS = 1_000;
const HORIZON_OPERATION_PAGE_SIZE = 200;
const MAX_OPERATION_PAGES_PER_CYCLE = 10;
const MAX_OPERATION_PAYLOAD_BYTES = 32_768;
const MAX_OPERATION_PAYLOAD_DEPTH = 8;
const SCV_SYMBOL_DISCRIMINANT = 15;
const MAX_SC_SYMBOL_BYTES = 32;
const CURRENT_FILENAME = 'current.json';
const ALERTS_FILENAME = 'alerts.json';
const ED25519_PUBLIC_KEY_VERSION = 6 << 3;
const CLASSIFICATIONS = new Set(['strict', 'weighted', 'single-key', 'locked']);
const OPERATION_METADATA_FIELDS = new Set([
  '_links',
  'id',
  'paging_token',
  'source_account',
  'transaction_hash',
  'transaction_successful',
  'created_at',
  'type',
  'type_i'
]);
const ROUTINE_ACTIVITY_RULES = [
  {
    protocol: 'Reflector',
    roles: new Set([
      'crypto-usd-oracle-admin',
      'stellar-assets-oracle-admin',
      'forex-oracle-admin'
    ]),
    operationType: 'invoke_host_function',
    functionSymbol: 'set_price'
  }
];

export function validatePublicKey(value) {
  if (typeof value !== 'string' || !/^G[A-Z2-7]{55}$/.test(value)) {
    throw new Error(`invalid Stellar public key: ${String(value)}`);
  }

  const decoded = decodeBase32(value);
  if (decoded.length !== 35 || decoded[0] !== ED25519_PUBLIC_KEY_VERSION) {
    throw new Error(`invalid Stellar public-key payload: ${value}`);
  }

  const payload = decoded.subarray(0, 33);
  const expectedChecksum = decoded[33] | (decoded[34] << 8);
  if (crc16Xmodem(payload) !== expectedChecksum) {
    throw new Error(`invalid Stellar public-key checksum: ${value}`);
  }

  return value;
}

export function validateRegistry(value) {
  if (!isObject(value) || value.schemaVersion !== 1 || value.network !== 'public') {
    throw new Error('controller registry must be schemaVersion 1 on the public network');
  }
  if (!Array.isArray(value.controllers) || value.controllers.length === 0) {
    throw new Error('controller registry must contain at least one controller');
  }

  const accountIds = new Set();
  const controllers = value.controllers.map((controller, index) => {
    if (!isObject(controller)) {
      throw new Error(`controllers[${index}] must be an object`);
    }
    const accountId = validatePublicKey(controller.accountId);
    if (accountIds.has(accountId)) {
      throw new Error(`duplicate controller account: ${accountId}`);
    }
    accountIds.add(accountId);

    const protocol = nonEmptyString(controller.protocol, `controllers[${index}].protocol`);
    const role = nonEmptyString(controller.role, `controllers[${index}].role`);
    const expectedClassification = nonEmptyString(
      controller.expectedClassification,
      `controllers[${index}].expectedClassification`
    );
    if (!CLASSIFICATIONS.has(expectedClassification)) {
      throw new Error(`unsupported expected classification: ${expectedClassification}`);
    }
    if (!Array.isArray(controller.sources) || controller.sources.length === 0) {
      throw new Error(`controllers[${index}].sources must not be empty`);
    }
    const sources = controller.sources.map((source, sourceIndex) => {
      const parsed = new URL(nonEmptyString(source, `controllers[${index}].sources[${sourceIndex}]`));
      if (parsed.protocol !== 'https:') {
        throw new Error(`controller source must use HTTPS: ${source}`);
      }
      return parsed.toString();
    });

    return { accountId, protocol, role, expectedClassification, sources };
  });

  return {
    schemaVersion: 1,
    network: 'public',
    observedAt: nonEmptyString(value.observedAt, 'registry.observedAt'),
    observationLedger: unsignedInteger(value.observationLedger, 'registry.observationLedger'),
    controllers: controllers.sort((left, right) => left.accountId.localeCompare(right.accountId))
  };
}

export function classifyPolicy({ thresholds, signers }) {
  const positiveSigners = signers.filter((signer) => signer.weight > 0);
  if (positiveSigners.length === 0) {
    return 'locked';
  }
  if (positiveSigners.length === 1) {
    return 'single-key';
  }

  const weights = new Set(positiveSigners.map((signer) => signer.weight));
  const thresholdValues = [thresholds.low, thresholds.medium, thresholds.high];
  if (weights.size === 1 && new Set(thresholdValues).size === 1) {
    const signerWeight = positiveSigners[0].weight;
    const effectiveThreshold = Math.max(1, thresholdValues[0]);
    const requiredSigners = Math.ceil(effectiveThreshold / signerWeight);
    if (requiredSigners > 1 && requiredSigners <= positiveSigners.length) {
      return 'strict';
    }
  }

  return 'weighted';
}

export function canonicalizeAccount(response, controller) {
  if (!isObject(response)) {
    throw new Error(`${controller.accountId}: Horizon response must be an object`);
  }
  if (response.account_id !== controller.accountId) {
    throw new Error(`${controller.accountId}: Horizon returned a different account_id`);
  }
  if (!isObject(response.thresholds)) {
    throw new Error(`${controller.accountId}: Horizon response has no thresholds`);
  }
  if (!Array.isArray(response.signers) || response.signers.length === 0) {
    throw new Error(`${controller.accountId}: Horizon response has no complete signer set`);
  }

  const thresholds = {
    low: byteInteger(response.thresholds.low_threshold, 'thresholds.low_threshold'),
    medium: byteInteger(response.thresholds.med_threshold, 'thresholds.med_threshold'),
    high: byteInteger(response.thresholds.high_threshold, 'thresholds.high_threshold')
  };
  const signerIdentities = new Set();
  const signers = response.signers.map((signer, index) => {
    if (!isObject(signer)) {
      throw new Error(`${controller.accountId}: signers[${index}] must be an object`);
    }
    const key = nonEmptyString(signer.key, `signers[${index}].key`);
    const type = nonEmptyString(signer.type, `signers[${index}].type`);
    const weight = byteInteger(signer.weight, `signers[${index}].weight`);
    if (type === 'ed25519_public_key') {
      validatePublicKey(key);
    } else if (!/^[0-9a-f]{64}$/i.test(key)) {
      throw new Error(`${controller.accountId}: unsupported signer key encoding for ${type}`);
    }
    const identity = `${type}\u0000${key}`;
    if (signerIdentities.has(identity)) {
      throw new Error(`${controller.accountId}: duplicate signer ${type}:${key}`);
    }
    signerIdentities.add(identity);
    return { key, type, weight };
  });
  signers.sort(
    (left, right) => left.type.localeCompare(right.type) || left.key.localeCompare(right.key)
  );

  const masterSigner = signers.find(
    (signer) => signer.type === 'ed25519_public_key' && signer.key === controller.accountId
  );
  if (!masterSigner) {
    throw new Error(`${controller.accountId}: complete signer set is missing the master key`);
  }

  const policy = {
    thresholds,
    masterWeight: masterSigner.weight,
    signers,
    classification: classifyPolicy({ thresholds, signers })
  };

  return {
    accountId: controller.accountId,
    protocol: controller.protocol,
    role: controller.role,
    sources: controller.sources,
    lastModifiedLedger: unsignedInteger(
      response.last_modified_ledger,
      `${controller.accountId}.last_modified_ledger`
    ),
    policy
  };
}

export async function runCycle({
  registry,
  horizonUrl = DEFAULT_HORIZON_URL,
  outputDir,
  timeoutMs = DEFAULT_TIMEOUT_MS,
  maxAlerts = DEFAULT_MAX_ALERTS,
  fetchImpl = globalThis.fetch,
  now = () => new Date()
}) {
  const validatedRegistry = validateRegistry(registry);
  if (typeof fetchImpl !== 'function') {
    throw new Error('fetch is unavailable');
  }
  const resolvedOutputDir = resolve(nonEmptyString(outputDir, 'outputDir'));
  const normalizedHorizonUrl = normalizeHorizonUrl(horizonUrl);
  positiveInteger(timeoutMs, 'timeoutMs');
  positiveInteger(maxAlerts, 'maxAlerts');

  await mkdir(resolvedOutputDir, { recursive: true });
  const currentPath = join(resolvedOutputDir, CURRENT_FILENAME);
  const alertsPath = join(resolvedOutputDir, ALERTS_FILENAME);
  const previousSnapshot = await readJsonIfPresent(currentPath);
  const priorAlertsDocument = await readJsonIfPresent(alertsPath);
  const priorAlerts = parsePriorAlerts(priorAlertsDocument);
  const retainedPriorAlerts = priorAlerts.filter(shouldRetainAlert);
  const alertsRemoved = priorAlerts.length - retainedPriorAlerts.length;
  const previousAccounts = previousSnapshot === null ? new Map() : parsePreviousSnapshot(previousSnapshot);

  const results = await Promise.allSettled(
    validatedRegistry.controllers.map(async (controller) => {
      const previous = previousAccounts.get(controller.accountId);
      const [response, activity] = await Promise.all([
        fetchHorizonAccount({
          controller,
          horizonUrl: normalizedHorizonUrl,
          timeoutMs,
          fetchImpl
        }),
        fetchControllerOperations({
          controller,
          cursor: typeof previous?.operationCursor === 'string' ? previous.operationCursor : null,
          horizonUrl: normalizedHorizonUrl,
          timeoutMs,
          fetchImpl
        })
      ]);
      const account = canonicalizeAccount(response, controller);
      if (!previous && account.policy.classification !== controller.expectedClassification) {
        throw new Error(
          `expected ${controller.expectedClassification} policy, observed ${account.policy.classification}`
        );
      }
      return {
        account: { ...account, operationCursor: activity.cursor },
        operations: activity.operations
      };
    })
  );

  const failures = results
    .map((result, index) =>
      result.status === 'rejected'
        ? {
            accountId: validatedRegistry.controllers[index].accountId,
            error: errorMessage(result.reason)
          }
        : null
    )
    .filter(Boolean);
  if (failures.length > 0) {
    return { status: 'failed', checked: results.length, failures };
  }

  const observedAt = isoTimestamp(now());
  const accounts = results
    .map((result) => result.value.account)
    .sort((left, right) => left.accountId.localeCompare(right.accountId));
  const nextSnapshot = {
    schemaVersion: 1,
    network: 'public',
    observedAt,
    registry: {
      observedAt: validatedRegistry.observedAt,
      observationLedger: validatedRegistry.observationLedger
    },
    accounts
  };

  if (previousSnapshot === null) {
    await writeAtomicJson(alertsPath, {
      schemaVersion: 1,
      alerts: retainedPriorAlerts.slice(0, maxAlerts)
    });
    await writeAtomicJson(currentPath, nextSnapshot);
    return alertsRemoved > 0
      ? { status: 'baseline', checked: accounts.length, alertsCreated: 0, alertsRemoved }
      : { status: 'baseline', checked: accounts.length, alertsCreated: 0 };
  }

  const newAlerts = [];
  for (const result of results) {
    const account = result.value.account;
    const previous = previousAccounts.get(account.accountId);
    if (!previous) {
      continue;
    }
    if (stableJson(previous.policy) !== stableJson(account.policy)) {
      newAlerts.push(
        createPolicyAlert({ previous, current: account, previousSnapshot, detectedAt: observedAt })
      );
    }
    newAlerts.push(
      ...createActivityAlerts({
        account,
        previousPolicy: previous.policy,
        operations: result.value.operations,
        detectedAt: observedAt
      })
    );
  }

  const retainedNewAlerts = newAlerts.filter(shouldRetainAlert);

  if (retainedNewAlerts.length > 0 || alertsRemoved > 0) {
    const byId = new Map();
    for (const alert of [...retainedNewAlerts, ...retainedPriorAlerts]) {
      if (!byId.has(alert.id)) {
        byId.set(alert.id, alert);
      }
    }
    await writeAtomicJson(alertsPath, {
      schemaVersion: 1,
      alerts: [...byId.values()].slice(0, maxAlerts)
    });
  } else if (priorAlertsDocument === null) {
    await writeAtomicJson(alertsPath, { schemaVersion: 1, alerts: [] });
  }
  await writeAtomicJson(currentPath, nextSnapshot);

  return {
    status: retainedNewAlerts.length > 0 || alertsRemoved > 0 ? 'changed' : 'unchanged',
    checked: accounts.length,
    alertsCreated: retainedNewAlerts.length,
    ...(alertsRemoved > 0 ? { alertsRemoved } : {})
  };
}

async function fetchHorizonAccount({ controller, horizonUrl, timeoutMs, fetchImpl }) {
  return fetchHorizonJson({
    url: `${horizonUrl}/accounts/${encodeURIComponent(controller.accountId)}`,
    timeoutMs,
    fetchImpl
  });
}

async function fetchControllerOperations({
  controller,
  cursor,
  horizonUrl,
  timeoutMs,
  fetchImpl
}) {
  const accountPath = `${horizonUrl}/accounts/${encodeURIComponent(controller.accountId)}/operations`;
  if (cursor === null) {
    const response = await fetchHorizonJson({
      url: `${accountPath}?include_failed=false&order=desc&limit=1`,
      timeoutMs,
      fetchImpl
    });
    const records = operationRecords(response, controller.accountId);
    return {
      cursor: records[0]?.pagingToken ?? '0',
      operations: []
    };
  }

  const operations = [];
  let nextCursor = decimalCursor(cursor, `${controller.accountId}.operationCursor`);
  for (let page = 0; page < MAX_OPERATION_PAGES_PER_CYCLE; page += 1) {
    const response = await fetchHorizonJson({
      url: `${accountPath}?include_failed=false&order=asc&limit=${HORIZON_OPERATION_PAGE_SIZE}&cursor=${encodeURIComponent(nextCursor)}`,
      timeoutMs,
      fetchImpl
    });
    const records = operationRecords(response, controller.accountId);
    for (const record of records) {
      if (BigInt(record.pagingToken) <= BigInt(nextCursor)) {
        throw new Error(`${controller.accountId}: Horizon operation cursors are not increasing`);
      }
      nextCursor = record.pagingToken;
      if (record.sourceAccount === controller.accountId) {
        operations.push(record.operation);
      }
    }
    if (records.length < HORIZON_OPERATION_PAGE_SIZE) {
      return { cursor: nextCursor, operations };
    }
  }

  throw new Error(
    `${controller.accountId}: more than ${HORIZON_OPERATION_PAGE_SIZE * MAX_OPERATION_PAGES_PER_CYCLE} operations are waiting; checkpoint not advanced`
  );
}

async function fetchHorizonJson({ url, timeoutMs, fetchImpl }) {
  const abortController = new AbortController();
  const timeout = setTimeout(() => abortController.abort(), timeoutMs);
  try {
    const response = await fetchImpl(url, {
      headers: { accept: 'application/json' },
      signal: abortController.signal
    });
    if (!response || typeof response.ok !== 'boolean' || typeof response.json !== 'function') {
      throw new Error('fetch returned an invalid response');
    }
    if (!response.ok) {
      throw new Error(`Horizon returned HTTP ${response.status}`);
    }
    return await response.json();
  } finally {
    clearTimeout(timeout);
  }
}

function operationRecords(response, accountId) {
  if (!isObject(response) || !isObject(response._embedded) || !Array.isArray(response._embedded.records)) {
    throw new Error(`${accountId}: Horizon operations response is malformed`);
  }
  if (response._embedded.records.length > HORIZON_OPERATION_PAGE_SIZE) {
    throw new Error(`${accountId}: Horizon returned too many operation records`);
  }
  return response._embedded.records.map((record, index) =>
    canonicalizeOperation(record, accountId, index)
  );
}

function canonicalizeOperation(record, accountId, index) {
  const field = `${accountId}.operations[${index}]`;
  if (!isObject(record)) {
    throw new Error(`${field} must be an object`);
  }
  if (record.transaction_successful !== true) {
    throw new Error(`${field} is not a successful operation`);
  }
  const operationId = decimalCursor(record.id, `${field}.id`);
  const pagingToken = decimalCursor(record.paging_token, `${field}.paging_token`);
  if (operationId !== pagingToken) {
    throw new Error(`${field} id and paging token differ`);
  }
  const sourceAccount = validatePublicKey(record.source_account);
  const transactionHash = lowercaseDigest(record.transaction_hash, `${field}.transaction_hash`);
  const type = boundedString(record.type, `${field}.type`, 128);
  const typeCode = unsignedInteger(record.type_i, `${field}.type_i`);
  const createdAt = isoTimestamp(new Date(nonEmptyString(record.created_at, `${field}.created_at`)));
  const payload = {};
  for (const key of Object.keys(record).sort()) {
    if (OPERATION_METADATA_FIELDS.has(key)) continue;
    if (key.length > 128) throw new Error(`${field} contains an oversized payload key`);
    payload[key] = sanitizeJson(record[key], `${field}.${key}`, 0);
  }
  if (Buffer.byteLength(JSON.stringify(payload), 'utf8') > MAX_OPERATION_PAYLOAD_BYTES) {
    throw new Error(`${field} payload exceeds ${MAX_OPERATION_PAYLOAD_BYTES} bytes`);
  }
  return {
    pagingToken,
    sourceAccount,
    operation: {
      id: operationId,
      type,
      typeCode,
      sourceAccount,
      transactionHash,
      occurredAt: createdAt,
      payload
    }
  };
}

function sanitizeJson(value, field, depth) {
  if (depth > MAX_OPERATION_PAYLOAD_DEPTH) {
    throw new Error(`${field} exceeds the maximum payload depth`);
  }
  if (value === null || typeof value === 'boolean') return value;
  if (typeof value === 'string') return boundedString(value, field, 8_192, true);
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new Error(`${field} must be a finite number`);
    return value;
  }
  if (Array.isArray(value)) {
    if (value.length > 256) throw new Error(`${field} contains too many array values`);
    return value.map((item, index) => sanitizeJson(item, `${field}[${index}]`, depth + 1));
  }
  if (isObject(value)) {
    const keys = Object.keys(value).sort();
    if (keys.length > 256) throw new Error(`${field} contains too many object fields`);
    const result = {};
    for (const key of keys) {
      if (key.length === 0 || key.length > 128) {
        throw new Error(`${field} contains an invalid object key`);
      }
      result[key] = sanitizeJson(value[key], `${field}.${key}`, depth + 1);
    }
    return result;
  }
  throw new Error(`${field} contains a non-JSON value`);
}

function createPolicyAlert({ previous, current, previousSnapshot, detectedAt }) {
  const previousObservedAt = nonEmptyString(previousSnapshot.observedAt, 'snapshot.observedAt');
  const idInput = [
    previousObservedAt,
    current.accountId,
    stableJson(previous.policy),
    stableJson(current.policy)
  ].join('\n');
  return {
    id: createHash('sha256').update(idInput).digest('hex'),
    type: 'classic-controller-policy-changed',
    severity: 'critical',
    detectedAt,
    accountId: current.accountId,
    protocol: current.protocol,
    role: current.role,
    beforeObservedAt: previousObservedAt,
    afterLastModifiedLedger: current.lastModifiedLedger,
    before: previous.policy,
    after: current.policy
  };
}

function createActivityAlerts({ account, previousPolicy, operations, detectedAt }) {
  if (previousPolicy.classification !== 'strict' && previousPolicy.classification !== 'weighted') {
    return [];
  }
  const byTransaction = new Map();
  for (const operation of operations) {
    const group = byTransaction.get(operation.transactionHash) ?? [];
    group.push(operation);
    byTransaction.set(operation.transactionHash, group);
  }
  return [...byTransaction.entries()].map(([transactionHash, transactionOperations]) => {
    transactionOperations.sort((left, right) => BigInt(left.id) < BigInt(right.id) ? -1 : 1);
    const idInput = [
      'classic-controller-transaction-executed',
      account.accountId,
      transactionHash
    ].join('\n');
    return {
      id: createHash('sha256').update(idInput).digest('hex'),
      type: 'classic-controller-transaction-executed',
      severity: 'high',
      detectedAt,
      occurredAt: transactionOperations[0].occurredAt,
      accountId: account.accountId,
      protocol: account.protocol,
      role: account.role,
      transactionHash,
      operations: transactionOperations.map(({ transactionHash: _transactionHash, occurredAt: _occurredAt, ...operation }) => operation)
    };
  });
}

function shouldRetainAlert(alert) {
  if (!isObject(alert) || alert.type !== 'classic-controller-transaction-executed') {
    return true;
  }
  return !ROUTINE_ACTIVITY_RULES.some((rule) =>
    alert.protocol === rule.protocol
      && rule.roles.has(alert.role)
      && Array.isArray(alert.operations)
      && alert.operations.length > 0
      && alert.operations.every((operation) =>
        matchesRoutineActivity(operation, rule, alert.accountId)
      )
  );
}

function matchesRoutineActivity(operation, rule, accountId) {
  if (!isObject(operation)
    || operation.type !== rule.operationType
    || operation.typeCode !== 24
    || operation.sourceAccount !== accountId
    || !isObject(operation.payload)
    || operation.payload.function !== 'HostFunctionTypeHostFunctionTypeInvokeContract'
    || !Array.isArray(operation.payload.parameters)
    || operation.payload.parameters.length < 2) {
    return false;
  }
  const functionParameter = operation.payload.parameters[1];
  return isObject(functionParameter)
    && functionParameter.type === 'Sym'
    && decodeScSymbol(functionParameter.value) === rule.functionSymbol;
}

function decodeScSymbol(value) {
  if (typeof value !== 'string' || value.length === 0 || value.length > 128) {
    return null;
  }
  let bytes;
  try {
    bytes = Buffer.from(value, 'base64');
  } catch {
    return null;
  }
  if (bytes.toString('base64') !== value || bytes.length < 8) {
    return null;
  }
  if (bytes.readUInt32BE(0) !== SCV_SYMBOL_DISCRIMINANT) {
    return null;
  }
  const length = bytes.readUInt32BE(4);
  if (length === 0 || length > MAX_SC_SYMBOL_BYTES) {
    return null;
  }
  const paddedLength = Math.ceil(length / 4) * 4;
  if (bytes.length !== 8 + paddedLength) {
    return null;
  }
  if (bytes.subarray(8 + length).some((byte) => byte !== 0)) {
    return null;
  }
  const symbolBytes = bytes.subarray(8, 8 + length);
  const symbol = symbolBytes.toString('utf8');
  return Buffer.from(symbol, 'utf8').equals(symbolBytes) ? symbol : null;
}

function parsePreviousSnapshot(value) {
  if (!isObject(value) || value.schemaVersion !== 1 || !Array.isArray(value.accounts)) {
    throw new Error('existing current.json is not a Stargazer governance snapshot');
  }
  const accounts = new Map();
  for (const account of value.accounts) {
    if (!isObject(account) || typeof account.accountId !== 'string' || !isObject(account.policy)) {
      throw new Error('existing current.json contains an invalid account');
    }
    accounts.set(account.accountId, account);
  }
  return accounts;
}

function parsePriorAlerts(value) {
  if (value === null) {
    return [];
  }
  if (!isObject(value) || value.schemaVersion !== 1 || !Array.isArray(value.alerts)) {
    throw new Error('existing alerts.json is not a Stargazer governance alert document');
  }
  return value.alerts;
}

async function readJsonIfPresent(path) {
  try {
    return JSON.parse(await readFile(path, 'utf8'));
  } catch (error) {
    if (error?.code === 'ENOENT') {
      return null;
    }
    throw error;
  }
}

async function writeAtomicJson(path, value) {
  await mkdir(dirname(path), { recursive: true });
  const temporaryPath = join(dirname(path), `.${path.split('/').at(-1)}.${process.pid}.${randomUUID()}`);
  let handle;
  try {
    handle = await open(temporaryPath, 'wx', 0o600);
    await handle.writeFile(`${JSON.stringify(value, null, 2)}\n`, 'utf8');
    await handle.sync();
    await handle.close();
    handle = undefined;
    await rename(temporaryPath, path);
  } catch (error) {
    await handle?.close().catch(() => {});
    await unlink(temporaryPath).catch(() => {});
    throw error;
  }
}

function decodeBase32(value) {
  const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
  let accumulator = 0;
  let bits = 0;
  const bytes = [];
  for (const character of value) {
    const digit = alphabet.indexOf(character);
    if (digit < 0) {
      throw new Error(`invalid base32 character: ${character}`);
    }
    accumulator = (accumulator << 5) | digit;
    bits += 5;
    if (bits >= 8) {
      bits -= 8;
      bytes.push((accumulator >>> bits) & 0xff);
      accumulator &= (1 << bits) - 1;
    }
  }
  if (bits > 0 && accumulator !== 0) {
    throw new Error('non-zero base32 padding bits');
  }
  return Uint8Array.from(bytes);
}

function crc16Xmodem(bytes) {
  let crc = 0;
  for (const byte of bytes) {
    crc ^= byte << 8;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = crc & 0x8000 ? ((crc << 1) ^ 0x1021) & 0xffff : (crc << 1) & 0xffff;
    }
  }
  return crc;
}

function stableJson(value) {
  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(',')}]`;
  }
  if (isObject(value)) {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`)
      .join(',')}}`;
  }
  return JSON.stringify(value);
}

function normalizeHorizonUrl(value) {
  const parsed = new URL(nonEmptyString(value, 'horizonUrl'));
  if (parsed.protocol !== 'https:' && parsed.protocol !== 'http:') {
    throw new Error('horizonUrl must use HTTP or HTTPS');
  }
  return parsed.toString().replace(/\/$/, '');
}

function isoTimestamp(value) {
  if (!(value instanceof Date) || Number.isNaN(value.valueOf())) {
    throw new Error('now() must return a valid Date');
  }
  return value.toISOString();
}

function nonEmptyString(value, field) {
  if (typeof value !== 'string' || value.trim() === '') {
    throw new Error(`${field} must be a non-empty string`);
  }
  return value;
}

function boundedString(value, field, maxLength, allowEmpty = false) {
  if (typeof value !== 'string' || (!allowEmpty && value.trim() === '') || value.length > maxLength) {
    throw new Error(`${field} must be a ${allowEmpty ? '' : 'non-empty '}string of at most ${maxLength} characters`);
  }
  return value;
}

function decimalCursor(value, field) {
  const cursor = boundedString(value, field, 64);
  if (!/^\d+$/.test(cursor)) {
    throw new Error(`${field} must be a decimal Horizon cursor`);
  }
  return cursor;
}

function lowercaseDigest(value, field) {
  const digest = boundedString(value, field, 64);
  if (!/^[0-9a-f]{64}$/.test(digest)) {
    throw new Error(`${field} must be a lowercase 32-byte digest`);
  }
  return digest;
}

function unsignedInteger(value, field) {
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${field} must be a non-negative safe integer`);
  }
  return value;
}

function positiveInteger(value, field) {
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new Error(`${field} must be a positive safe integer`);
  }
  return value;
}

function byteInteger(value, field) {
  unsignedInteger(value, field);
  if (value > 255) {
    throw new Error(`${field} must fit in one byte`);
  }
  return value;
}

function isObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function usage() {
  return `Usage: node watcher.mjs [options]\n\nOptions:\n  --once                 Run one complete cycle and exit\n  --interval-ms <ms>     Poll interval (default: ${DEFAULT_INTERVAL_MS})\n  --timeout-ms <ms>      Per-account request timeout (default: ${DEFAULT_TIMEOUT_MS})\n  --horizon-url <url>    Horizon base URL (default: ${DEFAULT_HORIZON_URL})\n  --output-dir <path>    Snapshot and alert directory\n  --registry <path>      Controller registry JSON\n  --max-alerts <count>   Maximum retained alerts (default: ${DEFAULT_MAX_ALERTS})\n  --help                 Show this help\n`;
}

function parseCli(argv) {
  const scriptDir = dirname(fileURLToPath(import.meta.url));
  const options = {
    once: false,
    intervalMs: envPositiveInteger('STARGAZER_GOVERNANCE_INTERVAL_MS', DEFAULT_INTERVAL_MS),
    timeoutMs: envPositiveInteger('STARGAZER_GOVERNANCE_TIMEOUT_MS', DEFAULT_TIMEOUT_MS),
    horizonUrl: process.env.STARGAZER_GOVERNANCE_HORIZON_URL ?? DEFAULT_HORIZON_URL,
    outputDir:
      process.env.STARGAZER_GOVERNANCE_OUTPUT_DIR ?? join(scriptDir, 'state'),
    registryPath:
      process.env.STARGAZER_GOVERNANCE_REGISTRY ?? join(scriptDir, 'controllers.json'),
    maxAlerts: envPositiveInteger('STARGAZER_GOVERNANCE_MAX_ALERTS', DEFAULT_MAX_ALERTS),
    help: false
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === '--once') {
      options.once = true;
    } else if (argument === '--help') {
      options.help = true;
    } else {
      const value = argv[index + 1];
      if (value === undefined) {
        throw new Error(`${argument} requires a value`);
      }
      index += 1;
      if (argument === '--interval-ms') options.intervalMs = cliPositiveInteger(value, argument);
      else if (argument === '--timeout-ms') options.timeoutMs = cliPositiveInteger(value, argument);
      else if (argument === '--horizon-url') options.horizonUrl = value;
      else if (argument === '--output-dir') options.outputDir = value;
      else if (argument === '--registry') options.registryPath = value;
      else if (argument === '--max-alerts') options.maxAlerts = cliPositiveInteger(value, argument);
      else throw new Error(`unknown option: ${argument}`);
    }
  }
  return options;
}

function cliPositiveInteger(value, option) {
  const parsed = Number(value);
  positiveInteger(parsed, option);
  return parsed;
}

function envPositiveInteger(name, fallback) {
  const value = process.env[name];
  return value === undefined ? fallback : cliPositiveInteger(value, name);
}

async function loadRegistry(path) {
  return JSON.parse(await readFile(resolve(path), 'utf8'));
}

function delay(ms) {
  return new Promise((resolvePromise) => setTimeout(resolvePromise, ms));
}

async function main() {
  const options = parseCli(process.argv.slice(2));
  if (options.help) {
    process.stdout.write(usage());
    return;
  }
  const registry = await loadRegistry(options.registryPath);
  const cycleOptions = {
    registry,
    horizonUrl: options.horizonUrl,
    outputDir: options.outputDir,
    timeoutMs: options.timeoutMs,
    maxAlerts: options.maxAlerts
  };

  do {
    const result = await runCycle(cycleOptions);
    process.stdout.write(`${JSON.stringify({ at: new Date().toISOString(), ...result })}\n`);
    if (options.once) {
      if (result.status === 'failed') process.exitCode = 1;
      break;
    }
    await delay(options.intervalMs);
  } while (true);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main().catch((error) => {
    process.stderr.write(`${errorMessage(error)}\n`);
    process.exitCode = 1;
  });
}
