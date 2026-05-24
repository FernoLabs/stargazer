import {
	findProgramMatch,
	programPageHref,
	protocolDisplayName,
	protocolPageHref
} from '$lib/catalog';
import { readableInstructionName, shortAddress } from '$lib/format';
import type { ChainEvent, DefiProtocolDescriptor } from '$lib/types';

export type AlertLevel = 'critical' | 'high' | 'standard';

export function payloadString(event: ChainEvent, key: string) {
	const value = event.payload[key];
	return typeof value === 'string' && value.length > 0 ? value : null;
}

export function eventAlertLevel(event: ChainEvent): AlertLevel {
	if (event.tags.includes('severity:critical') || event.tags.includes('signal:unknown_instruction')) {
		return 'critical';
	}
	if (event.tags.includes('severity:high') || event.tags.includes('impact:high')) return 'high';
	return 'standard';
}

export function eventAlertLabel(event: ChainEvent) {
	const level = eventAlertLevel(event);
	if (level === 'critical') return 'Critical alert';
	if (level === 'high') return 'High alert';
	return 'Standard alert';
}

export function eventProtocolSlug(event: ChainEvent) {
	const tag = event.tags.find((candidate) => candidate.startsWith('protocol:'));
	return tag?.replace('protocol:', '') ?? null;
}

export function eventProtocolLabel(event: ChainEvent, protocols: DefiProtocolDescriptor[]) {
	const slug = eventProtocolSlug(event);
	if (!slug) return null;
	return protocolDisplayName(protocols, slug);
}

export function eventProtocolHref(event: ChainEvent) {
	const slug = eventProtocolSlug(event);
	return slug ? protocolPageHref(slug) : null;
}

export function eventTargetDisplayName(event: ChainEvent, protocols: DefiProtocolDescriptor[]) {
	const domainName = payloadString(event, 'domain_name') ?? payloadString(event, 'target_name');
	if (domainName) return domainName;

	const match = findProgramMatch(protocols, event.target_address);
	if (match) {
		const protocol = match.protocol.name;
		const program = match.program.label;
		if (protocol.toLowerCase().includes(program.toLowerCase())) return protocol;
		if (program.toLowerCase() === 'core') return protocol;
		return `${protocol} ${program}`;
	}

	const payloadLabel = payloadString(event, 'program_label');
	if (payloadLabel) return payloadLabel;

	return shortAddress(event.target_address);
}

export function eventKindLabel(event: ChainEvent) {
	switch (event.kind) {
		case 'name_service_domain_created':
			return 'Domain create';
		case 'name_service_domain_updated':
			return 'Domain update';
		case 'name_service_domain_transferred':
			return 'Domain transfer';
		case 'name_service_domain_deleted':
			return 'Domain delete';
		case 'name_service_domain_reallocated':
			return 'Domain resize';
		case 'program_unknown_instruction':
			return 'Unknown instruction';
		case 'program_admin_instruction':
			return 'Admin instruction';
		case 'program_upgraded':
			return 'Program upgrade';
		case 'program_deployed':
			return 'Program deploy';
		case 'program_finalized':
			return 'Program finalized';
		case 'program_migrated_to_loader_v4':
			return 'Loader migration';
		case 'program_metric_alert':
			return 'Metric anomaly';
		case 'program_idl_initialized':
		case 'program_idl_updated':
		case 'program_idl_authority_changed':
		case 'program_idl_immutable':
		case 'program_idl_resized':
		case 'program_idl_closed':
			return 'IDL change';
		case 'multisig_proposal_activated':
			return 'Squads proposal';
		case 'multisig_proposal_approved':
			return 'Squads approval';
		case 'multisig_proposal_rejected':
			return 'Squads rejection';
		case 'multisig_proposal_cancelled':
			return 'Squads cancellation';
		case 'multisig_vault_executed':
			return 'Squads execution';
		case 'multisig_config_executed':
			return 'Squads config';
		default:
			return event.kind.replaceAll('_', ' ');
	}
}

export function eventTitle(event: ChainEvent, protocols: DefiProtocolDescriptor[]) {
	const protocol = payloadString(event, 'protocol_name') ?? eventProtocolLabel(event, protocols);
	const instruction = payloadString(event, 'instruction_name');
	const targetName = eventTargetDisplayName(event, protocols);

	if (event.kind === 'name_service_domain_created') return `${targetName} created`;
	if (event.kind === 'name_service_domain_updated') return `${targetName} updated`;
	if (event.kind === 'name_service_domain_transferred') return `${targetName} ownership changed`;
	if (event.kind === 'name_service_domain_deleted') return `${targetName} deleted`;
	if (event.kind === 'name_service_domain_reallocated') return `${targetName} resized`;
	if (event.kind === 'program_unknown_instruction' && protocol) {
		return `${protocol} unknown instruction`;
	}
	if (event.kind === 'program_upgraded') return `${targetName} upgraded`;
	if (event.kind === 'program_deployed') return `${targetName} deployed`;
	if (event.kind === 'program_finalized') return `${targetName} finalized`;
	if (event.kind === 'program_migrated_to_loader_v4') return `${targetName} migrated to loader v4`;
	if (event.kind === 'program_admin_instruction' && protocol && instruction) {
		return `${protocol} ${readableInstructionName(instruction)}`;
	}
	if (event.kind === 'program_metric_alert' && protocol) return `${protocol} metric anomaly`;
	if (event.kind === 'program_idl_updated' && protocol) return `${protocol} IDL updated`;
	if (event.kind === 'program_idl_initialized' && protocol) return `${protocol} IDL initialized`;
	if (event.kind === 'program_idl_authority_changed' && protocol) {
		return `${protocol} IDL authority changed`;
	}
	if (event.kind === 'program_idl_immutable' && protocol) return `${protocol} IDL immutable`;
	if (event.kind === 'program_idl_resized' && protocol) return `${protocol} IDL resized`;
	if (event.kind === 'program_idl_closed' && protocol) return `${protocol} IDL closed`;
	if (event.kind === 'multisig_proposal_activated') return 'Squads proposal activated';
	if (event.kind === 'multisig_proposal_approved') return 'Squads proposal approval';
	if (event.kind === 'multisig_proposal_rejected') return 'Squads proposal rejected';
	if (event.kind === 'multisig_proposal_cancelled') return 'Squads proposal cancelled';
	if (event.kind === 'multisig_vault_executed') return 'Squads vault execution';
	if (event.kind === 'multisig_config_executed') return 'Squads config execution';
	return compactLongTokens(event.summary);
}

export function eventDetailHref(event: ChainEvent) {
	return `/events/${encodeURIComponent(event.id)}`;
}

export function eventPrimaryAction(event: ChainEvent) {
	if (event.target_type === 'program') {
		const program = payloadString(event, 'program') ?? event.target_address;
		const versionId = payloadString(event, 'program_version_id');
		if (versionId) {
			return {
				label: 'Code diff',
				href: `/programs/${program}/versions/${encodeURIComponent(versionId)}/compare`
			};
		}

		return {
			label: 'Program page',
			href: programPageHref(program)
		};
	}

	return null;
}

export function compactLongTokens(input: string) {
	return input
		.split(/\s+/)
		.map((token) => {
			const match = token.match(/^([^A-Za-z0-9]*)([A-Za-z0-9]{32,})([^A-Za-z0-9]*)$/);
			if (!match) return token;
			const [, prefix, core, suffix] = match;
			return `${prefix}${core.slice(0, 6)}...${core.slice(-6)}${suffix}`;
		})
		.join(' ');
}

export function eventPreviewTitle(event: ChainEvent, protocols: DefiProtocolDescriptor[]) {
	return `${eventAlertLabel(event)}: ${eventTitle(event, protocols)}`;
}

export function eventPreviewDescription(event: ChainEvent) {
	const slot = new Intl.NumberFormat('en-US').format(event.slot);
	const summary = compactLongTokens(event.summary);
	return `${eventKindLabel(event)} at slot ${slot}. ${summary}`;
}
