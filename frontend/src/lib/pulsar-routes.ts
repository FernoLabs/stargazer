export const PULSAR_APPLICATION_PATH = '/scf/application';

export function isPulsarRoute(pathname: string): boolean {
	return pathname === '/' || pathname === PULSAR_APPLICATION_PATH;
}
