use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{U128, U64};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, AccountId, BorshStorageKey, Gas};

pub type BountyIndex = u64;

pub const GAS_FOR_ADD_PROPOSAL: Gas = Gas(25_000_000_000_000);
pub const GAS_FOR_CLAIM_APPROVAL: Gas = Gas(50_000_000_000_000);

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum BountyStatus {
  New,
  Claimed,
  Completed,
  Disputed,
  Canceled,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum BountyType {
  MarketingServices,
  SoftwareDevelopment,
  Other,
  // etc.
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct ValidatorsDaoParams {
  pub account_id: AccountId,
  pub add_proposal_bond: U128,
  pub gas_for_add_proposal: Option<U64>,
  pub gas_for_claim_approval: Option<U64>,
}

impl ValidatorsDaoParams {
  pub fn to_validators_dao(&self) -> ValidatorsDao {
    let mut gas_for_add_proposal = U64(GAS_FOR_ADD_PROPOSAL.0);
    if self.gas_for_add_proposal.is_some() {
      gas_for_add_proposal = self.gas_for_add_proposal.unwrap();
    }
    let mut gas_for_claim_approval = U64(GAS_FOR_CLAIM_APPROVAL.0);
    if self.gas_for_claim_approval.is_some() {
      gas_for_claim_approval = self.gas_for_claim_approval.unwrap();
    }
    ValidatorsDao {
      account_id: self.account_id.clone(),
      add_proposal_bond: self.add_proposal_bond.clone(),
      gas_for_add_proposal,
      gas_for_claim_approval,
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct BountyCreate {
  pub description: String,
  pub bounty_type: BountyType,
  pub max_deadline: U64,
  pub validators_dao: Option<ValidatorsDaoParams>,
}

impl BountyCreate {
  pub fn to_bounty(&self, payer_id: &AccountId, token_id: &AccountId, amount: U128) -> Bounty {
    let mut validators_dao: Option<ValidatorsDao> = None;
    if self.validators_dao.is_some() {
      validators_dao = Some(self.validators_dao.clone().unwrap().to_validators_dao());
    }

    Bounty {
      description: self.description.clone(),
      token: token_id.clone(),
      amount: amount.clone(),
      bounty_type: self.bounty_type.clone(),
      max_deadline: self.max_deadline.clone(),
      validators_dao,
      owner: payer_id.clone(),
      status: BountyStatus::New,
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct ValidatorsDao {
  pub account_id: AccountId,
  pub add_proposal_bond: U128,
  pub gas_for_add_proposal: U64,
  pub gas_for_claim_approval: U64,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Bounty {
  pub description: String,
  pub token: AccountId,
  pub amount: U128,
  pub bounty_type: BountyType,
  pub max_deadline: U64,
  pub validators_dao: Option<ValidatorsDao>,
  pub owner: AccountId,
  pub status: BountyStatus,
}

impl Bounty {
  pub fn assert_new_valid(&self) {
    assert!(
      !self.description.is_empty(),
      "The description cannot be empty"
    );
    let bounty_amount = self.amount.0;
    assert!(
      bounty_amount > 0,
      "Expected bounty amount to be positive",
    );
    assert!(
      env::block_timestamp() > self.max_deadline.0,
      "The deadline has passed"
    );
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum ClaimStatus {
  New,
  Completed,
  Approved,
  Rejected,
  Canceled,
  Expired,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct BountyClaim {
  /// Bounty id that was claimed.
  pub bounty_id: BountyIndex,
  /// Start time of the claim.
  pub start_time: U64,
  /// Deadline specified by claimer.
  pub deadline: U64,
  /// status
  pub status: ClaimStatus,
  /// Bounty payout proposal ID
  pub proposal_id: Option<u64>,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum BountyAction {
  ClaimApproved { receiver_id: AccountId },
  ClaimRejected { receiver_id: AccountId },
  CreateDispute,
  Finalize,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Config {
  pub bounty_claim_bond: U128,
  pub bounty_forgiveness_period: U64,
}

impl Default for Config {
  fn default() -> Self {
    Config {
      bounty_claim_bond: U128::from(1_000_000_000_000_000_000_000_000),
      bounty_forgiveness_period: U64::from(1_000_000_000 * 60 * 60 * 24),
    }
  }
}

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
  AccountBounties,
  Bounties,
  BountyClaimerAccounts,
  BountyClaimers,
  DepositWhitelist,
  TokenAccountIds,
}
