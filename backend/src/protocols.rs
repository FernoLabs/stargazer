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

#[derive(Clone, Copy, Debug)]
pub struct TreasuryTokenWatch {
    pub protocol_slug: &'static str,
    pub protocol_name: &'static str,
    pub treasury_wallet: &'static str,
    pub token_mint: &'static str,
    pub token_symbol: &'static str,
    pub token_decimals: u8,
    pub critical_outflow_raw_amount: u64,
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

#[derive(Clone, Debug, Deserialize)]
struct DefiLlamaSolanaSnapshot {
    items: Vec<DefiLlamaSolanaProtocol>,
}

#[derive(Clone, Debug, Deserialize)]
struct DefiLlamaSolanaProtocol {
    slug: String,
    name: String,
}

#[derive(Clone, Debug, Deserialize)]
struct OnchainPresenceSnapshot {
    items: Vec<OnchainPresenceProtocol>,
}

#[derive(Clone, Debug, Deserialize)]
struct OnchainPresenceProtocol {
    slug: String,
    name: String,
    programs: Vec<OnchainPresenceProgram>,
}

#[derive(Clone, Debug, Deserialize)]
struct OnchainPresenceProgram {
    label: String,
    program_id: String,
}

pub const FERNO_PROTOCOL_SLUG: &str = "ferno";
pub const BONK_DAO_PROTOCOL_SLUG: &str = "bonk_dao";
pub const REALMS_PROTOCOL_SLUG: &str = "realms";
pub const SQUADS_PROTOCOL_SLUG: &str = "squads";
pub const SQUADS_V4_PROGRAM_ID: &str = "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf";
pub const BONK_DAO_GOVERNANCE_PROGRAM_ID: &str = "GovER5Lthms3bLBqWub97yVrMmEogzX7xNjdXpPPCVZw";
pub const BONK_DAO_REALM_ADDRESS: &str = "84pGFuy1Y27ApK67ApethaPvexeDWA66zNV8gm38TVeQ";
pub const BONK_DAO_GOVERNANCE_ADDRESS: &str = "Uq5BRkVfdBpMknZJHw6huS3dunEgJpUDv3M2DG3BfQg";
pub const BONK_DAO_COMMUNITY_MINT_ADDRESS: &str = "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263";
pub const BONK_DAO_COUNCIL_MINT_ADDRESS: &str = "G3EzHHXPd434o31FKxd32eYQ673eDjyYiWWNZpMk9ppt";
pub const BONK_DAO_TREASURY_WALLET: &str = "AGkGWK1R669KDT4FCqgDgK7PgahGJPjD4J9xmVjuL9kn";
pub const BONK_TOKEN_SYMBOL: &str = "BONK";
pub const BONK_TOKEN_DECIMALS: u8 = 5;
pub const BONK_DAO_CRITICAL_OUTFLOW_RAW_AMOUNT: u64 = 100_000_000_000_000_000;
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
const BONK_DAO_GOVERNANCE_INSTRUCTION_FILTERS: &[&str] = &[
    "create_proposal",
    "insert_transaction",
    "cast_vote",
    "finalize_vote",
    "execute_transaction",
    "set_governance_config",
    "set_realm_authority",
    "set_realm_config",
];

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

const BAGS_FEE_SHARE_V2_INSTRUCTIONS: &[&str] = &[
    "claim_damm_v2",
    "claim_dbc",
    "claim_partner",
    "claim_platform_fees",
    "claim_user",
    "claim_user_vault",
    "confirm_admin",
    "create_fee_config",
    "create_partner_config",
    "dummy_1",
    "dummy_2",
    "extend_created_fee_config",
    "force_claim_user",
    "force_claim_user_to_vault",
    "force_sol_claim_user",
    "init_program_config",
    "manager_transfer_fee_config",
    "manager_update_fee_config",
    "manager_waive_fee_config",
    "update_fee_config",
    "update_fee_config_manager",
    "update_fee_config_partner",
    "update_partner_config",
    "update_partner_fee_collection",
    "update_program_config",
];

const BAGS_FEE_SHARE_V2_ADMIN_INSTRUCTIONS: &[&str] = &[
    "claim_platform_fees",
    "confirm_admin",
    "create_fee_config",
    "create_partner_config",
    "extend_created_fee_config",
    "force_claim_user",
    "force_claim_user_to_vault",
    "force_sol_claim_user",
    "init_program_config",
    "manager_transfer_fee_config",
    "manager_update_fee_config",
    "manager_waive_fee_config",
    "update_fee_config",
    "update_fee_config_manager",
    "update_fee_config_partner",
    "update_partner_config",
    "update_partner_fee_collection",
    "update_program_config",
];

const BAGS_FEE_SHARE_V2_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "confirm_admin",
    "create_fee_config",
    "create_partner_config",
    "extend_created_fee_config",
    "force_claim_user",
    "force_claim_user_to_vault",
    "force_sol_claim_user",
    "init_program_config",
    "manager_transfer_fee_config",
    "manager_update_fee_config",
    "manager_waive_fee_config",
    "update_fee_config",
    "update_fee_config_manager",
    "update_fee_config_partner",
    "update_partner_config",
    "update_partner_fee_collection",
    "update_program_config",
];

const BAGS_FEE_SHARE_V1_INSTRUCTIONS: &[&str] = &[
    "claim_a",
    "claim_b",
    "claim_damm_a",
    "claim_damm_b",
    "create_fee_vaults",
    "tweak_fee_vaults",
];

const BAGS_FEE_SHARE_V1_ADMIN_INSTRUCTIONS: &[&str] = &["create_fee_vaults", "tweak_fee_vaults"];
const BAGS_FEE_SHARE_V1_HIGH_IMPACT_INSTRUCTIONS: &[&str] = BAGS_FEE_SHARE_V1_ADMIN_INSTRUCTIONS;

const SPL_GOVERNANCE_SIGNAL_INSTRUCTIONS: &[InstructionSpec] = &[
    InstructionSpec {
        name: "create_proposal",
        discriminator: &[6],
    },
    InstructionSpec {
        name: "insert_transaction",
        discriminator: &[9],
    },
    InstructionSpec {
        name: "cancel_proposal",
        discriminator: &[11],
    },
    InstructionSpec {
        name: "sign_off_proposal",
        discriminator: &[12],
    },
    InstructionSpec {
        name: "cast_vote",
        discriminator: &[13],
    },
    InstructionSpec {
        name: "finalize_vote",
        discriminator: &[14],
    },
    InstructionSpec {
        name: "execute_transaction",
        discriminator: &[16],
    },
    InstructionSpec {
        name: "set_governance_config",
        discriminator: &[19],
    },
    InstructionSpec {
        name: "set_realm_authority",
        discriminator: &[21],
    },
    InstructionSpec {
        name: "set_realm_config",
        discriminator: &[22],
    },
    InstructionSpec {
        name: "create_token_owner_record",
        discriminator: &[23],
    },
    InstructionSpec {
        name: "complete_proposal",
        discriminator: &[28],
    },
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
    "create_customizable_pool",
    "create_pool",
    "create_support_mint_associated",
    "decrease_limit_order",
    "decrease_liquidity",
    "decrease_liquidity_v2",
    "increase_liquidity",
    "increase_liquidity_v2",
    "initialize_reward",
    "open_position",
    "open_position_v2",
    "open_position_with_token22_nft",
    "close_limit_order",
    "open_limit_order",
    "set_reward_params",
    "settle_limit_order",
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
    "create_customizable_pool",
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
    "create_amm_config",
    "set_reward_params",
    "transfer_reward_owner",
    "update_amm_config",
    "update_pool_status",
    "update_reward_infos",
];

const RAYDIUM_CPMM_INSTRUCTIONS: &[&str] = &[
    "collect_creator_fee",
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
    "collect_creator_fee",
    "collect_fund_fee",
    "collect_protocol_fee",
    "create_amm_config",
    "initialize",
    "update_amm_config",
    "update_pool_status",
];

const RAYDIUM_CPMM_HIGH_IMPACT_INSTRUCTIONS: &[&str] = &[
    "create_amm_config",
    "update_amm_config",
    "update_pool_status",
];

const RAYDIUM_AMM_V4_INSTRUCTIONS: &[InstructionSpec] = &[
    InstructionSpec {
        name: "initialize",
        discriminator: &[0],
    },
    InstructionSpec {
        name: "initialize2",
        discriminator: &[1],
    },
    InstructionSpec {
        name: "monitorStep",
        discriminator: &[2],
    },
    InstructionSpec {
        name: "deposit",
        discriminator: &[3],
    },
    InstructionSpec {
        name: "withdraw",
        discriminator: &[4],
    },
    InstructionSpec {
        name: "migrateToOpenBook",
        discriminator: &[5],
    },
    InstructionSpec {
        name: "setParams",
        discriminator: &[6],
    },
    InstructionSpec {
        name: "withdrawPnl",
        discriminator: &[7],
    },
    InstructionSpec {
        name: "withdrawSrm",
        discriminator: &[8],
    },
    InstructionSpec {
        name: "swapBaseIn",
        discriminator: &[9],
    },
    InstructionSpec {
        name: "preInitialize",
        discriminator: &[10],
    },
    InstructionSpec {
        name: "swapBaseOut",
        discriminator: &[11],
    },
    InstructionSpec {
        name: "simulateInfo",
        discriminator: &[12],
    },
    InstructionSpec {
        name: "adminCancelOrders",
        discriminator: &[13],
    },
    InstructionSpec {
        name: "createConfigAccount",
        discriminator: &[14],
    },
    InstructionSpec {
        name: "updateConfigAccount",
        discriminator: &[15],
    },
    InstructionSpec {
        name: "swapBaseInV2",
        discriminator: &[16],
    },
    InstructionSpec {
        name: "swapBaseOutV2",
        discriminator: &[17],
    },
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
        slug: "bags",
        name: "Bags",
        program_label: "Fee Share V2",
        program_id: "FEE2tBhCKAt7shrod19QttSVREUYPiyMzoku1mL1gqVK",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: Some(InstructionCatalogSpec::Standard(
            BAGS_FEE_SHARE_V2_INSTRUCTIONS,
        )),
        admin_instructions: BAGS_FEE_SHARE_V2_ADMIN_INSTRUCTIONS,
        high_impact_instructions: BAGS_FEE_SHARE_V2_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "bags",
        name: "Bags",
        program_label: "Fee Share V1 (legacy)",
        program_id: "FEEhPbKVKnco9EXnaY3i4R5rQVUx91wgVfu8qokixywi",
        instruction_name_style: InstructionNameStyle::SnakeCase,
        instruction_catalog: Some(InstructionCatalogSpec::Standard(
            BAGS_FEE_SHARE_V1_INSTRUCTIONS,
        )),
        admin_instructions: BAGS_FEE_SHARE_V1_ADMIN_INSTRUCTIONS,
        high_impact_instructions: BAGS_FEE_SHARE_V1_HIGH_IMPACT_INSTRUCTIONS,
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
        program_id: "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK",
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
        program_id: "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C",
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
        program_id: "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8",
        instruction_name_style: InstructionNameStyle::CamelCase,
        instruction_catalog: Some(InstructionCatalogSpec::Explicit(
            RAYDIUM_AMM_V4_INSTRUCTIONS,
        )),
        admin_instructions: RAYDIUM_AMM_V4_ADMIN_INSTRUCTIONS,
        high_impact_instructions: RAYDIUM_AMM_V4_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "raydium",
        name: "Raydium",
        program_label: "CLMM (devnet)",
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
        program_label: "CPMM (devnet)",
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
        program_label: "Legacy AMM v4 (devnet)",
        program_id: "DRaya7Kj3aMWQSy19kSjvmuwq9docCHofyP9kanQGaav",
        instruction_name_style: InstructionNameStyle::CamelCase,
        instruction_catalog: Some(InstructionCatalogSpec::Explicit(
            RAYDIUM_AMM_V4_INSTRUCTIONS,
        )),
        admin_instructions: RAYDIUM_AMM_V4_ADMIN_INSTRUCTIONS,
        high_impact_instructions: RAYDIUM_AMM_V4_HIGH_IMPACT_INSTRUCTIONS,
        unknown_alerts: true,
    },
    ProtocolSpec {
        slug: "raydium",
        name: "Raydium",
        program_label: "Legacy Liquidity Pool v2 (old)",
        program_id: "27haf8L6oxUeXrHrgEgsexjSY5hbVUWEmvv9Nyxg8vQv",
        instruction_name_style: InstructionNameStyle::CamelCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
    },
    ProtocolSpec {
        slug: "raydium",
        name: "Raydium",
        program_label: "Legacy Farm/Staking v3",
        program_id: "EhhTKczWMGQt46ynNeRX1WfeagwwJd7ufHvCDjRxjo5Q",
        instruction_name_style: InstructionNameStyle::CamelCase,
        instruction_catalog: None,
        admin_instructions: &[],
        high_impact_instructions: &[],
        unknown_alerts: false,
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
                related_multisigs: Vec::new(),
                related_governances: Vec::new(),
            });
        }
    }

    descriptors.extend(project_protocol_descriptors());
    append_defillama_solana_protocol_descriptors(&mut descriptors);

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

pub fn bonk_dao_protocol_match() -> DefiProtocolMatch {
    DefiProtocolMatch {
        slug: BONK_DAO_PROTOCOL_SLUG.to_owned(),
        name: "Bonk DAO".to_owned(),
        program_id: Pubkey::from_str(BONK_DAO_GOVERNANCE_PROGRAM_ID)
            .expect("valid Bonk DAO governance program id"),
        program_label: "SPL Governance".to_owned(),
    }
}

pub fn protocol_for_realms_governance_account(address: &str) -> Option<DefiProtocolMatch> {
    let address = address.trim();
    if [
        BONK_DAO_REALM_ADDRESS,
        BONK_DAO_GOVERNANCE_ADDRESS,
        BONK_DAO_COMMUNITY_MINT_ADDRESS,
        BONK_DAO_COUNCIL_MINT_ADDRESS,
    ]
    .contains(&address)
    {
        return Some(bonk_dao_protocol_match());
    }

    None
}

pub fn treasury_token_watches() -> &'static [TreasuryTokenWatch] {
    &[TreasuryTokenWatch {
        protocol_slug: BONK_DAO_PROTOCOL_SLUG,
        protocol_name: "Bonk DAO",
        treasury_wallet: BONK_DAO_TREASURY_WALLET,
        token_mint: BONK_DAO_COMMUNITY_MINT_ADDRESS,
        token_symbol: BONK_TOKEN_SYMBOL,
        token_decimals: BONK_TOKEN_DECIMALS,
        critical_outflow_raw_amount: BONK_DAO_CRITICAL_OUTFLOW_RAW_AMOUNT,
    }]
}

pub fn watched_treasury_addresses() -> Vec<&'static str> {
    treasury_token_watches()
        .iter()
        .map(|watch| watch.treasury_wallet)
        .collect()
}

pub fn treasury_token_watch_for_transfer(
    authority: &str,
    mint: &str,
    raw_amount: u64,
) -> Option<&'static TreasuryTokenWatch> {
    treasury_token_watches().iter().find(|watch| {
        watch.treasury_wallet == authority
            && watch.token_mint == mint
            && raw_amount >= watch.critical_outflow_raw_amount
    })
}

pub fn realms_governance_program_ids() -> Vec<Pubkey> {
    definitions()
        .iter()
        .filter(|definition| definition.slug == REALMS_PROTOCOL_SLUG)
        .map(|definition| definition.program_id)
        .collect()
}

pub fn protocol_slug_for_realm_name(name: &str) -> Option<String> {
    let normalized = normalize_protocol_key(name);
    if normalized.is_empty() {
        return None;
    }

    let alias = match normalized.as_str() {
        "bonk" | "bonkdao" | "bonk_dao" => Some(BONK_DAO_PROTOCOL_SLUG.to_owned()),
        _ => None,
    };
    if alias.is_some() {
        return alias;
    }

    let normalized_without_dao = normalized
        .strip_suffix("_dao")
        .unwrap_or(&normalized)
        .to_owned();

    curated_protocol_descriptors()
        .into_iter()
        .filter(|descriptor| {
            descriptor.slug != REALMS_PROTOCOL_SLUG && descriptor.slug != SQUADS_PROTOCOL_SLUG
        })
        .find_map(|descriptor| {
            let descriptor_name = normalize_protocol_key(&descriptor.name);
            let descriptor_name_without_dao = descriptor_name
                .strip_suffix("_dao")
                .unwrap_or(&descriptor_name)
                .to_owned();

            if descriptor.slug == normalized
                || descriptor_name == normalized
                || descriptor.slug == normalized_without_dao
                || descriptor_name_without_dao == normalized_without_dao
            {
                Some(descriptor.slug)
            } else {
                None
            }
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
    vec![
        bonk_dao_protocol_descriptor(),
        DefiProtocolDescriptor {
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
            related_multisigs: Vec::new(),
            related_governances: Vec::new(),
        },
    ]
}

fn bonk_dao_protocol_descriptor() -> DefiProtocolDescriptor {
    let program_id = Pubkey::from_str(BONK_DAO_GOVERNANCE_PROGRAM_ID)
        .expect("valid Bonk DAO governance program id");

    DefiProtocolDescriptor {
        slug: BONK_DAO_PROTOCOL_SLUG.to_owned(),
        name: "Bonk DAO".to_owned(),
        program_id: BONK_DAO_GOVERNANCE_PROGRAM_ID.to_owned(),
        programs: vec![DefiProtocolProgramDescriptor {
            label: "SPL Governance".to_owned(),
            program_id: BONK_DAO_GOVERNANCE_PROGRAM_ID.to_owned(),
            idl_metadata_address: program_metadata::canonical_idl_metadata_address(program_id)
                .to_string(),
            anchor_legacy_idl_address: anchor_idl::legacy_idl_address(program_id)
                .map(|address| address.to_string()),
            risk_summary: Default::default(),
        }],
        sol_domains: Vec::new(),
        coverage: ProtocolCoverage::AdminSignals,
        catalog_instruction_count: SPL_GOVERNANCE_SIGNAL_INSTRUCTIONS.len(),
        admin_instruction_filters: BONK_DAO_GOVERNANCE_INSTRUCTION_FILTERS
            .iter()
            .map(|instruction| (*instruction).to_owned())
            .collect(),
        high_impact_instruction_filters: BONK_DAO_GOVERNANCE_INSTRUCTION_FILTERS
            .iter()
            .map(|instruction| (*instruction).to_owned())
            .collect(),
        amount_triggers: amount_trigger_descriptors(BONK_DAO_PROTOCOL_SLUG),
        unknown_alerts: false,
        related_multisigs: Vec::new(),
        related_governances: Vec::new(),
    }
}

fn append_defillama_solana_protocol_descriptors(descriptors: &mut Vec<DefiProtocolDescriptor>) {
    let mut seen_slugs = descriptors
        .iter()
        .map(|descriptor| descriptor.slug.clone())
        .collect::<HashSet<_>>();

    for protocol in defillama_solana_protocols() {
        let slug = normalize_protocol_key(&protocol.slug);
        if slug.is_empty() || !seen_slugs.insert(slug.clone()) {
            continue;
        }

        descriptors.push(DefiProtocolDescriptor {
            slug,
            name: protocol.name.clone(),
            program_id: String::new(),
            programs: Vec::new(),
            sol_domains: Vec::new(),
            coverage: ProtocolCoverage::AddressOnly,
            catalog_instruction_count: 0,
            admin_instruction_filters: Vec::new(),
            high_impact_instruction_filters: Vec::new(),
            amount_triggers: Vec::new(),
            unknown_alerts: false,
            related_multisigs: Vec::new(),
            related_governances: Vec::new(),
        });
    }
}

fn defillama_solana_protocols() -> &'static [DefiLlamaSolanaProtocol] {
    static DEFILLAMA_SOLANA_PROTOCOLS: OnceLock<Vec<DefiLlamaSolanaProtocol>> = OnceLock::new();
    DEFILLAMA_SOLANA_PROTOCOLS.get_or_init(|| {
        let snapshot: DefiLlamaSolanaSnapshot = serde_json::from_str(include_str!(
            "../data/defillama_solana_protocols_snapshot.json"
        ))
        .expect("valid DefiLlama Solana protocol snapshot");

        snapshot.items
    })
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
        BONK_DAO_PROTOCOL_SLUG => vec![AmountTriggerDescriptor {
            label: "Treasury outflow".to_owned(),
            description:
                "Critical alert when the Bonk DAO treasury sends at least 1T BONK in one transfer."
                    .to_owned(),
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

    if definition.slug == REALMS_PROTOCOL_SLUG {
        if let Some((raw_name, discriminator)) = parse_spl_governance_instruction(data) {
            return Some(build_signal_with_high_impact(
                definition,
                Some(raw_name),
                Some(ProtocolSignalKind::Admin),
                discriminator,
                true,
            ));
        }

        return None;
    }

    if is_raydium_amm_v4_definition(definition) {
        if let Some((raw_name, discriminator)) = parse_raydium_amm_v4_instruction(data) {
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

        if definition.unknown_alerts {
            return Some(build_signal(
                definition,
                None,
                Some(ProtocolSignalKind::Unknown),
                unknown_discriminator_bytes(data).as_slice(),
            ));
        }

        return None;
    }

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

fn parse_spl_governance_instruction(data: &[u8]) -> Option<(&'static str, &'static [u8])> {
    let tag = data.first()?;
    SPL_GOVERNANCE_SIGNAL_INSTRUCTIONS
        .iter()
        .find(|instruction| instruction.discriminator == [*tag])
        .map(|instruction| (instruction.name, instruction.discriminator))
}

fn is_raydium_amm_v4_definition(definition: &ProtocolDefinition) -> bool {
    definition.slug == "raydium" && definition.program_label.starts_with("Legacy AMM v4")
}

fn parse_raydium_amm_v4_instruction(data: &[u8]) -> Option<(&'static str, &'static [u8])> {
    let (&tag, rest) = data.split_first()?;
    if !is_valid_raydium_amm_v4_payload(tag, rest) {
        return None;
    }

    RAYDIUM_AMM_V4_INSTRUCTIONS
        .iter()
        .find(|instruction| instruction.discriminator == [tag])
        .map(|instruction| (instruction.name, instruction.discriminator))
}

fn is_valid_raydium_amm_v4_payload(tag: u8, rest: &[u8]) -> bool {
    match tag {
        0 => rest.len() >= 9,
        1 => rest.len() >= 25,
        2 => rest.len() >= 6,
        3 => rest.len() >= 24,
        4 => rest.len() >= 8,
        5 => true,
        6 => is_valid_raydium_amm_v4_set_params_payload(rest),
        7 => true,
        8 => rest.len() >= 8,
        9 => rest.len() >= 16,
        10 => rest.len() >= 1,
        11 => rest.len() >= 16,
        12 => is_valid_raydium_amm_v4_simulate_payload(rest),
        13 => rest.len() >= 2,
        14 => true,
        15 => is_valid_raydium_amm_v4_update_config_payload(rest),
        16 => rest.len() >= 16,
        17 => rest.len() >= 16,
        _ => false,
    }
}

fn is_valid_raydium_amm_v4_set_params_payload(rest: &[u8]) -> bool {
    let Some((&param, value)) = rest.split_first() else {
        return false;
    };

    match param {
        9 => value.len() >= 64,
        10 => value.len() >= 32,
        12 => value.len() >= 16,
        0..=8 | 11 | 13..=17 => value.len() >= 8,
        _ => false,
    }
}

fn is_valid_raydium_amm_v4_simulate_payload(rest: &[u8]) -> bool {
    let Some((&param, value)) = rest.split_first() else {
        return false;
    };

    match param {
        0 | 3 => true,
        1 | 2 => value.len() >= 16,
        _ => false,
    }
}

fn is_valid_raydium_amm_v4_update_config_payload(rest: &[u8]) -> bool {
    let Some((&param, value)) = rest.split_first() else {
        return false;
    };

    match param {
        0 | 1 => value.len() >= 32,
        2 => value.len() >= 8,
        _ => false,
    }
}

fn build_signal(
    definition: &ProtocolDefinition,
    raw_instruction_name: Option<&'static str>,
    signal_kind: Option<ProtocolSignalKind>,
    discriminator: &[u8],
) -> ProtocolInstructionSignal {
    let high_impact = raw_instruction_name
        .map(|name| definition.high_impact_instructions.contains(&name))
        .unwrap_or(true);

    build_signal_with_high_impact(
        definition,
        raw_instruction_name,
        signal_kind,
        discriminator,
        high_impact,
    )
}

fn build_signal_with_high_impact(
    definition: &ProtocolDefinition,
    raw_instruction_name: Option<&'static str>,
    signal_kind: Option<ProtocolSignalKind>,
    discriminator: &[u8],
    high_impact: bool,
) -> ProtocolInstructionSignal {
    let instruction_name = raw_instruction_name
        .map(|name| canonical_instruction_name(name, definition.instruction_name_style));

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

        for protocol in onchain_presence_protocols() {
            for program in &protocol.programs {
                let Ok(program_id) = Pubkey::from_str(&program.program_id) else {
                    continue;
                };

                if !seen_programs.insert(program_id) {
                    continue;
                }

                definitions.push(ProtocolDefinition {
                    slug: normalize_protocol_key(&protocol.slug),
                    name: protocol.name.clone(),
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

fn onchain_presence_protocols() -> &'static [OnchainPresenceProtocol] {
    static ONCHAIN_PRESENCE_PROTOCOLS: OnceLock<Vec<OnchainPresenceProtocol>> = OnceLock::new();
    ONCHAIN_PRESENCE_PROTOCOLS.get_or_init(|| {
        let snapshot: OnchainPresenceSnapshot = serde_json::from_str(include_str!(
            "../data/solana_onchain_presence_snapshot.json"
        ))
        .expect("valid Solana on-chain presence snapshot");

        snapshot.items
    })
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
    fn ignores_raydium_amm_v4_swap_base_in_v2_as_non_admin() {
        let program_id = Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &[16, 142, 19, 95, 0, 0, 0, 0, 0, 3, 210, 13, 0, 0, 0, 0, 0],
        )
        .expect("known Raydium AMM v4 swapBaseInV2 instruction");

        assert_eq!(signal.protocol.slug, "raydium");
        assert!(signal.signal_kind.is_none());
        assert_eq!(signal.instruction_name.as_deref(), Some("swap_base_in_v2"));
        assert!(!signal.high_impact);
        assert_eq!(signal.discriminator_hex, "10");
    }

    #[test]
    fn ignores_historical_raydium_amm_v4_swap_base_in_v2_cpi_payloads() {
        let program_id = Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap();
        let payloads: &[&[u8]] = &[
            &[16, 207, 75, 193, 69, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            &[16, 8, 230, 194, 66, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            &[16, 178, 148, 26, 36, 45, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        ];

        for payload in payloads {
            let signal = resolve_protocol_instruction(program_id, payload)
                .expect("known historical Raydium AMM v4 swapBaseInV2 CPI");

            assert!(signal.signal_kind.is_none());
            assert_eq!(signal.instruction_name.as_deref(), Some("swap_base_in_v2"));
            assert!(!signal.high_impact);
            assert_eq!(signal.discriminator_hex, "10");
        }
    }

    #[test]
    fn ignores_raydium_amm_v4_swap_base_out_as_non_admin() {
        let program_id = Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &[11, 128, 150, 152, 0, 0, 0, 0, 0, 64, 66, 15, 0, 0, 0, 0, 0],
        )
        .expect("known Raydium AMM v4 swapBaseOut instruction");

        assert_eq!(signal.protocol.slug, "raydium");
        assert!(signal.signal_kind.is_none());
        assert_eq!(signal.instruction_name.as_deref(), Some("swap_base_out"));
        assert!(!signal.high_impact);
        assert_eq!(signal.discriminator_hex, "0b");
    }

    #[test]
    fn classifies_raydium_amm_v4_set_params_as_high_impact_admin() {
        let program_id = Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap();
        let signal =
            resolve_protocol_instruction(program_id, &[6, 11, 128, 150, 152, 0, 0, 0, 0, 0])
                .expect("known Raydium AMM v4 setParams instruction");

        assert_eq!(signal.protocol.slug, "raydium");
        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Admin));
        assert_eq!(signal.instruction_name.as_deref(), Some("set_params"));
        assert!(signal.high_impact);
        assert_eq!(signal.discriminator_hex, "06");
    }

    #[test]
    fn keeps_raydium_amm_v4_withdraw_pnl_as_non_high_impact_admin() {
        let program_id = Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap();
        let signal = resolve_protocol_instruction(program_id, &[7])
            .expect("known Raydium AMM v4 withdrawPnl");

        assert_eq!(signal.protocol.slug, "raydium");
        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Admin));
        assert_eq!(signal.instruction_name.as_deref(), Some("withdraw_pnl"));
        assert!(!signal.high_impact);
        assert_eq!(signal.discriminator_hex, "07");
    }

    #[test]
    fn keeps_raydium_clmm_protocol_fee_collection_as_non_high_impact_admin() {
        let program_id = Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name("collect_protocol_fee", InstructionNameStyle::SnakeCase),
        )
        .expect("known Raydium CLMM protocol fee collection");

        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Admin));
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("collect_protocol_fee")
        );
        assert!(!signal.high_impact);
    }

    #[test]
    fn keeps_raydium_cpmm_protocol_fee_collection_as_non_high_impact_admin() {
        let program_id = Pubkey::from_str("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name("collect_protocol_fee", InstructionNameStyle::SnakeCase),
        )
        .expect("known Raydium CPMM protocol fee collection");

        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Admin));
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("collect_protocol_fee")
        );
        assert!(!signal.high_impact);
    }

    #[test]
    fn ignores_raydium_clmm_open_limit_order_as_known_non_admin() {
        let program_id = Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &[
                0x9d, 0x20, 0xda, 0xb7, 0x47, 0x1d, 0x12, 0x93, 0x00, 0x01, 0x9c, 0x09, 0x00, 0x00,
                0x5f, 0x8f, 0x59, 0xd0, 0x00, 0x00, 0x00, 0x00,
            ],
        )
        .expect("known Raydium CLMM open limit order");

        assert_eq!(signal.protocol.slug, "raydium");
        assert!(signal.signal_kind.is_none());
        assert_eq!(signal.instruction_name.as_deref(), Some("open_limit_order"));
        assert!(!signal.high_impact);
        assert_eq!(signal.discriminator_hex, "9d20dab7471d1293");
    }

    #[test]
    fn ignores_raydium_clmm_close_limit_order_as_known_non_admin() {
        let program_id = Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &[
                0x4c, 0x7c, 0x80, 0x0f, 0xd5, 0x57, 0x25, 0xfa, 0x00, 0x01, 0x9c, 0x09, 0x00,
            ],
        )
        .expect("known Raydium CLMM close limit order");

        assert_eq!(signal.protocol.slug, "raydium");
        assert!(signal.signal_kind.is_none());
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("close_limit_order")
        );
        assert!(!signal.high_impact);
        assert_eq!(signal.discriminator_hex, "4c7c800fd55725fa");
    }

    #[test]
    fn keeps_raydium_clmm_customizable_pool_creation_as_non_high_impact_admin() {
        let program_id = Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &[0x2b, 0x44, 0xd4, 0xa7, 0x59, 0x2f, 0xa4, 0x01],
        )
        .expect("known Raydium CLMM customizable pool creation");

        assert_eq!(signal.protocol.slug, "raydium");
        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Admin));
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("create_customizable_pool")
        );
        assert!(!signal.high_impact);
        assert_eq!(signal.discriminator_hex, "2b44d4a7592fa401");
    }

    #[test]
    fn ignores_raydium_clmm_decrease_limit_order_as_known_non_admin() {
        let program_id = Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &[0x75, 0x9d, 0x3c, 0x67, 0x42, 0x31, 0xa3, 0x00],
        )
        .expect("known Raydium CLMM decrease limit order");

        assert_eq!(signal.protocol.slug, "raydium");
        assert!(signal.signal_kind.is_none());
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("decrease_limit_order")
        );
        assert!(!signal.high_impact);
        assert_eq!(signal.discriminator_hex, "759d3c674231a300");
    }

    #[test]
    fn ignores_raydium_clmm_settle_limit_order_as_known_non_admin() {
        let program_id = Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &[0xcd, 0x4e, 0x74, 0x21, 0x5c, 0x69, 0x1a, 0x60, 0x00, 0x00],
        )
        .expect("known Raydium CLMM settle limit order");

        assert_eq!(signal.protocol.slug, "raydium");
        assert!(signal.signal_kind.is_none());
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("settle_limit_order")
        );
        assert!(!signal.high_impact);
        assert_eq!(signal.discriminator_hex, "cd4e74215c691a60");
    }

    #[test]
    fn keeps_raydium_cpmm_creator_fee_collection_as_non_high_impact_admin() {
        let program_id = Pubkey::from_str("CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name("collect_creator_fee", InstructionNameStyle::SnakeCase),
        )
        .expect("known Raydium CPMM creator fee collection");

        assert_eq!(signal.protocol.slug, "raydium");
        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Admin));
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("collect_creator_fee")
        );
        assert!(!signal.high_impact);
        assert_eq!(signal.discriminator_hex, "1416567bc61cdb84");
    }

    #[test]
    fn classifies_malformed_raydium_amm_v4_known_tag_as_unknown() {
        let program_id = Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap();
        let signal =
            resolve_protocol_instruction(program_id, &[16, 1, 2, 3]).expect("malformed swap tag");

        assert_eq!(signal.protocol.slug, "raydium");
        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Unknown));
        assert!(signal.instruction_name.is_none());
        assert_eq!(signal.discriminator_hex, "10010203");
    }

    #[test]
    fn classifies_unknown_raydium_amm_v4_tag_as_unknown() {
        let program_id = Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8").unwrap();
        let signal = resolve_protocol_instruction(program_id, &[99, 1, 2, 3]).expect("unknown tag");

        assert_eq!(signal.protocol.slug, "raydium");
        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Unknown));
        assert!(signal.instruction_name.is_none());
        assert_eq!(signal.discriminator_hex, "63010203");
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
    fn keeps_jupiter_lend_liquidity_price_refresh_as_non_high_impact_admin() {
        let program_id = Pubkey::from_str("jupeiUmn818Jg1ekPURTpr4mFo29p46vygyykFJ3wZC").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name("update_exchange_price", InstructionNameStyle::SnakeCase),
        )
        .expect("known Jupiter Lend liquidity price refresh instruction");

        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Admin));
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("update_exchange_price")
        );
        assert!(!signal.high_impact);
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
    fn includes_bags_fee_share_protocol_descriptor() {
        let bags = curated_protocol_descriptors()
            .into_iter()
            .find(|protocol| protocol.slug == "bags")
            .expect("bags descriptor");

        assert_eq!(bags.name, "Bags");
        assert_eq!(bags.coverage, ProtocolCoverage::DeepDecode);
        assert_eq!(bags.catalog_instruction_count, 31);
        assert_eq!(bags.programs.len(), 2);
        assert!(
            bags.programs
                .iter()
                .any(|program| program.program_id == "FEE2tBhCKAt7shrod19QttSVREUYPiyMzoku1mL1gqVK")
        );
        assert!(
            bags.programs
                .iter()
                .any(|program| program.program_id == "FEEhPbKVKnco9EXnaY3i4R5rQVUx91wgVfu8qokixywi")
        );
        assert!(
            bags.admin_instruction_filters
                .iter()
                .any(|instruction| instruction == "update_program_config")
        );
    }

    #[test]
    fn classifies_bags_fee_share_admin_instructions() {
        let program_id = Pubkey::from_str("FEE2tBhCKAt7shrod19QttSVREUYPiyMzoku1mL1gqVK").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name("update_program_config", InstructionNameStyle::SnakeCase),
        )
        .expect("known Bags admin instruction");

        assert_eq!(signal.protocol.slug, "bags");
        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Admin));
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("update_program_config")
        );
        assert!(signal.high_impact);
    }

    #[test]
    fn keeps_bags_fee_claim_as_non_high_impact_admin() {
        let program_id = Pubkey::from_str("FEE2tBhCKAt7shrod19QttSVREUYPiyMzoku1mL1gqVK").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name("claim_platform_fees", InstructionNameStyle::SnakeCase),
        )
        .expect("known Bags fee claim instruction");

        assert_eq!(signal.protocol.slug, "bags");
        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Admin));
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("claim_platform_fees")
        );
        assert!(!signal.high_impact);
    }

    #[test]
    fn ignores_bags_user_claim_as_non_admin() {
        let program_id = Pubkey::from_str("FEE2tBhCKAt7shrod19QttSVREUYPiyMzoku1mL1gqVK").unwrap();
        let signal = resolve_protocol_instruction(
            program_id,
            &discriminator_for_name("claim_dbc", InstructionNameStyle::SnakeCase),
        )
        .expect("known Bags user claim instruction");

        assert_eq!(signal.protocol.slug, "bags");
        assert!(signal.signal_kind.is_none());
        assert_eq!(signal.instruction_name.as_deref(), Some("claim_dbc"));
    }

    #[test]
    fn imports_defillama_solana_protocols() {
        let collector = curated_protocol_descriptors()
            .into_iter()
            .find(|protocol| protocol.slug == "collector_crypt")
            .expect("collector crypt descriptor from DefiLlama Solana snapshot");

        assert_eq!(collector.name, "Collector Crypt");
        assert_eq!(collector.coverage, ProtocolCoverage::AddressOnly);
        assert!(collector.programs.is_empty());
    }

    #[test]
    fn imports_discovered_solana_onchain_presence_programs() {
        let alphaq = curated_protocol_descriptors()
            .into_iter()
            .find(|protocol| protocol.slug == "alphaq")
            .expect("alphaq descriptor from on-chain presence snapshot");

        assert_eq!(alphaq.name, "AlphaQ");
        assert_eq!(alphaq.coverage, ProtocolCoverage::AddressOnly);
        assert!(
            alphaq
                .programs
                .iter()
                .any(|program| program.program_id == "ALPHAQmeA7bjrVuccPsYPiCvsi428SNwte66Srvs4pHA")
        );

        let program = Pubkey::from_str("ALPHAQmeA7bjrVuccPsYPiCvsi428SNwte66Srvs4pHA").unwrap();
        let matched = protocol_for_program(program).expect("alphaq program match");
        assert_eq!(matched.slug, "alphaq");
        assert_eq!(matched.name, "AlphaQ");
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
        let program_id = Pubkey::from_str("CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK").unwrap();
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
    fn includes_bonk_dao_protocol_descriptor() {
        let bonk_dao = curated_protocol_descriptors()
            .into_iter()
            .find(|protocol| protocol.slug == BONK_DAO_PROTOCOL_SLUG)
            .expect("Bonk DAO protocol descriptor");

        assert_eq!(bonk_dao.name, "Bonk DAO");
        assert_eq!(bonk_dao.coverage, ProtocolCoverage::AdminSignals);
        assert_eq!(bonk_dao.programs.len(), 1);
        assert_eq!(
            bonk_dao.programs[0].program_id,
            BONK_DAO_GOVERNANCE_PROGRAM_ID
        );
        assert!(
            bonk_dao
                .admin_instruction_filters
                .iter()
                .any(|instruction| instruction == "execute_transaction")
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
    fn decodes_realms_execute_transaction_signal() {
        let program_id = Pubkey::from_str("GovER5Lthms3bLBqWub97yVrMmEogzX7xNjdXpPPCVZw").unwrap();
        let signal = resolve_protocol_instruction(program_id, &[16])
            .expect("realms execute transaction signal");

        assert_eq!(signal.protocol.slug, REALMS_PROTOCOL_SLUG);
        assert_eq!(
            signal.instruction_name.as_deref(),
            Some("execute_transaction")
        );
        assert_eq!(signal.signal_kind, Some(ProtocolSignalKind::Admin));
        assert!(signal.high_impact);
        assert_eq!(signal.discriminator_hex, "10");
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
