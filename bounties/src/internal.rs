use crate::*;

impl BountiesContract {
  pub(crate) fn assert_that_caller_is_allowed(&self, account_id: &AccountId) {
    assert!(
      self.token_account_ids.contains(account_id),
      "Predecessor Account ID is not an allowed FT contract"
    );
  }

  pub(crate) fn assert_admin_whitelist(&self, account_id: &AccountId) {
    assert!(
      self.admin_whitelist.contains(account_id),
      "Not in admin whitelist"
    );
  }

  pub(crate) fn internal_add_bounty(&mut self, bounty: &Bounty) -> BountyIndex {
    let id = self.last_bounty_id;
    self.bounties.insert(&id, bounty);
    let mut indices = self
      .account_bounties
      .get(&bounty.owner)
      .unwrap_or_default();
    indices.push(id);
    self.internal_save_account_bounties(&bounty.owner, indices);
    id
  }

  pub(crate) fn internal_save_account_bounties(
    &mut self,
    account_id: &AccountId,
    indices: Vec<BountyIndex>,
  ) {
    if indices.is_empty() {
      self.account_bounties.remove(account_id);
    } else {
      self.account_bounties.insert(account_id, &indices);
    }
  }

  pub(crate) fn internal_find_claim(
    &self,
    id: BountyIndex,
    claims: &[BountyClaim]
  ) -> Option<usize> {
    for i in 0..claims.len() {
      if claims[i].bounty_id == id {
        return Some(i);
      }
    }
    None
  }

  pub(crate) fn internal_add_claim(
    &self,
    id: BountyIndex,
    claims: &mut Vec<BountyClaim>,
    new_claim: BountyClaim
  ) {
    let claim_idx = self.internal_find_claim(id, claims);
    if claim_idx.is_some() {
      claims.insert(claim_idx.unwrap(), new_claim);
    } else {
      claims.push(new_claim);
    }
  }

  pub(crate) fn internal_get_claims(
    &self,
    id: BountyIndex,
    sender_id: &AccountId
  ) -> (Vec<BountyClaim>, usize) {
    let claims = self
      .bounty_claimers
      .get(&sender_id)
      .expect("No claimer found");
    let claim_idx = self
      .internal_find_claim(id, &claims)
      .expect("No bounty claim found");
    (claims, claim_idx)
  }

  pub(crate) fn internal_save_claims(
    &mut self,
    account_id: &AccountId,
    claims: &Vec<BountyClaim>,
  ) {
    if claims.is_empty() {
      self.bounty_claimers.remove(account_id);
    } else {
      self.bounty_claimers.insert(account_id, claims);
    }
  }

  pub(crate) fn internal_add_bounty_claimer_account(
    &mut self,
    id: BountyIndex,
    account_id: AccountId,
  ) {
    let mut claimer_accounts = self.bounty_claimer_accounts.get(&id).unwrap_or_default();
    if !claimer_accounts.contains(&account_id) {
      claimer_accounts.push(account_id);
      self.bounty_claimer_accounts.insert(&id, &claimer_accounts);
    }
  }

  pub(crate) fn internal_return_bonds(&mut self, receiver_id: AccountId) -> PromiseOrValue<()> {
    self.locked_amount -= self.config.bounty_claim_bond.0;
    Promise::new(receiver_id)
      .transfer(self.config.bounty_claim_bond.0)
      .into()
  }
}
