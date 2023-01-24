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
    self.assert_that_caller_is_allowed(token_id);
    // TODO: check for NFT token in sender_id

    let ft_message: FtMessage = serde_json::from_str(&msg).unwrap();
    match ft_message {
      FtMessage::BountyCreate(bounty_create) => {
        let bounty = bounty_create.to_bounty(&sender_id, token_id, amount);
        bounty.assert_new_valid();
        self.assert_bounty_type_is_correct(bounty.clone().metadata.bounty_type);
        let index = self.internal_add_bounty(bounty);
        log!(
          "Created new bounty for {} with index {}",
          sender_id,
          index
        );
      }
    }

    PromiseOrValue::Value(0.into())
  }
}
