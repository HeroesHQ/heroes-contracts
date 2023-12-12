use near_contract_standards::fungible_token::receiver::FungibleTokenReceiver;
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{LookupMap, UnorderedMap, UnorderedSet};
use near_sdk::json_types::{Base64VecU8, U128, U64};
use near_sdk::serde_json::json;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{assert_one_yocto, env, is_promise_success, log, near_bindgen, serde_json, AccountId,
               Balance, Gas, ONE_YOCTO, PanicOnDefault, Promise, PromiseError, PromiseOrValue};

pub use crate::types::*;

pub mod callbacks;
pub mod internal;
pub mod receiver;
pub mod types;
pub mod view;
mod upgrade;

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct BountiesContract {
  /// Token types allowed for use
  pub tokens: UnorderedMap<AccountId, TokenDetails>,

  /// Last available id for the bounty.
  pub last_bounty_id: BountyIndex,

  /// Bounties map from ID to bounty information.
  pub bounties: LookupMap<BountyIndex, VersionedBounty>,

  /// Bounty indexes map per owner account.
  pub account_bounties: LookupMap<AccountId, Vec<BountyIndex>>,

  /// Bounty claimers map per user. Allows quickly to query for each users their claims.
  pub bounty_claimers: LookupMap<AccountId, Vec<VersionedBountyClaim>>,

  /// Bounty claimer accounts map per bounty ID.
  pub bounty_claimer_accounts: LookupMap<BountyIndex, Vec<AccountId>>,

  /// Amount of $NEAR locked for bonds.
  pub locked_amount: Balance,

  /// Amount of non-refunded bonds.
  pub unlocked_amount: Balance,

  /// account ids that can perform all actions:
  /// - manage admins_whitelist
  /// - add new ft-token types
  /// - change contract config
  /// - etc.
  pub admins_whitelist: UnorderedSet<AccountId>,

  /// Bounty contract configuration.
  pub config: VersionedConfig,

  /// Reputation contract (optional)
  pub reputation_contract: Option<AccountId>,

  /// Dispute contract (optional)
  pub dispute_contract: Option<AccountId>,

  /// KYC whitelist contract (optional)
  pub kyc_whitelist_contract: Option<AccountId>,

  /// Whitelist accounts that can generate postpaid bounties
  pub owners_whitelist: UnorderedSet<AccountId>,

  /// Total platform fees map per token ID
  pub total_fees: LookupMap<AccountId, FeeStats>,

  /// Total validators DAO fees per DAO account ID
  pub total_validators_dao_fees: LookupMap<AccountId, Vec<DaoFeeStats>>,

  /// Recipient of platform fee (optional)
  pub recipient_of_platform_fee: Option<AccountId>,

  /// Contract status
  pub status: ContractStatus,
}

#[near_bindgen]
impl BountiesContract {
  #[init]
  pub fn new(
    admins_whitelist: Vec<AccountId>,
    config: Option<Config>,
    reputation_contract: Option<AccountId>,
    dispute_contract: Option<AccountId>,
    kyc_whitelist_contract: Option<AccountId>,
    recipient_of_platform_fee: Option<AccountId>,
  ) -> Self {
    assert!(
      admins_whitelist.len() > 0,
      "Admin whitelist must contain at least one account"
    );
    let mut admins_whitelist_set = UnorderedSet::new(StorageKey::AdminWhitelist);
    admins_whitelist_set.extend(admins_whitelist.into_iter().map(|a| a.into()));
    let versioned_config = if config.is_some() {
      config.unwrap().into()
    } else {
      Config::default().into()
    };

    Self {
      tokens: UnorderedMap::new(StorageKey::Tokens),
      last_bounty_id: 0,
      bounties: LookupMap::new(StorageKey::Bounties),
      account_bounties: LookupMap::new(StorageKey::AccountBounties),
      bounty_claimers: LookupMap::new(StorageKey::BountyClaimers),
      bounty_claimer_accounts: LookupMap::new(StorageKey::BountyClaimerAccounts),
      locked_amount: 0,
      unlocked_amount: 0,
      admins_whitelist: admins_whitelist_set,
      config: versioned_config,
      reputation_contract,
      dispute_contract,
      kyc_whitelist_contract,
      owners_whitelist: UnorderedSet::new(StorageKey::OwnersWhitelist),
      total_fees: LookupMap::new(StorageKey::TotalFees),
      total_validators_dao_fees: LookupMap::new(StorageKey::TotalValidatorsDaoFees),
      recipient_of_platform_fee,
      status: ContractStatus::Genesis,
    }
  }

  #[payable]
  pub fn add_to_admins_whitelist(
    &mut self,
    account_id: Option<AccountId>,
    account_ids: Option<Vec<AccountId>>,
  ) {
    self.assert_live();
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    let account_ids = if let Some(account_ids) = account_ids {
      account_ids
    } else {
      vec![account_id.expect("Expected either account_id or account_ids")]
    };
    for account_id in &account_ids {
      self.admins_whitelist.insert(account_id);
    }
  }

  #[payable]
  pub fn remove_from_admins_whitelist(
    &mut self,
    account_id: Option<AccountId>,
    account_ids: Option<Vec<AccountId>>,
  ) {
    self.assert_live();
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    let account_ids = if let Some(account_ids) = account_ids {
      account_ids
    } else {
      vec![account_id.expect("Expected either account_id or account_ids")]
    };
    for account_id in &account_ids {
      self.admins_whitelist.remove(&account_id);
    }
    assert!(
      !self.admins_whitelist.is_empty(),
      "Cannot remove all accounts from admin whitelist",
    );
  }

  #[payable]
  pub fn add_to_owners_whitelist(
    &mut self,
    account_id: Option<AccountId>,
    account_ids: Option<Vec<AccountId>>,
  ) {
    self.assert_live();
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    let account_ids = if let Some(account_ids) = account_ids {
      account_ids
    } else {
      vec![account_id.expect("Expected either account_id or account_ids")]
    };
    for account_id in &account_ids {
      self.owners_whitelist.insert(account_id);
    }
  }

  #[payable]
  pub fn remove_from_owners_whitelist(
    &mut self,
    account_id: Option<AccountId>,
    account_ids: Option<Vec<AccountId>>,
  ) {
    self.assert_live();
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    let account_ids = if let Some(account_ids) = account_ids {
      account_ids
    } else {
      vec![account_id.expect("Expected either account_id or account_ids")]
    };
    for account_id in &account_ids {
      self.owners_whitelist.remove(&account_id);
    }
  }

  #[payable]
  pub fn add_token_id(
    &mut self,
    token_id: AccountId,
    min_amount_for_kyc: Option<U128>
  ) -> PromiseOrValue<()> {
    self.assert_live();
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    assert!(
      self.tokens.get(&token_id).is_none(),
      "The token already exists"
    );
    self.internal_get_ft_metadata(token_id, min_amount_for_kyc)
  }

  #[payable]
  pub fn update_token(&mut self, token_id: AccountId, token_details: TokenDetails) {
    self.assert_live();
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    assert!(
      self.tokens.get(&token_id).is_some(),
      "No token found"
    );
    self.tokens.insert(&token_id, &token_details);
  }

  #[payable]
  pub fn update_kyc_whitelist_contract(&mut self, kyc_whitelist_contract: Option<AccountId>) {
    self.assert_live();
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    self.kyc_whitelist_contract = kyc_whitelist_contract;
  }

  /// Can be used only during migrations when updating contract versions
  #[payable]
  pub fn update_reputation_contract(&mut self, reputation_contract: AccountId) {
    self.assert_live();
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    assert!(
      self.reputation_contract.is_some(),
      "The reputation contract is not used",
    );
    self.reputation_contract = Some(reputation_contract);
  }

  /// Can be used only during migrations when updating contract versions
  #[payable]
  pub fn update_dispute_contract(&mut self, dispute_contract: AccountId) {
    self.assert_live();
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    assert!(
      self.dispute_contract.is_some(),
      "The dispute contract is not used",
    );
    self.dispute_contract = Some(dispute_contract);
  }

  #[payable]
  pub fn change_config(&mut self, config_create: ConfigCreate) {
    self.assert_live();
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    self.config = config_create.to_config(self.config.clone().to_config()).into();
  }

  #[payable]
  pub fn update_configuration_dictionary_entries(
    &mut self,
    dict: ReferenceType,
    entry: Option<String>,
    entries: Option<Vec<String>>
  ) {
    self.assert_live();
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    let (reference, entries) = self.get_configuration_dictionary(dict, entry, entries);

    for entry in entries {
      reference.push(entry);
    }
  }

  #[payable]
  pub fn remove_configuration_dictionary_entries(
    &mut self,
    dict: ReferenceType,
    entry: Option<String>,
    entries: Option<Vec<String>>
  ) {
    self.assert_live();
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    let (reference, entries) = self.get_configuration_dictionary(dict, entry, entries);

    for entry in entries {
      let index = reference.iter().position(|e| e.clone() == entry);
      if index.is_some() {
        reference.remove(index.unwrap());
      }
    }
  }

  #[payable]
  pub fn change_recipient_of_platform_fee(&mut self, recipient_of_platform_fee: AccountId) {
    self.assert_live();
    assert_one_yocto();
    self.assert_admins_whitelist(&env::predecessor_account_id());
    self.recipient_of_platform_fee = Some(recipient_of_platform_fee);
  }

  /// Claim given bounty by caller with given expected duration to execute.
  /// Bond must be attached to the claim.
  #[payable]
  pub fn bounty_claim(
    &mut self,
    id: BountyIndex,
    deadline: U64,
    description: String,
    slot: Option<usize>,
  ) -> PromiseOrValue<()> {
    self.assert_live();
    assert_eq!(
      env::attached_deposit(),
      self.config.clone().to_config().bounty_claim_bond.0,
      "Bounty wrong bond"
    );
    assert!(
      !description.is_empty(),
      "The description cannot be empty"
    );

    let sender_id = env::predecessor_account_id();
    let (bounty, _) = self.check_if_allowed_to_create_claim_by_status(
      id,
      sender_id.clone(),
      slot.clone()
    );

    assert!(
      bounty.is_claim_deadline_correct(deadline),
      "Bounty wrong deadline"
    );
    bounty.assert_account_is_not_owner_or_reviewer(sender_id.clone());
    assert!(
      !bounty.claimer_approval.not_allowed_to_create_claim(&sender_id),
      "{} is not whitelisted",
      sender_id
    );

    let place_of_check = PlaceOfCheckKYC::CreatingClaim { deadline, description };
    if self.is_kyc_check_required(bounty, None, None, place_of_check.clone()) {
      self.check_if_claimer_in_kyc_whitelist(id, sender_id, place_of_check, slot)

    } else {
      self.internal_add_proposal_and_create_claim(id, sender_id, place_of_check, slot)
    }
  }

  #[payable]
  pub fn decision_on_claim(
    &mut self,
    id: BountyIndex,
    claimer: AccountId,
    approve: bool,
    kyc_postponed: Option<DefermentOfKYC>
  ) -> PromiseOrValue<()> {
    self.assert_live();
    assert_one_yocto();
    let (
      bounty,
      claims,
      claim_idx
    ) = self.check_if_allowed_to_approve_claim_by_status(id, claimer.clone());
    let claim = claims[claim_idx].clone();

    bounty.check_access_rights();
    assert!(
      bounty.is_claim_deadline_correct(claim.deadline),
      "The claim deadline is no longer correct"
    );

    let place_of_check = PlaceOfCheckKYC::DecisionOnClaim {
      approve,
      is_kyc_delayed: kyc_postponed.clone()
    };
    if self.is_kyc_check_required(bounty, None, None, place_of_check.clone()) {
      self.check_if_claimer_in_kyc_whitelist(id, claimer, place_of_check, None)

    } else {
      self.internal_approval_and_save_claim(id, claimer, approve, kyc_postponed)
    }
  }

  /// Report that bounty is done. Creates a proposal to vote for paying out the bounty,
  /// if validators DAO are assigned.
  /// Only creator of the claim can call `done`.
  #[payable]
  pub fn bounty_done(&mut self, id: BountyIndex, description: String) -> PromiseOrValue<()> {
    self.assert_live();
    assert_one_yocto();

    let mut bounty = self.get_bounty(id.clone());
    assert!(
      bounty.status == BountyStatus::Claimed ||
        bounty.status == BountyStatus::ManyClaimed ||
        bounty.status == BountyStatus::Completed && bounty.is_contest_or_hackathon(),
      "Bounty status does not allow to completion"
    );

    let sender_id = env::predecessor_account_id();
    let (mut claims, claim_idx) = self.internal_get_claims(id.clone(), &sender_id);
    assert!(
      claims[claim_idx].status == ClaimStatus::InProgress ||
        claims[claim_idx].status == ClaimStatus::Competes,
      "The claim status does not allow to complete the bounty"
    );

    if claims[claim_idx].is_claim_expired(&bounty) {
      return self.internal_set_claim_expiry_status(id, &sender_id, &mut bounty, claim_idx, &mut claims);
    }

    if bounty.status == BountyStatus::Completed &&
      claims[claim_idx].status == ClaimStatus::Competes
    {
      self.internal_participants_decrement(&mut bounty);
      self.internal_update_bounty(&id, bounty.clone());
      return self.internal_claim_closure(
        &sender_id,
        bounty.owner,
        bounty.status,
        claim_idx,
        &mut claims,
        None,
        true
      );
    }

    let place_of_check = PlaceOfCheckKYC::ClaimDone { description };
    if self.is_kyc_check_required(
      bounty,
      Some(&claims[claim_idx]),
      Some(sender_id.clone()),
      place_of_check.clone()
    ) {
      self.check_if_claimer_in_kyc_whitelist(id, sender_id, place_of_check, None)

    } else {
      self.internal_add_proposal_and_update_claim(id, sender_id, place_of_check)
    }
  }

  /// Give up working on the bounty.
  /// Only the claimant can call this method.
  #[payable]
  pub fn bounty_give_up(&mut self, id: BountyIndex) -> PromiseOrValue<()> {
    self.assert_live();
    assert_one_yocto();

    let mut bounty = self.get_bounty(id.clone());
    let sender_id = env::predecessor_account_id();
    let (mut claims, claim_idx) = self.internal_get_claims(id.clone(), &sender_id);
    assert!(
      (claims[claim_idx].status == ClaimStatus::New ||
        claims[claim_idx].status == ClaimStatus::InProgress ||
        claims[claim_idx].status == ClaimStatus::Competes ||
        claims[claim_idx].status == ClaimStatus::ReadyToStart),
      "The claim status does not allow to give up the bounty"
    );

    if claims[claim_idx].status == ClaimStatus::InProgress ||
      claims[claim_idx].status == ClaimStatus::Competes &&
        bounty.status == BountyStatus::ManyClaimed
    {
      let return_bond = env::block_timestamp() - claims[claim_idx].get_start_time(&bounty).0 <=
        self.config.clone().to_config().bounty_forgiveness_period.0;

      self.internal_reset_bounty_to_initial_state(
        id,
        &sender_id,
        &mut bounty,
        claim_idx,
        &mut claims,
        Some(ClaimStatus::Canceled),
        return_bond
      )

    } else {
      if claims[claim_idx].status == ClaimStatus::Competes {
        self.internal_participants_decrement(&mut bounty);
      } else if claims[claim_idx].status == ClaimStatus::ReadyToStart {
        if bounty.is_one_bounty_for_many_claimants() {
          self.internal_occupied_slots_decrement(&mut bounty);
        } else {
          let slot = claims[claim_idx].slot.clone().unwrap();
          self.internal_reset_slot(&mut bounty, slot);
        }
      }
      self.internal_update_bounty(&id, bounty.clone());
      self.internal_claim_closure(
        &sender_id,
        bounty.owner,
        bounty.status,
        claim_idx,
        &mut claims,
        Some(ClaimStatus::Canceled),
        true
      )
    }
  }

  /// Cancel the bounty and return the funds to the owner.
  /// Only the owner of the bounty can call this method.
  #[payable]
  pub fn bounty_cancel(&mut self, id: BountyIndex) -> PromiseOrValue<()> {
    self.assert_live();
    assert_one_yocto();

    let bounty = self.get_bounty(id.clone());
    assert!(
      bounty.status == BountyStatus::New || bounty.status == BountyStatus::AwaitingClaims,
      "Bounty status does not allow cancellation"
    );

    let sender_id = env::predecessor_account_id();
    assert_eq!(bounty.owner, sender_id, "Only the owner of the bounty can call this method");

    self.internal_refund_bounty_amount(id, bounty)
  }

  #[payable]
  pub fn bounty_action(&mut self, id: BountyIndex, action: BountyAction) -> PromiseOrValue<()> {
    self.assert_live();
    assert_one_yocto();
    let mut bounty = self.get_bounty(id.clone());

    if !action.need_to_finalize_claim() {
      assert!(
        bounty.status == BountyStatus::Claimed ||
          bounty.status == BountyStatus::ManyClaimed,
        "Bounty status does not allow approval of the execution result"
      );

      match action.clone() {
        BountyAction::ClaimApproved { receiver_id } |
        BountyAction::ClaimRejected { receiver_id } => {
          bounty.check_access_rights();

          let (mut claims, claim_idx) = self.internal_get_claims(id.clone(), &receiver_id);
          assert!(
            matches!(claims[claim_idx].status, ClaimStatus::Completed),
            "The claim status does not allow approval of the execution result"
          );

          let approve = matches!(action, BountyAction::ClaimApproved { .. });
          Self::assert_postpaid_is_ready(&bounty, &claims[claim_idx], approve);
          self.assert_multitasking_requirements(id, &bounty, approve, &receiver_id);

          let result = if approve {
            self.internal_bounty_payout(id, Some(receiver_id))
          } else {
            self.internal_reject_claim(id, receiver_id, &mut bounty, claim_idx, &mut claims)
          };

          result
        }
        BountyAction::SeveralClaimsApproved => {
          bounty.check_access_rights();

          assert!(
            bounty.is_different_tasks(),
            "This action is only available for the DifferentTasks mode"
          );
          assert!(
            Self::internal_are_all_slots_complete(&bounty, None),
            "Not all tasks have already been completed"
          );
          if bounty.is_payment_outside_contract() {
            assert!(
              bounty.multitasking.clone().unwrap().are_all_slots_confirmed(),
              "Not all tasks are confirmed to be paid"
            );
          }

          self.internal_bounty_payout(id, None)
        }
        BountyAction::Finalize { .. } => {
          if bounty.multitasking.is_none() || bounty.is_different_tasks() {
            let active_claim = if bounty.multitasking.is_none() {
              Some(self.internal_find_active_claim(id.clone()))
            } else {
              None
            };
            let result = self.internal_finalize_active_claim(id, bounty, active_claim);
            if result.is_some() {
              return result.unwrap();
            }
          }

          env::panic_str("This bounty is not subject to finalization");
        }
      }
    } else {
      let receiver_id = action.get_finalize_action_receiver().unwrap();
      self.internal_finalize_some_claim(id, receiver_id, &mut bounty)
    }
  }

  #[payable]
  pub fn bounty_update(&mut self, id: BountyIndex, bounty_update: BountyUpdate) {
    self.assert_live();
    assert_one_yocto();

    let mut bounty = self.get_bounty(id.clone());
    assert!(
      bounty.status == BountyStatus::New ||
        bounty.status == BountyStatus::Claimed ||
        bounty.status == BountyStatus::ManyClaimed ||
        bounty.status == BountyStatus::AwaitingClaims,
      "Bounty status does not allow updating"
    );

    let sender_id = env::predecessor_account_id();
    assert_eq!(bounty.owner, sender_id, "Only the owner of the bounty can call this method");

    let claims = self.get_bounty_claims_by_id(id.clone());
    let found_claim = claims
      .into_iter()
      .find(
        |c|
          c.1.status == ClaimStatus::New ||
            c.1.status == ClaimStatus::InProgress ||
            c.1.status == ClaimStatus::Competes ||
            c.1.status == ClaimStatus::ReadyToStart
      )
      .is_some();
    let mut changed = false;

    if bounty_update.metadata.is_some() {
      assert!(
        !found_claim && bounty.status == BountyStatus::New,
        "Metadata cannot be changed if there are already claims"
      );
      bounty.metadata = bounty_update.metadata.unwrap();
      changed = true;
    }
    if bounty_update.claimer_approval.is_some() {
      assert!(
        !found_claim && bounty.status == BountyStatus::New,
        "The approval setting cannot be changed if there are already claims"
      );
      bounty.claimer_approval = bounty_update.claimer_approval.unwrap();
      changed = true;
    }
    if bounty_update.deadline.is_some() {
      if found_claim {
        assert_ne!(
          bounty.deadline.get_deadline_type(),
          3,
          "The deadline cannot be limited if there are already claims"
        );
        if bounty_update.deadline.clone().unwrap().get_deadline_type() != 3 {
          assert_eq!(
            bounty.deadline.clone().get_deadline_type(),
            bounty_update.deadline.clone().unwrap().get_deadline_type(),
            "The type of deadline cannot be changed if there are already claims"
          );
          assert!(
            bounty_update.deadline.clone().unwrap().get_deadline_value().0 >
              bounty.deadline.clone().get_deadline_value().0,
            "The deadline can be increased only in the upward direction"
          );
        }
      }
      bounty.deadline = bounty_update.deadline.unwrap();
      changed = true;
    }
    if bounty_update.reviewers.is_some() {
      assert!(
        matches!(bounty.status, BountyStatus::New),
        "Bounty status does not allow updating"
      );
      assert!(
        bounty.reviewers.is_none() ||
          match bounty.reviewers.clone().unwrap() {
            Reviewers::MoreReviewers { .. } => true,
            _ => false
          },
        "Validators DAO settings cannot be changed"
      );
      assert!(
        match bounty_update.reviewers.clone().unwrap() {
          ReviewersParams::MoreReviewers { .. } => true,
          _ => false
        },
        "It is not possible to start using Validators DAO after creating a bounty"
      );
      let more_reviewers = bounty_update.reviewers.clone().unwrap().get_more_reviewers();
      if bounty.reviewers.is_some() {
        let prev_more_reviewers = bounty.reviewers.clone().unwrap().get_more_reviewers();
        assert_ne!(
          more_reviewers,
          prev_more_reviewers,
          "The list of reviewers has not changed"
        );
      }
      bounty.reviewers = if more_reviewers.len() > 0 {
        Some(bounty_update.reviewers.unwrap().to_reviewers())
      } else {
        None
      };
      changed = true;
    }
    if bounty_update.amount.is_some() {
      assert!(
        bounty_update.amount.unwrap().0 > 0,
        "The bounty amount cannot be zero"
      );
      assert!(
        bounty.is_payment_outside_contract() &&
          !bounty.is_one_bounty_for_many_claimants() && !bounty.is_different_tasks(),
        "The amount of the bounty cannot be changed"
      );
      assert!(
        bounty.postpaid.clone().unwrap().get_payment_timestamps().payment_at.is_none(),
        "The amount of the bounty cannot be changed after the payment has been confirmed"
      );
      bounty.amount = bounty_update.amount.unwrap();
      changed = true;
    }

    assert!(changed, "No changes found");
    self.check_bounty(&bounty);
    self.internal_update_bounty(&id, bounty);
  }

  #[payable]
  pub fn extend_claim_deadline(&mut self, id: BountyIndex, claimer: AccountId, deadline: U64) {
    self.assert_live();
    assert_one_yocto();

    let bounty = self.get_bounty(id.clone());
    assert!(
      bounty.status == BountyStatus::Claimed ||
        bounty.status == BountyStatus::ManyClaimed,
      "Bounty status does not allow to postpone the claim deadline"
    );

    let sender_id = env::predecessor_account_id();
    assert_eq!(bounty.owner, sender_id, "Only the owner of the bounty can call this method");

    let (mut claims, claim_idx) = self.internal_get_claims(id, &claimer);
    let mut bounty_claim = claims[claim_idx].clone();
    assert!(
      bounty_claim.status == ClaimStatus::InProgress ||
        bounty_claim.status == ClaimStatus::Competes,
      "Claim status does not allow to postpone the deadline"
    );
    assert!(
      bounty.is_claim_deadline_correct(deadline) && deadline.0 > bounty_claim.deadline.0,
      "The claim deadline is incorrect"
    );

    bounty_claim.deadline = deadline;
    claims[claim_idx] = bounty_claim;
    self.internal_save_claims(&claimer, &claims);
  }

  #[payable]
  pub fn bounty_create(
    &mut self,
    bounty_create: BountyCreate,
    token_id: Option<AccountId>,
    amount: U128
  ) {
    self.assert_live();
    assert_one_yocto();

    let sender_id = env::predecessor_account_id();
    assert!(bounty_create.postpaid.is_some(), "The postpaid parameter is incorrect");
    assert!(
      self.is_owner_whitelisted(sender_id.clone()),
      "You are not allowed to create postpaid bounties"
    );
    if token_id.is_some() {
      self.assert_that_token_is_allowed(&token_id.clone().unwrap());
    }

    self.internal_create_bounty(bounty_create, &sender_id, token_id, amount);
  }

  #[payable]
  pub fn mark_as_paid(&mut self, id: BountyIndex, claimer: Option<AccountId>) {
    self.assert_live();
    assert_one_yocto();

    let sender_id = env::predecessor_account_id();
    let mut bounty = self.get_bounty(id);

    assert!(
      bounty.is_payment_outside_contract(),
      "This method is allowed to be used only when paying outside the contract"
    );
    assert!(
      bounty.status == BountyStatus::Claimed || bounty.status == BountyStatus::ManyClaimed,
      "Bounty status does not allow this action"
    );
    assert_eq!(bounty.owner, sender_id, "Only the owner of the bounty can call this method");

    let (mut claims, claim_idx) = if bounty.multitasking.is_some() {
      assert!(claimer.is_some(), "The claimer parameter is mandatory for multitasking bounties");
      self.internal_get_claims(id.clone(), &claimer.clone().unwrap())
    } else {
      assert!(claimer.is_none(), "The claimer parameter is required only for multitasking bounties");
      let result = self.internal_find_active_claim(id.clone());
      (result.1, result.2)
    };

    let mut bounty_claim = claims[claim_idx].clone();
    assert!(
      bounty_claim.status == ClaimStatus::Completed ||
        bounty_claim.status == ClaimStatus::CompletedWithDispute,
      "The status of the claim does not allow this action"
    );

    let payment_timestamps = Self::internal_get_payment_timestamps(&bounty, &bounty_claim);
    assert!(
      payment_timestamps.payment_at.is_none(),
      "This action has already been performed"
    );

    if bounty.is_one_bounty_for_many_claimants() || bounty.is_different_tasks() {
      bounty_claim.set_payment_at(Some(U64::from(env::block_timestamp())));
      claims[claim_idx] = bounty_claim;
      self.internal_save_claims(&claimer.clone().unwrap(), &claims);

    } else {
      if bounty.is_contest_or_hackathon() {
        let multitasking = bounty.multitasking.clone().unwrap();
        let competition_winner = multitasking.get_competition_winner();
        assert!(
          competition_winner.is_none(),
          "The winner of the contest has already been decided"
        );
        bounty.multitasking = Some(multitasking.set_competition_winner(claimer));
      }

      bounty.postpaid = Some(
        bounty.postpaid.clone().unwrap().set_payment_at(Some(U64::from(env::block_timestamp())))
      );
      self.internal_update_bounty(&id, bounty);
    }
  }

  #[payable]
  pub fn confirm_payment(&mut self, id: BountyIndex) {
    self.assert_live();
    assert_one_yocto();

    let sender_id = env::predecessor_account_id();
    let mut bounty = self.get_bounty(id);

    assert!(
      bounty.is_payment_outside_contract(),
      "This method is allowed to be used only when paying outside the contract"
    );
    assert!(
      bounty.status == BountyStatus::Claimed || bounty.status == BountyStatus::ManyClaimed,
      "Bounty status does not allow this action"
    );

    let (mut claims, claim_idx) = if bounty.multitasking.is_some() {
      self.internal_get_claims(id.clone(), &sender_id)
    } else {
      let result = self.internal_find_active_claim(id);
      assert_eq!(result.0, sender_id, "Only the owner of the claim is allowed this action");
      (result.1, result.2)
    };

    let mut bounty_claim = claims[claim_idx].clone();
    assert!(
      bounty_claim.status == ClaimStatus::Completed ||
        bounty_claim.status == ClaimStatus::CompletedWithDispute,
      "The status of the claim does not allow this action"
    );

    let payment_timestamps = Self::internal_get_payment_timestamps(&bounty, &bounty_claim);
    assert!(
      payment_timestamps.payment_at.is_some(),
      "Payment by the bounty owner has not yet been confirmed"
    );
    assert!(
      payment_timestamps.payment_confirmed_at.is_none(),
      "This action has already been performed"
    );

    if bounty.is_one_bounty_for_many_claimants() || bounty.is_different_tasks() {
      bounty_claim.set_payment_confirmed_at(Some(U64::from(env::block_timestamp())));
      claims[claim_idx] = bounty_claim.clone();
      self.internal_save_claims(&sender_id, &claims);

      if bounty.is_different_tasks() {
        self.internal_confirm_slot(&mut bounty, bounty_claim.slot.clone().unwrap());
        self.internal_update_bounty(&id, bounty);
      }

    } else {
      if bounty.is_contest_or_hackathon() {
        let competition_winner = bounty.multitasking.clone().unwrap().get_competition_winner();
        assert!(
          competition_winner.is_some() &&
            competition_winner.clone().unwrap() == sender_id,
          "You aren't a contest winner"
        );
      }

      bounty.postpaid = Some(
        bounty.postpaid.clone().unwrap().set_payment_confirmed_at(
          Some(U64::from(env::block_timestamp()))
        )
      );
      self.internal_update_bounty(&id, bounty);
    }
  }

  #[payable]
  pub fn start_competition(&mut self, id: BountyIndex) {
    self.assert_live();
    assert_one_yocto();

    let sender_id = env::predecessor_account_id();
    let mut bounty = self.get_bounty(id);

    assert!(
      matches!(bounty.status, BountyStatus::New),
      "Bounty status does not allow this action"
    );

    assert_eq!(bounty.owner, sender_id, "Only the owner of the bounty can call this method");

    let is_allowed_to_start = bounty.multitasking.is_some() &&
      match bounty.multitasking.clone().unwrap() {
        Multitasking::ContestOrHackathon {
          allowed_create_claim_to,
          start_conditions,
          ..
        } => {
          if allowed_create_claim_to.is_some() {
            match allowed_create_claim_to.unwrap() {
              DateOrPeriod::Date { date } => {
                assert!(
                  env::block_timestamp() > date.0,
                  "The start is possible only after the date before which it is allowed to create new claims"
                );
              },
              _ => {}
            }
          }
          assert!(
            !bounty.multitasking.clone().unwrap().has_competition_started(),
            "The competition has already started"
          );
          start_conditions.is_some() && matches!(start_conditions.unwrap(), StartConditions::ManuallyStart)
        },
        _ => false
      };
    if !is_allowed_to_start {
      env::panic_str("Manual start of the competition is not supported");
    }

    let claims = bounty.multitasking.clone().unwrap().get_participants();
    assert!(claims > 1, "Too few participants to start the competition");

    self.internal_start_competition(&mut bounty, Some(U64::from(env::block_timestamp())));
    self.internal_change_status_and_save_bounty(&id, &mut bounty, BountyStatus::ManyClaimed);
  }

  #[payable]
  pub fn withdraw_platform_fee(&mut self, token_id: AccountId) -> PromiseOrValue<()> {
    self.assert_live();
    assert_one_yocto();
    assert_eq!(
      self.recipient_of_platform_fee
        .clone().expect("The recipient of the platform fee is not specified"),
      env::predecessor_account_id(),
      "This account does not have permission to perform this action"
    );
    let balance = self.internal_get_unlocked_platform_fee_amount(token_id.clone());
    assert!(balance.0 > 0, "The available balance of commission is zero");

    Self::internal_fees_payout(
      token_id,
      balance,
      env::predecessor_account_id(),
      "Bounties platform fee transfer",
      true,
    )
  }

  #[payable]
  pub fn withdraw_validators_dao_fee(&mut self, token_id: AccountId) -> PromiseOrValue<()> {
    self.assert_live();
    assert_one_yocto();
    let dao_account_id = env::predecessor_account_id();
    let balance = self.internal_get_unlocked_validators_dao_fee_amount(
      dao_account_id.clone(),
      token_id.clone()
    );
    assert!(balance.0 > 0, "The available balance of commission is zero");

    Self::internal_fees_payout(
      token_id,
      balance,
      dao_account_id,
      "Bounties validators DAO fee transfer",
      false,
    )
  }

  #[payable]
  pub fn withdraw(&mut self, id: BountyIndex) -> PromiseOrValue<()> {
    self.assert_live();
    assert_one_yocto();

    let receiver_id = env::predecessor_account_id();
    let bounty = self.get_bounty(id);

    assert!(
      bounty.is_different_tasks(),
      "This action is only available for the DifferentTasks mode"
    );
    assert!(
      matches!(bounty.status, BountyStatus::Completed),
      "Bounty status does not allow this action"
    );

    let (claims, claim_idx) = self.internal_get_claims(id.clone(), &receiver_id);
    assert!(
      claims[claim_idx].status == ClaimStatus::Completed ||
        claims[claim_idx].status == ClaimStatus::CompletedWithDispute,
      "The claim status does not allow this action"
    );

    let slot = claims[claim_idx].slot.clone().unwrap();
    let slot_env = bounty.multitasking.clone().unwrap().get_slot_env(slot);
    assert_eq!(
      slot_env.expect("The slot is not occupied").participant,
      receiver_id,
      "This slot belongs to another account"
    );

    self.internal_bounty_withdraw(id, bounty, receiver_id, slot)
  }

  #[payable]
  pub fn update_validators_dao_params(&mut self, id: BountyIndex, dao_params: ValidatorsDaoParams) {
    self.assert_live();
    assert_one_yocto();
    let mut bounty = self.get_bounty(id.clone());

    assert_eq!(
      bounty.owner,
      env::predecessor_account_id(),
      "Only the owner of the bounty can call this method"
    );
    assert!(
      bounty.is_validators_dao_used(),
      "DAO validators are not used for this bounty"
    );
    bounty.assert_validators_dao_account_id(dao_params.clone().account_id);

    bounty.reviewers = Some(Reviewers::ValidatorsDao {validators_dao: dao_params.to_validators_dao()});
    self.internal_update_bounty(&id, bounty);
  }

  #[payable]
  pub fn open_dispute(&mut self, id: BountyIndex, description: String) -> PromiseOrValue<()> {
    self.assert_live();
    assert_one_yocto();

    assert!(
      self.dispute_contract.is_some(),
      "Opening a dispute is not supported by this contract"
    );

    let bounty = self.get_bounty(id.clone());
    assert!(
      bounty.status == BountyStatus::Claimed || bounty.status == BountyStatus::ManyClaimed,
      "Bounty status does not allow opening a dispute"
    );

    let sender_id = env::predecessor_account_id();
    let (claims, claim_idx) = self.internal_get_claims(id.clone(), &sender_id);
    assert!(
      matches!(claims[claim_idx].status, ClaimStatus::Rejected),
      "The claim status does not allow opening a dispute"
    );

    assert!(
      !self.is_deadline_for_opening_dispute_expired(&claims[claim_idx]),
      "The period for opening a dispute has expired"
    );

    self.internal_create_dispute(id, &sender_id, bounty, description)
  }

  #[payable]
  pub fn dispute_result(
    &mut self,
    id: BountyIndex,
    claimer: AccountId,
    success: bool
  ) -> PromiseOrValue<()> {
    self.assert_live();
    assert_one_yocto();

    assert!(
      self.dispute_contract.is_some(),
      "The use of dispute is not supported by this contract"
    );

    assert_eq!(
      self.dispute_contract.clone().unwrap(),
      env::predecessor_account_id(),
      "This method can only be called by a dispute contract"
    );

    let mut bounty = self.get_bounty(id.clone());
    assert!(
      bounty.status == BountyStatus::Claimed || bounty.status == BountyStatus::ManyClaimed,
      "Bounty status does not allow sending the result of the dispute"
    );

    let (mut claims, claim_idx) = self.internal_get_claims(id.clone(), &claimer);
    assert!(
      matches!(claims[claim_idx].status, ClaimStatus::Disputed),
      "The claim status does not allow opening a dispute"
    );

    if success {
      if bounty.is_different_tasks() {
        self.internal_claim_return_after_dispute(claimer, &mut claims, claim_idx)
      } else {
        self.internal_bounty_payout(id, Some(claimer))
      }
    } else {
      self.internal_reset_bounty_to_initial_state(
        id,
        &claimer,
        &mut bounty,
        claim_idx,
        &mut claims,
        None,
        true
      )
    }
  }

  #[payable]
  pub fn withdraw_non_refunded_bonds(&mut self) -> Promise {
    self.assert_live();
    assert_one_yocto();
    let receiver_id = env::predecessor_account_id();

    assert_eq!(
      self.recipient_of_platform_fee
        .clone().expect("The recipient of the non-refundable bonds is not specified"),
      receiver_id,
      "This account does not have permission to perform this action"
    );
    assert!(self.unlocked_amount > 0, "The amount of non-refunded bonds is now zero");

    let amount = self.unlocked_amount;
    self.unlocked_amount = 0;
    Promise::new(receiver_id).transfer(amount)
  }

  #[private]
  pub fn set_status(&mut self, status: ContractStatus) {
    assert!(
      !matches!(status, ContractStatus::Genesis),
      "The status can't be set to Genesis"
    );
    self.status = status;
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
  use near_sdk::test_utils::{accounts, VMContextBuilder};
  use near_sdk::json_types::{U128, U64};
  use near_sdk::{testing_env, AccountId, Balance};
  use crate::{DEFAULT_BOUNTY_CLAIM_BOND, BountiesContract, Bounty, BountyAction, BountyClaim,
              BountyIndex, BountyMetadata, BountyStatus, ClaimerApproval, ClaimStatus, Config,
              ConfigCreate, ContractStatus, Deadline, FeeStats, KycConfig, Reviewers, TokenDetails, ValidatorsDao,
              ValidatorsDaoParams};

  pub const TOKEN_DECIMALS: u8 = 18;
  pub const MAX_DEADLINE: U64 = U64(1_000_000_000 * 60 * 60 * 24 * 7);

  fn get_token_id() -> AccountId {
    "token_id".parse().unwrap()
  }

  fn get_disputes_contract() -> AccountId {
    "disputes".parse().unwrap()
  }

  pub const fn d(value: Balance, decimals: u8) -> Balance {
    value * 10u128.pow(decimals as _)
  }

  fn add_bounty(
    contract: &mut BountiesContract,
    owner: &AccountId,
    validators_dao: Option<ValidatorsDao>
  ) -> BountyIndex {
    let bounty_index: BountyIndex = 0;
    let bounty = Bounty {
      token: Some(get_token_id()),
      amount: U128(d(2_000, TOKEN_DECIMALS)),
      platform_fee: U128(d(200, TOKEN_DECIMALS)),
      dao_fee: if validators_dao.is_some() {
        U128(d(200, TOKEN_DECIMALS))
      } else { U128(0) },
      metadata: BountyMetadata {
        title: "test".to_string(),
        description: "test".to_string(),
        category: "Other".to_string(),
        attachments: None,
        experience: None,
        tags: None,
        acceptance_criteria: None,
        contact_details: None,
      },
      deadline: Deadline::MaxDeadline {max_deadline: MAX_DEADLINE},
      claimer_approval: ClaimerApproval::WithoutApproval,
      reviewers: if validators_dao.is_some() {
        Some(Reviewers::ValidatorsDao {validators_dao: validators_dao.unwrap()})
      } else {
        None
      },
      owner: owner.clone(),
      status: BountyStatus::New,
      created_at: U64::from(0),
      kyc_config: KycConfig::KycNotRequired,
      postpaid: None,
      multitasking: None,
    };
    contract.internal_update_bounty(&bounty_index, bounty.clone());
    contract.account_bounties.insert(owner, &vec![bounty_index]);
    contract.last_bounty_id = bounty_index.clone() + 1;
    add_token(contract);
    contract.internal_total_fees_receiving_funds(&bounty, bounty.platform_fee, bounty.dao_fee);

    bounty_index
  }

  fn add_token(
    contract: &mut BountiesContract,
  ) {
    contract.total_fees.insert(&get_token_id(), &FeeStats::new());
    contract.tokens.insert(&get_token_id(), &TokenDetails {
      enabled: true,
      min_amount_for_kyc: None,
    });
  }

  fn bounty_claim(
    context: &mut VMContextBuilder,
    contract: &mut BountiesContract,
    id: BountyIndex,
    claimer: &AccountId
  ) {
    let bond = Config::default().bounty_claim_bond.0;
    testing_env!(context
      .predecessor_account_id(claimer.clone())
      .attached_deposit(bond.clone())
      .block_timestamp(0)
      .build());
    let deadline = U64(1_000_000_000 * 60 * 60 * 24 * 2);
    let description = "Test description".to_string();

    contract.bounty_claim(id, deadline, description.clone(), None);

    let bounty = contract.bounties.get(&id).unwrap().to_bounty();
    if bounty.is_validators_dao_used() && contract.is_approval_required(&bounty, &claimer) {
      contract.internal_create_claim(
        id,
        claimer.clone(),
        deadline,
        description,
        Some(U64(1)),
        None,
      );
    }
  }

  fn bounty_done(
    context: &mut VMContextBuilder,
    contract: &mut BountiesContract,
    id: BountyIndex,
    claimer: &AccountId
  ) {
    testing_env!(context
      .predecessor_account_id(claimer.clone())
      .attached_deposit(1)
      .build());
    contract.bounty_done(id, "test description".to_string());
  }

  fn bounty_give_up(
    context: &mut VMContextBuilder,
    contract: &mut BountiesContract,
    id: BountyIndex,
    claimer: &AccountId,
    current_block_timestamp: u64
  ) {
    testing_env!(context
      .predecessor_account_id(claimer.clone())
      .attached_deposit(1)
      .block_timestamp(current_block_timestamp)
      .build());
    contract.bounty_give_up(id);
  }

  fn bounty_action(
    context: &mut VMContextBuilder,
    contract: &mut BountiesContract,
    id: BountyIndex,
    project_owner: &AccountId,
    action: BountyAction,
  ) {
    testing_env!(context
      .predecessor_account_id(project_owner.clone())
      .attached_deposit(1)
      .build());
    contract.bounty_action(id, action);
  }

  #[test]
  #[should_panic(expected = "Admin whitelist must contain at least one account")]
  fn test_failed_to_init_contract() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.predecessor_account_id(accounts(0)).build());
    BountiesContract::new(
      vec![],
      None,
      None,
      None,
      None,
      None
    );
  }

  #[test]
  #[should_panic(expected = "The contract status is not Live")]
  fn test_contract_status_is_not_live() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let id = add_bounty(&mut contract, &accounts(1), None);
    let claimer = accounts(2);
    contract.set_status(ContractStatus::ReadOnly);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
  }

  #[test]
  fn test_add_to_admins_whitelist() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let account_ids: Vec<AccountId> = vec![accounts(1), accounts(2)];
    contract.add_to_admins_whitelist(None, Some(account_ids.clone()));
    assert_eq!(contract.get_admins_whitelist(), vec![accounts(0), accounts(1), accounts(2)]);

    contract.add_to_admins_whitelist(Some(accounts(3)), None);
    assert_eq!(
      contract.get_admins_whitelist(),
      vec![accounts(0), accounts(1), accounts(2), accounts(3)]
    );
  }

  #[test]
  fn test_remove_from_admins_whitelist() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    contract.add_to_admins_whitelist(
      None,
      Some(vec![accounts(1), accounts(2), accounts(3)])
    );

    contract.remove_from_admins_whitelist(Some(accounts(3)), None);
    assert_eq!(contract.get_admins_whitelist(), vec![accounts(0), accounts(1), accounts(2)]);
    contract.remove_from_admins_whitelist(None, Some(vec![accounts(1), accounts(2)]));
    assert_eq!(contract.get_admins_whitelist(), vec![accounts(0)]);
  }

  #[test]
  #[should_panic(expected = "Expected either account_id or account_ids")]
  fn test_failed_to_whitelist_admin() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    contract.add_to_admins_whitelist(None, None);
  }

  #[test]
  #[should_panic(expected = "Requires attached deposit of exactly 1 yoctoNEAR")]
  fn test_add_to_admins_whitelist_without_deposit() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.predecessor_account_id(accounts(0)).build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    contract.add_to_admins_whitelist(Some(accounts(1)), None);
  }

  #[test]
  #[should_panic(expected = "Not in admin whitelist")]
  fn test_add_to_admins_whitelist_by_other_user() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.predecessor_account_id(accounts(0)).build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    testing_env!(context.predecessor_account_id(accounts(1)).attached_deposit(1).build());
    contract.set_status(ContractStatus::Live);
    contract.add_to_admins_whitelist(Some(accounts(1)), None);
  }

  #[test]
  #[should_panic(expected = "Cannot remove all accounts from admin whitelist")]
  fn test_remove_from_admins_whitelist_all_members() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    contract.remove_from_admins_whitelist(Some(accounts(0)), None);
  }

  #[test]
  fn test_add_token() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    contract.add_token_id(get_token_id(), None);
    // For the unit test, the list of tokens has not changed.
    // The action is performed in the promise callback function (see simulation test).
  }

  #[test]
  #[should_panic(expected = "The token already exists")]
  fn test_token_already_exists() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    add_token(&mut contract);

    contract.add_token_id(get_token_id(), None);
  }

  #[test]
  fn test_get_tokens() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    add_token(&mut contract);

    assert_eq!(
      contract.get_tokens(),
      [
        (
          get_token_id(),
          TokenDetails {
            enabled: true,
            min_amount_for_kyc: None
          }
        )
      ]
    );
  }

  #[test]
  fn test_change_config() {
    let mut context = VMContextBuilder::new();
    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let mut contract = BountiesContract::new(
      vec![accounts(0).into()],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);

    let config_create = ConfigCreate {
      bounty_claim_bond: U128::from(2_000_000_000_000_000_000_000_000),
      bounty_forgiveness_period: U64::from(1_000_000_000 * 60 * 60 * 24 * 7),
      period_for_opening_dispute: U64::from(1_000_000_000 * 60 * 60 * 24 * 20),
      platform_fee_percentage: 7_000,
      validators_dao_fee_percentage: 5_000,
      penalty_platform_fee_percentage: 500,
      penalty_validators_dao_fee_percentage: 500,
    };
    contract.change_config(config_create.clone());
    let config = contract.get_config();
    assert_eq!(config_create, ConfigCreate {
      bounty_claim_bond: config.bounty_claim_bond,
      bounty_forgiveness_period: config.bounty_forgiveness_period,
      period_for_opening_dispute: config.period_for_opening_dispute,
      platform_fee_percentage: config.platform_fee_percentage,
      validators_dao_fee_percentage: config.validators_dao_fee_percentage,
      penalty_platform_fee_percentage: config.penalty_platform_fee_percentage,
      penalty_validators_dao_fee_percentage: config.penalty_validators_dao_fee_percentage,
    });
  }

  #[test]
  fn test_bounty_claim() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![get_token_id()],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let id = add_bounty(&mut contract, &accounts(1), None);

    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    assert_eq!(
      contract.bounty_claimers.get(&claimer).unwrap()[0].clone().to_bounty_claim(),
      BountyClaim {
        bounty_id: id.clone(),
        created_at: U64(0),
        start_time: Some(U64(0)),
        deadline: U64(1_000_000_000 * 60 * 60 * 24 * 2),
        status: ClaimStatus::InProgress,
        bounty_payout_proposal_id: None,
        approve_claimer_proposal_id: None,
        rejected_timestamp: None,
        dispute_id: None,
        description: "Test description".to_string(),
        is_kyc_delayed: None,
        payment_timestamps: None,
        slot: None,
        bond: Some(DEFAULT_BOUNTY_CLAIM_BOND),
      }
    );
    assert_eq!(contract.bounty_claimer_accounts.get(&id).unwrap()[0], claimer);
    assert_eq!(contract.locked_amount, Config::default().bounty_claim_bond.0);
    assert_eq!(contract.bounties.get(&id).unwrap().to_bounty().status, BountyStatus::Claimed);
  }

  #[test]
  #[should_panic(expected = "Bounty wrong bond")]
  fn test_bounty_claim_without_bond() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let id = add_bounty(&mut contract, &accounts(1), None);

    testing_env!(context
      .predecessor_account_id(accounts(2))
      .build());
    contract.bounty_claim(
      id,
      U64(1_000_000_000 * 60 * 60 * 24 * 2),
      "Test description".to_string(),
      None
    );
  }

  #[test]
  #[should_panic(expected = "Bounty status does not allow to submit a claim")]
  fn test_bounty_claim_with_incorrect_status() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let id = add_bounty(&mut contract, &accounts(1), None);
    bounty_claim(&mut context, &mut contract, id.clone(), &accounts(2));

    testing_env!(context
      .predecessor_account_id(accounts(3))
      .attached_deposit(Config::default().bounty_claim_bond.0)
      .build());
    contract.bounty_claim(
      id,
      U64(1_000_000_000 * 60 * 60 * 24 * 2),
      "Test description".to_string(),
      None
    );
  }

  #[test]
  #[should_panic(expected = "Bounty wrong deadline")]
  fn test_bounty_claim_with_incorrect_deadline() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let id = add_bounty(&mut contract, &accounts(1), None);

    testing_env!(context
      .predecessor_account_id(accounts(2))
      .attached_deposit(Config::default().bounty_claim_bond.0)
      .build());
    contract.bounty_claim(
      id,
      U64(MAX_DEADLINE.0 + 1),
      "Test description".to_string(),
      None
    );
  }

  #[test]
  #[should_panic(expected = "Bounty not found")]
  fn test_claim_for_non_existent_bounty() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let id = add_bounty(&mut contract, &accounts(1), None);

    testing_env!(context
      .predecessor_account_id(accounts(2))
      .attached_deposit(Config::default().bounty_claim_bond.0)
      .build());
    contract.bounty_claim(
      id + 1,
      MAX_DEADLINE,
      "Test description".to_string(),
      None
    );
  }

  #[test]
  fn test_bounty_done() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let id = add_bounty(&mut contract, &accounts(1), None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);

    bounty_done(&mut context, &mut contract, id.clone(), &claimer);
    assert_eq!(
      contract.bounty_claimers.get(&claimer).unwrap()[0].clone().to_bounty_claim().status,
      ClaimStatus::Completed
    );
    assert_eq!(contract.bounties.get(&id).unwrap().to_bounty().status, BountyStatus::Claimed);
  }

  #[test]
  #[should_panic(expected = "Bounty status does not allow to completion")]
  fn test_bounty_done_with_incorrect_bounty_status() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let id = add_bounty(&mut contract, &accounts(1), None);
    let claimer = accounts(2);

    bounty_done(&mut context, &mut contract, id.clone(), &claimer);
  }

  #[test]
  #[should_panic(expected = "The claim status does not allow to complete the bounty")]
  fn test_bounty_done_with_incorrect_claim_status() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let id = add_bounty(&mut contract, &accounts(1), None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    bounty_done(&mut context, &mut contract, id.clone(), &claimer);
  }

  #[test]
  fn test_bounty_done_on_claim_that_has_expired() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let id = add_bounty(&mut contract, &accounts(1), None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    assert_eq!(contract.locked_amount, Config::default().bounty_claim_bond.0);
    assert_eq!(contract.unlocked_amount, 0);

    testing_env!(context
      .predecessor_account_id(claimer.clone())
      .attached_deposit(1)
      .block_timestamp(1_000_000_000 * 60 * 60 * 24 * 2 + 1)
      .build());
    contract.bounty_done(id, "test description".to_string());
    assert_eq!(
      contract.bounty_claimers.get(&claimer).unwrap()[0].clone().to_bounty_claim().status,
      ClaimStatus::Expired
    );
    assert_eq!(contract.bounties.get(&id).unwrap().to_bounty().status, BountyStatus::New);
    assert_eq!(contract.locked_amount, 0);
    assert_eq!(contract.unlocked_amount, Config::default().bounty_claim_bond.0);
  }

  #[test]
  fn test_bounty_give_up() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let id = add_bounty(&mut contract, &accounts(1), None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    assert_eq!(contract.locked_amount, Config::default().bounty_claim_bond.0);
    assert_eq!(contract.unlocked_amount, 0);

    let bounty_forgiveness_period = Config::default().bounty_forgiveness_period.0;
    bounty_give_up(
      &mut context,
      &mut contract,
      id.clone(),
      &claimer,
      bounty_forgiveness_period.clone() - 1
    );
    assert_eq!(
      contract.bounty_claimers.get(&claimer).unwrap()[0].clone().to_bounty_claim().status,
      ClaimStatus::Canceled
    );
    assert_eq!(contract.bounties.get(&id).unwrap().to_bounty().status, BountyStatus::New);
    assert_eq!(contract.locked_amount, 0);
    assert_eq!(contract.unlocked_amount, 0);

    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    assert_eq!(contract.locked_amount, Config::default().bounty_claim_bond.0);
    assert_eq!(contract.unlocked_amount, 0);

    bounty_give_up(
      &mut context,
      &mut contract,
      id.clone(),
      &claimer,
      bounty_forgiveness_period.clone() + 1
    );
    assert_eq!(
      contract.bounty_claimers.get(&claimer).unwrap()[0].clone().to_bounty_claim().status,
      ClaimStatus::Canceled
    );
    assert_eq!(contract.bounties.get(&id).unwrap().to_bounty().status, BountyStatus::New);
    assert_eq!(contract.locked_amount, 0);
    assert_eq!(contract.unlocked_amount, Config::default().bounty_claim_bond.0);
  }

  #[test]
  #[should_panic(expected = "No claimer found")]
  fn test_bounty_give_up_with_unknown_claimer() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let id = add_bounty(&mut contract, &accounts(1), None);
    let claimer = accounts(2);

    bounty_give_up(
      &mut context,
      &mut contract,
      id.clone(),
      &claimer,
      0,
    );
  }

  #[test]
  #[should_panic(expected = "The claim status does not allow to give up the bounty")]
  fn test_bounty_give_up_with_incorrect_claim_status() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let id = add_bounty(&mut contract, &accounts(1), None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    bounty_give_up(
      &mut context,
      &mut contract,
      id.clone(),
      &claimer,
      0,
    );
  }

  #[test]
  fn test_bounty_action() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let project_owner = accounts(1);
    let id = add_bounty(&mut contract, &project_owner, None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    bounty_action(
      &mut context,
      &mut contract,
      id.clone(),
      &project_owner,
      BountyAction::ClaimRejected { receiver_id: claimer.clone() },
    );
    assert_eq!(
      contract.bounty_claimers.get(&claimer).unwrap()[0].clone().to_bounty_claim().status,
      ClaimStatus::NotCompleted
    );
    assert_eq!(contract.bounties.get(&id).unwrap().to_bounty().status, BountyStatus::New);
    assert_eq!(contract.locked_amount, 0);

    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    bounty_action(
      &mut context,
      &mut contract,
      id.clone(),
      &project_owner,
      BountyAction::ClaimApproved { receiver_id: claimer },
    );
    // For the unit test, the object statuses have not changed.
    // The action is performed in the promise callback function (see simulation test).
  }

  #[test]
  #[should_panic(expected = "This method can only call DAO validators")]
  fn test_bounty_action_by_other_user() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let project_owner = accounts(1);
    let dao_params = ValidatorsDaoParams {
      account_id: "dao".parse().unwrap(),
      add_proposal_bond: U128(10u128.pow(24)),
      gas_for_add_proposal: None,
      gas_for_claim_approval: None,
      gas_for_claimer_approval: None,
    };
    let id = add_bounty(&mut contract, &project_owner, Some(dao_params.to_validators_dao()));
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    bounty_action(
      &mut context,
      &mut contract,
      id.clone(),
      &project_owner,
      BountyAction::ClaimApproved { receiver_id: claimer },
    );
  }

  #[test]
  #[should_panic(expected = "Only the owner of the bounty can call this method")]
  fn test_bounty_action_by_dao() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let project_owner = accounts(1);
    let id = add_bounty(&mut contract, &project_owner, None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    testing_env!(context
      .predecessor_account_id("dao".parse().unwrap())
      .attached_deposit(1)
      .build());
    let action = BountyAction::ClaimApproved { receiver_id: claimer };
    contract.bounty_action(id, action);
  }

  #[test]
  fn test_update_validators_dao_params() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let project_owner = accounts(1);
    let id = add_bounty(
      &mut contract,
      &project_owner,
      Some(
        ValidatorsDaoParams {
          account_id: "dao".parse().unwrap(),
          add_proposal_bond: U128(10u128.pow(24)),
          gas_for_add_proposal: None,
          gas_for_claim_approval: None,
          gas_for_claimer_approval: None,
        }
          .to_validators_dao()
      )
    );

    let new_dao_params = ValidatorsDaoParams {
      account_id: "dao".parse().unwrap(),
      add_proposal_bond: U128(2 * 10u128.pow(24)),
      gas_for_add_proposal: Some(U64(50_000_000_000_000)),
      gas_for_claim_approval: Some(U64(50_000_000_000_000)),
      gas_for_claimer_approval: Some(U64(25_000_000_000_000)),
    };
    testing_env!(context
      .predecessor_account_id(project_owner)
      .attached_deposit(1)
      .build());
    contract.update_validators_dao_params(id, new_dao_params.clone());
    assert_eq!(
      contract.get_bounty(id.clone()).reviewers.unwrap(),
      Reviewers::ValidatorsDao {validators_dao: new_dao_params.to_validators_dao()}
    );
  }

  #[test]
  #[should_panic(expected = "Opening a dispute is not supported by this contract")]
  fn test_open_dispute_without_dispute_support() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      None,
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let project_owner = accounts(1);
    let id = add_bounty(&mut contract, &project_owner, None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    bounty_action(
      &mut context,
      &mut contract,
      id.clone(),
      &project_owner,
      BountyAction::ClaimRejected { receiver_id: claimer.clone() },
    );

    testing_env!(context
      .predecessor_account_id(claimer)
      .attached_deposit(1)
      .build());
    contract.open_dispute(id, "Test description".to_string());
  }

  #[test]
  #[should_panic(expected = "Bounty status does not allow opening a dispute")]
  fn test_open_dispute_with_incorrect_bounty_status() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      Some(get_disputes_contract()),
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let project_owner = accounts(1);
    let id = add_bounty(&mut contract, &project_owner, None);

    testing_env!(context
      .predecessor_account_id(accounts(2))
      .attached_deposit(1)
      .build());
    contract.open_dispute(id, "Test description".to_string());
  }

  #[test]
  #[should_panic(expected = "No claimer found")]
  fn test_open_dispute_by_other_user() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      Some(get_disputes_contract()),
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let project_owner = accounts(1);
    let id = add_bounty(&mut contract, &project_owner, None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    bounty_action(
      &mut context,
      &mut contract,
      id.clone(),
      &project_owner,
      BountyAction::ClaimRejected { receiver_id: claimer.clone() },
    );
    assert_eq!(
      contract.bounty_claimers.get(&claimer).unwrap()[0].clone().to_bounty_claim().status,
      ClaimStatus::Rejected
    );

    testing_env!(context
      .predecessor_account_id(accounts(3))
      .attached_deposit(1)
      .build());
    contract.open_dispute(id, "Test description".to_string());
  }

  #[test]
  #[should_panic(expected = "The claim status does not allow opening a dispute")]
  fn test_open_dispute_with_incorrect_claim_status() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      Some(get_disputes_contract()),
      None,
      None
    );
    contract.set_status(ContractStatus::Live);
    let project_owner = accounts(1);
    let id = add_bounty(&mut contract, &project_owner, None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    testing_env!(context
      .predecessor_account_id(claimer.clone())
      .attached_deposit(1)
      .build());
    contract.open_dispute(id, "Test description".to_string());
  }

  #[test]
  #[should_panic(expected = "The period for opening a dispute has expired")]
  fn test_open_dispute_with_expired_opening_period() {
    let mut context = VMContextBuilder::new();
    testing_env!(context.build());
    let mut contract = BountiesContract::new(
      vec![accounts(0)],
      None,
      None,
      Some(get_disputes_contract()),
      None,
      None
    );
    contract.set_status(ContractStatus::Live);

    testing_env!(context
      .predecessor_account_id(accounts(0))
      .attached_deposit(1)
      .build());
    let config = Config::default();
    contract.change_config(ConfigCreate {
      bounty_claim_bond: config.bounty_claim_bond,
      period_for_opening_dispute: U64(10),
      bounty_forgiveness_period: config.bounty_forgiveness_period,
      platform_fee_percentage: config.platform_fee_percentage,
      validators_dao_fee_percentage: config.validators_dao_fee_percentage,
      penalty_platform_fee_percentage: config.penalty_platform_fee_percentage,
      penalty_validators_dao_fee_percentage: config.penalty_validators_dao_fee_percentage,
    });

    let project_owner = accounts(1);
    let id = add_bounty(&mut contract, &project_owner, None);
    let claimer = accounts(2);
    bounty_claim(&mut context, &mut contract, id.clone(), &claimer);
    bounty_done(&mut context, &mut contract, id.clone(), &claimer);

    testing_env!(context
      .block_timestamp(0)
      .build());
    bounty_action(
      &mut context,
      &mut contract,
      id.clone(),
      &project_owner,
      BountyAction::ClaimRejected { receiver_id: claimer.clone() },
    );

    testing_env!(context
      .predecessor_account_id(claimer.clone())
      .attached_deposit(1)
      .block_timestamp(20)
      .build());
    contract.open_dispute(id, "Test description".to_string());
  }
}
