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
    self.last_bounty_id += 1;
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

  pub(crate) fn internal_return_bonds(&mut self, receiver_id: &AccountId) -> PromiseOrValue<()> {
    self.locked_amount -= self.config.bounty_claim_bond.0;
    Promise::new(receiver_id.clone())
      .transfer(self.config.bounty_claim_bond.0)
      .into()
  }

  pub(crate) fn internal_reset_bounty_to_initial_state(
    &mut self,
    id: BountyIndex,
    receiver_id: &AccountId,
    bounty: &mut Bounty,
    claim_idx: usize,
    claims: &mut Vec<BountyClaim>
  ) {
    bounty.status = BountyStatus::New;
    self.bounties.insert(&id, &bounty);
    claims[claim_idx].status = ClaimStatus::NotCompleted;
    self.internal_save_claims(receiver_id, &claims);
    // TODO: update reputation
    self.internal_return_bonds(receiver_id);
  }

  pub(crate) fn is_deadline_for_opening_dispute_expired(&self, claim: &BountyClaim) -> bool {
    env::block_timestamp() >
      claim.rejected_timestamp.unwrap().0 + self.config.period_for_opening_dispute.0
  }

  pub(crate) fn internal_set_claim_expiry_status(
    &mut self,
    id: BountyIndex,
    receiver_id: &AccountId,
    bounty: &mut Bounty,
    claim_idx: usize,
    claims: &mut Vec<BountyClaim>
  ) {
    claims[claim_idx].status = ClaimStatus::Expired;
    self.internal_save_claims(receiver_id, &claims);
    bounty.status = BountyStatus::New;
    self.bounties.insert(&id, &bounty);
  }

  pub(crate) fn internal_find_active_claim(
    &mut self,
    id: BountyIndex,
  ) -> (AccountId, Vec<BountyClaim>, usize) {
    let bounty = self.get_bounty(id.clone());
    assert!(
      matches!(bounty.status, BountyStatus::Claimed),
      "Bounty status does not allow completion"
    );

    let account_ids = self
      .bounty_claimer_accounts
      .get(&id)
      .expect("No claims found");

    for i in 0..account_ids.len() {
      let account_id = account_ids[i].clone();
      let claims = self.get_bounty_claims(account_id.clone());
      let index = self.internal_find_claim(id, &claims).unwrap();
      let claim = claims[index].clone();
      if matches!(claim.status, ClaimStatus::New)
        || matches!(claim.status, ClaimStatus::Completed)
        || matches!(claim.status, ClaimStatus::Rejected)
        || matches!(claim.status, ClaimStatus::Disputed)
      {
        return (account_id, claims, index);
      }
    }

    env::panic_str("No active claims");
  }

  pub(crate) fn internal_bounty_payout(
    &mut self,
    id: BountyIndex,
    receiver_id: AccountId,
    bounty: &mut Bounty,
    claim_idx: usize,
    claims: &mut Vec<BountyClaim>,
  ) -> PromiseOrValue<()> {
    ext_ft_contract::ext(bounty.token.clone())
      .with_attached_deposit(ONE_YOCTO)
      .with_static_gas(GAS_FOR_FT_TRANSFER)
      .ft_transfer(
        receiver_id.clone(),
        bounty.amount.clone(),
        Some(format!("Bounty {} payout", id)),
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_FT_TRANSFER)
          .after_ft_transfer(id, receiver_id, bounty, claims, claim_idx)
      )
      .into()
  }

  pub(crate) fn internal_reject_claim(
    &mut self,
    id: BountyIndex,
    receiver_id: AccountId,
    bounty: &mut Bounty,
    claim_idx: usize,
    claims: &mut Vec<BountyClaim>,
  ) {
    if self.dispute_contract.is_some() {
      claims[claim_idx].status = ClaimStatus::Rejected;
      claims[claim_idx].rejected_timestamp = Some(env::block_timestamp().into());
      self.internal_save_claims(&receiver_id, &claims);
    } else {
      // If the creation of a dispute is not foreseen,
      // then the bounty reset to initial state
      self.internal_reset_bounty_to_initial_state(id, &receiver_id, bounty, claim_idx, claims);
    }
  }

  pub(crate) fn internal_add_proposal(
    &mut self,
    id: BountyIndex,
    sender_id: &AccountId,
    bounty: &mut Bounty,
    claim_idx: usize,
    claims: &mut Vec<BountyClaim>,
    description: String,
  ) -> PromiseOrValue<()> {
    let dao = bounty.validators_dao.clone().unwrap();
    Promise::new(dao.account_id)
      .function_call(
        "add_proposal".to_string(),
        json!({
              "proposal": {
                "description": description,
                "kind": {
                  "FunctionCall" : {
                    "receiver_id": env::current_account_id(),
                    "actions": [
                      {
                        "method_name": "bounty_action",
                        "args": Base64VecU8::from(json!({
                          "id": id.clone(),
                          "action": {
                            "ClaimApproved": {
                              "receiver_id": sender_id.to_string(),
                            }
                          }
                        })
                          .to_string()
                          .into_bytes()
                          .to_vec()),
                        "deposit": "1",
                        "gas": dao.gas_for_claim_approval,
                      }
                    ],
                  }
                }
              }
            })
          .to_string()
          .into_bytes(),
        dao.add_proposal_bond.0,
        Gas(dao.gas_for_add_proposal.0),
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_ON_ADDED_PROPOSAL_CALLBACK)
          .on_added_proposal_callback(sender_id, claims.as_mut(), claim_idx)
      )
      .into()
  }

  pub(crate) fn internal_get_proposal(
    &mut self,
    id: BountyIndex,
    receiver_id: AccountId,
    bounty: &mut Bounty,
    claim_idx: usize,
    claims: &mut Vec<BountyClaim>,
  ) -> PromiseOrValue<()> {
    let dao = bounty.validators_dao.clone().unwrap();
    let proposal_id = claims[claim_idx].proposal_id.unwrap();
    Promise::new(dao.account_id)
      .function_call(
        "get_proposal".to_string(),
        json!({"id": proposal_id.0}).to_string().into_bytes(),
        NO_DEPOSIT,
        GAS_FOR_CHECK_PROPOSAL,
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CHECK_PROPOSAL)
          .after_get_proposal(id, receiver_id, bounty, claims, claim_idx)
      )
      .into()
  }

  pub(crate) fn internal_create_dispute(
    &mut self,
    id: BountyIndex,
    receiver_id: &AccountId,
    bounty: Bounty,
    claim_idx: usize,
    claims: &mut Vec<BountyClaim>,
    description: String,
  ) -> PromiseOrValue<()> {
    let project_owner_delegate = if bounty.validators_dao.is_some() {
      bounty.validators_dao.unwrap().account_id
    } else {
      bounty.owner.clone()
    };
    let dispute_create = DisputeCreate {
      bounty_id: id.clone(),
      description,
      claimer: receiver_id.clone(),
      project_owner_delegate,
    };

    Promise::new(self.dispute_contract.clone().unwrap())
      .function_call(
        "create_dispute".to_string(),
        json!({
          "dispute_create": dispute_create,
        })
          .to_string()
          .into_bytes(),
        ONE_YOCTO,
        GAS_FOR_CREATE_DISPUTE,
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CREATE_DISPUTE)
          .after_create_dispute(receiver_id, claims, claim_idx)
      )
      .into()
  }

  pub(crate) fn internal_find_claimer(
    &self,
    id: BountyIndex,
  ) -> Option<AccountId> {
    let claims = self.get_bounty_claims_by_id(id.clone());
    for i in 0..claims.len() {
      if matches!(claims[i].1.status, ClaimStatus::Disputed) {
        return Some(claims[i].0.clone())
      }
    }
    None
  }

  pub(crate) fn internal_get_dispute(
    &mut self,
    id: BountyIndex,
    receiver_id: AccountId,
    bounty: &mut Bounty,
    claim_idx: usize,
    claims: &mut Vec<BountyClaim>,
  ) -> PromiseOrValue<()> {
    Promise::new(self.dispute_contract.clone().unwrap())
      .function_call(
        "get_dispute".to_string(),
        json!({
          "id": claims[claim_idx].dispute_id.unwrap().0,
        })
          .to_string()
          .into_bytes(),
        NO_DEPOSIT,
        GAS_FOR_CHECK_DISPUTE,
      )
      .then(
        Self::ext(env::current_account_id())
          .with_static_gas(GAS_FOR_AFTER_CHECK_DISPUTE)
          .after_get_dispute(id, receiver_id, bounty, claims, claim_idx)
      )
      .into()
  }
}
