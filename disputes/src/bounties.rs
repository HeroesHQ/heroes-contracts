use near_sdk::serde::{Deserialize};
use near_sdk::{ext_contract, AccountId, Gas, PromiseOrValue};

pub const GAS_FOR_CHECK_BOUNTY: Gas = Gas(30_000_000_000_000);
pub const GAS_FOR_CHECK_BOUNTY_CLAIM: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_AFTER_CHECK_BOUNTY: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_SEND_RESULT_OF_DISPUTE: Gas = Gas(45_000_000_000_000);
pub const GAS_FOR_CHECK_PROPOSAL: Gas = Gas(70_000_000_000_000);
pub const GAS_FOR_AFTER_CHECK_PROPOSAL: Gas = Gas(55_000_000_000_000);

#[derive(Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct ValidatorsDao {
  pub account_id: AccountId,
}

#[derive(Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Bounty {
  pub owner: AccountId,
  pub validators_dao: Option<ValidatorsDao>,
  pub status: String,
}

#[derive(Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct BountyClaim {
  pub status: String,
}

#[ext_contract(ext_bounty_contract)]
trait ExtBountyContract {
  fn get_bounty(&self, id: u64) -> Bounty;
  fn get_bounty_claims_by_id(
    &self,
    id: u64,
  ) -> Vec<(AccountId, BountyClaim)>;
  fn dispute_result(&self, id: u64, success: bool) -> PromiseOrValue<()>;
}
