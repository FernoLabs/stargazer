import type { ProgramRiskSummary, ProtocolProgramSurface } from '$lib/types';

export const EMPTY_PROGRAM_RISK_SUMMARY: ProgramRiskSummary = {
	critical_total: 0,
	reviewed_critical: 0,
	unreviewed_critical: 0,
	score: 100,
	grade: 'A'
};

export function programRiskSummary(program: ProtocolProgramSurface | null | undefined) {
	return program?.risk_summary ?? EMPTY_PROGRAM_RISK_SUMMARY;
}

export function riskGradeTone(summary: ProgramRiskSummary) {
	const grade = summary.grade.toLowerCase();
	return ['a', 'b', 'c', 'd', 'f'].includes(grade) ? grade : 'a';
}

export function programCriticalHref(programId: string) {
	const search = new URLSearchParams({
		target: programId,
		scope: 'critical'
	});

	return `/events?${search.toString()}`;
}
