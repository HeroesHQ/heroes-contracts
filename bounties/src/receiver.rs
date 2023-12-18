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
    self.assert_live();
    let token_id = &env::predecessor_account_id();
    self.assert_that_token_is_allowed(token_id);
    assert!(
      !self.config.clone().to_config().use_owners_whitelist ||
        self.is_owner_whitelisted(sender_id.clone()),
      "You are not allowed to create bounties"
    );

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
