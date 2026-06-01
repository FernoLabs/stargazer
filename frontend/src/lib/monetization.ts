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
		name: 'Free DeFi watcher',
		price: '$0',
		priceNote: 'default',
		description: 'Default shared /watch defi coverage for Telegram groups.',
		features: [
			'Shared Telegram feed with /watch defi',
			'Default DeFi protocol safety alerts',
			'No setup or payment required'
		],
		limits: ['No custom filters or wallet tracking', 'Shared feed only'],
		ctaLabel: 'Copy free command',
		ctaHref: '/#get-started'
	},
	{
		id: 'paid',
		name: 'Paid custom auto-tracked watcher',
		price: '0.42 USDC',
		priceNote: 'per month',
		description:
			'Custom watcher rules plus automatic protocol subscription when tracked wallets touch supported protocols.',
		features: [
			'Custom rules for protocols, programs, wallets, and .sol domains',
			'Auto protocol watch from wallet interactions',
			'Group setup through /config',
			'Monthly or yearly billing'
		],
		limits: ['Alerting only (no fund movement actions)'],
		ctaLabel: 'Build custom monitor',
		ctaHref: '/pricing#custom-plan'
	},
	{
		id: 'pro',
		name: 'On-demand Smart Wallet response',
		price: 'On demand',
		priceNote: 'incident or retainer',
		description:
			'We take emergency action on your funds through a safe on-chain non-custodial Smart Wallet policy.',
		features: [
			'Non-custodial on-chain policy: withdraw on critical alerts',
			'Redeploy only once the alert is over and safety is confirmed',
			'Dedicated infra: DoubleZero shred stream',
			'Dedicated infra: custom send + multi-tenant tx sender',
			'0-slot delay inclusion target'
		],
		limits: [
			'Requires explicit authorization to move funds',
			'Policy and custody review required before activation'
		],
		ctaLabel: 'Contact us',
		ctaHref: 'mailto:contact@ferno.ag?subject=FireWatch%20infra'
	}
];

export const proResponseSteps: ProResponseStep[] = [
	{
		title: 'Critical signal',
		description:
			'FireWatch raises a critical signal from program, IDL, multisig, dependency, or review-layer evidence.'
	},
	{
		title: 'Smart Wallet exits',
		description:
			'The on-chain Smart Wallet executes the preapproved emergency withdrawal policy.'
	},
	{
		title: 'Safety review',
		description:
			'The FireWatch team validates incident state before redeploy is allowed.'
	},
	{
		title: 'Funds redeploy',
		description:
			'When safety conditions are met, funds redeploy on-chain under the approved policy.'
	}
];

export function firewatchPlanById(id: FireWatchPlanId) {
	return firewatchPlans.find((plan) => plan.id === id) ?? firewatchPlans[0];
}

export function planPriceLine(plan: FireWatchPlan) {
	return plan.priceNote ? `${plan.price} ${plan.priceNote}` : plan.price;
}
