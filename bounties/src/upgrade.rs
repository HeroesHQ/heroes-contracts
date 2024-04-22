use crate::*;

// Contract state version 2.0.14
#[derive(BorshSerialize, BorshDeserialize)]
pub struct BountyClaimV1 {
  pub bounty_id: BountyIndex,
  pub created_at: U64,
  pub start_time: Option<U64>,
  pub deadline: U64,
  pub description: String,
  pub status: ClaimStatus,
  pub bounty_payout_proposal_id: Option<U64>,
  pub approve_claimer_proposal_id: Option<U64>,
  pub rejected_timestamp: Option<U64>,
  pub dispute_id: Option<U64>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct BountyClaimV2 {
  pub bounty_id: BountyIndex,
  pub created_at: U64,
  pub start_time: Option<U64>,
  pub deadline: U64,
  pub description: String,
  pub status: ClaimStatus,
  pub bounty_payout_proposal_id: Option<U64>,
  pub approve_claimer_proposal_id: Option<U64>,
  pub rejected_timestamp: Option<U64>,
  pub dispute_id: Option<U64>,
  pub is_kyc_delayed: Option<DefermentOfKYC>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct BountyClaimV3 {
  pub bounty_id: BountyIndex,
  pub created_at: U64,
  pub start_time: Option<U64>,
  pub deadline: U64,
  pub description: String,
  pub status: ClaimStatus,
  pub bounty_payout_proposal_id: Option<U64>,
  pub approve_claimer_proposal_id: Option<U64>,
  pub rejected_timestamp: Option<U64>,
  pub dispute_id: Option<U64>,
  pub is_kyc_delayed: Option<DefermentOfKYC>,
  pub payment_timestamps: Option<PaymentTimestamps>,
  pub slot: Option<usize>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct BountyClaimV4 {
  pub bounty_id: BountyIndex,
  pub created_at: U64,
  pub start_time: Option<U64>,
  pub deadline: U64,
  pub description: String,
  pub status: ClaimStatus,
  pub bounty_payout_proposal_id: Option<U64>,
  pub approve_claimer_proposal_id: Option<U64>,
  pub rejected_timestamp: Option<U64>,
  pub dispute_id: Option<U64>,
  pub is_kyc_delayed: Option<DefermentOfKYC>,
  pub payment_timestamps: Option<PaymentTimestamps>,
  pub slot: Option<usize>,
  pub bond: Option<U128>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct BountyClaimV5 {
  pub bounty_id: BountyIndex,
  pub created_at: U64,
  pub start_time: Option<U64>,
  pub deadline: Option<U64>,
  pub description: String,
  pub status: ClaimStatus,
  pub bounty_payout_proposal_id: Option<U64>,
  pub approve_claimer_proposal_id: Option<U64>,
  pub rejected_timestamp: Option<U64>,
  pub dispute_id: Option<U64>,
  pub is_kyc_delayed: Option<DefermentOfKYC>,
  pub payment_timestamps: Option<PaymentTimestamps>,
  pub slot: Option<usize>,
  pub bond: Option<U128>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct BountyClaimV6 {
  pub bounty_id: BountyIndex,
  pub created_at: U64,
  pub start_time: Option<U64>,
  pub deadline: Option<U64>,
  pub description: String,
  pub status: ClaimStatus,
  pub bounty_payout_proposal_id: Option<U64>,
  pub approve_claimer_proposal_id: Option<U64>,
  pub rejected_timestamp: Option<U64>,
  pub dispute_id: Option<U64>,
  pub is_kyc_delayed: Option<DefermentOfKYC>,
  pub payment_timestamps: Option<PaymentTimestamps>,
  pub slot: Option<usize>,
  pub bond: Option<U128>,
  pub claim_number: Option<u8>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub enum OldVersionedBountyClaim {
  V1(BountyClaimV1),
  V2(BountyClaimV2),
  V3(BountyClaimV3),
  V4(BountyClaimV4),
  V5(BountyClaimV5),
  Current(BountyClaimV6),
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct OldState {
  pub tokens: UnorderedMap<AccountId, TokenDetails>,
  pub last_bounty_id: BountyIndex,
  pub bounties: LookupMap<BountyIndex, VersionedBounty>,
  pub account_bounties: LookupMap<AccountId, Vec<BountyIndex>>,
  pub bounty_claimers: LookupMap<AccountId, Vec<OldVersionedBountyClaim>>,
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
      bounties: old_state.bounties,
      account_bounties: old_state.account_bounties,
      old_bounty_claimers: old_state.bounty_claimers,
      old_bounty_claimer_accounts: old_state.bounty_claimer_accounts,
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
      last_claim_id: 0,
      claims: LookupMap::new(StorageKey::Claims),
      bounty_claimers: LookupMap::new(StorageKey::BountyClaimers),
      bounty_claims: LookupMap::new(StorageKey::BountyClaims),
    }
  }

  #[private]
  pub fn migrate_claims(&mut self, claims: Vec<BountyClaim>) {
    assert!(
      matches!(self.status, ContractStatus::Genesis),
      "This action is only possible if the contract status is Genesis"
    );

    for (_, bounty_claim) in claims.iter().enumerate() {
      let claims = self.internal_get_claims_by_account_id_an_bounty_id(
        &bounty_claim.bounty_id,
        &bounty_claim.owner,
        true
      );
      let claim = self.internal_find_claim(
        &claims,
        bounty_claim.bounty_id.clone(),
        bounty_claim.owner.clone(),
        bounty_claim.claim_number.clone()
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

  #[private]
  pub fn clean_bounty_claimers(&mut self, claimers: Vec<AccountId>) {
    assert!(
      matches!(self.status, ContractStatus::Genesis),
      "This action is only possible if the contract status is Genesis"
    );

    for (_, account_id) in claimers.iter().enumerate() {
      self.old_bounty_claimers.remove(account_id);
    }
  }

  #[private]
  pub fn clean_bounty_claimer_accounts(&mut self, bounties: Vec<BountyIndex>) {
    assert!(
      matches!(self.status, ContractStatus::Genesis),
      "This action is only possible if the contract status is Genesis"
    );

    for (_, bounty_id) in bounties.iter().enumerate() {
      self.old_bounty_claimer_accounts.remove(bounty_id);
    }
  }
}
