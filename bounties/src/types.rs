use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{Base64VecU8, U128, U64};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, ext_contract, AccountId, Balance, BorshStorageKey, Gas, ONE_NEAR};

pub type BountyIndex = u64;

pub const GAS_FOR_ADD_PROPOSAL: Gas = Gas(25_000_000_000_000);
pub const GAS_FOR_ON_ADDED_PROPOSAL_CALLBACK: Gas = Gas(10_000_000_000_000);
pub const GAS_FOR_AFTER_ADD_PROPOSAL: Gas = Gas(45_000_000_000_000);
pub const GAS_FOR_CLAIM_APPROVAL: Gas = Gas(70_000_000_000_000);
pub const GAS_FOR_CLAIMER_APPROVAL: Gas = Gas(120_000_000_000_000);
pub const GAS_FOR_FT_TRANSFER: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_AFTER_FT_TRANSFER: Gas = Gas(40_000_000_000_000);
pub const GAS_FOR_AFTER_FT_TRANSACT: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_CHECK_PROPOSAL: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_AFTER_CHECK_BOUNTY_PAYOUT_PROPOSAL: Gas = Gas(70_000_000_000_000);
pub const GAS_FOR_AFTER_CHECK_APPROVE_CLAIMER_PROPOSAL: Gas = Gas(30_000_000_000_000);
pub const GAS_FOR_CREATE_DISPUTE: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_AFTER_CREATE_DISPUTE: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_CHECK_DISPUTE: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_AFTER_CHECK_DISPUTE: Gas = Gas(70_000_000_000_000);
pub const GAS_FOR_UPDATE_STATISTIC: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_GET_FT_METADATA: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_AFTER_GET_FT_METADATA: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_CHECK_IF_WHITELISTED: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_AFTER_CHECK_IF_WHITELISTED: Gas = Gas(85_000_000_000_000);

pub const DEFAULT_BOUNTY_CLAIM_BOND: U128 = U128(ONE_NEAR);
pub const DEFAULT_BOUNTY_FORGIVENESS_PERIOD: U64 = U64(1_000_000_000 * 60 * 60 * 24);
pub const DEFAULT_PERIOD_FOR_OPENING_DISPUTE: U64 = U64(1_000_000_000 * 60 * 60 * 24 * 10);
pub const DEFAULT_PLATFORM_FEE_PERCENTAGE: u32 = 10_000;
pub const DEFAULT_VALIDATORS_DAO_FEE_PERCENTAGE: u32 = 10_000;
pub const DEFAULT_PENALTY_PLATFORM_FEE_PERCENTAGE: u32 = 0;
pub const DEFAULT_PENALTY_VALIDATORS_DAO_FEE_PERCENTAGE: u32 = 0;
pub const MAX_SLOTS: u16 = 300;
pub const MIN_DEVIATION_FOR_TOTAL_BOUNTY_AMOUNT: u128 = 20;

pub const NO_DEPOSIT: Balance = 0;
pub const INITIAL_CATEGORIES: [&str; 4] = ["Marketing", "Development", "Design", "Other"];
pub const INITIAL_TAGS: [&str; 18] = ["API", "Blockchain", "Community", "CSS", "DAO", "dApp",
  "DeFi", "Design", "Documentation", "HTML", "Javascript", "NFT", "React", "Rust", "Smart contract",
  "Typescript", "UI/UX", "web3"];

#[ext_contract(ext_ft_contract)]
trait ExtFtContract {
  fn ft_transfer(
    &mut self,
    receiver_id: AccountId,
    amount: U128,
    memo: Option<String>
  );

  fn ft_metadata(&self) -> FungibleTokenMetadata;
}

#[derive(Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct FungibleTokenMetadata {
  pub spec: String,
  pub name: String,
  pub symbol: String,
  pub icon: Option<String>,
  pub reference: Option<String>,
  pub reference_hash: Option<Base64VecU8>,
  pub decimals: u8,
}

#[derive(Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Proposal {
  pub id: u64,
  pub proposer: AccountId,
  pub status: String,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct DisputeCreate {
  pub bounty_id: u64,
  pub description: String,
  pub claimer: AccountId,
  pub project_owner_delegate: AccountId,
}

#[derive(Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Dispute {
  pub status: String,
}

#[derive(Serialize)]
#[serde(crate = "near_sdk::serde")]
pub enum ReputationActionKind {
  BountyCreated,
  BountyCancelled,
  ClaimCreated,
  ClaimCancelled,
  ClaimerApproved,
  ClaimExpired,
  SuccessfulBountyAndClaim { with_dispute: bool },
  UnsuccessfulClaim { with_dispute: bool },
  SuccessfulClaim { with_dispute: bool },
  SuccessfulBounty,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum BountyStatus {
  New,
  Claimed,
  Completed,
  Canceled,
  ManyClaimed,
  AwaitingClaims,
  PartiallyCompleted,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub enum ContractStatus {
  Genesis,
  Live,
  ReadOnly,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub enum Deadline {
  DueDate { due_date: U64 },
  MaxDeadline { max_deadline: U64 },
  WithoutDeadline,
}

impl Deadline {
  pub fn get_deadline_type(&self) -> u8 {
    match self.clone() {
      Self::DueDate { .. } => 1,
      Self::MaxDeadline { .. } => 2,
      _ => 3
    }
  }

  pub fn get_deadline_value(&self) -> U64 {
    match self.clone() {
      Self::DueDate { due_date } => due_date,
      Self::MaxDeadline { max_deadline } => max_deadline,
      _ => env::panic_str("No value")
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub enum Reviewers {
  ValidatorsDao { validators_dao: ValidatorsDao },
  MoreReviewers { more_reviewers: Vec<AccountId> },
}

impl Reviewers {
  pub fn get_more_reviewers(&self) -> Vec<AccountId> {
    match self.clone() {
      Self::MoreReviewers { more_reviewers } => more_reviewers,
      _ => env::panic_str("There are no other reviewers")
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub enum Experience {
  Beginner,
  Intermediate,
  Advanced,
  AnyExperience,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub enum ContactType {
  Discord,
  Telegram,
  Twitter,
  Email,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct ContactDetails {
  pub contact: String,
  pub contact_type: ContactType,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub enum ClaimerApproval {
  ApprovalWithWhitelist, /// [deprecated]
  MultipleClaims,
  WithoutApproval,
  ApprovalByWhitelist { claimers_whitelist: Vec<AccountId> },
  WhitelistWithApprovals { claimers_whitelist: Vec<AccountId> },
}

impl ClaimerApproval {
  pub fn not_allowed_to_create_claim(&self, account_id: &AccountId) -> bool {
    match self.clone() {
      Self::ApprovalByWhitelist { claimers_whitelist } =>
        !claimers_whitelist.contains(account_id),
      Self::WhitelistWithApprovals { claimers_whitelist } =>
        !claimers_whitelist.contains(account_id),
      Self::ApprovalWithWhitelist =>
        env::panic_str("ApprovalWithWhitelist is not supported"),
      _ => false
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct BountyMetadata {
  pub title: String,
  pub description: String,
  pub category: String,
  pub attachments: Option<Vec<String>>,
  pub experience: Option<Experience>,
  pub tags: Option<Vec<String>>,
  pub acceptance_criteria: Option<String>,
  pub contact_details: Option<ContactDetails>,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct ValidatorsDaoParams {
  pub account_id: AccountId,
  pub add_proposal_bond: U128,
  pub gas_for_add_proposal: Option<U64>,
  pub gas_for_claim_approval: Option<U64>,
  pub gas_for_claimer_approval: Option<U64>,
}

impl ValidatorsDaoParams {
  pub fn to_validators_dao(&self) -> ValidatorsDao {
    let mut gas_for_add_proposal = GAS_FOR_ADD_PROPOSAL.0.into();
    if self.gas_for_add_proposal.is_some() {
      gas_for_add_proposal = self.gas_for_add_proposal.unwrap();
    }
    let mut gas_for_claim_approval = GAS_FOR_CLAIM_APPROVAL.0.into();
    if self.gas_for_claim_approval.is_some() {
      gas_for_claim_approval = self.gas_for_claim_approval.unwrap();
    }
    let mut gas_for_claimer_approval = GAS_FOR_CLAIMER_APPROVAL.0.into();
    if self.gas_for_claimer_approval.is_some() {
      gas_for_claimer_approval = self.gas_for_claimer_approval.unwrap();
    }
    ValidatorsDao {
      account_id: self.account_id.clone(),
      add_proposal_bond: self.add_proposal_bond.clone(),
      gas_for_add_proposal,
      gas_for_claim_approval,
      gas_for_claimer_approval,
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum ReviewersParams {
  ValidatorsDao { validators_dao: ValidatorsDaoParams },
  MoreReviewers { more_reviewers: Vec<AccountId> },
}

impl ReviewersParams {
  pub fn to_reviewers(&self) -> Reviewers {
    match self.clone() {
      Self::ValidatorsDao { validators_dao } =>
        Reviewers::ValidatorsDao { validators_dao: validators_dao.to_validators_dao() },
      Self::MoreReviewers { more_reviewers } =>
        Reviewers::MoreReviewers { more_reviewers },
    }
  }

  pub fn get_more_reviewers(&self) -> Vec<AccountId> {
    match self.clone() {
      Self::MoreReviewers { more_reviewers } => more_reviewers,
      _ => env::panic_str("There are no other reviewers")
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum KycVerificationMethod {
  WhenCreatingClaim,
  DuringClaimApproval,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub enum KycConfig {
  KycNotRequired,
  KycRequired { kyc_verification_method: KycVerificationMethod },
}

impl Default for KycConfig {
  fn default() -> Self {
    Self::KycRequired { kyc_verification_method: KycVerificationMethod::WhenCreatingClaim }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum PlaceOfCheckKYC {
  CreatingClaim { deadline: U64, description: String },
  DecisionOnClaim { approve: bool, is_kyc_delayed: Option<DefermentOfKYC> },
  ClaimDone { description: String },
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub enum DateOrPeriod {
  Date { date: U64 },
  Period { period: U64 },
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub enum StartConditions {
  MinAmountToStart { amount: u16 },
  ManuallyStart,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct Subtask {
  pub subtask_description: String,
  pub subtask_percent: u32,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct ContestOrHackathonEnv {
  pub started_at: Option<U64>,
  pub finished_at: Option<U64>,
  pub participants: u32,
  pub competition_winner: Option<AccountId>,
}

impl Default for ContestOrHackathonEnv {
  fn default() -> Self {
    Self {
      started_at: None,
      finished_at: None,
      participants: 0,
      competition_winner: None,
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct OneForAllEnv {
  pub occupied_slots: u16,
  pub paid_slots: u16,
}

impl Default for OneForAllEnv {
  fn default() -> Self {
    Self {
      occupied_slots: 0,
      paid_slots: 0,
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct SubtaskEnv {
  pub participant: AccountId,
  pub completed: bool,
  pub confirmed: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct DifferentTasksEnv {
  pub participants: Vec<Option<SubtaskEnv>>,
  pub bounty_payout_proposal_id: Option<U64>,
}

impl Default for DifferentTasksEnv {
  fn default() -> Self {
    Self {
      participants: vec![],
      bounty_payout_proposal_id: None,
    }
  }
}

impl DifferentTasksEnv {
  pub fn init(size: usize) -> Self {
    let mut env = DifferentTasksEnv::default();
    (0..size).into_iter().for_each(|_| env.participants.push(None));
    env
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub enum Multitasking {
  ContestOrHackathon {
    allowed_create_claim_to: Option<DateOrPeriod>,
    successful_claims_for_result: Option<u16>,
    start_conditions: Option<StartConditions>,
    runtime_env: Option<ContestOrHackathonEnv>,
  },
  OneForAll {
    number_of_slots: u16,
    amount_per_slot: U128,
    min_slots_to_start: Option<u16>,
    runtime_env: Option<OneForAllEnv>,
  },
  DifferentTasks {
    subtasks: Vec<Subtask>,
    runtime_env: Option<DifferentTasksEnv>,
  },
}

impl Multitasking {
  pub fn init(self) -> Self {
    match self {
      Self::ContestOrHackathon {
        allowed_create_claim_to,
        successful_claims_for_result,
        start_conditions,
        runtime_env,
      } => {
        assert!(
          runtime_env.is_none(),
          "The Multitasking instance is already initialized"
        );
        Self::ContestOrHackathon {
          allowed_create_claim_to,
          successful_claims_for_result,
          start_conditions,
          runtime_env: Some(ContestOrHackathonEnv::default())
        }
      },
      Self::OneForAll {
        number_of_slots,
        amount_per_slot,
        min_slots_to_start,
        runtime_env
      } => {
        assert!(
          runtime_env.is_none(),
          "The Multitasking instance is already initialized"
        );
        Self::OneForAll {
          number_of_slots,
          amount_per_slot,
          min_slots_to_start,
          runtime_env: Some(OneForAllEnv::default())
        }
      },
      Self::DifferentTasks {
        subtasks,
        runtime_env
      } => {
        assert!(
          runtime_env.is_none(),
          "The Multitasking instance is already initialized"
        );
        Self::DifferentTasks {
          subtasks: subtasks.clone(),
          runtime_env: Some(DifferentTasksEnv::init(subtasks.len()))
        }
      },
    }
  }

  pub fn is_allowed_to_create_or_approve_claims(&self, slot: Option<usize>) -> bool {
    match self {
      Self::ContestOrHackathon { allowed_create_claim_to, runtime_env, .. } => {
        if allowed_create_claim_to.is_none() {
          true
        } else {
          match allowed_create_claim_to.clone().unwrap() {
            DateOrPeriod::Date { date } => date.0 > env::block_timestamp(),
            DateOrPeriod::Period { period } => {
              if self.has_competition_started() {
                let started_at = runtime_env.clone().unwrap().started_at.unwrap();
                period.0 > env::block_timestamp() - started_at.0
              } else {
                true
              }
            }
          }
        }
      },
      Self::OneForAll { number_of_slots, runtime_env, .. } => {
        let env = runtime_env.clone().unwrap();
        env.occupied_slots + env.paid_slots < number_of_slots.clone()
      },
      Self::DifferentTasks { runtime_env, .. } => {
        runtime_env.clone().unwrap().participants[slot.unwrap()].is_none()
      }
    }
  }

  pub fn is_competition_mode(&self) -> bool {
    match self {
      Self::ContestOrHackathon { .. } => true,
      _ => false
    }
  }

  pub fn is_one_for_all_mode(&self) -> bool {
    match self {
      Self::OneForAll { .. } => true,
      _ => false
    }
  }

  pub fn is_different_tasks_mode(&self) -> bool {
    match self {
      Self::DifferentTasks { .. } => true,
      _ => false
    }
  }

  fn get_contest_or_hackathon_env(&self) -> ContestOrHackathonEnv {
    match self {
      Self::ContestOrHackathon { runtime_env, .. } => {
        runtime_env.clone().unwrap()
      },
      _ => unreachable!(),
    }
  }

  pub fn has_competition_started(&self) -> bool {
    let env = self.get_contest_or_hackathon_env();
    env.started_at.is_some() && env.finished_at.is_none()
  }

  pub fn get_competition_timestamps(&self) -> (Option<U64>, Option<U64>) {
    let env = self.get_contest_or_hackathon_env();
    (env.started_at, env.finished_at)
  }

  pub fn get_participants(&self) -> u32 {
    self.get_contest_or_hackathon_env().participants
  }

  pub fn get_competition_winner(&self) -> Option<AccountId> {
    self.get_contest_or_hackathon_env().competition_winner
  }

  pub fn get_one_for_all_env(&self) -> (u16, u16) {
    match self {
      Self::OneForAll { runtime_env, .. } => {
        let env = runtime_env.clone().unwrap();
        (env.occupied_slots, env.paid_slots)
      },
      _ => unreachable!(),
    }
  }

  pub fn get_different_tasks_env(&self) -> DifferentTasksEnv {
    match self {
      Self::DifferentTasks { runtime_env, .. } => {
        runtime_env.clone().unwrap()
      },
      _ => unreachable!(),
    }
  }

  fn get_slots(self) -> Vec<Option<SubtaskEnv>> {
    self.get_different_tasks_env().participants
  }

  pub fn get_slot_env(self, slot: usize) -> Option<SubtaskEnv> {
    self.get_slots()[slot].clone()
  }

  pub fn are_all_slots_taken(self) -> bool {
    self.get_slots().into_iter().find(|s| s.is_none()).is_none()
  }

  pub fn are_all_slots_available(self) -> bool {
    self.get_slots().into_iter().find(|s| s.is_some()).is_none()
  }

  pub fn are_all_slots_complete(self, except: Option<AccountId>) -> bool {
    self
      .get_slots()
      .into_iter()
      .find(|s| {
        if s.is_none() {
          return true;
        }
        let env = s.clone().unwrap();
        !env.completed && (except.is_none() || env.participant != except.clone().unwrap())
      })
      .is_none()
  }

  pub fn are_all_slots_confirmed(self) -> bool {
    self.get_slots().into_iter().find(|s| s.is_none() || !s.clone().unwrap().confirmed).is_none()
  }

  pub fn set_competition_timestamp(self, started_at: Option<U64>) -> Self {
    match self {
      Self::ContestOrHackathon {
        allowed_create_claim_to,
        successful_claims_for_result,
        start_conditions,
        runtime_env,
      } => {
        let mut new_runtime_env = runtime_env.clone().unwrap();
        new_runtime_env.finished_at = if started_at.is_none() {
          Some(U64::from(env::block_timestamp()))
        } else {
          None
        };
        new_runtime_env.started_at = if started_at.is_some() {
          started_at
        } else {
          runtime_env.clone().unwrap().started_at
        };
        Self::ContestOrHackathon {
          allowed_create_claim_to,
          successful_claims_for_result,
          start_conditions,
          runtime_env: Some(new_runtime_env),
        }
      },
      _ => unreachable!(),
    }
  }

  pub fn set_competition_participants(self, participants: u32) -> Self {
    match self {
      Self::ContestOrHackathon {
        allowed_create_claim_to,
        successful_claims_for_result,
        start_conditions,
        runtime_env,
      } => {
        let mut new_runtime_env = runtime_env.unwrap();
        new_runtime_env.participants = participants;
        Self::ContestOrHackathon {
          allowed_create_claim_to,
          successful_claims_for_result,
          start_conditions,
          runtime_env: Some(new_runtime_env),
        }
      },
      _ => unreachable!(),
    }
  }

  pub fn set_competition_winner(self, competition_winner: Option<AccountId>) -> Self {
    match self {
      Self::ContestOrHackathon {
        allowed_create_claim_to,
        successful_claims_for_result,
        start_conditions,
        runtime_env,
      } => {
        let mut new_runtime_env = runtime_env.unwrap();
        new_runtime_env.competition_winner = competition_winner;
        Self::ContestOrHackathon {
          allowed_create_claim_to,
          successful_claims_for_result,
          start_conditions,
          runtime_env: Some(new_runtime_env),
        }
      },
      _ => unreachable!(),
    }
  }

  pub fn set_one_for_all_env(self, occupied_slots: u16, paid_slots: u16) -> Self {
    match self {
      Self::OneForAll {
        number_of_slots,
        amount_per_slot,
        min_slots_to_start,
        ..
      } => {
        Self::OneForAll {
          number_of_slots,
          amount_per_slot,
          min_slots_to_start,
          runtime_env: Some(OneForAllEnv { occupied_slots, paid_slots }),
        }
      },
      _ => unreachable!(),
    }
  }

  pub fn set_slot_env(&mut self, slot: usize, slot_env: Option<SubtaskEnv>) {
    match self.clone() {
      Self::DifferentTasks { subtasks, runtime_env } => {
        let mut env = runtime_env.unwrap();
        env.participants[slot] = slot_env;
        *self = Self::DifferentTasks {
          subtasks,
          runtime_env: Some(env)
        };
      }
      _ => unreachable!(),
    }
  }

  pub fn set_bounty_payout_proposal_id(self, proposal_id: Option<U64>) -> Self {
    match self {
      Self::DifferentTasks { subtasks, runtime_env } => {
        let mut env = runtime_env.unwrap();
        env.bounty_payout_proposal_id = proposal_id;
        Self::DifferentTasks {
          subtasks,
          runtime_env: Some(env)
        }
      },
      _ => unreachable!(),
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct PaymentTimestamps {
  pub payment_at: Option<U64>,
  pub payment_confirmed_at: Option<U64>,
}

impl Default for PaymentTimestamps {
  fn default() -> Self {
    Self {
      payment_at: None,
      payment_confirmed_at: None,
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub enum Postpaid {
  PaymentOutsideContractV1 {
    currency: String,
  },
  PaymentOutsideContract {
    currency: String,
    payment_timestamps: Option<PaymentTimestamps>,
  },
}

impl Postpaid {
  pub fn init(self) -> Self {
    match self {
      Self::PaymentOutsideContract {
        currency,
        payment_timestamps
      } => {
        assert!(
          payment_timestamps.is_none(),
          "The PaymentTimestamps instance is already initialized"
        );
        Self::PaymentOutsideContract {
          currency,
          payment_timestamps: Some(PaymentTimestamps::default()),
        }
      },
      _ => unreachable!()
    }
  }

  pub fn get_payment_timestamps(&self) -> PaymentTimestamps {
    match self {
      Self::PaymentOutsideContract { payment_timestamps, .. } =>
        payment_timestamps.clone().unwrap_or_default(),
      _ => unreachable!()
    }
  }

  pub fn set_payment_at(self, payment_at: Option<U64>) -> Self {
    match self {
      Self::PaymentOutsideContract {
        currency,
        payment_timestamps,
      } => {
        let mut new_payment_timestamps = payment_timestamps.unwrap_or_default();
        new_payment_timestamps.payment_at = payment_at;
        Self::PaymentOutsideContract {
          currency,
          payment_timestamps: Some(new_payment_timestamps),
        }
      },
      _ => unreachable!()
    }
  }

  pub fn set_payment_confirmed_at(self, payment_confirmed_at: Option<U64>) -> Self {
    match self {
      Self::PaymentOutsideContract {
        currency,
        payment_timestamps,
      } => {
        let mut new_payment_timestamps = payment_timestamps.unwrap_or_default();
        new_payment_timestamps.payment_confirmed_at = payment_confirmed_at;
        Self::PaymentOutsideContract {
          currency,
          payment_timestamps: Some(new_payment_timestamps),
        }
      },
      _ => unreachable!()
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct BountyCreate {
  pub metadata: BountyMetadata,
  pub deadline: Deadline,
  pub claimer_approval: ClaimerApproval,
  pub reviewers: Option<ReviewersParams>,
  pub kyc_config: Option<KycConfig>,
  pub postpaid: Option<Postpaid>,
  pub multitasking: Option<Multitasking>,
}

impl BountyCreate {
  pub fn to_bounty(
    &self,
    payer_id: &AccountId,
    token_id: Option<AccountId>,
    amount: U128,
    config: Config
  ) -> Bounty {
    let (percentage_platform, percentage_dao) = Bounty::get_percentage_of_commissions(
      config,
      if self.reviewers.is_some() {
        Some(self.reviewers.clone().unwrap().to_reviewers())
      } else {
        None
      }
    );

    let platform_fee = if self.postpaid.is_none() {
      amount.0 * percentage_platform / (100_000 + percentage_dao)
    } else { 0 };

    let dao_fee = if self.postpaid.is_none() {
      amount.0 * percentage_dao / (100_000 + percentage_dao)
    } else { 0 };

    let bounty_amount = amount.0 - platform_fee - dao_fee;

    let kyc_config = if self.kyc_config.is_some() {
      self.kyc_config.clone().unwrap()
    } else {
      KycConfig::default()
    };

    Bounty {
      token: token_id,
      amount: U128(bounty_amount),
      platform_fee: U128(platform_fee),
      dao_fee: U128(dao_fee),
      metadata: self.metadata.clone(),
      deadline: self.deadline.clone(),
      claimer_approval: self.claimer_approval.clone(),
      reviewers: if self.reviewers.is_some() {
        Some(self.reviewers.clone().unwrap().to_reviewers())
      } else {
        None
      },
      owner: payer_id.clone(),
      status: BountyStatus::New,
      created_at: U64::from(env::block_timestamp()),
      kyc_config,
      postpaid: if self.postpaid.is_some() {
        Some(self.postpaid.clone().unwrap().init())
      } else {
        None
      },
      multitasking: if self.multitasking.is_some() {
        Some(self.multitasking.clone().unwrap().init())
      } else {
        None
      },
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct BountyUpdate {
  pub metadata: Option<BountyMetadata>,
  pub deadline: Option<Deadline>,
  pub claimer_approval: Option<ClaimerApproval>,
  pub reviewers: Option<ReviewersParams>,
  pub amount: Option<U128>,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct ValidatorsDao {
  pub account_id: AccountId,
  pub add_proposal_bond: U128,
  pub gas_for_add_proposal: U64,
  pub gas_for_claim_approval: U64,
  pub gas_for_claimer_approval: U64,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct BountyV1 {
  pub token: AccountId,
  pub amount: U128,
  pub platform_fee: U128,
  pub dao_fee: U128,
  pub metadata: BountyMetadata,
  pub deadline: Deadline,
  pub claimer_approval: ClaimerApproval,
  pub reviewers: Option<Reviewers>,
  pub owner: AccountId,
  pub status: BountyStatus,
  pub created_at: U64,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct BountyV2 {
  pub token: AccountId,
  pub amount: U128,
  pub platform_fee: U128,
  pub dao_fee: U128,
  pub metadata: BountyMetadata,
  pub deadline: Deadline,
  pub claimer_approval: ClaimerApproval,
  pub reviewers: Option<Reviewers>,
  pub owner: AccountId,
  pub status: BountyStatus,
  pub created_at: U64,
  pub kyc_config: KycConfig,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct BountyV3 {
  pub token: Option<AccountId>,
  pub amount: U128,
  pub platform_fee: U128,
  pub dao_fee: U128,
  pub metadata: BountyMetadata,
  pub deadline: Deadline,
  pub claimer_approval: ClaimerApproval,
  pub reviewers: Option<Reviewers>,
  pub owner: AccountId,
  pub status: BountyStatus,
  pub created_at: U64,
  pub kyc_config: KycConfig,
  pub postpaid: Option<Postpaid>,
  pub payment_at: Option<U64>,
  pub payment_confirmed_at: Option<U64>,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct Bounty {
  pub token: Option<AccountId>,
  pub amount: U128,
  pub platform_fee: U128,
  pub dao_fee: U128,
  pub metadata: BountyMetadata,
  pub deadline: Deadline,
  pub claimer_approval: ClaimerApproval,
  pub reviewers: Option<Reviewers>,
  pub owner: AccountId,
  pub status: BountyStatus,
  pub created_at: U64,
  pub kyc_config: KycConfig,
  pub postpaid: Option<Postpaid>,
  pub multitasking: Option<Multitasking>,
}

impl Bounty {
  pub fn assert_valid(&self) {
    assert!(
      !self.metadata.title.is_empty(),
      "The title cannot be empty"
    );
    assert!(
      !self.metadata.description.is_empty(),
      "The description cannot be empty"
    );
    assert!(
      !self.metadata.category.is_empty(),
      "The bounty type cannot be empty"
    );
    if self.metadata.attachments.is_some() {
      assert!(
        self.metadata.attachments.clone().unwrap().len() > 0,
        "The expected number of attachments is greater than zero"
      );
    }
    assert!(
      self.amount.0 > 0,
      "The bounty amount is incorrect",
    );
    assert!(
      if self.is_payment_outside_contract() {
        self.token.is_none()
      } else {
        self.token.is_some()
      },
      "Invalid token_id value"
    );
    match self.deadline {
      Deadline::DueDate { due_date } =>
        assert!(
          due_date.0 > env::block_timestamp(),
          "Incorrect due date",
        ),
      Deadline::MaxDeadline { max_deadline } =>
        assert!(
          max_deadline.0 > 0,
          "The max deadline is incorrect"
        ),
      Deadline::WithoutDeadline => ()
    }
    if self.reviewers.clone().is_some() {
      match self.reviewers.clone().unwrap() {
        Reviewers::MoreReviewers { more_reviewers } =>
          assert!(
            more_reviewers.len() > 0,
            "The expected number of reviewers is greater than zero",
          ),
        _ => (),
      }
    }
    if self.multitasking.is_some() {
      match self.multitasking.clone().unwrap() {
        Multitasking::ContestOrHackathon {
          allowed_create_claim_to,
          successful_claims_for_result,
          start_conditions,
          ..
        } => {
          if allowed_create_claim_to.is_some() {
            match allowed_create_claim_to.unwrap() {
              DateOrPeriod::Date { date } => {
                assert!(
                  env::block_timestamp() < date.0 &&
                    (
                      self.deadline.get_deadline_type() != 1 ||
                        date.0 <= self.deadline.get_deadline_value().0
                    ),
                  "The date until which it is allowed to create claims is incorrect"
                );
              },
              _ => (),
            }
          }
          if successful_claims_for_result.is_some() {
            assert!(
              successful_claims_for_result.unwrap() > 1,
              "Incorrect number of successful claims to obtain a result"
            );
          }
          if start_conditions.is_some() {
            match start_conditions.unwrap() {
              StartConditions::MinAmountToStart { amount } =>
                assert!(
                  amount > 1,
                  "The minimum number of claims to start is incorrect"
                ),
              _ => (),
            }
          }
        },
        Multitasking::OneForAll {
          number_of_slots,
          amount_per_slot,
          min_slots_to_start,
          ..
        } => {
          assert!(
            number_of_slots > 1,
            "The number of slots must be greater than one"
          );
          assert!(
            amount_per_slot.0 > 0,
            "The cost of one slot cannot be zero"
          );
          if min_slots_to_start.is_some() {
            assert!(
              min_slots_to_start.unwrap() > 1 && min_slots_to_start.unwrap() <= MAX_SLOTS,
              "The minimum number of claims to start is incorrect"
            );
          }
          assert!(
            self.amount.0 + self.platform_fee.0 >= amount_per_slot.0 * number_of_slots as u128 &&
            self.amount.0 + self.platform_fee.0 - amount_per_slot.0 * number_of_slots as u128 <=
              MIN_DEVIATION_FOR_TOTAL_BOUNTY_AMOUNT,
            "Total bounty amount is incorrect"
          );
        },
        Multitasking::DifferentTasks { subtasks, .. } => {
          assert!(
            subtasks.len() > 1 && subtasks.len() <= MAX_SLOTS as usize,
            "The number of subtasks must be greater than one"
          );
          assert!(
            subtasks
              .clone()
              .into_iter()
              .find(|subtask| subtask.subtask_description.is_empty())
              .is_none(),
            "The subtask description cannot be empty"
          );
          assert_eq!(
            subtasks.into_iter().map(|subtask| subtask.subtask_percent).sum::<u32>(),
            100_000,
            "The sum of the cost of all subtasks must equal 100%"
          );
        },
      }
    }
  }

  pub fn check_access_rights(&self) {
    let sender_id = env::predecessor_account_id();
    if self.reviewers.is_some() {
      match self.reviewers.clone().unwrap() {
        Reviewers::ValidatorsDao { validators_dao } =>
          assert_eq!(
            validators_dao.account_id,
            sender_id,
            "This method can only call DAO validators"
          ),
        Reviewers::MoreReviewers { more_reviewers } => {
          let mut all_reviewers = more_reviewers;
          all_reviewers.push(self.owner.clone());
          assert!(
            all_reviewers.contains(&sender_id),
            "This method can only be called by one of the reviewers"
          );
        }
      }
    } else {
      assert_eq!(
        self.owner,
        sender_id,
        "Only the owner of the bounty can call this method"
      );
    }
  }

  pub fn assert_validators_dao_account_id(&self, account_id: AccountId) {
    if self.reviewers.is_some() {
      match self.reviewers.clone().unwrap() {
        Reviewers::ValidatorsDao { validators_dao } => {
          assert_eq!(
            validators_dao.account_id,
            account_id,
            "DAO account ID does not match"
          );
          return;
        }
        _ => (),
      }
    }
    env::panic_str("Validators DAO are not used");
  }

  pub fn is_claim_deadline_correct(&self, deadline: U64) -> bool {
    match self.deadline {
      Deadline::DueDate { due_date } => env::block_timestamp() + deadline.0 <= due_date.0,
      Deadline::MaxDeadline { max_deadline } => deadline.0 <= max_deadline.0,
      Deadline::WithoutDeadline => true,
    }
  }

  pub fn is_validators_dao_used(&self) -> bool {
    self.reviewers.is_some() && match self.reviewers.clone().unwrap() {
      Reviewers::ValidatorsDao { .. } => true,
      _ => false
    }
  }

  pub fn get_bounty_owner_delegate(&self) -> AccountId {
    if self.reviewers.is_some() {
      match self.reviewers.clone().unwrap() {
        Reviewers::ValidatorsDao { validators_dao } => validators_dao.account_id,
        _ => self.clone().owner,
      }
    } else {
      self.clone().owner
    }
  }

  pub fn assert_account_is_not_owner_or_reviewer(&self, account_id: AccountId) {
    assert_ne!(self.owner, account_id, "The predecessor cannot be a bounty owner");
    if self.reviewers.clone().is_some() {
      match self.reviewers.clone().unwrap() {
        Reviewers::MoreReviewers { more_reviewers } =>
          assert!(
            !more_reviewers.contains(&account_id),
            "The predecessor cannot be one of the reviewers"
          ),
        Reviewers::ValidatorsDao { validators_dao } =>
          assert_ne!(
            validators_dao.account_id,
            account_id,
            "The predecessor cannot be a validators DAO"
          ),
      }
    }
  }

  pub fn get_percentage_of_commissions(
    config: Config,
    reviewers: Option<Reviewers>
  ) -> (u128, u128) {
    let percentage_platform: u128 = config.platform_fee_percentage.into();
    let percentage_dao: u128 = if reviewers.is_some() {
      match reviewers.unwrap() {
        Reviewers::ValidatorsDao { .. } =>
          config.validators_dao_fee_percentage.into(),
        _ => 0
      }
    } else { 0 };
    (percentage_platform, percentage_dao)
  }

  pub fn is_contest_or_hackathon(&self) -> bool {
    self.multitasking.is_some() && self.multitasking.clone().unwrap().is_competition_mode()
  }

  pub fn is_one_bounty_for_many_claimants(&self) -> bool {
    self.multitasking.is_some() && self.multitasking.clone().unwrap().is_one_for_all_mode()
  }

  pub fn is_different_tasks(&self) -> bool {
    self.multitasking.is_some() && self.multitasking.clone().unwrap().is_different_tasks_mode()
  }

  pub fn is_payment_outside_contract(&self) -> bool {
    self.postpaid.is_some() &&
      matches!(self.postpaid.clone().unwrap(), Postpaid::PaymentOutsideContract { .. })
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum VersionedBounty {
  V1(BountyV1),
  V2(BountyV2),
  V3(BountyV3),
  Current(Bounty),
}

impl VersionedBounty {
  fn upgrade_v1_to_v2(bounty: BountyV1) -> BountyV2 {
    BountyV2 {
      token: bounty.token,
      amount: bounty.amount,
      platform_fee: bounty.platform_fee,
      dao_fee: bounty.dao_fee,
      metadata: bounty.metadata,
      deadline: bounty.deadline,
      claimer_approval: bounty.claimer_approval,
      reviewers: bounty.reviewers,
      owner: bounty.owner,
      status: bounty.status,
      created_at: bounty.created_at,
      kyc_config: KycConfig::default(),
    }
  }

  fn upgrade_v2_to_v3(bounty: BountyV2) -> BountyV3 {
    BountyV3 {
      token: Some(bounty.token),
      amount: bounty.amount,
      platform_fee: bounty.platform_fee,
      dao_fee: bounty.dao_fee,
      metadata: bounty.metadata,
      deadline: bounty.deadline,
      claimer_approval: bounty.claimer_approval,
      reviewers: bounty.reviewers,
      owner: bounty.owner,
      status: bounty.status,
      created_at: bounty.created_at,
      kyc_config: bounty.kyc_config,
      postpaid: None,
      payment_at: None,
      payment_confirmed_at: None,
    }
  }

  fn upgrade_v3_to_v4(bounty: BountyV3) -> Bounty {
    let postpaid = if bounty.postpaid.is_some() {
      match bounty.postpaid.clone().unwrap() {
        Postpaid::PaymentOutsideContractV1 { currency } =>
          Some(Postpaid::PaymentOutsideContract {
            currency,
            payment_timestamps: Some(PaymentTimestamps {
              payment_at: bounty.payment_at.clone(),
              payment_confirmed_at: bounty.payment_confirmed_at.clone(),
            }),
          }),
        _ => unreachable!(),
      }
    } else {
      None
    };
    Bounty {
      token: bounty.token,
      amount: bounty.amount,
      platform_fee: bounty.platform_fee,
      dao_fee: bounty.dao_fee,
      metadata: bounty.metadata,
      deadline: bounty.deadline,
      claimer_approval: bounty.claimer_approval,
      reviewers: bounty.reviewers,
      owner: bounty.owner,
      status: bounty.status,
      created_at: bounty.created_at,
      kyc_config: bounty.kyc_config,
      postpaid,
      multitasking: None,
    }
  }

  pub fn to_bounty(self) -> Bounty {
    match self {
      VersionedBounty::Current(bounty) => bounty,
      VersionedBounty::V1(bounty_v1) =>
        VersionedBounty::upgrade_v3_to_v4(
          VersionedBounty::upgrade_v2_to_v3(
            VersionedBounty::upgrade_v1_to_v2(bounty_v1)
          )
        ),
      VersionedBounty::V2(bounty_v2) =>
        VersionedBounty::upgrade_v3_to_v4(
          VersionedBounty::upgrade_v2_to_v3(bounty_v2)
        ),
      VersionedBounty::V3(bounty_v3) => VersionedBounty::upgrade_v3_to_v4(bounty_v3),
    }
  }
}

impl From<VersionedBounty> for Bounty {
  fn from(value: VersionedBounty) -> Self {
    match value {
      VersionedBounty::Current(bounty) => bounty,
      VersionedBounty::V1(bounty_v1) =>
        VersionedBounty::upgrade_v3_to_v4(
          VersionedBounty::upgrade_v2_to_v3(
            VersionedBounty::upgrade_v1_to_v2(bounty_v1)
          )
        ),
      VersionedBounty::V2(bounty_v2) =>
        VersionedBounty::upgrade_v3_to_v4(
          VersionedBounty::upgrade_v2_to_v3(bounty_v2)
        ),
      VersionedBounty::V3(bounty_v3) => VersionedBounty::upgrade_v3_to_v4(bounty_v3),
    }
  }
}

impl From<Bounty> for VersionedBounty {
  fn from(value: Bounty) -> Self {
    VersionedBounty::Current(value)
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct TokenDetails {
  pub enabled: bool,
  pub min_amount_for_kyc: Option<U128>,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum ClaimStatus {
  New,
  InProgress,
  Completed,
  Approved,
  Rejected,
  Canceled,
  Expired,
  Disputed,
  NotCompleted,
  NotHired,
  Competes,
  ReadyToStart,
  CompletedWithDispute
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub enum DefermentOfKYC {
  AllowedToSkipKyc,
  BeforeDeadline,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct BountyClaimV1 {
  /// Bounty id that was claimed.
  pub bounty_id: BountyIndex,
  /// When a claim is created.
  pub created_at: U64,
  /// Start time of the claim.
  pub start_time: Option<U64>,
  /// Deadline specified by claimer.
  pub deadline: U64,
  /// Description
  pub description: String,
  /// status
  pub status: ClaimStatus,
  /// Bounty payout proposal ID
  pub bounty_payout_proposal_id: Option<U64>,
  /// Proposal ID for applicant approval
  pub approve_claimer_proposal_id: Option<U64>,
  /// Timestamp when the status is set to rejected
  pub rejected_timestamp: Option<U64>,
  /// Dispute ID
  pub dispute_id: Option<U64>,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct BountyClaimV2 {
  /// Bounty id that was claimed.
  pub bounty_id: BountyIndex,
  /// When a claim is created.
  pub created_at: U64,
  /// Start time of the claim.
  pub start_time: Option<U64>,
  /// Deadline specified by claimer.
  pub deadline: U64,
  /// Description
  pub description: String,
  /// status
  pub status: ClaimStatus,
  /// Bounty payout proposal ID
  pub bounty_payout_proposal_id: Option<U64>,
  /// Proposal ID for applicant approval
  pub approve_claimer_proposal_id: Option<U64>,
  /// Timestamp when the status is set to rejected
  pub rejected_timestamp: Option<U64>,
  /// Dispute ID
  pub dispute_id: Option<U64>,
  /// Bounty owner deferred KYC verification or verification status will be tracked manually
  pub is_kyc_delayed: Option<DefermentOfKYC>,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct BountyClaimV3 {
  /// Bounty id that was claimed.
  pub bounty_id: BountyIndex,
  /// When a claim is created.
  pub created_at: U64,
  /// Start time of the claim.
  pub start_time: Option<U64>,
  /// Deadline specified by claimer.
  pub deadline: U64,
  /// Description
  pub description: String,
  /// status
  pub status: ClaimStatus,
  /// Bounty payout proposal ID
  pub bounty_payout_proposal_id: Option<U64>,
  /// Proposal ID for applicant approval
  pub approve_claimer_proposal_id: Option<U64>,
  /// Timestamp when the status is set to rejected
  pub rejected_timestamp: Option<U64>,
  /// Dispute ID
  pub dispute_id: Option<U64>,
  /// Bounty owner deferred KYC verification or verification status will be tracked manually
  pub is_kyc_delayed: Option<DefermentOfKYC>,
  /// Payment data for PaymentOutsideContract mode
  pub payment_timestamps: Option<PaymentTimestamps>,
  /// Bounty slot number for different tasks
  pub slot: Option<usize>,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct BountyClaim {
  /// Bounty id that was claimed.
  pub bounty_id: BountyIndex,
  /// When a claim is created.
  pub created_at: U64,
  /// Start time of the claim.
  pub start_time: Option<U64>,
  /// Deadline specified by claimer.
  pub deadline: U64,
  /// Description
  pub description: String,
  /// status
  pub status: ClaimStatus,
  /// Bounty payout proposal ID
  pub bounty_payout_proposal_id: Option<U64>,
  /// Proposal ID for applicant approval
  pub approve_claimer_proposal_id: Option<U64>,
  /// Timestamp when the status is set to rejected
  pub rejected_timestamp: Option<U64>,
  /// Dispute ID
  pub dispute_id: Option<U64>,
  /// Bounty owner deferred KYC verification or verification status will be tracked manually
  pub is_kyc_delayed: Option<DefermentOfKYC>,
  /// Payment data for PaymentOutsideContract mode
  pub payment_timestamps: Option<PaymentTimestamps>,
  /// Bounty slot number for different tasks
  pub slot: Option<usize>,
  /// Bond that was at the time of making the claim
  pub bond: Option<U128>,
}

impl BountyClaim {
  pub fn get_start_time(&self, bounty: &Bounty) -> U64 {
    if bounty.is_contest_or_hackathon() {
      let multitasking = bounty.multitasking.clone().unwrap();
      self.start_time.unwrap_or(multitasking.get_competition_timestamps().0.unwrap())
    } else {
      self.start_time.unwrap()
    }
  }

  pub fn is_claim_expired(&self, bounty: &Bounty) -> bool {
    env::block_timestamp() > self.get_start_time(bounty).0 + self.deadline.0
  }

  pub fn set_payment_at(&mut self, payment_at: Option<U64>) {
    let mut new_payment_timestamps = self.payment_timestamps.clone().unwrap_or_default();
    new_payment_timestamps.payment_at = payment_at;
    self.payment_timestamps = Some(new_payment_timestamps);
  }

  pub fn set_payment_confirmed_at(&mut self, payment_confirmed_at: Option<U64>) {
    let mut new_payment_timestamps = self.payment_timestamps.clone().unwrap_or_default();
    new_payment_timestamps.payment_confirmed_at = payment_confirmed_at;
    self.payment_timestamps = Some(new_payment_timestamps);
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum VersionedBountyClaim {
  V1(BountyClaimV1),
  V2(BountyClaimV2),
  V3(BountyClaimV3),
  Current(BountyClaim),
}

impl VersionedBountyClaim {
  fn upgrade_v1_to_v2(bounty_claim: BountyClaimV1) -> BountyClaimV2 {
    BountyClaimV2 {
      bounty_id: bounty_claim.bounty_id,
      created_at: bounty_claim.created_at,
      start_time: bounty_claim.start_time,
      deadline: bounty_claim.deadline,
      description: bounty_claim.description,
      status: bounty_claim.status,
      bounty_payout_proposal_id: bounty_claim.bounty_payout_proposal_id,
      approve_claimer_proposal_id: bounty_claim.approve_claimer_proposal_id,
      rejected_timestamp: bounty_claim.rejected_timestamp,
      dispute_id: bounty_claim.dispute_id,
      is_kyc_delayed: None,
    }
  }

  fn upgrade_v2_to_v3(bounty_claim: BountyClaimV2) -> BountyClaimV3 {
    BountyClaimV3 {
      bounty_id: bounty_claim.bounty_id,
      created_at: bounty_claim.created_at,
      start_time: bounty_claim.start_time,
      deadline: bounty_claim.deadline,
      description: bounty_claim.description,
      status: bounty_claim.status,
      bounty_payout_proposal_id: bounty_claim.bounty_payout_proposal_id,
      approve_claimer_proposal_id: bounty_claim.approve_claimer_proposal_id,
      rejected_timestamp: bounty_claim.rejected_timestamp,
      dispute_id: bounty_claim.dispute_id,
      is_kyc_delayed: bounty_claim.is_kyc_delayed,
      payment_timestamps: None,
      slot: None,
    }
  }

  fn upgrade_v3_to_v4(bounty_claim: BountyClaimV3) -> BountyClaim {
    BountyClaim {
      bounty_id: bounty_claim.bounty_id,
      created_at: bounty_claim.created_at,
      start_time: bounty_claim.start_time,
      deadline: bounty_claim.deadline,
      description: bounty_claim.description,
      status: bounty_claim.status,
      bounty_payout_proposal_id: bounty_claim.bounty_payout_proposal_id,
      approve_claimer_proposal_id: bounty_claim.approve_claimer_proposal_id,
      rejected_timestamp: bounty_claim.rejected_timestamp,
      dispute_id: bounty_claim.dispute_id,
      is_kyc_delayed: bounty_claim.is_kyc_delayed,
      payment_timestamps: bounty_claim.payment_timestamps,
      slot: bounty_claim.slot,
      bond: None,
    }
  }

  pub fn to_bounty_claim(self) -> BountyClaim {
    match self {
      VersionedBountyClaim::Current(bounty_claim) => bounty_claim,
      VersionedBountyClaim::V1(bounty_claim_v1) =>
        VersionedBountyClaim::upgrade_v3_to_v4(
          VersionedBountyClaim::upgrade_v2_to_v3(
            VersionedBountyClaim::upgrade_v1_to_v2(bounty_claim_v1)
          )
        ),
      VersionedBountyClaim::V2(bounty_claim_v2) =>
        VersionedBountyClaim::upgrade_v3_to_v4(
          VersionedBountyClaim::upgrade_v2_to_v3(bounty_claim_v2)
        ),
      VersionedBountyClaim::V3(bounty_claim_v3) =>
        VersionedBountyClaim::upgrade_v3_to_v4(bounty_claim_v3)
    }
  }
}

impl From<VersionedBountyClaim> for BountyClaim {
  fn from(value: VersionedBountyClaim) -> Self {
    match value {
      VersionedBountyClaim::Current(bounty_claim) => bounty_claim,
      VersionedBountyClaim::V1(bounty_claim_v1) =>
        VersionedBountyClaim::upgrade_v3_to_v4(
          VersionedBountyClaim::upgrade_v2_to_v3(
            VersionedBountyClaim::upgrade_v1_to_v2(bounty_claim_v1)
          )
        ),
      VersionedBountyClaim::V2(bounty_claim_v2) =>
        VersionedBountyClaim::upgrade_v3_to_v4(
          VersionedBountyClaim::upgrade_v2_to_v3(bounty_claim_v2)
        ),
      VersionedBountyClaim::V3(bounty_claim_v3) =>
        VersionedBountyClaim::upgrade_v3_to_v4(bounty_claim_v3)
    }
  }
}

impl From<BountyClaim> for VersionedBountyClaim {
  fn from(value: BountyClaim) -> Self {
    VersionedBountyClaim::Current(value)
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum BountyAction {
  ClaimApproved { receiver_id: AccountId },
  ClaimRejected { receiver_id: AccountId },
  SeveralClaimsApproved,
  Finalize { receiver_id: Option<AccountId> },
}

impl BountyAction {
  pub fn need_to_finalize_claim(&self) -> bool {
    match self.clone() {
      Self::Finalize { receiver_id } => receiver_id.is_some(),
      _ => false,
    }
  }

  pub fn get_finalize_action_receiver(&self) -> Option<AccountId> {
    match self.clone() {
      Self::Finalize { receiver_id } => receiver_id,
      _ => None,
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum ReferenceType {
  Categories,
  Tags,
  Currencies,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct ConfigCreate {
  pub bounty_claim_bond: U128,
  pub bounty_forgiveness_period: U64,
  pub period_for_opening_dispute: U64,
  pub platform_fee_percentage: u32,
  pub validators_dao_fee_percentage: u32,
  pub penalty_platform_fee_percentage: u32,
  pub penalty_validators_dao_fee_percentage: u32,
  pub use_owners_whitelist: bool,
}

impl ConfigCreate {
  pub fn to_config(&self, config: Config) -> Config {
    Config {
      bounty_claim_bond: self.bounty_claim_bond,
      bounty_forgiveness_period: self.bounty_forgiveness_period,
      period_for_opening_dispute: self.period_for_opening_dispute,
      platform_fee_percentage: self.platform_fee_percentage,
      validators_dao_fee_percentage: self.validators_dao_fee_percentage,
      penalty_platform_fee_percentage: self.penalty_platform_fee_percentage,
      penalty_validators_dao_fee_percentage: self.penalty_validators_dao_fee_percentage,
      categories: config.categories,
      tags: config.tags,
      currencies: config.currencies,
      use_owners_whitelist: self.use_owners_whitelist,
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct ConfigV1 {
  pub bounty_claim_bond: U128,
  pub bounty_forgiveness_period: U64,
  pub period_for_opening_dispute: U64,
  pub categories: Vec<String>,
  pub tags: Vec<String>,
  pub platform_fee_percentage: u32,
  pub validators_dao_fee_percentage: u32,
  pub penalty_platform_fee_percentage: u32,
  pub penalty_validators_dao_fee_percentage: u32,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct ConfigV2 {
  pub bounty_claim_bond: U128,
  pub bounty_forgiveness_period: U64,
  pub period_for_opening_dispute: U64,
  pub categories: Vec<String>,
  pub tags: Vec<String>,
  pub platform_fee_percentage: u32,
  pub validators_dao_fee_percentage: u32,
  pub penalty_platform_fee_percentage: u32,
  pub penalty_validators_dao_fee_percentage: u32,
  pub currencies: Vec<String>,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct Config {
  pub bounty_claim_bond: U128,
  pub bounty_forgiveness_period: U64,
  pub period_for_opening_dispute: U64,
  pub categories: Vec<String>,
  pub tags: Vec<String>,
  pub platform_fee_percentage: u32,
  pub validators_dao_fee_percentage: u32,
  pub penalty_platform_fee_percentage: u32,
  pub penalty_validators_dao_fee_percentage: u32,
  pub currencies: Vec<String>,
  pub use_owners_whitelist: bool,
}

impl Config {
  fn default_categories() -> Vec<String> {
    let mut categories_set = vec![];
    categories_set.extend(INITIAL_CATEGORIES.into_iter().map(|t| t.into()));
    categories_set
  }

  fn default_tags() -> Vec<String> {
    let mut tags_set = vec![];
    tags_set.extend(INITIAL_TAGS.into_iter().map(|t| t.into()));
    tags_set
  }
}

impl Default for Config {
  fn default() -> Self {
    Self {
      bounty_claim_bond: DEFAULT_BOUNTY_CLAIM_BOND,
      bounty_forgiveness_period: DEFAULT_BOUNTY_FORGIVENESS_PERIOD,
      period_for_opening_dispute: DEFAULT_PERIOD_FOR_OPENING_DISPUTE,
      categories: Config::default_categories(),
      tags: Config::default_tags(),
      platform_fee_percentage: DEFAULT_PLATFORM_FEE_PERCENTAGE,
      validators_dao_fee_percentage: DEFAULT_VALIDATORS_DAO_FEE_PERCENTAGE,
      penalty_platform_fee_percentage: DEFAULT_PENALTY_PLATFORM_FEE_PERCENTAGE,
      penalty_validators_dao_fee_percentage: DEFAULT_PENALTY_VALIDATORS_DAO_FEE_PERCENTAGE,
      currencies: vec![],
      use_owners_whitelist: false,
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum VersionedConfig {
  V1(ConfigV1),
  V2(ConfigV2),
  Current(Config),
}

impl VersionedConfig {
  fn upgrade_v1_to_v2(config: ConfigV1) -> ConfigV2 {
    ConfigV2 {
      bounty_claim_bond: config.bounty_claim_bond,
      bounty_forgiveness_period: config.bounty_forgiveness_period,
      period_for_opening_dispute: config.period_for_opening_dispute,
      categories: config.categories,
      tags: config.tags,
      platform_fee_percentage: config.platform_fee_percentage,
      validators_dao_fee_percentage: config.validators_dao_fee_percentage,
      penalty_platform_fee_percentage: config.penalty_platform_fee_percentage,
      penalty_validators_dao_fee_percentage: config.penalty_validators_dao_fee_percentage,
      currencies: vec![],
    }
  }

  fn upgrade_v2_to_v3(config: ConfigV2) -> Config {
    Config {
      bounty_claim_bond: config.bounty_claim_bond,
      bounty_forgiveness_period: config.bounty_forgiveness_period,
      period_for_opening_dispute: config.period_for_opening_dispute,
      categories: config.categories,
      tags: config.tags,
      platform_fee_percentage: config.platform_fee_percentage,
      validators_dao_fee_percentage: config.validators_dao_fee_percentage,
      penalty_platform_fee_percentage: config.penalty_platform_fee_percentage,
      penalty_validators_dao_fee_percentage: config.penalty_validators_dao_fee_percentage,
      currencies: config.currencies,
      use_owners_whitelist: false,
    }
  }

  pub fn to_config(self) -> Config {
    match self {
      VersionedConfig::Current(config) => config,
      VersionedConfig::V1(config_v1) => VersionedConfig::upgrade_v2_to_v3(
        VersionedConfig::upgrade_v1_to_v2(config_v1)
      ),
      VersionedConfig::V2(config_v2) => VersionedConfig::upgrade_v2_to_v3(config_v2),
    }
  }

  pub fn to_config_mut(&mut self) -> &mut Config {
    *self = self.clone().to_config().into();
    if let Self::Current(config) = self {
      config
    } else {
      unreachable!();
    }
  }
}

impl From<VersionedConfig> for Config {
  fn from(value: VersionedConfig) -> Self {
    match value {
      VersionedConfig::Current(config) => config,
      VersionedConfig::V1(config_v1) => VersionedConfig::upgrade_v2_to_v3(
        VersionedConfig::upgrade_v1_to_v2(config_v1)
      ),
      VersionedConfig::V2(config_v2) => VersionedConfig::upgrade_v2_to_v3(config_v2),
    }
  }
}

impl From<Config> for VersionedConfig {
  fn from(value: Config) -> Self {
    VersionedConfig::Current(value)
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct FeeStats {
  pub balance: U128,
  pub locked_balance: U128,
  pub amount_received: U128,
  pub amount_withdrawn: U128,
  pub refunded_amount: U128,
  pub penalty_amount: U128,
}

impl FeeStats {
  pub fn new() -> Self {
    Self {
      balance: U128(0),
      locked_balance: U128(0),
      amount_received: U128(0),
      amount_withdrawn: U128(0),
      refunded_amount: U128(0),
      penalty_amount: U128(0),
    }
  }

  pub fn receiving_commission(&mut self, amount: &U128) {
    self.balance = U128(self.balance.0 + amount.0);
    self.locked_balance = U128(self.locked_balance.0 + amount.0);
    self.amount_received = U128(self.amount_received.0 + amount.0);
  }

  pub fn get_available_balance(&self) -> U128 {
    U128(self.balance.0 - self.locked_balance.0)
  }

  pub fn withdrawal_of_commission(&mut self, amount: &U128) {
    self.balance = U128(self.balance.0 - amount.0);
    self.amount_withdrawn = U128(self.amount_withdrawn.0 + amount.0);
  }

  pub fn assert_locked_amount_greater_than_or_equal_transaction_amount(&self, amount: &U128) {
    assert!(
      self.locked_balance.0 >= amount.0,
      "The locked amount cannot be less than the transaction amount"
    );
  }

  pub fn commission_unlocking(&mut self, amount: &U128) {
    self.locked_balance = U128(self.locked_balance.0 - amount.0);
  }

  pub fn refund_commission(&mut self, amount: &U128, penalty_amount: &U128) {
    if amount.0 == 0 {
      return;
    }
    self.balance = U128(self.balance.0 - amount.0);
    self.locked_balance = U128(self.locked_balance.0 - amount.0);
    self.refunded_amount = U128(self.refunded_amount.0 + amount.0 - penalty_amount.0);
    self.penalty_amount = U128(self.penalty_amount.0 + penalty_amount.0);
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct DaoFeeStats {
  pub token_id: AccountId,
  pub fee_stats: FeeStats,
}

impl DaoFeeStats {
  pub fn new(token_id: AccountId) -> Self {
    Self {
      token_id,
      fee_stats: FeeStats::new(),
    }
  }
}

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
  AccountBounties,
  AdminWhitelist,
  Bounties,
  BountyClaimerAccounts,
  BountyClaimers,
  OwnersWhitelist,
  Tokens,
  TotalFees,
  TotalValidatorsDaoFees,
  PostpaidSubscribersWhitelist,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum WhitelistType {
  AdministratorsWhitelist,
  OwnersWhitelist,
  PostpaidSubscribersWhitelist,
}
