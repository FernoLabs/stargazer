export type FireWatchPlanId = 'watch' | 'paid' | 'pro';

export type FireWatchPlan = {
	id: FireWatchPlanId;
	name: string;
	price?: string;
	priceNote?: string;
	description: string;
	features: string[];
	limits: string[];
	ctaLabel: string;
	ctaHref: string;
};

export const firewatchPlans: FireWatchPlan[] = [
	{
		id: 'watch',
		name: 'Public DeFi watcher',
		price: '$0',
		priceNote: 'default',
		description: 'Ferno-maintained protocol coverage for Telegram groups.',
		features: [
			'Shared /watch defi feed',
			'Public protocol pages maintained by Ferno',
			'Telegram setup, no payment required'
		],
		limits: ['No private protocol pages', 'No custom account limits or integrations'],
		ctaLabel: 'Copy free command',
		ctaHref: '/#get-started'
	},
	{
		id: 'paid',
		name: 'Custom watcher',
		price: '5 USDC',
		priceNote: 'per month',
		description:
			'Custom monitoring rules for teams that need more than the public DeFi feed.',
		features: [
			'Custom rules for protocols, programs, wallets, and .sol domains',
			'Auto protocol watch from wallet interactions',
			'Group setup through /config',
			'Monthly or yearly billing'
		],
		limits: ['Telegram delivery only', 'Uses Ferno-maintained public protocol pages'],
		ctaLabel: 'Build custom monitor',
		ctaHref: '/pricing#custom-plan'
	},
	{
		id: 'pro',
		name: 'Company Pro',
		description:
			'Private protocol pages for companies that need their own watch surface, account list, and alert routing.',
		features: [
			'Create and manage private protocol pages',
			'Track multisigs, wallets, programs, and internal account lists',
			'Base monitored-account allowance with paid add-ons',
			'Slack, Discord, email, and ticketing integrations by request'
		],
		limits: [
			'Private pages are visible only to approved company users',
			'Extra monitored accounts are billed in blocks'
		],
		ctaLabel: 'Contact us',
		ctaHref: '/contact?plan=company-pro'
	}
];

export function firewatchPlanById(id: FireWatchPlanId) {
	return firewatchPlans.find((plan) => plan.id === id) ?? firewatchPlans[0];
}

export function planPriceLine(plan: FireWatchPlan) {
	if (!plan.price) return '';
	return plan.priceNote ? `${plan.price} ${plan.priceNote}` : plan.price;
}
