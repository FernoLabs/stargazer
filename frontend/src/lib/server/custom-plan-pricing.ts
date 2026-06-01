import { env } from '$env/dynamic/private';

export const DEFAULT_CUSTOM_PLAN_MONTHLY_AMOUNT = '0.42';
export const DEFAULT_CUSTOM_PLAN_TOKEN = 'USDC';

export type CustomPlanPricing = {
	token: string;
	monthly: string;
	yearly: string;
};

function formatAmount(amount: number) {
	return Number.isFinite(amount)
		? amount.toFixed(9).replace(/\.?0+$/, '')
		: DEFAULT_CUSTOM_PLAN_MONTHLY_AMOUNT;
}

export function customPlanAmountForBilling(billing: string) {
	const monthlyAmount =
		env.SOLANA_PAY_CUSTOM_PLAN_MONTHLY_AMOUNT?.trim() ||
		env.SOLANA_PAY_CUSTOM_PLAN_AMOUNT?.trim() ||
		env.SOLANA_PAY_AMOUNT?.trim() ||
		DEFAULT_CUSTOM_PLAN_MONTHLY_AMOUNT;

	if (billing !== 'year') return monthlyAmount;

	return (
		env.SOLANA_PAY_CUSTOM_PLAN_YEARLY_AMOUNT?.trim() ||
		formatAmount(Number(monthlyAmount) * 11)
	);
}

export function customPlanToken() {
	return env.SOLANA_PAY_TOKEN?.trim() || DEFAULT_CUSTOM_PLAN_TOKEN;
}

export function customPlanPricing(): CustomPlanPricing {
	return {
		token: customPlanToken(),
		monthly: customPlanAmountForBilling('month'),
		yearly: customPlanAmountForBilling('year')
	};
}
