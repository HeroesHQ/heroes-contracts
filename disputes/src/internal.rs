use crate::*;

impl DisputesContract {
  pub(crate) fn assert_admin_whitelist(&self, account_id: &AccountId) {
    assert!(
      self.admin_whitelist.contains(account_id),
      "Not in admin whitelist"
    );
  }

  pub(crate) fn internal_save_bounty_disputes(
    &mut self,
    bounty_id: &u64,
    indices: Vec<DisputeIndex>,
  ) {
    if indices.is_empty() {
      self.bounty_disputes.remove(bounty_id);
    } else {
      self.bounty_disputes.insert(bounty_id, &indices);
    }
  }

  pub(crate) fn internal_add_dispute(&mut self, dispute: Dispute) -> DisputeIndex {
    let id = self.last_dispute_id;
    self.disputes.insert(&id, &dispute);
    let bounty_id = dispute.bounty_id.0;
    let mut indices = self
      .bounty_disputes
      .get(&bounty_id)
      .unwrap_or_default();
    indices.push(id);
    self.internal_save_bounty_disputes(&bounty_id, indices);
    self.last_dispute_id += 1;
    id
  }

  pub(crate) fn internal_get_bounty_info(
    &mut self,
    bounty_id: u64,
    description: String,
  ) -> PromiseOrValue<()> {
    ext_bounty_contract::ext(self.bounties_contract.clone())
      .with_static_gas(GAS_FOR_CHECK_BOUNTY)
      .get_bounty(bounty_id.clone())
      .and(
        ext_bounty_contract::ext(self.bounties_contract.clone())
          .with_static_gas(GAS_FOR_CHECK_BOUNTY_CLAIM)
          .get_bounty_claims_by_id(bounty_id.clone())
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CHECK_BOUNTY)
          .after_get_bounty_info(bounty_id, description)
      )
      .into()
  }

  pub(crate) fn internal_find_claimer(
    claims: Vec<(AccountId, BountyClaim)>,
  ) -> Option<AccountId> {
    for i in 0..claims.len() {
      if claims[i].1.status == "Rejected" {
        return Some(claims[i].0.clone())
      }
    }
    None
  }
}
