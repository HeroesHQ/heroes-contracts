use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{U128, U64};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, AccountId, Balance, BorshStorageKey, Gas, ONE_NEAR};

use crate::DisputesContract;

pub type DisputeIndex = u64;

pub const GAS_FOR_ADD_PROPOSAL: Gas = Gas(35_000_000_000_000);
pub const GAS_FOR_ON_ADDED_PROPOSAL_CALLBACK: Gas = Gas(10_000_000_000_000);
pub const GAS_FOR_CLAIM_APPROVAL: Gas = Gas(60_000_000_000_000);
pub const GAS_FOR_AFTER_CLAIM_APPROVAL: Gas = Gas(15_000_000_000_000);

pub const DEFAULT_ARGUMENT_PERIOD: U64 = U64(1_000_000_000 * 60 * 60 * 24 * 10);
pub const DEFAULT_DECISION_PERIOD: U64 = U64(1_000_000_000 * 60 * 60 * 24 * 7);
pub const DEFAULT_ADD_PROPOSAL_BOND: U128 = U128(ONE_NEAR);

pub const NO_DEPOSIT: Balance = 0;

#[derive(Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Proposal {
  pub id: u64,
  pub proposer: AccountId,
  pub status: String,
}

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
  pub side: Side,
  pub argument_timestamp: U64,
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

impl Dispute {
  pub fn add_argument(
    &mut self,
    new_reason: Reason,
  ) -> usize {
    self.arguments_of_participants.push(new_reason);
    self.arguments_of_participants.len() - 1
  }

  pub fn get_side_of_dispute(&self) -> Side {
    let sender = env::predecessor_account_id();
    if sender == self.claimer { Side::Claimer }
    else if sender == self.project_owner_delegate { Side::ProjectOwner }
    else {
      env::panic_str("You do not have access rights to perform this action");
    }
  }

  fn chunk_of_description(&self, reason: Reason) -> String {
    let mut chunk = "".to_string().to_owned();
    let (side_str, account_id) = match reason.side {
      Side::Claimer => ("Claimer", self.claimer.clone()),
      _ => ("Project owner", self.project_owner_delegate.clone()),
    };
    chunk.push_str("\n\n");
    chunk.push_str(DisputesContract::timestamp_to_text_date(reason.argument_timestamp).as_str());
    chunk.push_str(format!(" {} ({}):", side_str, account_id).as_str());
    chunk.push_str("\n");
    chunk.push_str(&reason.description);
    chunk
  }

  pub fn get_proposal_description(&self) -> String {
    let mut full_description = self.description.to_owned();
    for i in 0..self.arguments_of_participants.len() {
      let reason = self.arguments_of_participants[i].clone();
      full_description.push_str(self.chunk_of_description(reason).as_str());
    }
    full_description
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
}
