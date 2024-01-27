use crate::*;

#[derive(BorshDeserialize, BorshSerialize)]
pub struct OldState {
  pub bounties_contract: AccountId,
  pub dispute_dao: AccountId,
  pub last_dispute_id: DisputeIndex,
  pub disputes: LookupMap<DisputeIndex, DisputeV1>,
  pub arguments: LookupMap<DisputeIndex, Vec<Reason>>,
  pub bounty_disputes: LookupMap<u64, Vec<DisputeIndex>>,
  pub admin_whitelist: UnorderedSet<AccountId>,
  pub config: Config,
}

#[near_bindgen]
impl DisputesContract {
  #[private]
  #[init(ignore_state)]
  pub fn migrate() -> Self {
    let old_state: OldState = env::state_read().expect("Old state doesn't exist");
    Self {
      bounties_contract: old_state.bounties_contract,
      dispute_dao: old_state.dispute_dao,
      last_dispute_id: old_state.last_dispute_id,
      disputes: LookupMap::new(StorageKey::Disputes),
      arguments: LookupMap::new(StorageKey::Reasons),
      bounty_disputes: old_state.bounty_disputes,
      admin_whitelist: old_state.admin_whitelist,
      config: old_state.config.into(),
      status: ContractStatus::Genesis,
    }
  }
}
