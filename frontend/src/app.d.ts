// See https://svelte.dev/docs/kit/types#app.d.ts
// for information about these interfaces
declare global {
	namespace App {
		// interface Error {}
		// interface Locals {}
		// interface PageData {}
		// interface PageState {}
		interface Platform {
			env: {
				STARGAZER_NAS_GATEWAY?: {
					fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response>;
				};
			};
		}
	}
}

export {};
