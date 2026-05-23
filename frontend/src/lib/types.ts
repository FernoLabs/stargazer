export interface ChainEvent {
	id: string;
	signature: string;
	instruction_path: string;
	kind: string;
	target_type: 'program' | 'multisig' | 'domain' | string;
	target_address: string;
	actor: string | null;
	slot: number;
	block_time: number | null;
	indexed_at: number;
	summary: string;
	tags: string[];
	payload: Record<string, unknown>;
	delivery_attempts: number;
	delivered_at: number | null;
	last_error: string | null;
	next_retry_at: number;
}

export interface EventsSummary {
	total_count: number;
	delivered_count: number;
	pending_delivery_count: number;
	first_indexed_slot: number | null;
	backfill_start_slot: number | null;
	last_processed_slot: number | null;
	processed_transaction_count: number;
	matched_transaction_count: number;
}

export interface EventScopeCounts {
	all: number;
	critical: number;
	defi: number;
	programs: number;
	idl: number;
	multisig: number;
}

export interface EventsResponse {
	items: ChainEvent[];
	summary: EventsSummary;
	total_count: number;
	scope_counts: EventScopeCounts;
	next_offset: number | null;
}

export type TelegramEventScope = 'program_updates' | 'multisig_activity' | 'defi_watch';

export interface TelegramSubscription {
	id: string;
	label: string;
	chat_id: string;
	thread_id: number | null;
	scope: TelegramEventScope;
	target_address: string | null;
	required_tags: string[];
	created_at: number;
}

export interface TelegramSubscriptionsResponse {
	items: TelegramSubscription[];
	telegram_enabled: boolean;
}

export interface TelegramBotProfile {
	id: number;
	first_name: string;
	username: string | null;
}

export interface LiveStatusResponse {
	summary: EventsSummary;
	telegram_enabled: boolean;
	telegram_bot: TelegramBotProfile | null;
	sampled_at_ms: number;
	estimated_slot_time_ms: number;
}

export interface MetricAlertRule {
	id: string;
	label: string;
	protocol_slug: string;
	protocol_name: string;
	instruction_name: string;
	metric_key: string;
	metric_label: string;
	subject_address: string | null;
	subject_kind: string | null;
	subject_label: string | null;
	min_value: number | null;
	max_value: number | null;
	created_at: number;
}

export interface MetricAlertPreset {
	protocol_slug: string;
	protocol_name: string;
	instruction_name: string;
	metric_key: string;
	metric_label: string;
	subject_address: string | null;
	subject_kind: string | null;
	subject_label: string | null;
	latest_value: number;
	suggested_min_value: number | null;
	suggested_max_value: number | null;
	observation_count: number;
	latest_slot: number;
}

export interface MetricAlertsResponse {
	presets: MetricAlertPreset[];
	rules: MetricAlertRule[];
}

export type ProtocolCoverage = 'address_only' | 'admin_signals' | 'deep_decode';

export interface ProgramRiskSummary {
	critical_total: number;
	reviewed_critical: number;
	unreviewed_critical: number;
	score: number;
	grade: string;
}

export interface ProtocolProgramSurface {
	label: string;
	program_id: string;
	idl_metadata_address: string;
	anchor_legacy_idl_address: string | null;
	risk_summary?: ProgramRiskSummary;
}

export interface AmountTriggerDescriptor {
	label: string;
	description: string;
}

export interface DefiProtocolDescriptor {
	slug: string;
	name: string;
	program_id: string;
	programs: ProtocolProgramSurface[];
	sol_domains: string[];
	coverage: ProtocolCoverage;
	catalog_instruction_count: number;
	admin_instruction_filters: string[];
	high_impact_instruction_filters: string[];
	amount_triggers: AmountTriggerDescriptor[];
	unknown_alerts: boolean;
}

export interface DefiProtocolsResponse {
	items: DefiProtocolDescriptor[];
}

export interface ProgramSignerStat {
	member: string;
	member_label: string | null;
	actions: number;
	last_slot: number;
	multisig_count: number;
}

export interface ProgramSignerStatsResponse {
	items: ProgramSignerStat[];
}

export interface ProgramByteRange {
	start: number;
	end: number;
}

export interface ProgramVersionRecord {
	id: string;
	event_id: string;
	signature: string;
	instruction_path: string;
	kind: string;
	program_address: string;
	loader: string;
	source_account: string;
	authority: string | null;
	slot: number;
	block_time: number | null;
	indexed_at: number;
	complete: boolean;
	chunk_count: number;
	program_len: number | null;
	program_sha256: string | null;
	missing_ranges: ProgramByteRange[];
	previous_version_id: string | null;
	previous_program_len: number | null;
	previous_program_sha256: string | null;
}

export interface ProgramVersionsResponse {
	items: ProgramVersionRecord[];
}

export interface ProgramIdlVersionRecord {
	id: string;
	event_id: string;
	signature: string;
	instruction_path: string;
	kind: string;
	origin: string;
	program_address: string;
	metadata_account: string;
	source_account: string;
	authority: string | null;
	slot: number;
	block_time: number | null;
	indexed_at: number;
	complete: boolean;
	chunk_count: number;
	idl_len: number | null;
	idl_sha256: string | null;
	encoding: string | null;
	compression: string | null;
	format: string | null;
	data_source: string | null;
	missing_ranges: ProgramByteRange[];
	previous_idl_version_id: string | null;
	previous_idl_len: number | null;
	previous_idl_sha256: string | null;
}

export interface ProgramIdlVersionsResponse {
	items: ProgramIdlVersionRecord[];
}

export interface ProgramDisassemblyRecord {
	version_id: string;
	program_address: string;
	program_sha256: string;
	format: string;
	instruction_count: number;
	entrypoint_instruction: number | null;
	asm_sha256: string;
	asm_text: string;
	created_at: number;
	updated_at: number;
}

export interface ProgramDisassemblyResponse {
	item: ProgramDisassemblyRecord;
}
