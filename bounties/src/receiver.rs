use crate::*;

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[serde(untagged)]
pub enum FtMessage {
  BountyCreate(BountyCreate),
  BountyDeposit(BountyIndex),
}

#[near_bindgen]
impl FungibleTokenReceiver for BountiesContract {
  fn ft_on_transfer(
    &mut self,
    sender_id: AccountId,
    amount: U128,
    msg: String,
  ) -> PromiseOrValue<U128> {
    let token_id = &env::predecessor_account_id();
    self.assert_that_token_is_allowed(token_id);

    let ft_message: FtMessage = serde_json::from_str(&msg).unwrap();
    match ft_message {
      FtMessage::BountyCreate(bounty_create) => {
        self.internal_create_bounty(bounty_create, &sender_id, token_id, amount);
      },
      FtMessage::BountyDeposit(id) => {
        self.internal_bounty_deposit(id, &sender_id, token_id, amount);
      }
    }

    PromiseOrValue::Value(0.into())
  }
}
