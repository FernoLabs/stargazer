use crate::{
    anchor_idl,
    models::{
        AmountTriggerDescriptor, DefiProtocolDescriptor, DefiProtocolProgramDescriptor,
        ProtocolCoverage,
    },
    program_metadata,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use solana_sdk::pubkey::Pubkey;
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::OnceLock,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolSignalKind {
    Admin,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct DefiProtocolMatch {
    pub slug: String,
    pub name: String,
    pub program_id: Pubkey,
    pub program_label: String,
}

#[derive(Clone, Debug)]
pub struct DomainProtocolMatch {
    pub slug: String,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct ProtocolInstructionSignal {
    pub protocol: DefiProtocolMatch,
    pub instruction_name: Option<String>,
    pub signal_kind: Option<ProtocolSignalKind>,
    pub high_impact: bool,
    pub discriminator_hex: String,
}

#[derive(Clone, Debug)]
pub struct ProtocolInstructionDescriptor {
    pub name: String,
    pub discriminator: [u8; 8],
}

#[derive(Clone, Copy)]
enum InstructionNameStyle {
    CamelCase,
    SnakeCase,
}

#[derive(Clone, Copy)]
struct InstructionSpec {
    name: &'static str,
    discriminator: &'static [u8],
}

#[derive(Clone, Copy)]
enum InstructionCatalogSpec {
    Standard(&'static [&'static str]),
    Explicit(&'static [InstructionSpec]),
}

#[derive(Clone, Copy)]
struct ProtocolSpec {
    slug: &'static str,
    name: &'static str,
    program_label: &'static str,
    program_id: &'static str,
    instruction_name_style: InstructionNameStyle,
    instruction_catalog: Option<InstructionCatalogSpec>,
    admin_instructions: &'static [&'static str],
    high_impact_instructions: &'static [&'static str],
    unknown_alerts: bool,
}

struct ProtocolDefinition {
    slug: String,
    name: String,
    program_label: String,
    program_id: Pubkey,
    instruction_name_style: InstructionNameStyle,
    instruction_catalog: Option<HashMap<Vec<u8>, &'static str>>,
    admin_instruction_lookup: HashMap<Vec<u8>, &'static str>,
    admin_instructions: &'static [&'static str],
    high_impact_instructions: &'static [&'static str],
    unknown_alerts: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct RegistrySnapshot {
    items: Vec<RegistryPlatform>,
}

#[derive(Clone, Debug, Deserialize)]
struct RegistryPlatform {
    slug: String,
    name: String,
    is_deprecated: bool,
    tags: Vec<String>,
    programs: Vec<RegistryProgram>,
}

#[derive(Clone, Debug, Deserialize)]
struct RegistryProgram {
    label: String,
    program_id: String,
}

pub const FERNO_PROTOCOL_SLUG: &str = "ferno";
pub const SQUADS_PROTOCOL_SLUG: &str = "squads";
pub const SQUADS_V4_PROGRAM_ID: &str = "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf";
pub const FERNO_SOL_DOMAINS: &[&str] = &[
    "ach-wallet.sol",
    "chibrator.sol",
    "odomart.sol",
    "ferno.sol",
    "limboman.sol",
    "blockzilla.sol",
];

const SQUADS_ADMIN_INSTRUCTIONS: &[&str] = &["config_transaction_execute"];
const SQUADS_HIGH_IMPACT_INSTRUCTIONS: &[&str] = SQUADS_ADMIN_INSTRUCTIONS;

const PUMPFUN_INSTRUCTIONS: &[&str] = &[
    "add_quote_mint",
    "admin_set_creator",
    "admin_set_idl_authority",
    "admin_update_token_incentives",
    "buy",
    "buy_exact_quote_in_v2",
    "buy_exact_sol_in",
    "buy_v2",
    "claim_cashback",
    "claim_cashback_v2",
    "claim_token_incentives",
    "close_user_volume_accumulator",
    "collect_creator_fee",
    "collect_creator_fee_v2",
    "create",
    "create_v2",
    "distribute_creator_fees",
    "distribute_creator_fees_v2",
    "extend_account",
    "get_minimum_distributable_fee",
    "init_user_volume_accumulator",
    "initialize",
    "migrate",
    "migrate_bonding_curve_creator",
    "migrate_v2",
    "remove_quote_mint",
    "sell",
    "sell_v2",
    "set_creator",
    "set_mayhem_virtual_params",
    "set_metaplex_creator",
    "set_params",
    "set_reserved_fee_recipients",
    "set_virtual_quote_reserves",
    "sync_user_volume_accumulator",
    "toggle_cashback_enabled",
    "toggle_create_v2",
    "toggle_mayhem_mode",
    "update_buyback_config",
    "update_global_authority",
];

impl ProtocolDefinition {
    fn coverage(&self) -> ProtocolCoverage {
        if self.instruction_catalog.is_some() {
            ProtocolCoverage::DeepDecode
        } else if !self.admin_instructions.is_empty() {
            ProtocolCoverage::AdminSignals
        } else {
            ProtocolCoverage::AddressOnly
        }
    }

    fn catalog_instruction_count(&self) -> usize {
        self.instruction_catalog
            .as_ref()
            .map(HashMap::len)
            .unwrap_or(0)
    }
}

const ORCA_WHIRLPOOLS_INSTRUCTIONS: &[&str] = &[
    "closeBundledPosition",
    "closePosition",
    "closePositionWithTokenExtensions",
    "collectFees",
    "collectFeesV2",
    "collectProtocolFees",
    "collectProtocolFeesV2",
    "collectReward",
    "collectRewardV2",
    "decreaseLiquidity",
    "decreaseLiquidityV2",
    "deletePositionBundle",
    "deleteTokenBadge",
    "idlInclude",
    "increaseLiquidity",
    "increaseLiquidityByTokenAmountsV2",
    "increaseLiquidityV2",
    "initializeAdaptiveFeeTier",
    "initializeConfig",
    "initializeConfigExtension",
    "initializeDynamicTickArray",
    "initializeFeeTier",
    "initializePool",
    "initializePoolV2",
    "initializePoolWithAdaptiveFee",
    "initializePositionBundle",
    "initializePositionBundleWithMetadata",
    "initializeReward",
    "initializeRewardV2",
    "initializeTickArray",
    "initializeTokenBadge",
    "lockPosition",
    "migrateRepurposeRewardAuthoritySpace",
    "openBundledPosition",
    "openPosition",
    "openPositionWithMetadata",
    "openPositionWithTokenExtensions",
    "repositionLiquidityV2",
    "resetPositionRange",
    "setAdaptiveFeeConstants",
    "setCollectProtocolFeesAuthority",
    "setConfigExtensionAuthority",
    "setConfigFeatureFlag",
    "setDefaultBaseFeeRate",
    "setDefaultFeeRate",
    "setDefaultProtocolFeeRate",
    "setDelegatedFeeAuthority",
    "setFeeAuthority",
    "setFeeRate",
    "setFeeRateByDelegatedFeeAuthority",
    "setInitializePoolAuthority",
    "setPresetAdaptiveFeeConstants",
    "setProtocolFeeRate",
    "setRewardAuthority",
    "setRewardAuthorityBySuperAuthority",
    "setRewardEmissions",
    "setRewardEmissionsSuperAuthority",
    "setRewardEmissionsV2",
    "setTokenBadgeAttribute",
    "setTokenBadgeAuthority",
    "swap",
    "swapV2",
    "transferLockedPosition",
    "twoHopSwap",
    "twoHopSwapV2",
    "updateFeesAndRewards",
];

const ORCA_ADMIN_INSTRUCTIONS: &[&str] = &[
    "initializeAdaptiveFeeTier",
    "initializeConfig",
    "initializeConfigExtension",
    "initializeFeeTier",
    "initializeReward",
    "initializeRewardV2",
    "initializeTokenBadge",
    "migrateRepurposeRewardAuthoritySpace",
    "setAdaptiveFeeConstants",
    "setCollectProtocolFeesAuthority",
    "setConfigExtensionAuthority",
    "setConfigFeatureFlag",
    "setDefaultBaseFeeRate",
    "setDefaultFeeRate",
    "setDefaultProtocolFeeRate",
    "setDelegatedFeeAuthority",
    "setFeeAuthority",
    "setFeeRate",
    "setFeeRateByDelegatedFeeAuthority",
    "setInitializePoolAuthority",
    "setPresetAdaptiveFeeConstants",
    "setProtocolFeeRate",
    "setRewardAuthority",
    "setRewardAuthorityBySuperAuthority",
    "setRewardEmissions",
    "setRewardEmissionsSuperAuthority",
    "setRewardEmissionsV2",
    "setTokenBadgeAttribute",
    "setTokenBadgeAuthority",
];

const ORCA_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "setCollectProtocolFeesAuthority",
    "setConfigExtensionAuthority",
    "setConfigFeatureFlag",
    "setDefaultProtocolFeeRate",
    "setFeeAuthority",
    "setInitializePoolAuthority",
    "setProtocolFeeRate",
    "setRewardAuthorityBySuperAuthority",
];

const KAMINO_LEND_INSTRUCTIONS: &[&str] = &[
    "initLendingMarket",
    "updateLendingMarket",
    "updateLendingMarketOwner",
    "initReserve",
    "initFarmsForReserve",
    "updateReserveConfig",
    "redeemFees",
    "withdrawProtocolFee",
    "seedDepositOnInitReserve",
    "topupReserveRewards",
    "socializeLoss",
    "socializeLossV2",
    "markObligationForDeleveraging",
    "refreshReserve",
    "refreshReservesBatch",
    "depositReserveLiquidity",
    "redeemReserveCollateral",
    "initObligation",
    "initObligationFarmsForReserve",
    "refreshObligationFarmsForReserve",
    "refreshObligation",
    "depositObligationCollateral",
    "depositObligationCollateralV2",
    "withdrawObligationCollateral",
    "withdrawObligationCollateralV2",
    "borrowObligationLiquidity",
    "borrowObligationLiquidityV2",
    "repayObligationLiquidity",
    "repayObligationLiquidityV2",
    "repayAndWithdrawAndRedeem",
    "depositAndWithdraw",
    "depositReserveLiquidityAndObligationCollateral",
    "depositReserveLiquidityAndObligationCollateralV2",
    "withdrawObligationCollateralAndRedeemReserveCollateral",
    "withdrawObligationCollateralAndRedeemReserveCollateralV2",
    "liquidateObligationAndRedeemReserveCollateral",
    "liquidateObligationAndRedeemReserveCollateralV2",
    "flashRepayReserveLiquidity",
    "flashBorrowReserveLiquidity",
    "requestElevationGroup",
    "initReferrerTokenState",
    "initUserMetadata",
    "withdrawReferrerFees",
    "initReferrerStateAndShortUrl",
    "deleteReferrerStateAndShortUrl",
    "setObligationOrder",
    "setBorrowOrder",
    "updateObligationConfig",
    "rolloverFixedTermBorrow",
    "fillBorrowOrder",
    "enqueueToWithdraw",
    "withdrawQueuedLiquidity",
    "recoverInvalidTicketCollateral",
    "initGlobalConfig",
    "updateGlobalConfig",
    "updateGlobalConfigAdmin",
    "idlMissingTypes",
];

const KAMINO_ADMIN_INSTRUCTIONS: &[&str] = &[
    "initLendingMarket",
    "updateLendingMarket",
    "updateLendingMarketOwner",
    "initReserve",
    "initFarmsForReserve",
    "updateReserveConfig",
    "redeemFees",
    "withdrawProtocolFee",
    "socializeLoss",
    "socializeLossV2",
    "markObligationForDeleveraging",
    "initGlobalConfig",
    "updateGlobalConfig",
    "updateGlobalConfigAdmin",
];

const KAMINO_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "updateLendingMarketOwner",
    "withdrawProtocolFee",
    "socializeLoss",
    "socializeLossV2",
    "markObligationForDeleveraging",
    "updateGlobalConfigAdmin",
];

const DRIFT_ADMIN_INSTRUCTIONS: &[&str] = &[
    "pauseSpotMarketDepositWithdraw",
    "settleRevenueToInsuranceFund",
    "transferProtocolIfSharesToRevenuePool",
    "adminWithdrawFromInsuranceFundVault",
    "moveAmmPrice",
    "recenterPerpMarketAmm",
    "recenterPerpMarketAmmCrank",
    "depositIntoPerpMarketFeePool",
    "depositIntoSpotMarketVault",
    "depositIntoSpotMarketRevenuePool",
    "repegAmmCurve",
    "updateAdmin",
    "updateExchangeStatus",
    "updateFeatureBitFlagsMmOracle",
    "zeroMmOracleFields",
    "updateFeatureBitFlagsMedianTriggerPrice",
    "updateFeatureBitFlagsBuilderCodes",
    "changeApprovedBuilder",
    "adminDeposit",
];

const DRIFT_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "pauseSpotMarketDepositWithdraw",
    "settleRevenueToInsuranceFund",
    "transferProtocolIfSharesToRevenuePool",
    "adminWithdrawFromInsuranceFundVault",
    "moveAmmPrice",
    "repegAmmCurve",
    "updateAdmin",
    "updateExchangeStatus",
    "zeroMmOracleFields",
];

const MARGINFI_ADMIN_INSTRUCTIONS: &[&str] = &[
    "marginfi_group_initialize",
    "marginfi_group_configure",
    "lending_pool_add_bank",
    "lending_pool_add_bank_with_seed",
    "lending_pool_clone_bank",
    "lending_pool_configure_bank",
    "lending_pool_configure_bank_interest_only",
    "lending_pool_configure_bank_limits_only",
    "lending_pool_force_tokenless_repay_complete",
    "lending_pool_configure_bank_oracle",
    "lending_pool_set_fixed_oracle_price",
    "lending_pool_configure_bank_emode",
    "lending_pool_clone_emode",
    "lending_pool_handle_bankruptcy",
    "super_admin_withdraw",
    "super_admin_deposit",
    "lending_pool_collect_bank_fees",
    "lending_pool_withdraw_fees",
    "lending_pool_update_fees_destination_account",
    "lending_pool_withdraw_insurance",
    "lending_pool_close_bank",
    "marginfi_account_set_freeze",
    "init_global_fee_state",
    "edit_global_fee_state",
    "config_group_fee",
    "init_staked_settings",
    "edit_staked_settings",
    "panic_pause",
    "panic_unpause",
    "panic_unpause_permissionless",
    "migrate_curve",
    "init_bank_metadata",
    "write_bank_metadata",
    "configure_deleverage_withdrawal_limit",
    "update_deleverage_withdrawals",
    "configure_bank_rate_limits",
    "configure_group_rate_limits",
    "update_group_rate_limiter",
    "purge_deleverage_balance",
    "lending_pool_add_bank_kamino",
    "lending_pool_add_bank_drift",
    "lending_pool_add_bank_solend",
    "lending_pool_add_bank_juplend",
];

const MARGINFI_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "lending_pool_force_tokenless_repay_complete",
    "lending_pool_handle_bankruptcy",
    "super_admin_withdraw",
    "panic_pause",
    "panic_unpause",
    "panic_unpause_permissionless",
    "purge_deleverage_balance",
];

const JUPITER_LENDING_INSTRUCTIONS: &[&str] = &[
    "deposit",
    "deposit_with_min_amount_out",
    "init_lending",
    "init_lending_admin",
    "mint",
    "mint_with_max_assets",
    "rebalance",
    "redeem",
    "redeem_with_min_amount_out",
    "set_rewards_rate_model",
    "update_authority",
    "update_auths",
    "update_rate",
    "update_rebalancer",
    "withdraw",
    "withdraw_with_max_shares_burn",
];

const JUPITER_LENDING_ADMIN_INSTRUCTIONS: &[&str] = &[
    "init_lending",
    "init_lending_admin",
    "set_rewards_rate_model",
    "update_authority",
    "update_auths",
    "update_rebalancer",
];

const JUPITER_LENDING_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "set_rewards_rate_model",
    "update_authority",
    "update_auths",
    "update_rebalancer",
];

const JUPITER_FLASHLOAN_INSTRUCTIONS: &[&str] = &[
    "activate_protocol",
    "flashloan_borrow",
    "flashloan_payback",
    "init_flashloan_admin",
    "pause_protocol",
    "set_flashloan_fee",
    "update_authority",
];

const JUPITER_FLASHLOAN_ADMIN_INSTRUCTIONS: &[&str] = &[
    "activate_protocol",
    "init_flashloan_admin",
    "pause_protocol",
    "set_flashloan_fee",
    "update_authority",
];

const JUPITER_FLASHLOAN_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "activate_protocol",
    "pause_protocol",
    "set_flashloan_fee",
    "update_authority",
];

const JUPITER_REWARDS_MODEL_INSTRUCTIONS: &[&str] = &[
    "cancel_queued_rewards",
    "init_lending_rewards_admin",
    "init_lending_rewards_rate_model",
    "queue_next_rewards",
    "start_rewards",
    "stop_rewards",
    "transition_to_next_rewards",
    "update_authority",
    "update_auths",
];

const JUPITER_REWARDS_MODEL_ADMIN_INSTRUCTIONS: &[&str] = &[
    "cancel_queued_rewards",
    "init_lending_rewards_admin",
    "init_lending_rewards_rate_model",
    "queue_next_rewards",
    "start_rewards",
    "stop_rewards",
    "transition_to_next_rewards",
    "update_authority",
    "update_auths",
];

const JUPITER_REWARDS_MODEL_HIGH_IMPACT_INSTRUCTIONS: &[&str] =
    &["update_authority", "update_auths"];

const JUPITER_LIQUIDITY_INSTRUCTIONS: &[&str] = &[
    "change_status",
    "claim",
    "close_claim_account",
    "collect_revenue",
    "init_claim_account",
    "init_liquidity",
    "init_new_protocol",
    "init_token_reserve",
    "operate",
    "pause_user",
    "pre_operate",
    "unpause_user",
    "update_authority",
    "update_auths",
    "update_exchange_price",
    "update_guardians",
    "update_rate_data_v1",
    "update_rate_data_v2",
    "update_revenue_collector",
    "update_token_config",
    "update_user_borrow_config",
    "update_user_class",
    "update_user_supply_config",
    "update_user_withdrawal_limit",
];

const JUPITER_LIQUIDITY_ADMIN_INSTRUCTIONS: &[&str] = &[
    "change_status",
    "collect_revenue",
    "init_liquidity",
    "init_new_protocol",
    "init_token_reserve",
    "pause_user",
    "unpause_user",
    "update_authority",
    "update_auths",
    "update_exchange_price",
    "update_guardians",
    "update_rate_data_v1",
    "update_rate_data_v2",
    "update_revenue_collector",
    "update_token_config",
    "update_user_borrow_config",
    "update_user_class",
    "update_user_supply_config",
    "update_user_withdrawal_limit",
];

const JUPITER_LIQUIDITY_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "change_status",
    "collect_revenue",
    "pause_user",
    "unpause_user",
    "update_authority",
    "update_auths",
    "update_exchange_price",
    "update_guardians",
    "update_rate_data_v2",
    "update_revenue_collector",
    "update_token_config",
    "update_user_borrow_config",
    "update_user_supply_config",
    "update_user_withdrawal_limit",
];

const JUPITER_ORACLE_INSTRUCTIONS: &[&str] = &[
    "chainlink_data_streams_feed_access_controller",
    "get_both_exchange_rate",
    "get_exchange_rate",
    "get_exchange_rate_liquidate",
    "get_exchange_rate_operate",
    "init_admin",
    "init_chainlink_data_streams_cache",
    "init_oracle_config",
    "refresh_price_feed_with_chainlink",
    "update_authority",
    "update_auths",
    "update_chainlink_data_streams_cache_feeds",
    "update_chainlink_data_streams_cache_keepers",
];

const JUPITER_ORACLE_ADMIN_INSTRUCTIONS: &[&str] = &[
    "init_admin",
    "init_chainlink_data_streams_cache",
    "init_oracle_config",
    "update_authority",
    "update_auths",
    "update_chainlink_data_streams_cache_feeds",
    "update_chainlink_data_streams_cache_keepers",
];

const JUPITER_ORACLE_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "update_authority",
    "update_auths",
    "update_chainlink_data_streams_cache_feeds",
    "update_chainlink_data_streams_cache_keepers",
];

const JUPITER_VAULTS_INSTRUCTIONS: &[&str] = &[
    "get_exchange_prices",
    "init_branch",
    "init_position",
    "init_tick",
    "init_tick_has_debt_array",
    "init_tick_id_liquidation",
    "init_vault_admin",
    "init_vault_config",
    "init_vault_state",
    "liquidate",
    "operate",
    "rebalance",
    "update_authority",
    "update_auths",
    "update_borrow_fee",
    "update_borrow_rate_magnifier",
    "update_collateral_factor",
    "update_core_settings",
    "update_exchange_prices",
    "update_liquidation_max_limit",
    "update_liquidation_penalty",
    "update_liquidation_threshold",
    "update_lookup_table",
    "update_oracle",
    "update_rebalancer",
    "update_supply_rate_magnifier",
    "update_withdraw_gap",
];

const JUPITER_VAULTS_ADMIN_INSTRUCTIONS: &[&str] = &[
    "init_branch",
    "init_vault_admin",
    "init_vault_config",
    "init_vault_state",
    "rebalance",
    "update_authority",
    "update_auths",
    "update_borrow_fee",
    "update_borrow_rate_magnifier",
    "update_collateral_factor",
    "update_core_settings",
    "update_exchange_prices",
    "update_liquidation_max_limit",
    "update_liquidation_penalty",
    "update_liquidation_threshold",
    "update_lookup_table",
    "update_oracle",
    "update_rebalancer",
    "update_supply_rate_magnifier",
    "update_withdraw_gap",
];

const JUPITER_VAULTS_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "update_authority",
    "update_auths",
    "update_borrow_fee",
    "update_borrow_rate_magnifier",
    "update_collateral_factor",
    "update_core_settings",
    "update_liquidation_max_limit",
    "update_liquidation_penalty",
    "update_liquidation_threshold",
    "update_lookup_table",
    "update_oracle",
    "update_rebalancer",
    "update_supply_rate_magnifier",
    "update_withdraw_gap",
];

const EXPONENT_CORE_INSTRUCTION_SPECS: &[InstructionSpec] = &[
    InstructionSpec {
        name: "add_emission",
        discriminator: &[18],
    },
    InstructionSpec {
        name: "add_farm",
        discriminator: &[22],
    },
    InstructionSpec {
        name: "add_lp_tokens_metadata",
        discriminator: &[41],
    },
    InstructionSpec {
        name: "add_market_emission",
        discriminator: &[25],
    },
    InstructionSpec {
        name: "buy_yt",
        discriminator: &[0],
    },
    InstructionSpec {
        name: "claim_farm_emissions",
        discriminator: &[24],
    },
    InstructionSpec {
        name: "collect_emission",
        discriminator: &[19],
    },
    InstructionSpec {
        name: "collect_interest",
        discriminator: &[6],
    },
    InstructionSpec {
        name: "collect_treasury_emission",
        discriminator: &[20],
    },
    InstructionSpec {
        name: "collect_treasury_interest",
        discriminator: &[21],
    },
    InstructionSpec {
        name: "deposit_yt",
        discriminator: &[7],
    },
    InstructionSpec {
        name: "init_lp_position",
        discriminator: &[13],
    },
    InstructionSpec {
        name: "init_market_two",
        discriminator: &[10],
    },
    InstructionSpec {
        name: "initialize_vault",
        discriminator: &[2],
    },
    InstructionSpec {
        name: "initialize_yield_position",
        discriminator: &[3],
    },
    InstructionSpec {
        name: "market_collect_emission",
        discriminator: &[16],
    },
    InstructionSpec {
        name: "market_deposit_lp",
        discriminator: &[14],
    },
    InstructionSpec {
        name: "market_two_deposit_liquidity",
        discriminator: &[11],
    },
    InstructionSpec {
        name: "market_two_withdraw_liquidity",
        discriminator: &[12],
    },
    InstructionSpec {
        name: "market_withdraw_lp",
        discriminator: &[15],
    },
    InstructionSpec {
        name: "merge",
        discriminator: &[5],
    },
    InstructionSpec {
        name: "modify_farm",
        discriminator: &[23],
    },
    InstructionSpec {
        name: "modify_market_setting",
        discriminator: &[27],
    },
    InstructionSpec {
        name: "modify_vault_setting",
        discriminator: &[26],
    },
    InstructionSpec {
        name: "realloc_market",
        discriminator: &[40],
    },
    InstructionSpec {
        name: "sell_yt",
        discriminator: &[1],
    },
    InstructionSpec {
        name: "stage_yt_yield",
        discriminator: &[9],
    },
    InstructionSpec {
        name: "strip",
        discriminator: &[4],
    },
    InstructionSpec {
        name: "trade_pt",
        discriminator: &[17],
    },
    InstructionSpec {
        name: "withdraw_yt",
        discriminator: &[8],
    },
    InstructionSpec {
        name: "wrapper_buy_pt",
        discriminator: &[29],
    },
    InstructionSpec {
        name: "wrapper_buy_yt",
        discriminator: &[31],
    },
    InstructionSpec {
        name: "wrapper_collect_interest",
        discriminator: &[33],
    },
    InstructionSpec {
        name: "wrapper_merge",
        discriminator: &[39],
    },
    InstructionSpec {
        name: "wrapper_provide_liquidity",
        discriminator: &[28],
    },
    InstructionSpec {
        name: "wrapper_provide_liquidity_base",
        discriminator: &[36],
    },
    InstructionSpec {
        name: "wrapper_provide_liquidity_classic",
        discriminator: &[37],
    },
    InstructionSpec {
        name: "wrapper_sell_pt",
        discriminator: &[30],
    },
    InstructionSpec {
        name: "wrapper_sell_yt",
        discriminator: &[32],
    },
    InstructionSpec {
        name: "wrapper_strip",
        discriminator: &[38],
    },
    InstructionSpec {
        name: "wrapper_withdraw_liquidity",
        discriminator: &[34],
    },
    InstructionSpec {
        name: "wrapper_withdraw_liquidity_classic",
        discriminator: &[35],
    },
];

const EXPONENT_CORE_ADMIN_INSTRUCTIONS: &[&str] = &[
    "add_emission",
    "add_farm",
    "add_lp_tokens_metadata",
    "add_market_emission",
    "collect_treasury_emission",
    "collect_treasury_interest",
    "init_market_two",
    "initialize_vault",
    "modify_farm",
    "modify_market_setting",
    "modify_vault_setting",
    "realloc_market",
];

const EXPONENT_CORE_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "add_emission",
    "add_market_emission",
    "collect_treasury_emission",
    "collect_treasury_interest",
    "initialize_vault",
    "modify_market_setting",
    "modify_vault_setting",
    "realloc_market",
];

const EXPONENT_ADMIN_INSTRUCTIONS: &[&str] = &[
    "initialize_admin",
    "invite_admin",
    "accept_invitation",
    "add_principle_admin",
    "remove_principle_admin",
    "realloc_admin",
];

const EXPONENT_ADMIN_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "invite_admin",
    "accept_invitation",
    "add_principle_admin",
    "remove_principle_admin",
    "realloc_admin",
];

const CARROT_INSTRUCTIONS: &[&str] = &[
    "initVault",
    "transferVaultAuthority",
    "pauseVault",
    "migrateVaultAccount",
    "distributeVaultFees",
    "updateFee",
    "issue",
    "redeem",
    "addAsset",
    "updateAsset",
    "removeAsset",
    "removeStrategy",
    "updateStrategy",
    "marginfiSupplyStrategyInit",
    "marginfiSupplyStrategyDeposit",
    "marginfiSupplyStrategyWithdraw",
    "marginfiSupplyStrategyClaimEmissions",
    "klendSupplyStrategyInit",
    "klendSupplyStrategyDeposit",
    "klendSupplyStrategyWithdraw",
    "klendSupplyStrategyClaimEmissions",
    "solendSupplyStrategyInit",
    "solendSupplyStrategyDeposit",
    "solendSupplyStrategyWithdraw",
    "mangoSupplyStrategyInit",
    "mangoSupplyStrategyDeposit",
    "mangoSupplyStrategyWithdraw",
    "driftSupplyStrategyInit",
    "driftSupplyStrategyDeposit",
    "driftSupplyStrategyWithdraw",
    "driftInsuranceFundStrategyInit",
    "driftInsuranceFundStrategyStake",
    "driftInsuranceFundStrategyUnstake",
    "driftInsuranceFundStrategyWithdraw",
    "clendSupplyStrategyInit",
    "clendSupplyStrategyDeposit",
    "clendSupplyStrategyWithdraw",
    "juplendSupplyStrategyInit",
    "juplendSupplyStrategyDeposit",
    "juplendSupplyStrategyWithdraw",
    "chestStrategyInit",
    "chestStrategyDeposit",
    "chestStrategyRequestWithdraw",
    "chestStrategyWithdraw",
    "updateSwitchboardOraclePrice",
];

const CARROT_ADMIN_INSTRUCTIONS: &[&str] = &[
    "initVault",
    "transferVaultAuthority",
    "pauseVault",
    "migrateVaultAccount",
    "distributeVaultFees",
    "updateFee",
    "addAsset",
    "updateAsset",
    "removeAsset",
    "removeStrategy",
    "updateStrategy",
    "marginfiSupplyStrategyInit",
    "klendSupplyStrategyInit",
    "solendSupplyStrategyInit",
    "mangoSupplyStrategyInit",
    "driftSupplyStrategyInit",
    "driftInsuranceFundStrategyInit",
    "clendSupplyStrategyInit",
    "juplendSupplyStrategyInit",
    "chestStrategyInit",
    "updateSwitchboardOraclePrice",
];

const CARROT_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "transferVaultAuthority",
    "pauseVault",
    "updateFee",
    "removeAsset",
    "removeStrategy",
    "updateStrategy",
    "updateSwitchboardOraclePrice",
];

const RAYDIUM_CLMM_INSTRUCTIONS: &[&str] = &[
    "close_position",
    "collect_fund_fee",
    "collect_protocol_fee",
    "collect_remaining_rewards",
    "create_amm_config",
    "create_operation_account",
    "create_pool",
    "create_support_mint_associated",
    "decrease_liquidity",
    "decrease_liquidity_v2",
    "increase_liquidity",
    "increase_liquidity_v2",
    "initialize_reward",
    "open_position",
    "open_position_v2",
    "open_position_with_token22_nft",
    "set_reward_params",
    "swap",
    "swap_router_base_in",
    "swap_v2",
    "transfer_reward_owner",
    "update_amm_config",
    "update_operation_account",
    "update_pool_status",
    "update_reward_infos",
];

const RAYDIUM_CLMM_ADMIN_INSTRUCTIONS: &[&str] = &[
    "collect_fund_fee",
    "collect_protocol_fee",
    "create_amm_config",
    "create_operation_account",
    "create_pool",
    "initialize_reward",
    "set_reward_params",
    "transfer_reward_owner",
    "update_amm_config",
    "update_operation_account",
    "update_pool_status",
    "update_reward_infos",
];

const RAYDIUM_CLMM_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "collect_protocol_fee",
    "create_amm_config",
    "set_reward_params",
    "transfer_reward_owner",
    "update_amm_config",
    "update_pool_status",
    "update_reward_infos",
];

const RAYDIUM_CPMM_INSTRUCTIONS: &[&str] = &[
    "collect_fund_fee",
    "collect_protocol_fee",
    "create_amm_config",
    "deposit",
    "initialize",
    "swap_base_input",
    "swap_base_output",
    "update_amm_config",
    "update_pool_status",
    "withdraw",
];

const RAYDIUM_CPMM_ADMIN_INSTRUCTIONS: &[&str] = &[
    "collect_fund_fee",
    "collect_protocol_fee",
    "create_amm_config",
    "initialize",
    "update_amm_config",
    "update_pool_status",
];

const RAYDIUM_CPMM_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "collect_protocol_fee",
    "create_amm_config",
    "update_amm_config",
    "update_pool_status",
];

const RAYDIUM_AMM_V4_INSTRUCTIONS: &[&str] = &[
    "initialize",
    "initialize2",
    "monitorStep",
    "deposit",
    "withdraw",
    "migrateToOpenBook",
    "setParams",
    "withdrawPnl",
    "withdrawSrm",
    "swapBaseIn",
    "preInitialize",
    "swapBaseOut",
    "simulateInfo",
    "adminCancelOrders",
    "createConfigAccount",
    "updateConfigAccount",
];

const RAYDIUM_AMM_V4_ADMIN_INSTRUCTIONS: &[&str] = &[
    "initialize",
    "initialize2",
    "migrateToOpenBook",
    "setParams",
    "withdrawPnl",
    "withdrawSrm",
    "adminCancelOrders",
    "createConfigAccount",
    "updateConfigAccount",
];

const RAYDIUM_AMM_V4_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "migrateToOpenBook",
    "setParams",
    "withdrawPnl",
    "withdrawSrm",
    "adminCancelOrders",
    "updateConfigAccount",
];

const METEORA_DLMM_INSTRUCTIONS: &[&str] = &[
    "add_liquidity",
    "add_liquidity2",
    "add_liquidity_by_strategy",
    "add_liquidity_by_strategy2",
    "add_liquidity_by_strategy_one_side",
    "add_liquidity_by_weight",
    "add_liquidity_by_weight2",
    "add_liquidity_one_side",
    "add_liquidity_one_side_precise",
    "add_liquidity_one_side_precise2",
    "cancel_limit_order",
    "claim_fee",
    "claim_fee2",
    "claim_reward",
    "claim_reward2",
    "close_bin_array",
    "close_claim_protocol_fee_operator",
    "close_claim_fee_operator_account",
    "close_limit_order_if_empty",
    "close_operator_account",
    "close_position",
    "close_position2",
    "close_position_if_empty",
    "close_preset_parameter",
    "close_preset_parameter2",
    "close_token_badge",
    "create_operator_account",
    "create_claim_protocol_fee_operator",
    "decrease_position_length",
    "for_idl_type_generation_do_not_call",
    "fund_reward",
    "go_to_a_bin",
    "increase_oracle_length",
    "increase_position_length",
    "increase_position_length2",
    "initialize_bin_array",
    "initialize_bin_array_bitmap_extension",
    "initialize_customizable_permissionless_lb_pair",
    "initialize_customizable_permissionless_lb_pair2",
    "initialize_lb_pair",
    "initialize_lb_pair2",
    "initialize_permission_lb_pair",
    "initialize_position",
    "initialize_position2",
    "initialize_position_by_operator",
    "initialize_position_pda",
    "initialize_preset_parameter",
    "initialize_preset_parameter2",
    "initialize_reward",
    "initialize_token_badge",
    "migrate_bin_array",
    "migrate_position",
    "place_limit_order",
    "rebalance_liquidity",
    "remove_all_liquidity",
    "remove_liquidity",
    "remove_liquidity2",
    "remove_liquidity_by_range",
    "remove_liquidity_by_range2",
    "reset_bin_array_tombstone_fields",
    "reset_pool_tombstone_fields",
    "reset_position_tombstone_fields",
    "set_activation_point",
    "set_pair_status",
    "set_pair_status_permissionless",
    "set_permissionless_operation_bits",
    "set_pre_activation_duration",
    "set_pre_activation_swap_address",
    "swap",
    "swap2",
    "swap_exact_out",
    "swap_exact_out2",
    "swap_with_price_impact",
    "swap_with_price_impact2",
    "update_base_fee_parameters",
    "update_dynamic_fee_parameters",
    "update_fees_and_reward2",
    "update_fees_and_rewards",
    "update_position_operator",
    "update_reward_duration",
    "update_reward_funder",
    "withdraw_ineligible_reward",
    "withdraw_protocol_fee",
    "zap_protocol_fee",
];

const METEORA_DLMM_ADMIN_INSTRUCTIONS: &[&str] = &[
    "close_claim_protocol_fee_operator",
    "create_claim_protocol_fee_operator",
    "create_operator_account",
    "fund_reward",
    "initialize_customizable_permissionless_lb_pair",
    "initialize_customizable_permissionless_lb_pair2",
    "initialize_lb_pair",
    "initialize_lb_pair2",
    "initialize_permission_lb_pair",
    "initialize_preset_parameter",
    "initialize_preset_parameter2",
    "initialize_reward",
    "initialize_token_badge",
    "set_activation_point",
    "set_pair_status",
    "set_pair_status_permissionless",
    "set_permissionless_operation_bits",
    "set_pre_activation_duration",
    "set_pre_activation_swap_address",
    "update_base_fee_parameters",
    "update_dynamic_fee_parameters",
    "update_position_operator",
    "update_reward_duration",
    "update_reward_funder",
    "withdraw_ineligible_reward",
    "withdraw_protocol_fee",
    "zap_protocol_fee",
];

const METEORA_DLMM_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "fund_reward",
    "set_activation_point",
    "set_pair_status",
    "set_pair_status_permissionless",
    "set_permissionless_operation_bits",
    "update_base_fee_parameters",
    "update_dynamic_fee_parameters",
    "update_reward_duration",
    "update_reward_funder",
];

const PROTOCOL_SPECS: &[ProtocolSpec] = &[
    ProtocolSpec {
        slug: SQUADS_PROTOCOL_SLUG,
        name: "Squads",
        program_label: "v4 Multisig",
        program_id: SQUADS_V4_PROGRAM_ID,
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: SQUADS_ADMIN_INSTRUCTIONS,
        high_impact_instructions: SQUADS_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "pumpfun",
        name: "Pumpfun",
        program_label: "Launchpad",
        program_id: "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: Some(InstructionCatalogSpec::Standard(PUMPFUN_INSTRUCTIONS)),
        admin_instructions: &[
            "add_quote_mint",
            "admin_set_creator",
            "admin_set_idl_authority",
            "admin_update_token_incentives",
            "remove_quote_mint",
            "set_mayhem_virtual_params",
            "set_metaplex_creator",
            "set_params",
            "set_reserved_fee_recipients",
            "set_virtual_quote_reserves",
            "toggle_cashback_enabled",
            "toggle_create_v2",
            "toggle_mayhem_mode",
            "update_buyback_config",
            "update_global_authority",
        ],
        high_impact_instructions: &[
            "add_quote_mint",
            "admin_set_creator",
            "admin_set_idl_authority",
            "admin_update_token_incentives",
            "remove_quote_mint",
            "set_mayhem_virtual_params",
            "set_params",
            "set_reserved_fee_recipients",
            "set_virtual_quote_reserves",
            "toggle_cashback_enabled",
            "toggle_create_v2",
            "toggle_mayhem_mode",
            "update_buyback_config",
            "update_global_authority",
        ],
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "orca_whirlpools",
        name: "Orca Whirlpools",
        program_label: "Whirlpools",
        program_id: "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc",
        instruction_name_style: InstructionNameStyle::CamelCase,
        instruction_catalog: Some(InstructionCatalogSpec::Standard(
            ORCA_WHIRLPOOLS_INSTRUCTIONS,
        )),
        admin_instructions: ORCA_ADMIN_INSTRUCTIONS,
        high_impact_instructions: ORCA_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "kamino_lend",
        name: "Kamino Lend",
        program_label: "Lend",
        program_id: "KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD",
        instruction_name_style: InstructionNameStyle::CamelCase,
        instruction_catalog: Some(InstructionCatalogSpec::Standard(KAMINO_LEND_INSTRUCTIONS)),
        admin_instructions: KAMINO_ADMIN_INSTRUCTIONS,
        high_impact_instructions: KAMINO_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "drift",
        name: "Drift",
        program_label: "Core",
        program_id: "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH",
        instruction_name_style: InstructionNameStyle::CamelCase,
        instruction_catalog: None,
        admin_instructions: DRIFT_ADMIN_INSTRUCTIONS,
        high_impact_instructions: DRIFT_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "drift",
        name: "Drift",
        program_label: "Vaults",
        program_id: "vAuLTsyrvSfZRuRB3XgvkPwNGgYSs9YRYymVebLKoxR",
        instruction_name_style: InstructionNameStyle::CamelCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "drift",
        name: "Drift",
        program_label: "Jit Proxy",
        program_id: "J1TnP8zvVxbtF5KFp5xRmWuvG9McnhzmBd9XGfCyuxFP",
        instruction_name_style: InstructionNameStyle::CamelCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "marginfi",
        name: "Marginfi",
        program_label: "v2",
        program_id: "MFv2hWf31Z9kbCa1snEPYctwafyhdvnV7FZnsebVacA",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: MARGINFI_ADMIN_INSTRUCTIONS,
        high_impact_instructions: MARGINFI_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "jupiter_lend",
        name: "Jupiter Lend",
        program_label: "Lending",
        program_id: "jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: Some(InstructionCatalogSpec::Standard(
            JUPITER_LENDING_INSTRUCTIONS,
        )),
        admin_instructions: JUPITER_LENDING_ADMIN_INSTRUCTIONS,
        high_impact_instructions: JUPITER_LENDING_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "jupiter_lend",
        name: "Jupiter Lend",
        program_label: "Flashloan",
        program_id: "jupgfSgfuAXv4B6R2Uxu85Z1qdzgju79s6MfZekN6XS",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: Some(InstructionCatalogSpec::Standard(
            JUPITER_FLASHLOAN_INSTRUCTIONS,
        )),
        admin_instructions: JUPITER_FLASHLOAN_ADMIN_INSTRUCTIONS,
        high_impact_instructions: JUPITER_FLASHLOAN_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "jupiter_lend",
        name: "Jupiter Lend",
        program_label: "Rewards Model",
        program_id: "jup7TthsMgcR9Y3L277b8Eo9uboVSmu1utkuXHNUKar",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: Some(InstructionCatalogSpec::Standard(
            JUPITER_REWARDS_MODEL_INSTRUCTIONS,
        )),
        admin_instructions: JUPITER_REWARDS_MODEL_ADMIN_INSTRUCTIONS,
        high_impact_instructions: JUPITER_REWARDS_MODEL_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "jupiter_lend",
        name: "Jupiter Lend",
        program_label: "Liquidity",
        program_id: "jupeiUmn818Jg1ekPURTpr4mFo29p46vygyykFJ3wZC",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: Some(InstructionCatalogSpec::Standard(
            JUPITER_LIQUIDITY_INSTRUCTIONS,
        )),
        admin_instructions: JUPITER_LIQUIDITY_ADMIN_INSTRUCTIONS,
        high_impact_instructions: JUPITER_LIQUIDITY_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "jupiter_lend",
        name: "Jupiter Lend",
        program_label: "Oracle",
        program_id: "jupnw4B6Eqs7ft6rxpzYLJZYSnrpRgPcr589n5Kv4oc",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: Some(InstructionCatalogSpec::Standard(
            JUPITER_ORACLE_INSTRUCTIONS,
        )),
        admin_instructions: JUPITER_ORACLE_ADMIN_INSTRUCTIONS,
        high_impact_instructions: JUPITER_ORACLE_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "jupiter_lend",
        name: "Jupiter Lend",
        program_label: "Vaults / Borrow",
        program_id: "jupr81YtYssSyPt8jbnGuiWon5f6x9TcDEFxYe3Bdzi",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: Some(InstructionCatalogSpec::Standard(
            JUPITER_VAULTS_INSTRUCTIONS,
        )),
        admin_instructions: JUPITER_VAULTS_ADMIN_INSTRUCTIONS,
        high_impact_instructions: JUPITER_VAULTS_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "raydium",
        name: "Raydium",
        program_label: "CLMM",
        program_id: "DRayAUgENGQBKVaX8owNhgzkEDyoHTGVEGHVJT1E9pfH",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: Some(InstructionCatalogSpec::Standard(RAYDIUM_CLMM_INSTRUCTIONS)),
        admin_instructions: RAYDIUM_CLMM_ADMIN_INSTRUCTIONS,
        high_impact_instructions: RAYDIUM_CLMM_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "raydium",
        name: "Raydium",
        program_label: "CPMM",
        program_id: "DRaycpLY18LhpbydsBWbVJtxpNv9oXPgjRSfpF2bWpYb",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: Some(InstructionCatalogSpec::Standard(RAYDIUM_CPMM_INSTRUCTIONS)),
        admin_instructions: RAYDIUM_CPMM_ADMIN_INSTRUCTIONS,
        high_impact_instructions: RAYDIUM_CPMM_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "raydium",
        name: "Raydium",
        program_label: "Legacy AMM v4",
        program_id: "DRaya7Kj3aMWQSy19kSjvmuwq9docCHofyP9kanQGaav",
        instruction_name_style: InstructionNameStyle::CamelCase,
        instruction_catalog: Some(InstructionCatalogSpec::Standard(
            RAYDIUM_AMM_V4_INSTRUCTIONS,
        )),
        admin_instructions: RAYDIUM_AMM_V4_ADMIN_INSTRUCTIONS,
        high_impact_instructions: RAYDIUM_AMM_V4_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "meteora_dlmm",
        name: "Meteora DLMM",
        program_label: "DLMM",
        program_id: "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: Some(InstructionCatalogSpec::Standard(METEORA_DLMM_INSTRUCTIONS)),
        admin_instructions: METEORA_DLMM_ADMIN_INSTRUCTIONS,
        high_impact_instructions: METEORA_DLMM_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "meteora",
        name: "Meteora",
        program_label: "Stable Swap",
        program_id: "MERLuDFBMmsHnsBPZw2sDQZHvXFMwp8EdjudcU2HKky",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "meteora",
        name: "Meteora",
        program_label: "Vaults",
        program_id: "24Uqj9JCLxUeoC3hGfh5W3s9FM9uCHDS2SG3LYwBpyTi",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "meteora",
        name: "Meteora",
        program_label: "Pools",
        program_id: "Eo7WjKq67rjJQSZxS6z3YkapzY3eMj6Xy8X5EQVn5UaB",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "meteora",
        name: "Meteora",
        program_label: "Farms",
        program_id: "FarmuwXPWXvefWUeqFAa5w6rifLkq5X6E8bimYvrhCB1",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "meteora",
        name: "Meteora",
        program_label: "DLMM Vaults",
        program_id: "vaU6kP7iNEGkbmPkLmZfGwiGxd4Mob24QQCie5R9kd2",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "meteora",
        name: "Meteora",
        program_label: "M3M3 Staking",
        program_id: "FEESngU3neckdwib9X3KWqdL7Mjmqk9XNp3uh5JbP4KP",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "meteora",
        name: "Meteora",
        program_label: "AMM V2",
        program_id: "cpamdpZCGKUy5JxQXB4dcpGPiikHawvSWAd6mEn1sGG",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "meteora",
        name: "Meteora",
        program_label: "Zap",
        program_id: "zapvX9M3uf5pvy4wRPAbQgdQsM1xmuiFnkfHKPvwMiz",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "meteora",
        name: "Meteora",
        program_label: "Bonding Curve",
        program_id: "dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "exponent",
        name: "Exponent",
        program_label: "Core",
        program_id: "ExponentnaRg3CQbW6dqQNZKXp7gtZ9DGMp1cwC4HAS7",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: Some(InstructionCatalogSpec::Explicit(
            EXPONENT_CORE_INSTRUCTION_SPECS,
        )),
        admin_instructions: EXPONENT_CORE_ADMIN_INSTRUCTIONS,
        high_impact_instructions: EXPONENT_CORE_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "exponent",
        name: "Exponent",
        program_label: "Admin",
        program_id: "3D6ojc8vBfDteLBDTTRznZbZRh7bkEGQaYqNkudoTCBQ",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: EXPONENT_ADMIN_INSTRUCTIONS,
        high_impact_instructions: EXPONENT_ADMIN_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "carrot",
        name: "Carrot",
        program_label: "Protocol",
        program_id: "CarrotwivhMpDnm27EHmRLeQ683Z1PufuqEmBZvD282s",
        instruction_name_style: InstructionNameStyle::CamelCase,
        instruction_catalog: Some(InstructionCatalogSpec::Standard(CARROT_INSTRUCTIONS)),
        admin_instructions: CARROT_ADMIN_INSTRUCTIONS,
        high_impact_instructions: CARROT_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "carrot",
        name: "Carrot",
        program_label: "Boost",
        program_id: "C73nDAFn23RYwiFa6vtHshSbcg8x6BLYjw3bERJ3vHxf",
        instruction_name_style: InstructionNameStyle::CamelCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "save",
        name: "Save",
        program_label: "Lending",
        program_id: "So1endDq2YkqhipRh3WViPa8hdiSpxWy6z3Z6tMCpAo",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "openbook",
        name: "OpenBook",
        program_label: "V1",
        program_id: "srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "openbook",
        name: "OpenBook",
        program_label: "V2",
        program_id: "opnbkNkqux64GppQhwbyEVc3axhssFhVYuwar8rDHCu",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "openbook",
        name: "OpenBook",
        program_label: "V3",
        program_id: "opnb2LAfJYbRMAHHvqjCwQxanZn7ReEHp1k81EohpZb",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "manifest",
        name: "Manifest",
        program_label: "Order Book",
        program_id: "wMNFSTkir3HgyZTsB7uqu3i7FA73grFCptPXgrZjksL",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "manifest",
        name: "Manifest",
        program_label: "Vault",
        program_id: "MNFSTqtC93rEfYHB6hF82sKdZpUDFWkViLByLd1k1Ms",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "glam",
        name: "GLAM",
        program_label: "Protocol",
        program_id: "GLAMpaME8wdTEzxtiYEAa5yD8fZbxZiz2hNtV58RZiEz",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "glam",
        name: "GLAM",
        program_label: "Mint",
        program_id: "GM1NtvvnSXUptTrMCqbogAdZJydZSNv98DoU5AZVLmGh",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "glam",
        name: "GLAM",
        program_label: "CCTP Integration",
        program_id: "G1NTcMDYgNLpDwgnrpSZvoSKQuR9NXG7S3DmtNQCDmrK",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "glam",
        name: "GLAM",
        program_label: "Drift Integration",
        program_id: "G1NTdrBmBpW43msRQmsf7qXSw3MFBNaqJcAkGiRmRq2F",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "glam",
        name: "GLAM",
        program_label: "Kamino Integration",
        program_id: "G1NTkDEUR3pkEqGCKZtmtmVzCUEdYa86pezHkwYbLyde",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "glam",
        name: "GLAM",
        program_label: "SPL Integration",
        program_id: "G1NTsQ36mjPe89HtPYqxKsjY5HmYsDR6CbD2gd2U2pta",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "adrena",
        name: "Adrena",
        program_label: "Core",
        program_id: "13gDzEXCdocbj8iAiqrScGo47NiSuYENGsRqi3SEAwet",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "loopscale",
        name: "Loopscale",
        program_label: "Lending",
        program_id: "1oopBoJG58DgkUVKkEzKgyG9dvRmpgeEm1AVjoHkF78",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "loopscale",
        name: "Loopscale",
        program_label: "CreditBook",
        program_id: "abfcSQac2vK2Pa6UAJb37DzarVxF15bDTdphJzAqYYp",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "neutral",
        name: "Neutral",
        program_label: "Bundle",
        program_id: "BUNDDh4P5XviMm1f3gCvnq2qKx6TGosAGnoUK12e7cXU",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "neutral",
        name: "Neutral",
        program_label: "Vaults A",
        program_id: "9Fcn3Fd4d5ocrb12xCUtEvezxcjFEAyHBPfrZDiPt9Qj",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "neutral",
        name: "Neutral",
        program_label: "Vaults B",
        program_id: "EuSLjg23BrtwYAk1t4TFe5ArYSXCVXLBqrHRBfWQiTeJ",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "neutral",
        name: "Neutral",
        program_label: "Vaults C",
        program_id: "BVddkVtFJLCihbVrtLo8e3iEd9NftuLunaznAxFFW8vf",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "fragmetric",
        name: "Fragmetric",
        program_label: "Core",
        program_id: "fragnAis7Bp6FTsMoa6YcH8UffhEw43Ph79qAiK3iF3",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "marinade",
        name: "Marinade",
        program_label: "Ticket",
        program_id: "MarBmsSgKXdrN1egZf5sqe1TMai9K1rChYNDJgjq7aD",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "jito",
        name: "Jito",
        program_label: "Governance",
        program_id: "jtogvBNH3WBSWDYD5FJfQP2ZxNTuf82zL8GkEhPeaJx",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "jito",
        name: "Jito",
        program_label: "Restaking",
        program_id: "Vau1t6sLNxnzB7ZDsef8TLbPLfyZMYXH8WTNqUdm9g8",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "kamino",
        name: "Kamino",
        program_label: "Liquidity",
        program_id: "6LtLpnUFNByNXLyCoK9wA2MykKAmQNZKBdY8s47dehDc",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "kamino",
        name: "Kamino",
        program_label: "Farm",
        program_id: "FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "kamino",
        name: "Kamino",
        program_label: "Limit Order",
        program_id: "LiMoM9rMhrdYrfzUCxQppvxCSG1FcrUK9G8uLq4A1GF",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "kamino",
        name: "Kamino",
        program_label: "Vault",
        program_id: "KvauGMspG5k6rtzrqqn7WNn3oZdyKqLKwK2XWQ8FLjd",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "kamino",
        name: "Kamino",
        program_label: "Leverage",
        program_id: "CRhtqXk98ATqo1R8gLg7qcpEMuvoPzqD5GNicPPqLMD",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "kamino",
        name: "Kamino",
        program_label: "Scope Price",
        program_id: "HFn8GnPADiny6XqUoWE8uRPPxb29ikn4yTuPa9MF2fWJ",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "lulo",
        name: "Lulo",
        program_label: "Router",
        program_id: "FL3X2pRsQ9zHENpZSKDRREtccwJuei8yg9fwDu9UN69Q",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "parcl",
        name: "Parcl",
        program_label: "Liquidity & Trading",
        program_id: "3parcLrT7WnXAcyPfkCz49oofuuf2guUKkjuFkAhZW8Y",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "parcl",
        name: "Parcl",
        program_label: "Staking",
        program_id: "2gWf5xLAzZaKX9tQj9vuXsaxTWtzTZDFRn21J3zjNVgu",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "perena",
        name: "Perena",
        program_label: "Numeraire",
        program_id: "NUMERUNsFCP3kuNmWZuXtm1AaQCPj9uw6Guv2Ekoi5P",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "perena",
        name: "Perena",
        program_label: "usdstar",
        program_id: "save8RQVPMWNTzU18t3GBvBkN9hT7jsGjiCQ28FpD9H",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "sanctum",
        name: "Sanctum",
        program_label: "Infinity Pool",
        program_id: "5ocnV1qiCgaQR8Jb8xWnVbApfaygJ8tNoZfgPwsgx9kx",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "sanctum",
        name: "Sanctum",
        program_label: "Router",
        program_id: "stkitrT1Uoy18Dk1fTrgPw8W6MVzoCfYoAFT4MLsmhq",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "sanctum",
        name: "Sanctum",
        program_label: "Vote",
        program_id: "VAU1T7S5UuEHmMvXtXMVmpEoQtZ2ya7eRb7gcN47wDp",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "sanctum",
        name: "Sanctum",
        program_label: "Staking",
        program_id: "bon4Kh3x1uQK16w9b9DKgz3Aw4AP1pZxBJk55Q6Sosb",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "sanctum",
        name: "Sanctum",
        program_label: "ASR",
        program_id: "9R2CZDyVjBBK2bxx6NfKdnYMYfh8TQEphUUgZ4861XqJ",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "solayer",
        name: "Solayer",
        program_label: "Staking",
        program_id: "sSo1iU21jBrU9VaJ8PJib1MtorefUV4fzC9GURa2KNn",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "solayer",
        name: "Solayer",
        program_label: "sUSD",
        program_id: "s1aysqpEyZyijPybUV89oBGeooXrR22wMNLjnG2SWJA",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "solayer",
        name: "Solayer",
        program_label: "Endo AVS",
        program_id: "endoLNCKTqDn8gSVnN2hDdpgACUPWHZTwoYnnMybpAT",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "huma",
        name: "Huma",
        program_label: "Permissionless",
        program_id: "HumaXepHnjaRCpjYTokxY4UtaJcmx41prQ8cxGmFC5fn",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "huma",
        name: "Huma",
        program_label: "Institutional",
        program_id: "EVQ4s1b6N1vmWFDv8PRNc77kufBP8HcrSNWXQAhRsJq9",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "huma",
        name: "Huma",
        program_label: "Staking",
        program_id: "vsRJM68m7i18PwzTFphgPYXTujCgxEi28knpUwSmg3q",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "huma",
        name: "Huma",
        program_label: "Prime",
        program_id: "prm1azdDGzyqP76s3Hv2nuG3uLnBgR5u2d7pANwmmzC",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "maple",
        name: "Maple",
        program_label: "Lending",
        program_id: "5D9yi4BKrxF8h65NkVE1raCCWFKUs5ngub2ECxhvfaZe",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "flashtrade",
        name: "FlashTrade",
        program_label: "Earn",
        program_id: "FLASH6Lo6h3iasJKWDs2F8TkW2UKf3s15C8PMGuVfgBn",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "wasabi",
        name: "Wasabi",
        program_label: "Trade",
        program_id: "spicyTHtbmarmUxwFSHYpA8G4uP2nRNq38RReMpoZ9c",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
];

pub fn watched_program_ids() -> Vec<Pubkey> {
    definitions()
        .iter()
        .map(|definition| definition.program_id)
        .collect()
}

pub fn instruction_descriptors_for_program(program_id: &str) -> Vec<ProtocolInstructionDescriptor> {
    let Ok(program_id) = Pubkey::from_str(program_id) else {
        return Vec::new();
    };

    let Some(definition) = definitions()
        .iter()
        .find(|definition| definition.program_id == program_id)
    else {
        return Vec::new();
    };

    let Some(catalog) = definition.instruction_catalog.as_ref() else {
        return Vec::new();
    };

    let mut descriptors = catalog
        .iter()
        .filter_map(|(discriminator, name)| {
            let discriminator: [u8; 8] = discriminator.as_slice().try_into().ok()?;
            Some(ProtocolInstructionDescriptor {
                name: (*name).to_owned(),
                discriminator,
            })
        })
        .collect::<Vec<_>>();
    descriptors.sort_by(|a, b| a.name.cmp(&b.name));
    descriptors
}

pub fn curated_protocol_descriptors() -> Vec<DefiProtocolDescriptor> {
    let mut descriptors = Vec::new();

    for definition in definitions() {
        let admin_instruction_filters = definition
            .admin_instructions
            .iter()
            .map(|instruction| {
                canonical_instruction_name(instruction, definition.instruction_name_style)
            })
            .collect::<Vec<_>>();
        let high_impact_instruction_filters = definition
            .high_impact_instructions
            .iter()
            .map(|instruction| {
                canonical_instruction_name(instruction, definition.instruction_name_style)
            })
            .collect::<Vec<_>>();

        if let Some(existing) = descriptors
            .iter_mut()
            .find(|descriptor: &&mut DefiProtocolDescriptor| descriptor.slug == definition.slug)
        {
            existing.programs.push(DefiProtocolProgramDescriptor {
                label: definition.program_label.clone(),
                program_id: definition.program_id.to_string(),
                idl_metadata_address: program_metadata::canonical_idl_metadata_address(
                    definition.program_id,
                )
                .to_string(),
                anchor_legacy_idl_address: anchor_idl::legacy_idl_address(definition.program_id)
                    .map(|address| address.to_string()),
                risk_summary: Default::default(),
            });
            existing.catalog_instruction_count += definition.catalog_instruction_count();
            existing.coverage = match (existing.coverage.clone(), definition.coverage()) {
                (ProtocolCoverage::DeepDecode, _) | (_, ProtocolCoverage::DeepDecode) => {
                    ProtocolCoverage::DeepDecode
                }
                (ProtocolCoverage::AdminSignals, _) | (_, ProtocolCoverage::AdminSignals) => {
                    ProtocolCoverage::AdminSignals
                }
                _ => ProtocolCoverage::AddressOnly,
            };

            for instruction in admin_instruction_filters {
                push_unique(&mut existing.admin_instruction_filters, instruction);
            }

            for instruction in high_impact_instruction_filters {
                push_unique(&mut existing.high_impact_instruction_filters, instruction);
            }

            for trigger in amount_trigger_descriptors(&definition.slug) {
                push_unique_amount_trigger(&mut existing.amount_triggers, trigger);
            }

            existing.unknown_alerts |= definition.unknown_alerts;
        } else {
            descriptors.push(DefiProtocolDescriptor {
                slug: definition.slug.clone(),
                name: definition.name.clone(),
                program_id: definition.program_id.to_string(),
                programs: vec![DefiProtocolProgramDescriptor {
                    label: definition.program_label.clone(),
                    program_id: definition.program_id.to_string(),
                    idl_metadata_address: program_metadata::canonical_idl_metadata_address(
                        definition.program_id,
                    )
                    .to_string(),
                    anchor_legacy_idl_address: anchor_idl::legacy_idl_address(
                        definition.program_id,
                    )
                    .map(|address| address.to_string()),
                    risk_summary: Default::default(),
                }],
                sol_domains: Vec::new(),
                coverage: definition.coverage(),
                catalog_instruction_count: definition.catalog_instruction_count(),
                admin_instruction_filters,
                high_impact_instruction_filters,
                amount_triggers: amount_trigger_descriptors(&definition.slug),
                unknown_alerts: definition.unknown_alerts,
            });
        }
    }

    descriptors.extend(project_protocol_descriptors());

    descriptors
}

pub fn curated_protocol_descriptor(slug_or_name: &str) -> Option<DefiProtocolDescriptor> {
    let normalized = normalize_protocol_key(slug_or_name);

    curated_protocol_descriptors()
        .into_iter()
        .find(|descriptor| {
            descriptor.slug == normalized || normalize_protocol_key(&descriptor.name) == normalized
        })
}

pub fn protocol_for_program(program_id: Pubkey) -> Option<DefiProtocolMatch> {
    definitions()
        .iter()
        .find(|definition| definition.program_id == program_id)
        .map(|definition| DefiProtocolMatch {
            slug: definition.slug.clone(),
            name: definition.name.clone(),
            program_id: definition.program_id,
            program_label: definition.program_label.clone(),
        })
}

pub fn protocol_for_sol_domain(domain: &str) -> Option<DomainProtocolMatch> {
    let normalized = domain.trim().to_ascii_lowercase();
    if FERNO_SOL_DOMAINS
        .iter()
        .any(|domain| *domain == normalized.as_str())
    {
        return Some(DomainProtocolMatch {
            slug: FERNO_PROTOCOL_SLUG.to_owned(),
            name: "Ferno".to_owned(),
        });
    }

    None
}

fn project_protocol_descriptors() -> Vec<DefiProtocolDescriptor> {
    vec![DefiProtocolDescriptor {
        slug: FERNO_PROTOCOL_SLUG.to_owned(),
        name: "Ferno".to_owned(),
        program_id: String::new(),
        programs: Vec::new(),
        sol_domains: FERNO_SOL_DOMAINS
            .iter()
            .map(|domain| (*domain).to_owned())
            .collect(),
        coverage: ProtocolCoverage::AddressOnly,
        catalog_instruction_count: 0,
        admin_instruction_filters: Vec::new(),
        high_impact_instruction_filters: Vec::new(),
        amount_triggers: amount_trigger_descriptors(FERNO_PROTOCOL_SLUG),
        unknown_alerts: false,
    }]
}

fn amount_trigger_descriptors(slug: &str) -> Vec<AmountTriggerDescriptor> {
    match slug {
        "jupiter_lend" => vec![
            AmountTriggerDescriptor {
                label: "Token price".to_owned(),
                description:
                    "Alert when the observed token exchange price exits the configured bound."
                        .to_owned(),
            },
            AmountTriggerDescriptor {
                label: "Liquidity price".to_owned(),
                description:
                    "Alert when the observed liquidity exchange price exits the configured bound."
                        .to_owned(),
            },
        ],
        FERNO_PROTOCOL_SLUG => vec![AmountTriggerDescriptor {
            label: "Drain move".to_owned(),
            description: "Proposed: alert when a watched Ferno-controlled wallet sends at least 70% of tracked SOL or token value, or at least 25k USD equivalent, within 10 minutes to new recipients across 3 or more outbound transfers.".to_owned(),
        }],
        _ => Vec::new(),
    }
}

fn normalize_protocol_key(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

pub fn protocol_for_idl_metadata_account(metadata_account: Pubkey) -> Option<DefiProtocolMatch> {
    definitions()
        .iter()
        .find(|definition| {
            program_metadata::canonical_idl_metadata_address(definition.program_id)
                == metadata_account
        })
        .map(|definition| DefiProtocolMatch {
            slug: definition.slug.clone(),
            name: definition.name.clone(),
            program_id: definition.program_id,
            program_label: definition.program_label.clone(),
        })
}

#[allow(dead_code)]
pub fn protocol_for_anchor_legacy_idl_account(idl_account: Pubkey) -> Option<DefiProtocolMatch> {
    definitions()
        .iter()
        .find(|definition| {
            anchor_idl::legacy_idl_address(definition.program_id) == Some(idl_account)
        })
        .map(|definition| DefiProtocolMatch {
            slug: definition.slug.clone(),
            name: definition.name.clone(),
            program_id: definition.program_id,
            program_label: definition.program_label.clone(),
        })
}

pub fn resolve_protocol_instruction(
    program_id: Pubkey,
    data: &[u8],
) -> Option<ProtocolInstructionSignal> {
    let definition = definitions()
        .iter()
        .find(|definition| definition.program_id == program_id)?;

    if let Some((raw_name, discriminator)) = definition
        .instruction_catalog
        .as_ref()
        .and_then(|lookup| lookup_instruction(lookup, data))
    {
        return Some(build_signal(
            definition,
            Some(raw_name),
            definition
                .admin_instructions
                .contains(&raw_name)
                .then_some(ProtocolSignalKind::Admin),
            discriminator,
        ));
    }

    if let Some((raw_name, discriminator)) =
        lookup_instruction(&definition.admin_instruction_lookup, data)
    {
        return Some(build_signal(
            definition,
            Some(raw_name),
            Some(ProtocolSignalKind::Admin),
            discriminator,
        ));
    }

    if definition.unknown_alerts {
        return Some(build_signal(
            definition,
            None,
            Some(ProtocolSignalKind::Unknown),
            unknown_discriminator_bytes(data).as_slice(),
        ));
    }

    None
}

fn build_signal(
    definition: &ProtocolDefinition,
    raw_instruction_name: Option<&'static str>,
    signal_kind: Option<ProtocolSignalKind>,
    discriminator: &[u8],
) -> ProtocolInstructionSignal {
    let instruction_name = raw_instruction_name
        .map(|name| canonical_instruction_name(name, definition.instruction_name_style));
    let high_impact = raw_instruction_name
        .map(|name| definition.high_impact_instructions.contains(&name))
        .unwrap_or(true);

    ProtocolInstructionSignal {
        protocol: DefiProtocolMatch {
            slug: definition.slug.clone(),
            name: definition.name.clone(),
            program_id: definition.program_id,
            program_label: definition.program_label.clone(),
        },
        instruction_name,
        signal_kind,
        high_impact,
        discriminator_hex: hex::encode(discriminator),
    }
}

fn definitions() -> &'static [ProtocolDefinition] {
    static DEFINITIONS: OnceLock<Vec<ProtocolDefinition>> = OnceLock::new();
    DEFINITIONS.get_or_init(|| {
        let mut definitions = PROTOCOL_SPECS
            .iter()
            .map(|spec| ProtocolDefinition {
                slug: spec.slug.to_owned(),
                name: spec.name.to_owned(),
                program_label: spec.program_label.to_owned(),
                program_id: Pubkey::from_str(spec.program_id).expect("valid curated program id"),
                instruction_name_style: spec.instruction_name_style,
                instruction_catalog: build_instruction_lookup(spec),
                admin_instruction_lookup: build_admin_instruction_lookup(spec),
                admin_instructions: spec.admin_instructions,
                high_impact_instructions: spec.high_impact_instructions,
                unknown_alerts: spec.unknown_alerts,
            })
            .collect::<Vec<_>>();

        let mut seen_programs = definitions
            .iter()
            .map(|definition| definition.program_id)
            .collect::<HashSet<_>>();

        for platform in registry_watchable_platforms() {
            for program in &platform.programs {
                let Ok(program_id) = Pubkey::from_str(&program.program_id) else {
                    continue;
                };

                if !seen_programs.insert(program_id) {
                    continue;
                }

                let slug = normalize_protocol_key(&platform.slug);
                definitions.push(ProtocolDefinition {
                    slug,
                    name: platform.name.clone(),
                    program_label: program.label.clone(),
                    program_id,
                    instruction_name_style: InstructionNameStyle::SnakeCase,
                    instruction_catalog: None,
                    admin_instruction_lookup: HashMap::new(),
                    admin_instructions: &[],
                    high_impact_instructions: &[],
                    unknown_alerts: false,
                });
            }
        }

        definitions
    })
}

fn registry_watchable_platforms() -> &'static [RegistryPlatform] {
    static REGISTRY_PLATFORMS: OnceLock<Vec<RegistryPlatform>> = OnceLock::new();
    REGISTRY_PLATFORMS.get_or_init(|| {
        let snapshot: RegistrySnapshot =
            serde_json::from_str(include_str!("../data/platform_registry_snapshot.json"))
                .expect("valid platform registry snapshot");

        snapshot
            .items
            .into_iter()
            .filter(is_watchable_registry_platform)
            .collect()
    })
}

fn is_watchable_registry_platform(platform: &RegistryPlatform) -> bool {
    const WATCHABLE_TAGS: &[&str] = &[
        "amm",
        "bridge",
        "defi",
        "dex",
        "institutional",
        "launchpad",
        "lending",
        "liquidity-provider",
        "options",
        "oracle",
        "payments",
        "prediction",
        "restaking",
        "rwa",
        "stablecoin",
        "trading",
        "vault",
    ];

    !platform.is_deprecated
        && !platform.programs.is_empty()
        && platform
            .tags
            .iter()
            .any(|tag| WATCHABLE_TAGS.contains(&tag.as_str()))
}

fn build_instruction_lookup(spec: &ProtocolSpec) -> Option<HashMap<Vec<u8>, &'static str>> {
    match spec.instruction_catalog {
        Some(InstructionCatalogSpec::Standard(instruction_catalog)) => Some(
            instruction_catalog
                .iter()
                .map(|instruction_name| {
                    (
                        discriminator_for_name(instruction_name, spec.instruction_name_style)
                            .to_vec(),
                        *instruction_name,
                    )
                })
                .collect(),
        ),
        Some(InstructionCatalogSpec::Explicit(instruction_catalog)) => Some(
            instruction_catalog
                .iter()
                .map(|instruction| (instruction.discriminator.to_vec(), instruction.name))
                .collect(),
        ),
        None => None,
    }
}

fn build_admin_instruction_lookup(spec: &ProtocolSpec) -> HashMap<Vec<u8>, &'static str> {
    match spec.instruction_catalog {
        Some(InstructionCatalogSpec::Explicit(instruction_catalog)) => spec
            .admin_instructions
            .iter()
            .filter_map(|instruction_name| {
                instruction_catalog
                    .iter()
                    .find(|instruction| instruction.name == *instruction_name)
                    .map(|instruction| (instruction.discriminator.to_vec(), instruction.name))
            })
            .collect(),
        _ => spec
            .admin_instructions
            .iter()
            .map(|instruction_name| {
                (
                    discriminator_for_name(instruction_name, spec.instruction_name_style).to_vec(),
                    *instruction_name,
                )
            })
            .collect(),
    }
}

fn lookup_instruction<'a>(
    lookup: &'a HashMap<Vec<u8>, &'static str>,
    data: &[u8],
) -> Option<(&'static str, &'a [u8])> {
    lookup
        .iter()
        .find(|(discriminator, _)| data.starts_with(discriminator.as_slice()))
        .map(|(discriminator, name)| (*name, discriminator.as_slice()))
}

fn unknown_discriminator_bytes(data: &[u8]) -> Vec<u8> {
    data.iter().copied().take(8).collect()
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn push_unique_amount_trigger(
    values: &mut Vec<AmountTriggerDescriptor>,
    value: AmountTriggerDescriptor,
) {
    if !values.iter().any(|existing| existing.label == value.label) {
        values.push(value);
    }
}

fn discriminator_for_name(name: &str, style: InstructionNameStyle) -> [u8; 8] {
    let canonical_name = canonical_instruction_name(name, style);
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{canonical_name}").as_bytes());
    let hash = hasher.finalize();
    let mut discriminator = [0_u8; 8];
    discriminator.copy_from_slice(&hash[..8]);
    discriminator
}

fn canonical_instruction_name(name: &str, style: InstructionNameStyle) -> String {
    match style {
        InstructionNameStyle::SnakeCase => name.to_owned(),
        InstructionNameStyle::CamelCase => camel_to_snake_case(name),
    }
}

fn camel_to_snake_case(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 8);

    for (index, character) in value.chars().enumerate() {
        if character.is_ascii_uppercase() {
            if index > 0 {
                output.push('_');
            }
            output.push(character.to_ascii_lowercase());
        } else {
            output.push(character);
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_kamino_admin_instruction() {
        let program_id = Pubkey::from_str("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name("updateLendingMarketOwner", InstructionNameStyle::CamelCase),
        )
        .expect("kamino admin instruction should be classified");

        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Admin));
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("update_lending_market_owner")
        );
    }

    #[test]
    fn keeps_kamino_reserve_config_update_as_standard_admin() {
        let program_id = Pubkey::from_str("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name("updateReserveConfig", InstructionNameStyle::CamelCase),
        )
        .expect("kamino reserve config update should be classified");

        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Admin));
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("update_reserve_config")
        );
        assert!(!signal.high_impact);
    }

    #[test]
    fn classifies_kamino_unknown_instruction() {
        let program_id = Pubkey::from_str("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD").unwrap();
        let signal = resolve_protocol_instruction(program_id, &[9, 9, 9, 9, 9, 9, 9, 9])
            .expect("unknown signal");

        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Unknown));
        assert!(signal.instruction_name.is_none());
    }

    #[test]
    fn ignores_known_kamino_non_admin_instruction() {
        let program_id = Pubkey::from_str("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name("rolloverFixedTermBorrow", InstructionNameStyle::CamelCase),
        )
        .expect("known instruction");

        assert!(signal.signal_kind.is_none());
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("rollover_fixed_term_borrow")
        );
    }

    #[test]
    fn ignores_kamino_rewards_topup_instruction() {
        let program_id = Pubkey::from_str("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name("topupReserveRewards", InstructionNameStyle::CamelCase),
        )
        .expect("known instruction");

        assert!(signal.signal_kind.is_none());
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("topup_reserve_rewards")
        );
        assert_eq!(signal.discriminator_hex, "3fff82d36ed858ad");
    }

    #[test]
    fn classifies_exponent_custom_admin_instruction() {
        let program_id = Pubkey::from_str("ExponentnaRg3CQbW6dqQNZKXp7gtZ9DGMp1cwC4HAS7").unwrap();
        let signal = resolve_protocol_instruction(program_id, &[27]).expect("exponent admin");

        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Admin));
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("modify_market_setting")
        );
    }

    #[test]
    fn groups_multi_program_protocol_descriptors() {
        let descriptor = curated_protocol_descriptors()
            .into_iter()
            .find(|protocol| protocol.slug == "jupiter_lend")
            .expect("jupiter lend descriptor");

        assert_eq!(descriptor.programs.len(), 6);
        assert!(
            descriptor
                .programs
                .iter()
                .any(|program| program.label == "Oracle")
        );
    }

    #[test]
    fn ignores_jupiter_lend_update_rate_as_non_admin() {
        let program_id = Pubkey::from_str("jup3YeL8QhtSx1e253b2FDvsMNC87fDrgQZivbrndc9").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name("update_rate", InstructionNameStyle::SnakeCase),
        )
        .expect("known instruction");

        assert!(signal.signal_kind.is_none());
        assert_eq!(signal.instruction_name.as_deref(), Some("update_rate"));
    }

    #[test]
    fn ignores_pumpfun_creator_migration_as_non_admin() {
        let program_id = Pubkey::from_str("6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name(
                "migrate_bonding_curve_creator",
                InstructionNameStyle::SnakeCase,
            ),
        )
        .expect("known Pump.fun creator migration instruction");

        assert!(signal.signal_kind.is_none());
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("migrate_bonding_curve_creator")
        );
    }

    #[test]
    fn ignores_known_meteora_limit_order_instruction() {
        let program_id = Pubkey::from_str("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name("place_limit_order", InstructionNameStyle::SnakeCase),
        )
        .expect("known Meteora DLMM instruction");

        assert!(signal.signal_kind.is_none());
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("place_limit_order")
        );
    }

    #[test]
    fn classifies_meteora_permissionless_bits_as_admin() {
        let program_id = Pubkey::from_str("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name(
                "set_permissionless_operation_bits",
                InstructionNameStyle::SnakeCase,
            ),
        )
        .expect("known Meteora DLMM admin instruction");

        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Admin));
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("set_permissionless_operation_bits")
        );
        assert!(signal.high_impact);
    }

    #[test]
    fn keeps_meteora_protocol_fee_withdrawal_as_non_high_impact_admin() {
        let program_id = Pubkey::from_str("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name("withdraw_protocol_fee", InstructionNameStyle::SnakeCase),
        )
        .expect("known Meteora DLMM admin instruction");

        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Admin));
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("withdraw_protocol_fee")
        );
        assert!(!signal.high_impact);
    }

    #[test]
    fn keeps_meteora_protocol_fee_zap_as_non_high_impact_admin() {
        let program_id = Pubkey::from_str("LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name("zap_protocol_fee", InstructionNameStyle::SnakeCase),
        )
        .expect("known Meteora DLMM admin instruction");

        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Admin));
        assert_eq!(signal.instruction_name.as_deref(), Some("zap_protocol_fee"));
        assert!(!signal.high_impact);
    }

    #[test]
    fn resolves_curated_protocol_program() {
        let program_id = Pubkey::from_str("DRayAUgENGQBKVaX8owNhgzkEDyoHTGVEGHVJT1E9pfH").unwrap();
        let protocol = protocol_for_program(program_id).expect("raydium should be curated");

        assert_eq!(protocol.slug, "raydium");
        assert_eq!(protocol.name, "Raydium");
        assert_eq!(protocol.program_label, "CLMM");
    }

    #[test]
    fn groups_expanded_protocol_program_sets() {
        let drift = curated_protocol_descriptors()
            .into_iter()
            .find(|protocol| protocol.slug == "drift")
            .expect("drift descriptor");

        assert!(drift.programs.len() >= 3);
        assert!(
            drift
                .programs
                .iter()
                .any(|program| program.label == "Jit Proxy")
        );
    }

    #[test]
    fn imports_watchable_registry_protocols() {
        let descriptors = curated_protocol_descriptors();
        let pyth = descriptors
            .into_iter()
            .find(|protocol| protocol.slug == "pyth")
            .expect("pyth descriptor from registry snapshot");

        assert_eq!(pyth.coverage, ProtocolCoverage::AddressOnly);
        assert!(
            pyth.programs
                .iter()
                .any(|program| program.label == "Express Relay")
        );
    }

    #[test]
    fn includes_squads_protocol_descriptor() {
        let squads = curated_protocol_descriptors()
            .into_iter()
            .find(|protocol| protocol.slug == SQUADS_PROTOCOL_SLUG)
            .expect("squads protocol descriptor");

        assert_eq!(squads.name, "Squads");
        assert_eq!(squads.coverage, ProtocolCoverage::AdminSignals);
        assert_eq!(squads.programs.len(), 1);
        assert_eq!(squads.programs[0].program_id, SQUADS_V4_PROGRAM_ID);
        assert!(
            squads
                .admin_instruction_filters
                .iter()
                .any(|instruction| instruction == "config_transaction_execute")
        );
    }

    #[test]
    fn classifies_squads_config_execution_as_admin() {
        let program_id = Pubkey::from_str(SQUADS_V4_PROGRAM_ID).unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name(
                "config_transaction_execute",
                InstructionNameStyle::SnakeCase,
            ),
        )
        .expect("squads config execution");

        assert_eq!(signal.protocol.slug, SQUADS_PROTOCOL_SLUG);
        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Admin));
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("config_transaction_execute")
        );
        assert!(signal.high_impact);
    }

    #[test]
    fn includes_ferno_domain_project_descriptor() {
        let ferno = curated_protocol_descriptors()
            .into_iter()
            .find(|protocol| protocol.slug == FERNO_PROTOCOL_SLUG)
            .expect("ferno project descriptor");

        assert!(ferno.programs.is_empty());
        assert_eq!(ferno.sol_domains.len(), 6);
        assert!(
            ferno
                .sol_domains
                .iter()
                .any(|domain| domain == "ach-wallet.sol")
        );
        assert_eq!(ferno.amount_triggers.len(), 1);
        assert_eq!(ferno.amount_triggers[0].label, "Drain move");
    }

    #[test]
    fn matches_ferno_sol_domain_case_insensitively() {
        let matched = protocol_for_sol_domain("Ferno.SOL").expect("ferno domain match");

        assert_eq!(matched.slug, FERNO_PROTOCOL_SLUG);
        assert_eq!(matched.name, "Ferno");
    }
}
