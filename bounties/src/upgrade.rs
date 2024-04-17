use crate::*;

// Contract state version 2.0.14
#[derive(BorshDeserialize, BorshSerialize)]
pub struct OldState {
  pub tokens: UnorderedMap<AccountId, TokenDetails>,
  pub last_bounty_id: BountyIndex,
  pub bounties: LookupMap<BountyIndex, VersionedBounty>,
  pub account_bounties: LookupMap<AccountId, Vec<BountyIndex>>,
  pub bounty_claimers: LookupMap<AccountId, Vec<VersionedBountyClaim>>,
  pub bounty_claimer_accounts: LookupMap<BountyIndex, Vec<AccountId>>,
  pub locked_amount: Balance,
  pub unlocked_amount: Balance,
  pub admins_whitelist: UnorderedSet<AccountId>,
  pub config: VersionedConfig,
  pub reputation_contract: Option<AccountId>,
  pub dispute_contract: Option<AccountId>,
  pub kyc_whitelist_contract: Option<AccountId>,
  pub owners_whitelist: UnorderedSet<AccountId>,
  pub postpaid_subscribers_whitelist: UnorderedSet<AccountId>,
  pub total_fees: LookupMap<AccountId, FeeStats>,
  pub total_validators_dao_fees: LookupMap<AccountId, Vec<DaoFeeStats>>,
  pub recipient_of_platform_fee: Option<AccountId>,
  pub status: ContractStatus,
}

#[near_bindgen]
impl BountiesContract {
  #[private]
  #[init(ignore_state)]
  pub fn migrate() -> Self {
    let old_state: OldState = env::state_read().expect("Old state doesn't exist");
    Self {
      tokens: old_state.tokens,
      last_bounty_id: old_state.last_bounty_id,
      last_claim_id: 0,
      bounties: old_state.bounties,
      account_bounties: old_state.account_bounties,
      claims: LookupMap::new(StorageKey::Claims),
      bounty_claimers: LookupMap::new(StorageKey::BountyClaimers),
      bounty_claims: LookupMap::new(StorageKey::BountyClaims),
      locked_amount: old_state.locked_amount,
      unlocked_amount: old_state.unlocked_amount,
      admins_whitelist: old_state.admins_whitelist,
      config: old_state.config,
      reputation_contract: old_state.reputation_contract,
      dispute_contract: old_state.dispute_contract,
      kyc_whitelist_contract: old_state.kyc_whitelist_contract,
      owners_whitelist: old_state.owners_whitelist,
      postpaid_subscribers_whitelist: old_state.postpaid_subscribers_whitelist,
      total_fees: old_state.total_fees,
      total_validators_dao_fees: old_state.total_validators_dao_fees,
      recipient_of_platform_fee: old_state.recipient_of_platform_fee,
      status: ContractStatus::Genesis,
    }
  }

  #[private]
  pub fn migrate_claims(&mut self, claims: Vec<BountyClaim>) {
    assert!(
      matches!(self.status, ContractStatus::Genesis),
      "This action is only possible if the contract status is Genesis"
    );

    for (_, bounty_claim) in claims.iter().enumerate() {
      let claims = self.get_bounty_claims(bounty_claim.owner.clone());
      let claim = claims.into_iter().find(
        |c|
          c.1.bounty_id == bounty_claim.bounty_id && c.1.claim_number == bounty_claim.claim_number
      );
      let claim_info = json!({
        "bounty_id": bounty_claim.bounty_id,
        "owner": bounty_claim.owner,
        "claim_number": bounty_claim.claim_number
      });

      if claim.is_none() {
        self.internal_add_claim(bounty_claim);

        env::log_str(format!("Claim {} added", claim_info).as_str());

      } else {
        env::log_str(format!("Claim {} skipped", claim_info).as_str());
      }
    }
  }
}
