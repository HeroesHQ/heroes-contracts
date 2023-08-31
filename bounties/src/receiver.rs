use crate::*;

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
#[serde(untagged)]
pub enum FtMessage {
  BountyCreate(BountyCreate),
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
        assert!(bounty_create.postpaid.is_none(), "The postpaid parameter is incorrect");
        self.internal_create_bounty(bounty_create, &sender_id, Some(token_id.clone()), amount);
      },
    }

    PromiseOrValue::Value(0.into())
  }
}
