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
      LookupMap::new(StorageKey::WhitelistEntries);

    for (_, provider) in config.providers.iter().enumerate() {
      Self::validate_provider(provider, true);
    }
    for (_, profile) in config.service_profiles.iter().enumerate() {
      Self::validate_service_profile(profile, &config, true);
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
          config.service_profiles
            .clone()
            .into_iter()
            .find(
              |p|
                Self::math_profile_params(
                  config.clone(),
                  p.clone(),
                  whitelist_entry.provider.clone(),
                  whitelist_entry.verification_type.clone(),
                  whitelist_entry.verification_level.clone(),
                  true,
                )
            )
            .is_some(),
          "For account {}, unknown provider {} to verify with {:?} and {:?}",
          account_id,
          whitelist_entry.provider,
          whitelist_entry.verification_type,
          whitelist_entry.verification_level,
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
          .collect()
      );
    }

    Self {
      admin_account: old_state.admin_account,
      whitelist: new_whitelist,
      config: config.into(),
    }
  }
}
