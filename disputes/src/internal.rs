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

  pub(crate) fn internal_send_result_of_dispute(
    &self,
    id: DisputeIndex,
    dispute: &mut Dispute,
    success: bool,
  ) -> PromiseOrValue<()> {
    ext_bounty_contract::ext(self.bounties_contract.clone())
      .with_static_gas(GAS_FOR_SEND_RESULT_OF_DISPUTE)
      .with_attached_deposit(1)
      .dispute_result(dispute.bounty_id.0, success)
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CLAIM_APPROVAL)
          .after_claim_approval(id, dispute, success)
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

  pub(crate) fn is_argument_period_expired(&self, dispute: &Dispute) -> bool {
    env::block_timestamp() > dispute.start_time.0 + self.config.argument_period.0
  }

  pub(crate) fn is_decision_period_expired(&self, dispute: &Dispute) -> bool {
    env::block_timestamp() > dispute.proposal_timestamp.unwrap().0 + self.config.decision_period.0
  }

  pub(crate) fn internal_add_proposal(
    &self,
    id: DisputeIndex,
    dispute: &mut Dispute,
  ) -> PromiseOrValue<()> {
    Promise::new(self.dispute_dao.clone())
      .function_call(
        "add_proposal".to_string(),
        json!({
          "proposal": {
            "description": dispute.get_proposal_description(),
            "kind": {
              "FunctionCall" : {
                "receiver_id": env::current_account_id(),
                "actions": [
                  {
                    "method_name": "result_of_dispute",
                    "args": Base64VecU8::from(json!({
                      "id": id.clone(),
                      "success": true,
                    })
                      .to_string()
                      .into_bytes()
                      .to_vec()),
                    "deposit": "1",
                    "gas": U64(GAS_FOR_CLAIM_APPROVAL.0),
                  }
                ],
              }
            }
          }
        })
          .to_string()
          .into_bytes(),
        self.config.add_proposal_bond.0,
        Gas(GAS_FOR_ADD_PROPOSAL.0),
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_ON_ADDED_PROPOSAL_CALLBACK)
          .on_added_proposal_callback(id, dispute)
      )
      .into()
  }

  pub(crate) fn internal_get_proposal(
    &mut self,
    id: DisputeIndex,
    dispute: &mut Dispute,
  ) -> PromiseOrValue<()> {
    Promise::new(self.dispute_dao.clone())
      .function_call(
        "get_proposal".to_string(),
        json!({"id": dispute.proposal_id.unwrap().0}).to_string().into_bytes(),
        NO_DEPOSIT,
        GAS_FOR_CHECK_PROPOSAL,
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CHECK_PROPOSAL)
          .after_get_proposal(id, dispute)
      )
      .into()
  }
}
