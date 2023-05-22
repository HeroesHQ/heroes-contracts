use crate::*;

#[near_bindgen]
impl DisputesContract {
  pub fn get_bounties_contract_account_id(&self) -> AccountId {
    self.bounties_contract.clone()
  }

  pub fn get_dispute_dao_account_id(&self) -> AccountId {
    self.dispute_dao.clone()
  }

  pub fn get_admin_whitelist(&self) -> Vec<AccountId> {
    self.admin_whitelist.to_vec()
  }

  pub fn get_config(&self) -> Config {
    self.config.clone()
  }

  pub fn get_bounty_disputes(
    &self,
    bounty_id: u64,
  ) -> Vec<(DisputeIndex, Dispute)> {
    self.bounty_disputes
      .get(&bounty_id)
      .unwrap_or_default()
      .into_iter()
      .map(|id| (id, self.disputes.get(&id).unwrap()))
      .collect()
  }

  pub fn get_dispute(&self, id: DisputeIndex) -> Dispute {
    let dispute = self.disputes.get(&id).expect("Dispute not found");
    dispute
  }

  pub fn get_last_dispute_id(&self) -> DisputeIndex {
    self.last_dispute_id
  }

  pub fn get_disputes_by_ids(&self, ids: Vec<DisputeIndex>) -> Vec<(DisputeIndex, Dispute)> {
    ids
      .into_iter()
      .filter_map(|id| self.disputes.get(&id).map(|dispute| (id, dispute)))
      .collect()
  }

  pub fn get_disputes(
    &self,
    from_index: Option<DisputeIndex>,
    limit: Option<DisputeIndex>,
  ) -> Vec<(DisputeIndex, Dispute)> {
    let from_index = from_index.unwrap_or(0);
    let limit = limit.unwrap_or(self.last_dispute_id);
    (from_index..std::cmp::min(from_index + limit, self.last_dispute_id))
      .filter_map(|id| self.disputes.get(&id).map(|dispute| (id, dispute)))
      .collect()
  }

  pub fn get_arguments(&self, id: DisputeIndex) -> Vec<Reason> {
    let arguments = self.arguments.get(&id).expect("No arguments");
    arguments
  }

  pub fn get_version() -> String {
    "2.0.0".to_string()
  }
}
