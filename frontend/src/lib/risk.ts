import type {
	DefiProtocolDescriptor,
	ProgramRiskSummary,
	ProtocolProgramSurface
} from '$lib/types';

export const EMPTY_PROGRAM_RISK_SUMMARY: ProgramRiskSummary = {
	critical_total: 0,
	reviewed_critical: 0,
	unreviewed_critical: 0,
	score: 100,
	grade: 'A'
};

export const UNSCORED_PROTOCOL_RISK_SUMMARY: ProgramRiskSummary = {
	critical_total: 0,
	reviewed_critical: 0,
	unreviewed_critical: 0,
	score: 0,
	grade: '-'
};

export function programRiskSummary(program: ProtocolProgramSurface | null | undefined) {
	return program?.risk_summary ?? EMPTY_PROGRAM_RISK_SUMMARY;
}

export function riskGradeFromScore(score: number) {
	if (score >= 90) return 'A';
	if (score >= 75) return 'B';
	if (score >= 60) return 'C';
	if (score >= 40) return 'D';
	return 'F';
}

export function projectRiskSummary(
	protocol: DefiProtocolDescriptor | null | undefined,
	scannedEventCount?: number | null
): ProgramRiskSummary {
	if (scannedEventCount === 0) {
		return UNSCORED_PROTOCOL_RISK_SUMMARY;
	}

	if (!protocol || protocol.programs.length === 0) {
		return UNSCORED_PROTOCOL_RISK_SUMMARY;
	}

	let scoreTotal = 0;
	let critical = 0;
	let unresolved = 0;
	let reviewed = 0;

	for (const program of protocol.programs) {
		const risk = programRiskSummary(program);
		scoreTotal += risk.score;
		critical += risk.critical_total;
		unresolved += risk.unreviewed_critical;
		reviewed += risk.reviewed_critical;
	}

	const score = Math.round(scoreTotal / protocol.programs.length);
	return {
		critical_total: critical,
		reviewed_critical: reviewed,
		unreviewed_critical: unresolved,
		score,
		grade: riskGradeFromScore(score)
	};
}

export function riskGradeTone(summary: ProgramRiskSummary) {
	const grade = summary.grade.toLowerCase();
	return ['a', 'b', 'c', 'd', 'f'].includes(grade) ? grade : 'unscored';
}

export function hasRiskGrade(summary: ProgramRiskSummary) {
	return ['A', 'B', 'C', 'D', 'F'].includes(summary.grade.toUpperCase());
}

export function riskGradeDisplay(summary: ProgramRiskSummary) {
	return hasRiskGrade(summary) ? summary.grade : 'Not scanned';
}

export function riskGradeLabel(summary: ProgramRiskSummary) {
	if (!hasRiskGrade(summary)) {
		return 'Not scanned';
	}
	if (summary.unreviewed_critical === 0 && summary.critical_total === 0) {
		return 'No critical proof';
	}
	if (summary.unreviewed_critical === 0) {
		return 'Critical proof reviewed';
	}
	if (summary.score >= 75) {
		return 'Limited unresolved proof';
	}
	if (summary.score >= 60) {
		return 'Needs review';
	}
	return 'High unresolved risk';
}

export function riskGradeDescription(summary: ProgramRiskSummary) {
	if (!hasRiskGrade(summary)) {
		return 'FireWatch has not scanned protocol activity for this project yet.';
	}
	if (summary.critical_total === 0) {
		return 'FireWatch has not attached critical evidence to this program yet.';
	}
	if (summary.unreviewed_critical === 0) {
		return 'Critical evidence exists, but the current dataset marks it reviewed.';
	}
	return `${summary.unreviewed_critical} unresolved critical signal${
		summary.unreviewed_critical === 1 ? '' : 's'
	} should be inspected in the proof trail.`;
}

export function programCriticalHref(programId: string) {
	const search = new URLSearchParams({
		target: programId,
		scope: 'critical'
	});

	return `/events?${search.toString()}`;
}
