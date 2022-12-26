use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::U64;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{AccountId, BorshStorageKey};

pub type DisputeIndex = u64;

pub const DEFAULT_ARGUMENT_PERIOD: U64 = U64(1_000_000_000 * 60 * 60 * 24 * 10);
pub const DEFAULT_DECISION_PERIOD: U64 = U64(1_000_000_000 * 60 * 60 * 24 * 7);

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
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
pub enum Side {
  Claimer,
  ProjectOwner,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Reason {
  side: Side,
  argument_timestamp: U64,
  pub description: String,
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
  /// Arguments of the parties
  pub arguments_of_participants: Vec<Reason>,
  /// Dispute resolution proposal ID
  pub proposal_id: Option<U64>,
  /// The timestamp when the proposal was created
  pub proposal_timestamp: Option<U64>,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct Config {
  /// The period of presentation of arguments by the parties after the opening of the dispute
  pub argument_period: U64,
  /// The period for the decision of the DAO after submission of arguments
  pub decision_period: U64,
}

impl Default for Config {
  fn default() -> Self {
    Config {
      argument_period: DEFAULT_ARGUMENT_PERIOD,
      decision_period: DEFAULT_DECISION_PERIOD,
    }
  }
}

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
  Disputes,
  BountyDisputes,
  AdminWhitelist,
}
