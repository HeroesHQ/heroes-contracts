use crate::*;

#[derive(BorshDeserialize, BorshSerialize)]
pub struct OldKycWhitelist {
  pub admin_account: AccountId,
  pub service_account: Option<AccountId>,
  pub whitelist: LookupSet<AccountId>,
}

#[near_bindgen]
impl KycWhitelist {
  #[private]
  #[init(ignore_state)]
  pub fn migrate(config: Config, entries: Vec<(AccountId, Vec<WhitelistEntry>)>) -> Self {
    let old_state: OldKycWhitelist = env::state_read().expect("Old state doesn't exist");
    let mut new_whitelist: LookupMap<AccountId, Vec<VersionedWhitelistEntry>> =
      LookupMap::new(StorageKey::Whitelist);

    assert_eq!(
      env::predecessor_account_id(),
      old_state.admin_account,
      "Only an administrator can perform this action"
    );

    for (_, profile) in config.service_profiles.iter().enumerate() {
      assert!(!profile.service_name.is_empty(), "The name of the service profile cannot be empty");
      for (_, provider) in profile.providers.iter().enumerate() {
        assert!(
          config.providers.contains(&ProviderDetails { name: provider.clone(), enabled: true }),
          "Unknown provider {} in profile {}",
          provider,
          profile.service_name
        );
      }
    }

    assert!(
      config.default_profile.is_none() ||
        config.service_profiles
          .clone()
          .into_iter()
          .find(|p| p.service_name == config.default_profile.clone().unwrap())
          .is_some(),
      "Unknown default profile"
    );

    for (_, entry) in entries.iter().enumerate() {
      let account_id = &entry.0;
      let whitelist_entries = &entry.1;

      assert!(
        old_state.whitelist.contains(account_id),
        "{} account is not whitelisted",
        account_id
      );

      for (_, whitelist_entry) in whitelist_entries.iter().enumerate() {
        assert!(
          whitelist_entry.verification_date.0 < env::block_timestamp(),
          "Verification date is greater than current date for {}",
          account_id
        );

        assert!(
          config.providers.contains(
            &ProviderDetails { name: whitelist_entry.clone().provider, enabled: true }
          ),
          "The {} has an unknown provider",
          account_id
        );

        assert!(
          whitelist_entry.ident_document_date_of_expiry.is_none() ||
            whitelist_entry.ident_document_date_of_expiry.unwrap().0 > env::block_timestamp(),
          "The document expires in {}",
          account_id
        );
      }

      new_whitelist.insert(
        account_id,
        &whitelist_entries
          .into_iter()
          .map(|e| e.clone().into())
          .collect::<Vec<_>>()
      );
    }

    Self {
      admin_account: old_state.admin_account,
      whitelist: new_whitelist,
      config: config.into(),
    }
  }
}
