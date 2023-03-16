use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{U128, U64};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, ext_contract, AccountId, Balance, BorshStorageKey, Gas, ONE_NEAR};

pub type BountyIndex = u64;

pub const GAS_FOR_ADD_PROPOSAL: Gas = Gas(25_000_000_000_000);
pub const GAS_FOR_ON_ADDED_PROPOSAL_CALLBACK: Gas = Gas(10_000_000_000_000);
pub const GAS_FOR_CLAIM_APPROVAL: Gas = Gas(70_000_000_000_000);
pub const GAS_FOR_FT_TRANSFER: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_AFTER_FT_TRANSFER: Gas = Gas(40_000_000_000_000);
pub const GAS_FOR_AFTER_FT_TRANSACT: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_CHECK_PROPOSAL: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_AFTER_CHECK_PROPOSAL: Gas = Gas(70_000_000_000_000);
pub const GAS_FOR_CREATE_DISPUTE: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_AFTER_CREATE_DISPUTE: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_CHECK_DISPUTE: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_AFTER_CHECK_DISPUTE: Gas = Gas(70_000_000_000_000);
pub const GAS_FOR_UPDATE_STATISTIC: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_GET_FT_METADATA: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_AFTER_GET_FT_METADATA: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_CHECK_IF_WHITELISTED: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_AFTER_CHECK_IF_WHITELISTED: Gas = Gas(15_000_000_000_000);

pub const DEFAULT_BOUNTY_CLAIM_BOND: U128 = U128(ONE_NEAR);
pub const DEFAULT_BOUNTY_FORGIVENESS_PERIOD: U64 = U64(1_000_000_000 * 60 * 60 * 24);
pub const DEFAULT_PERIOD_FOR_OPENING_DISPUTE: U64 = U64(1_000_000_000 * 60 * 60 * 24 * 10);
pub const DEFAULT_PLATFORM_FEE_PERCENTAGE: u32 = 10_000;
pub const DEFAULT_VALIDATORS_DAO_FEE_PERCENTAGE: u32 = 10_000;
pub const DEFAULT_PENALTY_PLATFORM_FEE_PERCENTAGE: u32 = 0;
pub const DEFAULT_PENALTY_VALIDATORS_DAO_FEE_PERCENTAGE: u32 = 0;

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
  ClaimExpired,
  SuccessfulClaim { with_dispute: bool },
  UnsuccessfulClaim { with_dispute: bool },
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub enum BountyStatus {
  New,
  Claimed,
  Completed,
  Canceled,
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
      Self::DueDate { due_date: _due_date } => 1,
      Self::MaxDeadline { max_deadline: _max_deadline } => 2,
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
  ApprovalWithWhitelist,
  MultipleClaims,
  WithoutApproval,
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

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct BountyCreate {
  pub metadata: BountyMetadata,
  pub deadline: Deadline,
  pub claimer_approval: ClaimerApproval,
  pub reviewers: Option<ReviewersParams>,
}

impl BountyCreate {
  pub fn to_bounty(&self, payer_id: &AccountId, token_id: &AccountId, amount: U128, config: Config) -> Bounty {
    let percentage_platform: u128 = config.platform_fee_percentage.into();
    let percentage_dao: u128 = if self.reviewers.is_some() {
      match self.reviewers.clone().unwrap() {
        ReviewersParams::ValidatorsDao { validators_dao: _validators_dao } =>
          config.validators_dao_fee_percentage.into(),
        _ => 0
      }
    } else { 0 };
    let bounty_amount = amount.0 * 100_000 / (100_000 + percentage_platform + percentage_dao );
    let platform_fee = bounty_amount * percentage_platform / 100_000;
    let dao_fee = amount.0 - bounty_amount - platform_fee;

    Bounty {
      token: token_id.clone(),
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
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct ValidatorsDao {
  pub account_id: AccountId,
  pub add_proposal_bond: U128,
  pub gas_for_add_proposal: U64,
  pub gas_for_claim_approval: U64,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct Bounty {
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

impl Bounty {
  pub fn assert_new_valid(&self) {
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
    let bounty_amount = self.amount.0;
    assert!(
      bounty_amount > 0,
      "Expected bounty amount to be positive",
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
      Reviewers::ValidatorsDao { validators_dao: _validators_dao } => true,
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
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum VersionedBounty {
  Current(Bounty),
}

impl VersionedBounty {
  pub fn to_bounty(self) -> Bounty {
    match self {
      VersionedBounty::Current(bounty) => bounty,
    }
  }
}

impl From<VersionedBounty> for Bounty {
  fn from(value: VersionedBounty) -> Self {
    match value {
      VersionedBounty::Current(bounty) => bounty,
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

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
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
  /// Sescription
  pub description: String,
  /// status
  pub status: ClaimStatus,
  /// Bounty payout proposal ID
  pub proposal_id: Option<U64>,
  /// Timestamp when the status is set to rejected
  pub rejected_timestamp: Option<U64>,
  /// Dispute ID
  pub dispute_id: Option<U64>,
}

impl BountyClaim {
  pub fn is_claim_expired(&self) -> bool {
    env::block_timestamp() > self.start_time.unwrap().0 + self.deadline.0
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum VersionedBountyClaim {
  Current(BountyClaim),
}

impl VersionedBountyClaim {
  pub fn to_bounty_claim(self) -> BountyClaim {
    match self {
      VersionedBountyClaim::Current(bounty_claim) => bounty_claim,
    }
  }
}

impl From<VersionedBountyClaim> for BountyClaim {
  fn from(value: VersionedBountyClaim) -> Self {
    match value {
      VersionedBountyClaim::Current(claim) => claim,
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
  Finalize,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub enum ReferenceType {
  Categories,
  Tags,
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
    }
  }
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
    Config {
      bounty_claim_bond: DEFAULT_BOUNTY_CLAIM_BOND,
      bounty_forgiveness_period: DEFAULT_BOUNTY_FORGIVENESS_PERIOD,
      period_for_opening_dispute: DEFAULT_PERIOD_FOR_OPENING_DISPUTE,
      categories: Config::default_categories(),
      tags: Config::default_tags(),
      platform_fee_percentage: DEFAULT_PLATFORM_FEE_PERCENTAGE,
      validators_dao_fee_percentage: DEFAULT_VALIDATORS_DAO_FEE_PERCENTAGE,
      penalty_platform_fee_percentage: DEFAULT_PENALTY_PLATFORM_FEE_PERCENTAGE,
      penalty_validators_dao_fee_percentage: DEFAULT_PENALTY_VALIDATORS_DAO_FEE_PERCENTAGE,
    }
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

  pub fn refund_commission(&mut self, amount: &U128, full_percentage: u32, penalty_percentage: u32) {
    if full_percentage == 0 {
      return;
    }
    self.balance = U128(self.balance.0 - amount.0);
    self.locked_balance = U128(self.locked_balance.0 - amount.0);
    let penalty_amount: u128 = amount.0 * penalty_percentage as u128 / full_percentage as u128;
    self.refunded_amount = U128(self.refunded_amount.0 + amount.0 - penalty_amount);
    self.penalty_amount = U128(self.penalty_amount.0 + penalty_amount);
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
  ClaimersWhitelist,
  Tokens,
  TotalFees,
  TotalValidatorsDaoFees,
}
