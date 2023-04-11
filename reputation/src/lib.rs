use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::{UnorderedMap, UnorderedSet};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{assert_one_yocto, env, near_bindgen, AccountId, BorshStorageKey, PanicOnDefault};

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct ClaimerMetrics {
  /// Total number of claims created
  pub number_of_claims: u64,
  /// Number of successfully accepted claims
  pub number_of_accepted_claims: u64,
  /// Number of successfully completed claims
  pub number_of_successful_claims: u64,
  /// Number of claims for which the result of execution was not finally accepted
  pub number_of_unsuccessful_claims: u64,
  /// Number of overdue claims
  pub number_of_overdue_claims: u64,
  /// Number of claims canceled by the claimer
  pub number_of_canceled_claims: u64,
  /// Number of open disputes
  pub number_of_open_disputes: u64,
  /// Number of disputes won
  pub number_of_disputes_won: u64,
}

impl Default for ClaimerMetrics {
  fn default() -> Self {
    Self {
      number_of_claims: 0,
      number_of_accepted_claims: 0,
      number_of_successful_claims: 0,
      number_of_unsuccessful_claims: 0,
      number_of_overdue_claims: 0,
      number_of_canceled_claims: 0,
      number_of_open_disputes: 0,
      number_of_disputes_won: 0,
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct BountyOwnerMetrics {
  /// Total number of bounties created
  pub number_of_bounties: u64,
  /// Number of successfully completed bounties
  pub number_of_successful_bounties: u64,
  /// Number of canceled bounties
  pub number_of_canceled_bounties: u64,
  /// Number of claims for his bounties
  pub number_of_claims: u64,
  /// Number of approved claimers
  pub number_of_approved_claimers: u64,
  /// Number of approved claims
  pub number_of_approved_claims: u64,
  /// Number of rejected claims
  pub number_of_rejected_claims: u64,
  /// Number of open disputes for his bounties
  pub number_of_open_disputes: u64,
  /// Number of disputes won
  pub number_of_disputes_won: u64,
}

impl Default for BountyOwnerMetrics {
  fn default() -> Self {
    Self {
      number_of_bounties: 0,
      number_of_successful_bounties: 0,
      number_of_canceled_bounties: 0,
      number_of_claims: 0,
      number_of_approved_claimers: 0,
      number_of_approved_claims: 0,
      number_of_rejected_claims: 0,
      number_of_open_disputes: 0,
      number_of_disputes_won: 0,
    }
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum ActionKind {
  /// Bounty created
  BountyCreated,
  /// Bounty cancelled
  BountyCancelled,
  /// Claim created
  ClaimCreated,
  /// Claim cancelled
  ClaimCancelled,
  /// Claimer approved
  ClaimerApproved,
  /// The claim has expired
  ClaimExpired,
  /// Successful bounty and claim
  SuccessfulClaim {with_dispute: bool},
  /// Unsuccessful claim
  UnsuccessfulClaim {with_dispute: bool},
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct ReputationContract {
  /// Bounties contract.
  pub bounties_contract: AccountId,
  /// Stats for bounty claimers.
  pub claimers_entries: UnorderedMap<AccountId, ClaimerMetrics>,
  /// Bounty owner statistics.
  pub bounty_owners_entries: UnorderedMap<AccountId, BountyOwnerMetrics>,
  /// account ids that can perform all actions:
  /// - manage admin_whitelist
  /// - change bounties contract
  pub admin_whitelist: UnorderedSet<AccountId>,
}

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
  AdminWhitelist,
  ClaimersEntries,
  BountyOwnersEntries,
}

#[near_bindgen]
impl ReputationContract {
  #[init]
  pub fn new(bounties_contract: AccountId, admin_whitelist: Vec<AccountId>) -> Self {
    assert!(
      admin_whitelist.len() > 0,
      "Admin whitelist must contain at least one account"
    );
    let mut admin_whitelist_set = UnorderedSet::new(StorageKey::AdminWhitelist);
    admin_whitelist_set.extend(admin_whitelist.into_iter().map(|a| a.into()));

    Self {
      bounties_contract,
      claimers_entries: UnorderedMap::new(StorageKey::ClaimersEntries),
      bounty_owners_entries: UnorderedMap::new(StorageKey::BountyOwnersEntries),
      admin_whitelist: admin_whitelist_set,
    }
  }

  /**
    Getters
  **/

  pub fn get_statistics(
    &self,
    account_id: &AccountId
  ) -> (Option<ClaimerMetrics>, Option<BountyOwnerMetrics>) {
    (
      self.claimers_entries.get(account_id),
      self.bounty_owners_entries.get(account_id),
    )
  }

  pub fn get_bounties_contract_account_id(&self) -> AccountId {
    self.bounties_contract.clone()
  }

  pub fn get_claimers_entries(
    &self,
    from_index: Option<u64>,
    limit: Option<u64>,
  ) -> Vec<(AccountId, ClaimerMetrics)> {
    let from_index = from_index.unwrap_or(0);
    let limit = limit.unwrap_or(self.claimers_entries.len());
    let keys = self.claimers_entries.keys_as_vector();
    let values = self.claimers_entries.values_as_vector();
    (from_index..std::cmp::min(from_index + limit, self.claimers_entries.len()))
      .map(|id| {
        (
          keys.get(id).unwrap(),
          values.get(id).unwrap(),
        )
      })
      .collect()
  }

  pub fn get_num_claimers_entries(&self) -> u64 {
    self.claimers_entries.len()
  }

  pub fn get_bounty_owners_entries(
    &self,
    from_index: Option<u64>,
    limit: Option<u64>,
  ) -> Vec<(AccountId, BountyOwnerMetrics)> {
    let from_index = from_index.unwrap_or(0);
    let limit = limit.unwrap_or(self.bounty_owners_entries.len());
    let keys = self.bounty_owners_entries.keys_as_vector();
    let values = self.bounty_owners_entries.values_as_vector();
    (from_index..std::cmp::min(from_index + limit, self.bounty_owners_entries.len()))
      .map(|id| {
        (
          keys.get(id).unwrap(),
          values.get(id).unwrap(),
        )
      })
      .collect()
  }

  pub fn get_num_bounty_owners_entries(&self) -> u64 {
    self.bounty_owners_entries.len()
  }

  pub fn get_admin_whitelist(&self) -> Vec<AccountId> {
    self.admin_whitelist.to_vec()
  }

  /**
    Internal
  **/

  fn assert_called_by_bounties_contract(&self) {
    assert_eq!(
      self.bounties_contract,
      env::predecessor_account_id(),
      "Only a bounties contract can call this method"
    );
  }

  pub(crate) fn assert_admin_whitelist(&self) {
    assert!(
      self.admin_whitelist.contains(&env::predecessor_account_id()),
      "Not in admin whitelist"
    );
  }

  /**
    Update statistics
  **/

  pub fn emit(
    &mut self,
    claimer: Option<AccountId>,
    bounty_owner: Option<AccountId>,
    action_kind: ActionKind
  ) {
    self.assert_called_by_bounties_contract();
    let without_claimer = matches!(action_kind, ActionKind::BountyCreated) ||
      matches!(action_kind, ActionKind::BountyCancelled);
    let without_bounty_owner = matches!(action_kind, ActionKind::ClaimCancelled) ||
      matches!(action_kind, ActionKind::ClaimExpired);

    if without_claimer {
      assert!(
        bounty_owner.is_some() && claimer.is_none(),
        "Bounty owner is required and Claimer is not required"
      );
    }
    if without_bounty_owner {
      assert!(
        bounty_owner.is_none() && claimer.is_some(),
        "Claimer is required and Bounty owner is not required"
      );
    }
    if !without_claimer && !without_bounty_owner {
      assert!(
        bounty_owner.is_some() && claimer.is_some(),
        "Claimer and bounty owner required"
      );
    }

    let mut claimer_metrics = if claimer.is_some() {
      self.claimers_entries.get(&claimer.clone().unwrap()).unwrap_or_default()
    } else {
      ClaimerMetrics::default()
    };
    let mut bounty_owner_metrics = if bounty_owner.is_some() {
      self.bounty_owners_entries.get(&bounty_owner.clone().unwrap()).unwrap_or_default()
    } else {
      BountyOwnerMetrics::default()
    };

    match action_kind {
      ActionKind::BountyCreated => {
        bounty_owner_metrics.number_of_bounties += 1;
      }
      ActionKind::ClaimCreated => {
        bounty_owner_metrics.number_of_claims += 1;
        claimer_metrics.number_of_claims += 1;
      }
      ActionKind::ClaimerApproved => {
        bounty_owner_metrics.number_of_approved_claimers += 1;
        claimer_metrics.number_of_accepted_claims += 1;
      }
      ActionKind::ClaimCancelled => {
        claimer_metrics.number_of_canceled_claims += 1;
      }
      ActionKind::BountyCancelled => {
        bounty_owner_metrics.number_of_canceled_bounties += 1;
      }
      ActionKind::ClaimExpired => {
        claimer_metrics.number_of_overdue_claims += 1;
      }
      ActionKind::SuccessfulClaim {with_dispute} => {
        bounty_owner_metrics.number_of_successful_bounties += 1;
        claimer_metrics.number_of_successful_claims += 1;
        if with_dispute {
          bounty_owner_metrics.number_of_rejected_claims += 1;
          bounty_owner_metrics.number_of_open_disputes += 1;
          claimer_metrics.number_of_open_disputes += 1;
          claimer_metrics.number_of_disputes_won += 1;
        } else {
          bounty_owner_metrics.number_of_approved_claims += 1;
        }
      }
      ActionKind::UnsuccessfulClaim {with_dispute} => {
        bounty_owner_metrics.number_of_rejected_claims += 1;
        claimer_metrics.number_of_unsuccessful_claims += 1;
        if with_dispute {
          bounty_owner_metrics.number_of_open_disputes += 1;
          bounty_owner_metrics.number_of_disputes_won += 1;
          claimer_metrics.number_of_open_disputes += 1;
        }
      }
    }

    if !without_claimer {
      self.claimers_entries.insert(&claimer.unwrap(), &claimer_metrics);
    }
    if !without_bounty_owner {
      self.bounty_owners_entries.insert(&bounty_owner.unwrap(), &bounty_owner_metrics);
    }
  }

  /**
    Administrator
  **/

  #[payable]
  pub fn add_to_admin_whitelist(
    &mut self,
    account_id: Option<AccountId>,
    account_ids: Option<Vec<AccountId>>,
  ) {
    assert_one_yocto();
    self.assert_admin_whitelist();
    let account_ids = if let Some(account_ids) = account_ids {
      account_ids
    } else {
      vec![account_id.expect("Expected either account_id or account_ids")]
    };
    for account_id in &account_ids {
      self.admin_whitelist.insert(account_id);
    }
  }

  #[payable]
  pub fn remove_from_admin_whitelist(
    &mut self,
    account_id: Option<AccountId>,
    account_ids: Option<Vec<AccountId>>,
  ) {
    assert_one_yocto();
    self.assert_admin_whitelist();
    let account_ids = if let Some(account_ids) = account_ids {
      account_ids
    } else {
      vec![account_id.expect("Expected either account_id or account_ids")]
    };
    for account_id in &account_ids {
      self.admin_whitelist.remove(&account_id);
    }
    assert!(
      !self.admin_whitelist.is_empty(),
      "Cannot remove all accounts from admin whitelist",
    );
  }

  /// Can be used only during migrations when updating contract versions
  #[payable]
  pub fn update_bounties_contract(&mut self, bounties_contract: AccountId) {
    assert_one_yocto();
    self.assert_admin_whitelist();
    self.bounties_contract = bounties_contract;
  }
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
  use near_sdk::{AccountId, testing_env};
  use near_sdk::test_utils::{accounts, VMContextBuilder};
  use crate::{ActionKind, BountyOwnerMetrics, ClaimerMetrics, ReputationContract};

  fn get_bounties_contract() -> AccountId {
    "bounties".parse().unwrap()
  }

  fn get_admin_whitelist() -> Vec<AccountId> {
    vec!["admin".parse().unwrap()]
  }

  #[test]
  #[should_panic(expected = "Bounty owner is required and Claimer is not required")]
  fn test_stats_after_bounty_created_with_claimer_account() {
    let mut context = VMContextBuilder::new();
    let mut contract = ReputationContract::new(get_bounties_contract(), get_admin_whitelist());
    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());
    contract.emit(Some(accounts(0)), Some(accounts(1)), ActionKind::BountyCreated);
  }

  #[test]
  #[should_panic(expected = "Bounty owner is required and Claimer is not required")]
  fn test_stats_after_bounty_created_without_bounty_owner_account() {
    let mut context = VMContextBuilder::new();
    let mut contract = ReputationContract::new(get_bounties_contract(), get_admin_whitelist());
    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());
    contract.emit(Some(accounts(0)), None, ActionKind::BountyCreated);
  }

  #[test]
  #[should_panic(expected = "Only a bounties contract can call this method")]
  fn test_stats_after_bounty_created_by_other_account() {
    let mut context = VMContextBuilder::new();
    let mut contract = ReputationContract::new(accounts(0), get_admin_whitelist());
    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());
    contract.emit(None, Some(accounts(1)), ActionKind::BountyCreated);
  }

  #[test]
  fn test_stats_after_bounty_created() {
    let mut context = VMContextBuilder::new();
    let mut contract = ReputationContract::new(get_bounties_contract(), get_admin_whitelist());
    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());

    contract.emit(None, Some(accounts(1)), ActionKind::BountyCreated);

    let bounty_owner_stats = contract.bounty_owners_entries.get(&accounts(1)).unwrap();
    assert_eq!(contract.bounty_owners_entries.len(), 1);
    assert_eq!(
      bounty_owner_stats,
      BountyOwnerMetrics {
        number_of_bounties: 1,
        number_of_successful_bounties: 0,
        number_of_canceled_bounties: 0,
        number_of_claims: 0,
        number_of_approved_claimers: 0,
        number_of_approved_claims: 0,
        number_of_rejected_claims: 0,
        number_of_open_disputes: 0,
        number_of_disputes_won: 0,
      }
    );
    assert_eq!(contract.claimers_entries.len(), 0);
  }

  #[test]
  #[should_panic(expected = "Claimer and bounty owner required")]
  fn test_stats_after_claim_created_without_claimer_account() {
    let mut context = VMContextBuilder::new();
    let mut contract = ReputationContract::new(get_bounties_contract(), get_admin_whitelist());
    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());
    contract.emit(None, Some(accounts(1)), ActionKind::ClaimCreated);
  }

  #[test]
  #[should_panic(expected = "Claimer and bounty owner required")]
  fn test_stats_after_claim_created_without_bounty_owner_account() {
    let mut context = VMContextBuilder::new();
    let mut contract = ReputationContract::new(get_bounties_contract(), get_admin_whitelist());
    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());
    contract.emit(Some(accounts(0)), None, ActionKind::ClaimCreated);
  }

  #[test]
  fn test_stats_after_claim_created() {
    let mut context = VMContextBuilder::new();
    let mut contract = ReputationContract::new(get_bounties_contract(), get_admin_whitelist());
    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());

    contract.emit(Some(accounts(0)), Some(accounts(1)), ActionKind::ClaimCreated);

    let claimer_stats = contract.claimers_entries.get(&accounts(0)).unwrap();
    let bounty_owner_stats = contract.bounty_owners_entries.get(&accounts(1)).unwrap();
    assert_eq!(contract.claimers_entries.len(), 1);
    assert_eq!(
      claimer_stats,
      ClaimerMetrics {
        number_of_claims: 1,
        number_of_accepted_claims: 0,
        number_of_successful_claims: 0,
        number_of_unsuccessful_claims: 0,
        number_of_overdue_claims: 0,
        number_of_canceled_claims: 0,
        number_of_open_disputes: 0,
        number_of_disputes_won: 0,
      }
    );
    assert_eq!(contract.bounty_owners_entries.len(), 1);
    assert_eq!(
      bounty_owner_stats,
      BountyOwnerMetrics {
        number_of_bounties: 0,
        number_of_successful_bounties: 0,
        number_of_canceled_bounties: 0,
        number_of_claims: 1,
        number_of_approved_claimers: 0,
        number_of_approved_claims: 0,
        number_of_rejected_claims: 0,
        number_of_open_disputes: 0,
        number_of_disputes_won: 0,
      }
    );
  }

  #[test]
  fn test_stats_after_claimer_approved() {
    let mut context = VMContextBuilder::new();
    let mut contract = ReputationContract::new(get_bounties_contract(), get_admin_whitelist());
    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());

    contract.emit(Some(accounts(0)), Some(accounts(1)), ActionKind::ClaimerApproved);

    let claimer_stats = contract.claimers_entries.get(&accounts(0)).unwrap();
    let bounty_owner_stats = contract.bounty_owners_entries.get(&accounts(1)).unwrap();
    assert_eq!(contract.claimers_entries.len(), 1);
    assert_eq!(
      claimer_stats,
      ClaimerMetrics {
        number_of_claims: 0,
        number_of_accepted_claims: 1,
        number_of_successful_claims: 0,
        number_of_unsuccessful_claims: 0,
        number_of_overdue_claims: 0,
        number_of_canceled_claims: 0,
        number_of_open_disputes: 0,
        number_of_disputes_won: 0,
      }
    );
    assert_eq!(contract.bounty_owners_entries.len(), 1);
    assert_eq!(
      bounty_owner_stats,
      BountyOwnerMetrics {
        number_of_bounties: 0,
        number_of_successful_bounties: 0,
        number_of_canceled_bounties: 0,
        number_of_claims: 0,
        number_of_approved_claimers: 1,
        number_of_approved_claims: 0,
        number_of_rejected_claims: 0,
        number_of_open_disputes: 0,
        number_of_disputes_won: 0,
      }
    );
  }

  #[test]
  fn test_stats_after_bounty_cancelled() {
    let mut context = VMContextBuilder::new();
    let mut contract = ReputationContract::new(get_bounties_contract(), get_admin_whitelist());
    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());

    contract.emit(None, Some(accounts(1)), ActionKind::BountyCancelled);

    let bounty_owner_stats = contract.bounty_owners_entries.get(&accounts(1)).unwrap();
    assert_eq!(contract.bounty_owners_entries.len(), 1);
    assert_eq!(
      bounty_owner_stats,
      BountyOwnerMetrics {
        number_of_bounties: 0,
        number_of_successful_bounties: 0,
        number_of_canceled_bounties: 1,
        number_of_claims: 0,
        number_of_approved_claimers: 0,
        number_of_approved_claims: 0,
        number_of_rejected_claims: 0,
        number_of_open_disputes: 0,
        number_of_disputes_won: 0,
      }
    );
    assert_eq!(contract.claimers_entries.len(), 0);
  }

  #[test]
  #[should_panic(expected = "Claimer is required and Bounty owner is not required")]
  fn test_stats_after_claim_cancelled_with_bounty_owner_account() {
    let mut context = VMContextBuilder::new();
    let mut contract = ReputationContract::new(get_bounties_contract(), get_admin_whitelist());
    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());
    contract.emit(Some(accounts(0)), Some(accounts(1)), ActionKind::ClaimCancelled);
  }

  #[test]
  #[should_panic(expected = "Claimer is required and Bounty owner is not required")]
  fn test_stats_after_claim_cancelled_without_claimer_account() {
    let mut context = VMContextBuilder::new();
    let mut contract = ReputationContract::new(get_bounties_contract(), get_admin_whitelist());
    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());
    contract.emit(None, Some(accounts(1)), ActionKind::ClaimCancelled);
  }

  #[test]
  fn test_stats_after_claim_cancelled() {
    let mut context = VMContextBuilder::new();
    let mut contract = ReputationContract::new(get_bounties_contract(), get_admin_whitelist());
    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());

    contract.emit(Some(accounts(0)), None, ActionKind::ClaimCancelled);

    let claimer_stats = contract.claimers_entries.get(&accounts(0)).unwrap();
    assert_eq!(contract.claimers_entries.len(), 1);
    assert_eq!(
      claimer_stats,
      ClaimerMetrics {
        number_of_claims: 0,
        number_of_accepted_claims: 0,
        number_of_successful_claims: 0,
        number_of_unsuccessful_claims: 0,
        number_of_overdue_claims: 0,
        number_of_canceled_claims: 1,
        number_of_open_disputes: 0,
        number_of_disputes_won: 0,
      }
    );
    assert_eq!(contract.bounty_owners_entries.len(), 0);
  }

  #[test]
  fn test_stats_after_claim_expired() {
    let mut context = VMContextBuilder::new();
    let mut contract = ReputationContract::new(get_bounties_contract(), get_admin_whitelist());
    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());

    contract.emit(Some(accounts(0)), None, ActionKind::ClaimExpired);

    let claimer_stats = contract.claimers_entries.get(&accounts(0)).unwrap();
    assert_eq!(contract.claimers_entries.len(), 1);
    assert_eq!(
      claimer_stats,
      ClaimerMetrics {
        number_of_claims: 0,
        number_of_accepted_claims: 0,
        number_of_successful_claims: 0,
        number_of_unsuccessful_claims: 0,
        number_of_overdue_claims: 1,
        number_of_canceled_claims: 0,
        number_of_open_disputes: 0,
        number_of_disputes_won: 0,
      }
    );
    assert_eq!(contract.bounty_owners_entries.len(), 0);
  }

  #[test]
  fn test_stats_after_successful_claim() {
    let mut context = VMContextBuilder::new();
    let mut contract = ReputationContract::new(get_bounties_contract(), get_admin_whitelist());
    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());

    contract.emit(
      Some(accounts(0)),
      Some(accounts(1)),
      ActionKind::SuccessfulClaim { with_dispute: false }
    );

    let claimer_stats = contract.claimers_entries.get(&accounts(0)).unwrap();
    let bounty_owner_stats = contract.bounty_owners_entries.get(&accounts(1)).unwrap();
    assert_eq!(contract.claimers_entries.len(), 1);
    assert_eq!(
      claimer_stats,
      ClaimerMetrics {
        number_of_claims: 0,
        number_of_accepted_claims: 0,
        number_of_successful_claims: 1,
        number_of_unsuccessful_claims: 0,
        number_of_overdue_claims: 0,
        number_of_canceled_claims: 0,
        number_of_open_disputes: 0,
        number_of_disputes_won: 0,
      }
    );
    assert_eq!(contract.bounty_owners_entries.len(), 1);
    assert_eq!(
      bounty_owner_stats,
      BountyOwnerMetrics {
        number_of_bounties: 0,
        number_of_successful_bounties: 1,
        number_of_canceled_bounties: 0,
        number_of_claims: 0,
        number_of_approved_claimers: 0,
        number_of_approved_claims: 1,
        number_of_rejected_claims: 0,
        number_of_open_disputes: 0,
        number_of_disputes_won: 0,
      }
    );

    let (stats_for_claimer, stats_for_bounty_owner) = contract.get_statistics(&accounts(0));
    assert_eq!(stats_for_claimer.unwrap(), claimer_stats);
    assert!(stats_for_bounty_owner.is_none());
    let (stats_for_claimer, stats_for_bounty_owner) = contract.get_statistics(&accounts(1));
    assert_eq!(stats_for_bounty_owner.unwrap(), bounty_owner_stats);
    assert!(stats_for_claimer.is_none());

    let stats = contract.get_bounty_owners_entries(None, None);
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].0, accounts(1));
    assert_eq!(stats[0].1, bounty_owner_stats);

    let stats = contract.get_claimers_entries(None, None);
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].0, accounts(0));
    assert_eq!(stats[0].1, claimer_stats);

    assert_eq!(contract.get_num_bounty_owners_entries(), 1);
    assert_eq!(contract.get_num_claimers_entries(), 1);
  }

  #[test]
  fn test_stats_after_successful_claim_with_dispute() {
    let mut context = VMContextBuilder::new();
    let mut contract = ReputationContract::new(get_bounties_contract(), get_admin_whitelist());
    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());

    contract.emit(
      Some(accounts(0)),
      Some(accounts(1)),
      ActionKind::SuccessfulClaim { with_dispute: true }
    );

    let claimer_stats = contract.claimers_entries.get(&accounts(0)).unwrap();
    let bounty_owner_stats = contract.bounty_owners_entries.get(&accounts(1)).unwrap();
    assert_eq!(contract.claimers_entries.len(), 1);
    assert_eq!(
      claimer_stats,
      ClaimerMetrics {
        number_of_claims: 0,
        number_of_accepted_claims: 0,
        number_of_successful_claims: 1,
        number_of_unsuccessful_claims: 0,
        number_of_overdue_claims: 0,
        number_of_canceled_claims: 0,
        number_of_open_disputes: 1,
        number_of_disputes_won: 1,
      }
    );
    assert_eq!(contract.bounty_owners_entries.len(), 1);
    assert_eq!(
      bounty_owner_stats,
      BountyOwnerMetrics {
        number_of_bounties: 0,
        number_of_successful_bounties: 1,
        number_of_canceled_bounties: 0,
        number_of_claims: 0,
        number_of_approved_claimers: 0,
        number_of_approved_claims: 0,
        number_of_rejected_claims: 1,
        number_of_open_disputes: 1,
        number_of_disputes_won: 0,
      }
    );
  }

  #[test]
  fn test_stats_after_unsuccessful_claim() {
    let mut context = VMContextBuilder::new();
    let mut contract = ReputationContract::new(get_bounties_contract(), get_admin_whitelist());
    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());

    contract.emit(
      Some(accounts(0)),
      Some(accounts(1)),
      ActionKind::UnsuccessfulClaim { with_dispute: false }
    );

    let claimer_stats = contract.claimers_entries.get(&accounts(0)).unwrap();
    let bounty_owner_stats = contract.bounty_owners_entries.get(&accounts(1)).unwrap();
    assert_eq!(contract.claimers_entries.len(), 1);
    assert_eq!(
      claimer_stats,
      ClaimerMetrics {
        number_of_claims: 0,
        number_of_accepted_claims: 0,
        number_of_successful_claims: 0,
        number_of_unsuccessful_claims: 1,
        number_of_overdue_claims: 0,
        number_of_canceled_claims: 0,
        number_of_open_disputes: 0,
        number_of_disputes_won: 0,
      }
    );
    assert_eq!(contract.bounty_owners_entries.len(), 1);
    assert_eq!(
      bounty_owner_stats,
      BountyOwnerMetrics {
        number_of_bounties: 0,
        number_of_successful_bounties: 0,
        number_of_canceled_bounties: 0,
        number_of_claims: 0,
        number_of_approved_claimers: 0,
        number_of_approved_claims: 0,
        number_of_rejected_claims: 1,
        number_of_open_disputes: 0,
        number_of_disputes_won: 0,
      }
    );
  }

  #[test]
  fn test_stats_after_unsuccessful_claim_with_dispute() {
    let mut context = VMContextBuilder::new();
    let mut contract = ReputationContract::new(get_bounties_contract(), get_admin_whitelist());
    testing_env!(context
      .predecessor_account_id(get_bounties_contract())
      .build());

    contract.emit(
      Some(accounts(0)),
      Some(accounts(1)),
      ActionKind::UnsuccessfulClaim { with_dispute: true }
    );

    let claimer_stats = contract.claimers_entries.get(&accounts(0)).unwrap();
    let bounty_owner_stats = contract.bounty_owners_entries.get(&accounts(1)).unwrap();
    assert_eq!(contract.claimers_entries.len(), 1);
    assert_eq!(
      claimer_stats,
      ClaimerMetrics {
        number_of_claims: 0,
        number_of_accepted_claims: 0,
        number_of_successful_claims: 0,
        number_of_unsuccessful_claims: 1,
        number_of_overdue_claims: 0,
        number_of_canceled_claims: 0,
        number_of_open_disputes: 1,
        number_of_disputes_won: 0,
      }
    );
    assert_eq!(contract.bounty_owners_entries.len(), 1);
    assert_eq!(
      bounty_owner_stats,
      BountyOwnerMetrics {
        number_of_bounties: 0,
        number_of_successful_bounties: 0,
        number_of_canceled_bounties: 0,
        number_of_claims: 0,
        number_of_approved_claimers: 0,
        number_of_approved_claims: 0,
        number_of_rejected_claims: 1,
        number_of_open_disputes: 1,
        number_of_disputes_won: 1,
      }
    );
  }
}
