export function watchDefiCommand() {
	return '/watch defi';
}

export function watchProtocolCommand(slug: string) {
	return `/watch protocol ${slug}`;
}

export function watchProgramCommand(programId: string) {
	return `/watch program ${programId}`;
}

export function watchMultisigCommand(multisig: string) {
	return `/watch multisig ${multisig}`;
}

export function joinCommandLines(commands: string[]) {
	return commands.join('\n');
}
