use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::json_types::U64;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{env, near_bindgen, AccountId, BorshStorageKey, PanicOnDefault};

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone, PartialEq)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug))]
pub enum VerificationType {
  KYC,
  KYB
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct WhitelistEntry {
  /// KYC/KYB verification date
  pub verification_date: U64,
  /// Verification type: Know Your Customer or Know Your Business
  pub verification_type: VerificationType,
  /// Alias of the provider that performed the verification
  pub provider: String,
  /// The validity period of the identity document
  pub ident_document_date_of_expiry: Option<U64>,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub enum VersionedWhitelistEntry {
  Current(WhitelistEntry),
}

impl VersionedWhitelistEntry {
  pub fn to_whitelist_entry(self) -> WhitelistEntry {
    match self {
      VersionedWhitelistEntry::Current(whitelist_entry) => whitelist_entry,
    }
  }
}

impl From<VersionedWhitelistEntry> for WhitelistEntry {
  fn from(value: VersionedWhitelistEntry) -> Self {
    match value {
      VersionedWhitelistEntry::Current(whitelist_entry) => whitelist_entry,
    }
  }
}

impl From<WhitelistEntry> for VersionedWhitelistEntry {
  fn from (value: WhitelistEntry) -> Self {
    VersionedWhitelistEntry::Current(value)
  }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct ProviderDetails {
  pub name: String,
  pub enabled: bool,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct ServiceProfile {
  pub service_name: String,
  pub service_account: AccountId,
  pub providers: Vec<String>,
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
pub struct Config {
  pub providers: Vec<ProviderDetails>,
  pub service_profiles: Vec<ServiceProfile>,
  pub default_profile: Option<String>,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      providers: vec![],
      service_profiles: vec![],
      default_profile: None,
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

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct KycWhitelist {
  /// Whitelist administrator
  pub admin_account: AccountId,
  /// KYC/KYB verified whitelisted account details.
  pub whitelist: LookupMap<AccountId, Vec<VersionedWhitelistEntry>>,
  /// KYC/KYB contract configuration.
  pub config: VersionedConfig,
}

#[near_bindgen]
impl KycWhitelist {
  #[init]
  pub fn new(
    admin_account: AccountId,
    config: Option<Config>,
  ) -> Self {
    let versioned_config = if config.is_some() {
      config.unwrap().into()
    } else {
      Config::default().into()
    };

    Self {
      admin_account,
      whitelist: LookupMap::new(StorageKey::Whitelist),
      config: versioned_config,
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

  pub(crate) fn find_profile_by_alias(&self, name: String) -> ServiceProfile {
    let config = self.config.clone().to_config();
    config.service_profiles
      .into_iter()
      .find(|p| p.service_name == name)
      .expect("Service profile not found")
  }

  pub(crate) fn find_profile_by_account(&self, account_id: AccountId) -> ServiceProfile {
    let config = self.config.clone().to_config();
    config.service_profiles
      .into_iter()
      .find(|p| p.service_account == account_id)
      .expect("Service profile not found")
  }

  pub(crate) fn find_profile_index(
    config: &Config,
    service_name: Option<String>,
    service_account: Option<AccountId>,
  ) -> usize {
    assert_ne!(
      service_name.is_some(),
      service_account.is_some(),
      "Expected either service_name or service_account"
    );

    config.service_profiles
      .iter()
      .position(|e|
        if service_name.is_some() {
          e.service_name == service_name.clone().unwrap()
        } else {
          e.service_account == service_account.clone().unwrap()
        }
      )
      .expect("Service profile not found")
  }

  pub(crate) fn check_service_profile(&self, provider: String) {
    let service_profile = self.find_profile_by_account(env::predecessor_account_id());

    assert!(
      self.provider_found(provider.clone()) && service_profile.providers.contains(&provider),
      "The {} provider is not in use now",
      provider
    );
  }

  pub(crate) fn get_default_profile(&self, service_name: Option<String>) -> ServiceProfile {
    let config = self.config.clone().to_config();
    let profile_name = service_name
      .unwrap_or(config.default_profile.expect("Service name required"));
    self.find_profile_by_alias(profile_name)
  }

  pub(crate) fn provider_found(&self, name: String) -> bool {
    self.config
      .clone()
      .to_config()
      .providers
      .into_iter()
      .find(|p| p.name == name && p.enabled)
      .is_some()
  }

  pub(crate) fn find_whitelist_entry(
    &self,
    account_id: AccountId,
    verification_type: VerificationType,
    provider: String,
  ) -> (Vec<VersionedWhitelistEntry>, Option<usize>) {
    let whitelist_entries = self.whitelist.get(&account_id).unwrap_or_default();
    let index = whitelist_entries.iter().position(|e| {
      let entry = e.clone().to_whitelist_entry();
      entry.provider == provider && entry.verification_type == verification_type
    });
    (whitelist_entries, index)
  }

  /**
    Getters
  **/

  /// Returns 'true' if the given account ID is whitelisted.
  pub fn is_whitelisted(&self, account_id: AccountId, service_name: Option<String>) -> bool {
    let profile = self.get_default_profile(service_name);
    let whitelist_entries = self.whitelist.get(&account_id).unwrap_or_default();
    whitelist_entries
      .into_iter()
      .find(
        |e| {
          let entry = e.clone().to_whitelist_entry();
          self.provider_found(entry.provider.clone()) &&
            profile.providers.contains(&entry.provider) &&
            (entry.ident_document_date_of_expiry.is_none() ||
              env::block_timestamp() < entry.ident_document_date_of_expiry.unwrap().0)
        })
      .is_some()
  }

  /// Returns a whitelist items
  pub fn get_whitelist_entry(
    &self, account_id: AccountId,
    service_name: Option<String>,
  ) -> Vec<WhitelistEntry> {
    let profile = self.get_default_profile(service_name);
    let whitelist_entries = self.whitelist.get(&account_id).unwrap_or_default();
    whitelist_entries
      .into_iter()
      .filter(|e| {
        let entry = e.clone().to_whitelist_entry();
        profile.providers.contains(&entry.provider) && self.provider_found(entry.provider)
      })
      .map(|e| e.into())
      .collect()
  }

  pub fn get_config(&self) -> Config {
    self.config.clone().into()
  }

  pub fn get_version() -> String {
    "2.0.0".to_string()
  }

  /**
    Administrator
  **/

  /// Create service profile
  pub fn create_service_profile(&mut self, service_profile: ServiceProfile) {
    self.assert_is_whitelist_admin();
    let config = self.config.to_config_mut();

    let index = config.service_profiles
      .iter()
      .position(|e|
        e.service_account == service_profile.service_account ||
          e.service_name == service_profile.service_name
      );
    assert!(index.is_none(), "Profile with this account or name already exists");

    config.service_profiles.push(service_profile);
  }

  /// Update service profile
  pub fn update_service_profile(
    &mut self,
    service_name: Option<String>,
    service_account: Option<AccountId>,
    service_profile: ServiceProfile
  ) {
    self.assert_is_whitelist_admin();

    let config = self.config.to_config_mut();
    let index = Self::find_profile_index(&config, service_name, service_account);

    let index_of_account = config.service_profiles
      .iter()
      .position(|e| e.service_account == service_profile.service_account);
    assert!(
      index_of_account.is_none() || index_of_account.unwrap() == index,
      "Account {} is already used in another profile",
      service_profile.service_account
    );

    let index_of_alias = config.service_profiles
      .iter()
      .position(|e| e.service_name == service_profile.service_name);
    assert!(
      index_of_alias.is_none() || index_of_alias.unwrap() == index,
      "Name {} is already used in another profile",
      service_profile.service_name
    );

    config.service_profiles[index] = service_profile;
  }

  /// Remove service profile
  pub fn remove_service_profile(
    &mut self,
    service_name: Option<String>,
    service_account: Option<AccountId>
  ) {
    self.assert_is_whitelist_admin();

    let config = self.config.to_config_mut();
    let index = Self::find_profile_index(&config, service_name, service_account);

    config.service_profiles.remove(index);
  }

  /// Insert provider
  pub fn create_provider(&mut self, provider: ProviderDetails) {
    self.assert_is_whitelist_admin();

    let config = self.config.to_config_mut();
    let index = config.providers.iter().position(|p| p.name == provider.name);
    assert!(index.is_none(), "Provider with this name already exists");

    config.providers.push(provider);
  }

  /// Update provider
  pub fn update_provider(&mut self, name: String, provider: ProviderDetails) {
    self.assert_is_whitelist_admin();

    let config = self.config.to_config_mut();
    let index = config.providers.iter().position(|p| p.name == name).expect("Provider not found");

    config.providers[index] = provider;
  }

  /// Remove provider
  pub fn remove_provider(&mut self, name: String) {
    self.assert_is_whitelist_admin();

    let config = self.config.to_config_mut();
    let index = config.providers.iter().position(|p| p.name == name).expect("Provider not found");

    config.providers.remove(index);
  }

  /// Set or unset default profile
  pub fn set_default_profile(&mut self, service_name: Option<String>) {
    self.assert_is_whitelist_admin();

    let config = self.config.to_config_mut();

    config.default_profile = service_name;
  }

  /**
    Service
  **/

  /// Adds a verified account ID to the whitelist.
  pub fn add_account(
    &mut self,
    account_id: AccountId,
    verification_type: VerificationType,
    provider: String,
    ident_document_date_of_expiry: Option<U64>,
  ) -> bool {
    self.check_service_profile(provider.clone());

    let whitelist_entry = WhitelistEntry {
      verification_date: env::block_timestamp().into(),
      verification_type: verification_type.clone(),
      provider: provider.clone(),
      ident_document_date_of_expiry,
    };
    let (
      mut whitelist_entries,
      index
    ) = self.find_whitelist_entry(account_id.clone(), verification_type, provider);

    if index.is_some() {
      whitelist_entries.insert(index.unwrap(), whitelist_entry.into());
    } else {
      whitelist_entries.push(whitelist_entry.into());
    }
    self.whitelist.insert(&account_id, &whitelist_entries);
    true
  }

  /// Removes the given account ID from the whitelist.
  pub fn remove_account(
    &mut self,
    account_id: AccountId,
    verification_type: VerificationType,
    provider: String,
  ) -> bool {
    self.check_service_profile(provider.clone());

    let (
      mut whitelist_entries,
      index
    ) = self.find_whitelist_entry(account_id.clone(), verification_type, provider);

    if index.is_some() {
      whitelist_entries.remove(index.unwrap());
      if whitelist_entries.is_empty() {
        self.whitelist.remove(&account_id);
      } else {
        self.whitelist.insert(&account_id, &whitelist_entries);
      }
    }

    true
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
  use near_sdk::json_types::U64;
  use near_sdk::test_utils::{accounts, VMContextBuilder};
  use crate::{KycWhitelist, ProviderDetails, ServiceProfile, VerificationType, WhitelistEntry};

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
    let mut contract = KycWhitelist::new(admin_account(), None);

    testing_env!(context
      .predecessor_account_id(admin_account())
      .build());

    contract.create_provider(ProviderDetails {
      name: "fractal".to_string(),
      enabled: true,
    });
    assert_eq!(
      contract.config.clone().to_config().providers[0],
      ProviderDetails {
        name: "fractal".to_string(),
        enabled: true,
      }
    );

    contract.create_service_profile(ServiceProfile {
      service_name: "heroes".to_string(),
      service_account: service_account(),
      providers: vec!["fractal".to_string()],
    });
    assert_eq!(
      contract.config.clone().to_config().service_profiles[0],
      ServiceProfile {
        service_name: "heroes".to_string(),
        service_account: service_account(),
        providers: vec!["fractal".to_string()],
      }
    );

    contract.set_default_profile(Some("heroes".to_string()));
    assert_eq!(
      contract.config.clone().to_config().default_profile,
      Some("heroes".to_string())
    );

    testing_env!(context
      .predecessor_account_id(service_account())
      .build());

    assert!(!contract.is_whitelisted(user_account(), Some("heroes".to_string())));
    contract.add_account(user_account(), VerificationType::KYC, "fractal".to_string(), None);
    assert!(contract.is_whitelisted(user_account(), Some("heroes".to_string())));
    assert!(contract.is_whitelisted(user_account(), None));
    assert_eq!(
      contract.get_whitelist_entry(user_account(), Some("heroes".to_string())),
      vec![
        WhitelistEntry {
          verification_date: U64(0),
          verification_type: VerificationType::KYC,
          provider: "fractal".to_string(),
          ident_document_date_of_expiry: None,
        }
      ]
    );

    contract.remove_account(user_account(), VerificationType::KYC, "fractal".to_string());
    assert!(!contract.is_whitelisted(user_account(), Some("heroes".to_string())));
    assert!(!contract.is_whitelisted(user_account(), None));

    testing_env!(context
      .predecessor_account_id(admin_account())
      .build());

    contract.update_service_profile(Some("heroes".to_string()), None, ServiceProfile {
      service_name: "heroes".to_string(),
      service_account: second_service_account(),
      providers: vec!["fractal".to_string()],
    });
    assert_eq!(
      contract.config.clone().to_config().service_profiles[0],
      ServiceProfile {
        service_name: "heroes".to_string(),
        service_account: second_service_account(),
        providers: vec!["fractal".to_string()],
      }
    );

    contract.remove_service_profile(Some("heroes".to_string()), None);
    assert!(contract.config.clone().to_config().service_profiles.is_empty());

    contract.create_service_profile(ServiceProfile {
      service_name: "heroes2".to_string(),
      service_account: second_service_account(),
      providers: vec!["fractal".to_string()],
    });
    assert_eq!(
      contract.config.clone().to_config().service_profiles[0],
      ServiceProfile {
        service_name: "heroes2".to_string(),
        service_account: second_service_account(),
        providers: vec!["fractal".to_string()],
      }
    );

    contract.update_service_profile(None, Some(second_service_account()), ServiceProfile {
      service_name: "heroes3".to_string(),
      service_account: service_account(),
      providers: vec!["fractal".to_string()],
    });
    assert_eq!(
      contract.config.clone().to_config().service_profiles[0],
      ServiceProfile {
        service_name: "heroes3".to_string(),
        service_account: service_account(),
        providers: vec!["fractal".to_string()],
      }
    );

    contract.remove_service_profile(None, Some(service_account()));
    assert!(contract.config.clone().to_config().service_profiles.is_empty());

    contract.create_provider(ProviderDetails {
      name: "fractal+idOS".to_string(),
      enabled: true,
    });
    assert_eq!(contract.config.clone().to_config().providers.len(), 2);
    assert_eq!(
      contract.config.clone().to_config().providers[1],
      ProviderDetails {
        name: "fractal+idOS".to_string(),
        enabled: true,
      }
    );

    contract.update_provider("fractal+idOS".to_string(), ProviderDetails {
      name: "fractal+idOS".to_string(),
      enabled: false,
    });
    assert_eq!(
      contract.config.clone().to_config().providers[1],
      ProviderDetails {
        name: "fractal+idOS".to_string(),
        enabled: false,
      }
    );

    contract.remove_provider("fractal+idOS".to_string());
    assert_eq!(contract.config.clone().to_config().providers.len(), 1);

    contract.remove_provider("fractal".to_string());
    assert!(contract.config.clone().to_config().providers.is_empty());
  }

  #[test]
  #[should_panic(expected = "Predecessor is not whitelist administrator")]
  fn test_update_service_account_as_non_admin() {
    let mut context = VMContextBuilder::new();
    let mut contract = KycWhitelist::new(admin_account(), None);

    testing_env!(context
      .predecessor_account_id(admin_account())
      .build());

    contract.create_provider(ProviderDetails {
      name: "fractal".to_string(),
      enabled: true,
    });

    testing_env!(context
      .predecessor_account_id(service_account())
      .build());

    contract.create_service_profile(ServiceProfile {
      service_name: "heroes".to_string(),
      service_account: service_account(),
      providers: vec!["fractal".to_string()],
    });
  }

  #[test]
  #[should_panic(expected = "Service profile not found")]
  fn test_service_account_is_not_set() {
    let mut context = VMContextBuilder::new();
    let mut contract = KycWhitelist::new(admin_account(), None);

    testing_env!(context
      .predecessor_account_id(service_account())
      .build());

    contract.add_account(user_account(), VerificationType::KYC, "fractal".to_string(), None);
  }

  #[test]
  #[should_panic(expected = "Service profile not found")]
  fn test_whitelist_using_non_service_account() {
    let mut context = VMContextBuilder::new();
    let mut contract = KycWhitelist::new(admin_account(), None);

    testing_env!(context
      .predecessor_account_id(admin_account())
      .build());

    contract.create_provider(ProviderDetails {
      name: "fractal".to_string(),
      enabled: true,
    });

    contract.create_service_profile(ServiceProfile {
      service_name: "heroes".to_string(),
      service_account: service_account(),
      providers: vec!["fractal".to_string()],
    });

    testing_env!(context
      .predecessor_account_id(user_account())
      .build());

    contract.add_account(user_account(), VerificationType::KYC, "fractal".to_string(), None);
  }

  #[test]
  #[should_panic(expected = "Service name required")]
  fn test_whitelist_default_service_is_not_specified() {
    let mut context = VMContextBuilder::new();
    let mut contract = KycWhitelist::new(admin_account(), None);

    testing_env!(context
      .predecessor_account_id(admin_account())
      .build());

    contract.create_provider(ProviderDetails {
      name: "fractal".to_string(),
      enabled: true,
    });

    contract.create_service_profile(ServiceProfile {
      service_name: "heroes".to_string(),
      service_account: service_account(),
      providers: vec!["fractal".to_string()],
    });

    testing_env!(context
      .predecessor_account_id(service_account())
      .build());

    contract.add_account(user_account(), VerificationType::KYC, "fractal".to_string(), None);

    let _result = contract.is_whitelisted(user_account(), None);
  }
}
