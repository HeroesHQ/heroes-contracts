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
  SuccessfulClaim { with_dispute: bool },
  UnsuccessfulClaim { with_dispute: bool },
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
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
    KycConfig::KycRequired { kyc_verification_method: KycVerificationMethod::WhenCreatingClaim }
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
pub enum Postpaid {
  PaymentViaContract,
  PaymentOutsideContract,
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
}

impl BountyCreate {
  pub fn to_bounty(&self, payer_id: &AccountId, token_id: &AccountId, amount: U128, config: Config) -> Bounty {
    let (percentage_platform, percentage_dao) = Bounty::get_percentage_of_commissions(
      config,
      if self.reviewers.is_some() {
        Some(self.reviewers.clone().unwrap().to_reviewers())
      } else {
        None
      }
    );

    let platform_fee = amount.0 * percentage_platform / (100_000 + percentage_dao);
    let dao_fee = amount.0 * percentage_dao / (100_000 + percentage_dao);
    let bounty_amount = amount.0 - platform_fee - dao_fee;
    let kyc_config = if self.kyc_config.is_some() {
      self.kyc_config.clone().unwrap()
    } else {
      KycConfig::default()
    };

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
      kyc_config,
      postpaid: self.postpaid.clone(),
      payment_at: None,
      payment_confirmed_at: None,
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
  pub kyc_config: KycConfig,
  pub postpaid: Option<Postpaid>,
  pub payment_at: Option<U64>,
  pub payment_confirmed_at: Option<U64>,
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
    let bounty_amount_is_correct = if self.postpaid.is_some() {
      bounty_amount == 0
    } else {
      bounty_amount > 0
    };
    assert!(
      bounty_amount_is_correct,
      "The bounty amount is incorrect",
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

  pub fn assert_postpaid_is_ready(&self) {
    if self.postpaid.is_some() {
      assert!(
        self.amount.0 > 0,
        "The price of the bounty has not yet been determined"
      );
      match self.postpaid.clone().unwrap() {
        Postpaid::PaymentOutsideContract => assert!(
          self.payment_at.is_some() && self.payment_confirmed_at.is_some(),
          "No payment confirmation"
        ),
        _ => {}
      }
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum VersionedBounty {
  V1(BountyV1),
  V2(BountyV2),
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

  fn upgrade_v2_to_v3(bounty: BountyV2) -> Bounty {
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
      postpaid: None,
      payment_at: None,
      payment_confirmed_at: None,
    }
  }

  pub fn to_bounty(self) -> Bounty {
    match self {
      VersionedBounty::Current(bounty) => bounty,
      VersionedBounty::V1(bounty_v1) =>
        VersionedBounty::upgrade_v2_to_v3(
          VersionedBounty::upgrade_v1_to_v2(bounty_v1)
        ),
      VersionedBounty::V2(bounty_v2) => VersionedBounty::upgrade_v2_to_v3(bounty_v2),
    }
  }
}

impl From<VersionedBounty> for Bounty {
  fn from(value: VersionedBounty) -> Self {
    match value {
      VersionedBounty::Current(bounty) => bounty,
      VersionedBounty::V1(bounty_v1) =>
        VersionedBounty::upgrade_v2_to_v3(
          VersionedBounty::upgrade_v1_to_v2(bounty_v1)
        ),
      VersionedBounty::V2(bounty_v2) => VersionedBounty::upgrade_v2_to_v3(bounty_v2),
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
}

impl BountyClaim {
  pub fn is_claim_expired(&self) -> bool {
    env::block_timestamp() > self.start_time.unwrap().0 + self.deadline.0
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum VersionedBountyClaim {
  V1(BountyClaimV1),
  Current(BountyClaim),
}

impl VersionedBountyClaim {
  fn upgrade_v1_to_v2(bounty_claim: BountyClaimV1) -> BountyClaim {
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
      is_kyc_delayed: None,
    }
  }

  pub fn to_bounty_claim(self) -> BountyClaim {
    match self {
      VersionedBountyClaim::Current(bounty_claim) => bounty_claim,
      VersionedBountyClaim::V1(bounty_claim_v1) =>
        VersionedBountyClaim::upgrade_v1_to_v2(bounty_claim_v1),
    }
  }
}

impl From<VersionedBountyClaim> for BountyClaim {
  fn from(value: VersionedBountyClaim) -> Self {
    match value {
      VersionedBountyClaim::Current(claim) => claim,
      VersionedBountyClaim::V1(bounty_claim_v1) =>
        VersionedBountyClaim::upgrade_v1_to_v2(bounty_claim_v1),
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
pub enum VersionedConfig {
  Current(Config),
}

impl VersionedConfig {
  pub fn to_config(self) -> Config {
    match self {
      VersionedConfig::Current(config) => config,
    }
  }

  pub fn to_config_mut(&mut self) -> &mut Config {
    match self {
      VersionedConfig::Current(config) => config,
    }
  }
}

impl From<VersionedConfig> for Config {
  fn from(value: VersionedConfig) -> Self {
    match value {
      VersionedConfig::Current(config) => config,
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
