use crate::*;

#[near_bindgen]
impl BountiesContract {
  pub fn get_tokens(&self) -> Vec<(AccountId, TokenDetails)> {
    self.tokens
      .keys_as_vector()
      .to_vec()
      .into_iter()
      .map(|token_id| (token_id.clone(), self.tokens.get(&token_id).unwrap().into()))
      .collect()
  }

  pub fn get_admins_whitelist(&self) -> Vec<AccountId> {
    self.admins_whitelist.to_vec()
  }

  pub fn get_claimers_whitelist(&self, account_id: AccountId) -> Vec<AccountId> {
    self.claimers_whitelist.get(&account_id).unwrap_or_default()
  }

  pub fn get_reputation_contract_account_id(&self) -> Option<AccountId> {
    self.reputation_contract.clone()
  }

  pub fn get_dispute_contract_account_id(&self) -> Option<AccountId> {
    self.dispute_contract.clone()
  }

  pub fn get_kyc_whitelist_contract_id(&self) -> Option<AccountId> {
    self.kyc_whitelist_contract.clone()
  }

  pub fn get_config(&self) -> Config {
    self.config.clone().into()
  }

  /// Returns locked amount of NEAR that is used for storage.
  pub fn get_locked_storage_amount(&self) -> U128 {
    let locked_storage_amount = env::storage_byte_cost() * (env::storage_usage() as u128);
    U128(locked_storage_amount)
  }

  /// Returns available amount of NEAR that can be spent (outside of amount for storage and bonds).
  pub fn get_available_amount(&self) -> U128 {
    U128(env::account_balance() - self.get_locked_storage_amount().0 - self.locked_amount)
  }

  pub fn get_account_bounties(
    &self,
    account_id: AccountId,
  ) -> Vec<(BountyIndex, Bounty)> {
    self.account_bounties
      .get(&account_id)
      .unwrap_or_default()
      .into_iter()
      .map(|id| (id, self.bounties.get(&id).unwrap().into()))
      .collect()
  }

  pub fn get_bounty(&self, id: BountyIndex) -> Bounty {
    let bounty = self.bounties.get(&id).expect("Bounty not found");
    bounty.into()
  }

  pub fn get_last_bounty_id(&self) -> BountyIndex {
    self.last_bounty_id
  }

  pub fn get_bounties_by_ids(&self, ids: Vec<BountyIndex>) -> Vec<(BountyIndex, Bounty)> {
    ids
      .into_iter()
      .filter_map(|id| self.bounties.get(&id).map(|bounty| (id, bounty.into())))
      .collect()
  }

  pub fn get_bounties(
    &self,
    from_index: Option<BountyIndex>,
    limit: Option<BountyIndex>,
  ) -> Vec<(BountyIndex, Bounty)> {
    let from_index = from_index.unwrap_or(0);
    let limit = limit.unwrap_or(self.last_bounty_id);
    (from_index..std::cmp::min(from_index + limit, self.last_bounty_id))
      .filter_map(|id| self.bounties.get(&id).map(|bounty| (id, bounty.into())))
      .collect()
  }

  /// Get bounty claims for given user.
  pub fn get_bounty_claims(&self, account_id: AccountId) -> Vec<BountyClaim> {
    self.bounty_claimers
      .get(&account_id)
      .unwrap_or_default()
      .into_iter()
      .map(|c| c.into())
      .collect()
  }

  /// Get claims for bounty id.
  pub fn get_bounty_claims_by_id(
    &self,
    id: BountyIndex,
  ) -> Vec<(AccountId, BountyClaim)> {
    self.bounty_claimer_accounts
      .get(&id)
      .unwrap_or_default()
      .into_iter()
      .map(|account_id|
        {
          let (claims, index) = self.internal_get_claims(id, &account_id);
          (
            account_id,
            claims[index].clone()
          )
        }
      ).collect()
  }

  pub fn get_total_fees(&self, token_id: AccountId) -> FeeStats {
    self.total_fees.get(&token_id).expect("Token not found")
  }

  pub fn get_total_validators_dao_fees(&self, dao_account_id: AccountId) -> Vec<DaoFeeStats> {
    self.total_validators_dao_fees
      .get(&dao_account_id)
      .unwrap_or_default()
  }

  pub fn get_version() -> String {
    "2.0.0".to_string()
  }
}
