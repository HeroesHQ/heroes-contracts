use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{U128, U64};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, ext_contract, AccountId, Balance, BorshStorageKey, Gas, ONE_NEAR,
               PromiseOrValue};

pub type DisputeIndex = u64;
pub type TimestampSec = u32;

pub const GAS_FOR_ADD_PROPOSAL: Gas = Gas(25_000_000_000_000);
pub const GAS_FOR_ON_ADDED_PROPOSAL_CALLBACK: Gas = Gas(10_000_000_000_000);
pub const GAS_FOR_CLAIM_APPROVAL: Gas = Gas(100_000_000_000_000);
pub const GAS_FOR_AFTER_CLAIM_APPROVAL: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_SEND_RESULT_OF_DISPUTE: Gas = Gas(70_000_000_000_000);
pub const GAS_FOR_CHECK_PROPOSAL: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_AFTER_CHECK_PROPOSAL: Gas = Gas(100_000_000_000_000);

pub const DEFAULT_ARGUMENT_PERIOD: U64 = U64(1_000_000_000 * 60 * 60 * 24 * 10);
pub const DEFAULT_DECISION_PERIOD: U64 = U64(1_000_000_000 * 60 * 60 * 24 * 7);
pub const DEFAULT_ADD_PROPOSAL_BOND: U128 = U128(ONE_NEAR);

pub const NO_DEPOSIT: Balance = 0;
pub const MESSAGE_DISPUTE_IS_NOT_NEW: &str = "This action can be performed only for a dispute with the status 'New'";
pub const MESSAGE_DISPUTE_IS_NOT_PENDING: &str = "This action can be performed only for a dispute with the status 'DecisionPending'";

#[ext_contract(ext_bounty_contract)]
trait ExtBountyContract {
  fn dispute_result(&self, id: u64, claimer: AccountId, success: bool) -> PromiseOrValue<()>;
}

#[derive(Deserialize, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Proposal {
  pub id: u64,
  pub proposer: AccountId,
  pub description: String,
  pub status: String,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum DisputeStatus {
  New,
  DecisionPending,
  InFavorOfClaimer,
  InFavorOfProjectOwner,
  CanceledByClaimer,
  CanceledByProjectOwner,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub enum Side {
  Claimer,
  ProjectOwner,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct Reason {
  pub side: Side,
  pub argument_timestamp: U64,
  pub description: String,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct DisputeCreate {
  pub bounty_id: u64,
  pub description: String,
  pub claimer: AccountId,
  pub project_owner_delegate: AccountId,
}

impl DisputeCreate {
  pub fn to_dispute(&self) -> Dispute {
    Dispute {
      start_time: U64::from(env::block_timestamp()),
      description: self.description.clone(),
      bounty_id: self.bounty_id.into(),
      claimer: self.claimer.clone(),
      project_owner_delegate: self.project_owner_delegate.clone(),
      status: DisputeStatus::New,
      proposal_id: None,
      proposal_timestamp: None,
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct Dispute {
  /// Time to start the dispute
  pub start_time: U64,
  /// Description of the cause of the dispute, provided by the claimant
  pub description: String,
  /// ID bounty which became the cause of the dispute
  pub bounty_id: U64,
  /// Claimer account
  pub claimer: AccountId,
  /// Account of the project owner or validators DAO
  pub project_owner_delegate: AccountId,
  /// Dispute status
  pub status: DisputeStatus,
  /// Dispute resolution proposal ID
  pub proposal_id: Option<U64>,
  /// The timestamp when the proposal was created
  pub proposal_timestamp: Option<U64>,
}

impl Dispute {
  pub fn get_side_of_dispute(&self) -> Side {
    let sender = env::predecessor_account_id();
    if sender == self.claimer { Side::Claimer }
    else if sender == self.project_owner_delegate { Side::ProjectOwner }
    else {
      env::panic_str("You do not have access rights to perform this action");
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct Config {
  /// The period of presentation of arguments by the parties after the opening of the dispute
  pub argument_period: U64,
  /// The period for the decision of the DAO after submission of arguments
  pub decision_period: U64,
  /// Proposal bond
  pub add_proposal_bond: U128,
}

impl Default for Config {
  fn default() -> Self {
    Config {
      argument_period: DEFAULT_ARGUMENT_PERIOD,
      decision_period: DEFAULT_DECISION_PERIOD,
      add_proposal_bond: DEFAULT_ADD_PROPOSAL_BOND,
    }
  }
}

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
  Disputes,
  BountyDisputes,
  AdminWhitelist,
  Reasons,
}
