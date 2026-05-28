export type FireWatchPlanId = 'watch' | 'paid' | 'pro';

export type FireWatchPlan = {
	id: FireWatchPlanId;
	name: string;
	price: string;
	priceNote: string;
	description: string;
	features: string[];
	limits: string[];
	ctaLabel: string;
	ctaHref: string;
};

export type ProResponseStep = {
	title: string;
	description: string;
};

export const firewatchPlans: FireWatchPlan[] = [
	{
		id: 'watch',
		name: 'Free DeFi Alert',
		price: '$0',
		priceNote: 'shared feed',
		description: 'Free users can subscribe to the unfiltered FireWatch DeFi alert feed.',
		features: [
			'Shared Telegram bot',
			'Unfiltered /watch defi alert feed',
			'Curated DeFi risk signals',
			'Program upgrades and IDL drift inside the shared feed',
			'Public activity stream'
		],
		limits: [
			'No program filters',
			'No protocol-only filters',
			'No wallet or threshold rules',
			'No custom bot'
		],
		ctaLabel: 'Copy free command',
		ctaHref: '/#free-alert'
	},
	{
		id: 'paid',
		name: 'Custom Plan',
		price: 'From $750',
		priceNote: 'per month',
		description:
			'Paid users can save custom program, protocol, wallet, and alert filters through Telegram.',
		features: [
			'Custom program watch',
			'Protocol and program filters',
			'Wallet and threshold alert drafts',
			'Telegram-saved pending monitors',
			'Solana Pay link generated in bot',
			'Private setup and test alert',
			'Included for Ferno members',
			'Superteam promo eligible for 12 months free'
		],
		limits: ['Smart Account response not included', 'FireWatch review signal available as add-on'],
		ctaLabel: 'Build custom monitor',
		ctaHref: '/#custom-monitor'
	},
	{
		id: 'pro',
		name: 'Pro Response',
		price: 'On demand',
		priceNote: 'incident or retainer',
		description:
			'Company response for reviewed signals and authorized Smart Account action paths.',
		features: [
			'Reviewed signal access',
			'Extra alert and security coverage',
			'Issue-detection withdrawal runbook',
			'Team review before redeploy',
			'Supported-app redeploy policies',
			'Response channel for urgent events'
		],
		limits: ['Requires explicit authorization', 'Requires custody and policy review before activation'],
		ctaLabel: 'Review response flow',
		ctaHref: '/pricing#pro-response'
	}
];

export const proResponseSteps: ProResponseStep[] = [
	{
		title: 'Issue detected',
		description:
			'FireWatch raises a critical signal from program, IDL, multisig, dependency, or review-layer evidence.'
	},
	{
		title: 'Liquidity exits',
		description:
			'The Smart Account follows the preapproved policy and attempts the fastest supported withdrawal path.'
	},
	{
		title: 'Team review',
		description:
			'The FireWatch team checks the event context before the account is cleared for redeployment.'
	},
	{
		title: 'Funds redeploy',
		description:
			'When the issue is reviewed and cleared, the Smart Account can redeploy under the approved policy.'
	}
];

export function firewatchPlanById(id: FireWatchPlanId) {
	return firewatchPlans.find((plan) => plan.id === id) ?? firewatchPlans[0];
}

export function planPriceLine(plan: FireWatchPlan) {
	return plan.priceNote ? `${plan.price} ${plan.priceNote}` : plan.price;
}
