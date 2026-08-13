import type {
	StellarAccountAddress,
	StellarMuxedAddress,
	StellarWalletAddress
} from './types';

const BASE32_ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
const PUBLIC_KEY_ED25519_VERSION = 6 << 3;
const MUXED_ACCOUNT_ED25519_VERSION = 12 << 3;
const PUBLIC_KEY_PAYLOAD_LENGTH = 32;
const MUXED_ACCOUNT_PAYLOAD_LENGTH = 40;

export type StellarWalletKind = 'account' | 'muxed-account';

export type StellarWalletValidationErrorCode =
	| 'required'
	| 'invalid-characters'
	| 'invalid-encoding'
	| 'invalid-length'
	| 'invalid-version'
	| 'invalid-checksum';

export interface StellarWalletValidationError {
	readonly code: StellarWalletValidationErrorCode;
	readonly message: string;
}

export interface ParsedStellarAccount {
	readonly kind: 'account';
	readonly address: StellarAccountAddress;
	readonly accountAddress: StellarAccountAddress;
}

export interface ParsedStellarMuxedAccount {
	readonly kind: 'muxed-account';
	readonly address: StellarMuxedAddress;
	/** The underlying G-address used by the compact wallet index. */
	readonly accountAddress: StellarAccountAddress;
	/** Kept as a decimal string so every unsigned 64-bit value remains exact. */
	readonly muxedId: string;
}

export type ParsedStellarWallet = ParsedStellarAccount | ParsedStellarMuxedAccount;

export type StellarWalletParseResult =
	| { readonly ok: true; readonly value: ParsedStellarWallet }
	| { readonly ok: false; readonly error: StellarWalletValidationError };

/** Trims paste whitespace and canonicalizes StrKey input to uppercase. */
export function normalizeStellarWalletInput(input: string): string {
	return input.trim().toUpperCase();
}

/** Validates the complete StrKey payload, including its CRC16-XModem checksum. */
export function parseStellarWalletAddress(input: string): StellarWalletParseResult {
	const normalized = normalizeStellarWalletInput(input);
	if (normalized.length === 0) {
		return validationError('required', 'Enter a Stellar G- or M-address.');
	}
	if (!/^[A-Z2-7]+$/.test(normalized)) {
		return validationError(
			'invalid-characters',
			'Stellar addresses use only uppercase A–Z and digits 2–7.'
		);
	}

	const decoded = decodeBase32(normalized);
	if (!decoded) {
		return validationError('invalid-encoding', 'This is not a valid unpadded Stellar StrKey.');
	}
	if (decoded.length < 3) {
		return validationError('invalid-length', 'This Stellar address has an invalid length.');
	}

	const data = decoded.subarray(0, decoded.length - 2);
	const expectedChecksum = crc16Xmodem(data);
	const actualChecksum = decoded[decoded.length - 2] | (decoded[decoded.length - 1] << 8);
	if (actualChecksum !== expectedChecksum) {
		return validationError('invalid-checksum', 'This Stellar address checksum does not match.');
	}

	const version = data[0];
	const payload = data.subarray(1);
	if (version === PUBLIC_KEY_ED25519_VERSION) {
		if (payload.length !== PUBLIC_KEY_PAYLOAD_LENGTH) {
			return validationError('invalid-length', 'A Stellar G-address must contain a 32-byte key.');
		}
		const address = normalized as StellarAccountAddress;
		return { ok: true, value: { kind: 'account', address, accountAddress: address } };
	}

	if (version === MUXED_ACCOUNT_ED25519_VERSION) {
		if (payload.length !== MUXED_ACCOUNT_PAYLOAD_LENGTH) {
			return validationError(
				'invalid-length',
				'A Stellar M-address must contain a 32-byte key and an 8-byte ID.'
			);
		}
		const accountAddress = encodeStrKey(
			PUBLIC_KEY_ED25519_VERSION,
			payload.subarray(0, PUBLIC_KEY_PAYLOAD_LENGTH)
		) as StellarAccountAddress;
		const address = normalized as StellarMuxedAddress;
		return {
			ok: true,
			value: {
				kind: 'muxed-account',
				address,
				accountAddress,
				muxedId: bytesToUnsignedDecimal(payload.subarray(PUBLIC_KEY_PAYLOAD_LENGTH))
			}
		};
	}

	return validationError('invalid-version', 'Enter a Stellar wallet address beginning with G or M.');
}

export function isStellarWalletAddress(input: string): input is StellarWalletAddress {
	const result = parseStellarWalletAddress(input);
	return result.ok && result.value.address === input;
}

/**
 * Compares canonical StrKeys using Stellar's base32 digit order.
 *
 * JavaScript string ordering is incorrect here because ASCII places `2`–`7`
 * before letters, while Stellar base32 places those digits after `Z`.
 */
export function compareStellarStrKeyOrder(left: string, right: string): number {
	const length = Math.min(left.length, right.length);
	for (let index = 0; index < length; index += 1) {
		const leftDigit = BASE32_ALPHABET.indexOf(left[index] ?? '');
		const rightDigit = BASE32_ALPHABET.indexOf(right[index] ?? '');
		if (leftDigit < 0 || rightDigit < 0) {
			throw new TypeError('Stellar StrKeys must use canonical base32 characters.');
		}
		if (leftDigit !== rightDigit) return leftDigit - rightDigit;
	}
	return left.length - right.length;
}

/** Returns a form-friendly error message, or null when the address is valid. */
export function getStellarWalletValidationMessage(input: string): string | null {
	const result = parseStellarWalletAddress(input);
	return result.ok ? null : result.error.message;
}

function validationError(
	code: StellarWalletValidationErrorCode,
	message: string
): StellarWalletParseResult {
	return { ok: false, error: { code, message } };
}

function decodeBase32(value: string): Uint8Array | null {
	const output: number[] = [];
	let accumulator = 0;
	let bitCount = 0;

	for (const character of value) {
		const digit = BASE32_ALPHABET.indexOf(character);
		if (digit < 0) return null;
		accumulator = (accumulator << 5) | digit;
		bitCount += 5;

		while (bitCount >= 8) {
			bitCount -= 8;
			output.push((accumulator >>> bitCount) & 0xff);
			accumulator &= (1 << bitCount) - 1;
		}
	}

	// Canonical unpadded base32 requires all unused trailing bits to be zero.
	if (accumulator !== 0) return null;
	return Uint8Array.from(output);
}

function encodeStrKey(version: number, payload: Uint8Array): string {
	const data = new Uint8Array(1 + payload.length + 2);
	data[0] = version;
	data.set(payload, 1);
	const checksum = crc16Xmodem(data.subarray(0, data.length - 2));
	data[data.length - 2] = checksum & 0xff;
	data[data.length - 1] = checksum >>> 8;
	return encodeBase32(data);
}

function encodeBase32(bytes: Uint8Array): string {
	let output = '';
	let accumulator = 0;
	let bitCount = 0;

	for (const byte of bytes) {
		accumulator = (accumulator << 8) | byte;
		bitCount += 8;
		while (bitCount >= 5) {
			bitCount -= 5;
			output += BASE32_ALPHABET[(accumulator >>> bitCount) & 0x1f];
			accumulator &= (1 << bitCount) - 1;
		}
	}

	if (bitCount > 0) output += BASE32_ALPHABET[(accumulator << (5 - bitCount)) & 0x1f];
	return output;
}

function crc16Xmodem(bytes: Uint8Array): number {
	let crc = 0;
	for (const byte of bytes) {
		crc ^= byte << 8;
		for (let bit = 0; bit < 8; bit += 1) {
			crc = crc & 0x8000 ? (crc << 1) ^ 0x1021 : crc << 1;
			crc &= 0xffff;
		}
	}
	return crc;
}

function bytesToUnsignedDecimal(bytes: Uint8Array): string {
	let value = 0n;
	for (const byte of bytes) value = (value << 8n) | BigInt(byte);
	return value.toString(10);
}
