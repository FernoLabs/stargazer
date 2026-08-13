import type {
	DashboardDatasetMetadata,
	Leaderboard,
	LeaderboardMetric,
	StellarWalletAddress,
	WalletDashboard
} from './types';

export interface LeaderboardQuery {
	readonly metric: LeaderboardMetric;
	readonly limit?: number;
}

/**
 * Read-only boundary between the dashboard UI and either demo fixtures or the
 * eventual compact-index API.
 */
export interface DashboardRepository {
	readonly metadata: DashboardDatasetMetadata;

	getWallet(address: StellarWalletAddress): Promise<WalletDashboard | null>;

	getLeaderboard(query: LeaderboardQuery): Promise<Leaderboard>;
}
