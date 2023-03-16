use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupSet;
use near_sdk::{env, near_bindgen, AccountId, BorshStorageKey, PanicOnDefault};

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct KycWhitelist {
  /// Whitelist administrator
  pub admin_account: AccountId,
  /// Service account
  pub service_account: Option<AccountId>,
  /// Whitelisted account IDs that completed KYC verification.
  pub whitelist: LookupSet<AccountId>,
}

#[near_bindgen]
impl KycWhitelist {
  #[init]
  pub fn new(admin_account: AccountId, service_account: Option<AccountId>) -> Self {
    Self {
      admin_account,
      service_account,
      whitelist: LookupSet::new(StorageKey::Whitelist),
    }
  }

  /**
    Internal
  **/

  pub(crate) fn assert_is_whitelist_admin(&self) {
    assert_eq!(
      self.admin_account,
      env::predecessor_account_id(),
      "Predecessor is not whitelist administrator"
    );
  }

  pub(crate) fn assert_is_service_account(&self) {
    assert_eq!(
      self.service_account.clone().expect("The service account is not set"),
      env::predecessor_account_id(),
      "Predecessor is not service account"
    );
  }

  /**
    Getters
  **/

  /// Returns 'true' if the given account ID is whitelisted.
  pub fn is_whitelisted(&self, account_id: AccountId) -> bool {
    self.whitelist.contains(&account_id)
  }

  /// Returns the public key for the applicant's account
  pub fn get_service_account(&self) -> Option<AccountId> {
    self.service_account.clone()
  }

  /**
    Administrator
  **/

  /// Update or remove service account
  pub fn update_service_account(&mut self, service_account: Option<AccountId>) {
    self.assert_is_whitelist_admin();
    self.service_account = service_account;
  }

  /**
    Service
  **/

  /// Adds a verified account ID to the whitelist.
  pub fn add_account(&mut self, account_id: AccountId) -> bool {
    self.assert_is_service_account();
    self.whitelist.insert(&account_id)
  }

  /// Removes the given account ID from the whitelist.
  pub fn remove_account(&mut self, account_id: AccountId) -> bool {
    self.assert_is_service_account();
    self.whitelist.remove(&account_id)
  }
}

#[derive(BorshStorageKey, BorshSerialize)]
pub(crate) enum StorageKey {
  Whitelist,
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(test)]
mod tests {
  use near_sdk::{AccountId, testing_env};
  use near_sdk::test_utils::{accounts, VMContextBuilder};
  use crate::KycWhitelist;

  fn admin_account() -> AccountId {
    accounts(0)
  }

  fn service_account() -> AccountId {
    accounts(1)
  }

  fn second_service_account() -> AccountId {
    accounts(2)
  }

  fn user_account() -> AccountId {
    accounts(3)
  }

  #[test]
  fn test_whitelist_flow() {
    let mut context = VMContextBuilder::new();
    let mut contract = KycWhitelist::new(admin_account(), Some(service_account()));

    testing_env!(context
      .predecessor_account_id(service_account())
      .build());

    contract.add_account(user_account());
    assert!(contract.is_whitelisted(user_account()));

    contract.remove_account(user_account());
    assert!(!contract.is_whitelisted(user_account()));

    testing_env!(context
      .predecessor_account_id(admin_account())
      .build());

    contract.update_service_account(Some(second_service_account()));
    assert_eq!(
      contract.get_service_account(),
      Some(second_service_account())
    );

    contract.update_service_account(None);
    assert_eq!(
      contract.get_service_account(),
      None
    );
  }

  #[test]
  #[should_panic(expected = "Predecessor is not whitelist administrator")]
  fn test_update_service_account_as_non_admin() {
    let mut context = VMContextBuilder::new();
    let mut contract = KycWhitelist::new(admin_account(), None);

    testing_env!(context
      .predecessor_account_id(service_account())
      .build());
    contract.update_service_account(Some(service_account()));
  }

  #[test]
  #[should_panic(expected = "The service account is not set")]
  fn test_service_account_is_not_set() {
    let mut context = VMContextBuilder::new();
    let mut contract = KycWhitelist::new(admin_account(), None);

    testing_env!(context
      .predecessor_account_id(service_account())
      .build());

    contract.add_account(user_account());
  }

  #[test]
  #[should_panic(expected = "Predecessor is not service account")]
  fn test_whitelist_user_account_using_non_service_account() {
    let mut context = VMContextBuilder::new();
    let mut contract = KycWhitelist::new(admin_account(), Some(service_account()));

    testing_env!(context
      .predecessor_account_id(user_account())
      .build());

    contract.add_account(user_account());
  }
}
