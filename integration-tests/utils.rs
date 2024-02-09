use near_sdk::json_types::{U128, U64};
use near_sdk::serde::Deserialize;
use near_sdk::{Balance, ONE_NEAR, ONE_YOCTO};
use near_units::parse_near;
use serde_json::{json, Value};
use workspaces::{Account, AccountId, Contract, Worker};
use workspaces::network::Sandbox;
use workspaces::result::ExecutionFinalResult;
use bounties::{Bounty, BountyAction, BountyClaim, BountyFlow, BountyStatus, BountyUpdate,
               ClaimStatus, ConfigCreate, DaoFeeStats, DefermentOfKYC, FeeStats, KycConfig,
               Multitasking, Postpaid, ReferenceType, Reviewers, ReviewersParams, TokenDetails,
               ValidatorsDaoParams, WhitelistType};
use disputes::{Dispute, Proposal};
use kyc_whitelist::{ActivationType, Config, VerificationType};
use reputation::{ClaimerMetrics, BountyOwnerMetrics};

#[derive(Deserialize)]
#[serde(crate = "near_sdk::serde")]
struct DaoPolicy {
  proposal_bond: U128,
}

pub const TEST_TOKEN_WASM: &str = "./test-token/res/test_token.wasm";
pub const BOUNTIES_WASM: &str = "./bounties/res/bounties.wasm";
pub const SPUTNIK_DAO2_WASM: &str = "./integration-tests/res/sputnikdao2.wasm";
pub const DISPUTES_WASM: &str = "./disputes/res/disputes.wasm";
pub const REPUTATION_WASM: &str = "./reputation/res/reputation.wasm";
pub const KYC_WHITELIST_WASM: &str = "./kyc-whitelist/res/kyc-whitelist.wasm";

pub const BOUNTY_AMOUNT: U128 = U128(99 * 10u128.pow(16)); // 0.99 TFT (Test fungible token)
pub const PLATFORM_FEE: U128 = U128(11 * 10u128.pow(16)); // 0.11 TFT
pub const DAO_FEE: U128 = U128(11 * 10u128.pow(16)); // 0.11 TFT
pub const MIN_AMOUNT_FOR_KYC: U128 = U128(2 * 10u128.pow(18)); // 2 TFT
pub const MAX_DEADLINE: U64 = U64(7 * 24 * 60 * 60 * 1_000 * 1_000_000); // 7 days
pub const BOND: Balance = ONE_NEAR;
pub const MAX_GAS: Balance = 3 * 10u128.pow(22); // 300 Tgas

pub struct Env {
  pub worker: Worker<Sandbox>,
  pub root: Account,
  pub bounties_contract_admin: Account,
  pub project_owner: Account,
  pub dao_council_member: Account,
  pub freelancer: Account,
  pub test_token: Contract,
  pub bounties: Contract,
  pub validators_dao: Contract,
  pub arbitrator: Account,
  pub dispute_dao: Contract,
  pub disputed_bounties: Contract,
  pub dispute_contract: Contract,
  pub reputation_contract: Contract,
  pub kyc_whitelist_contract: Contract,
  pub kyc_service_account: Account,
}

pub async fn get_bounty(
  bounties: &Contract,
  bounty_id: u64,
) -> anyhow::Result<Bounty> {
  let bounty = bounties
    .call("get_bounty")
    .args_json((bounty_id,))
    .view()
    .await?
    .json()?;
  Ok(bounty)
}

pub async fn get_bounties_by_ids(
  bounties: &Contract,
  bounty_ids: Vec<u64>,
) -> anyhow::Result<Vec<(u64, Bounty)>> {
  let result = bounties
    .call("get_bounties_by_ids")
    .args_json((bounty_ids,))
    .view()
    .await?
    .json()?;
  Ok(result)
}

pub async fn get_last_bounty_id(bounties: &Contract) -> anyhow::Result<u64> {
  let last_bounty_id = bounties.call("get_last_bounty_id").view().await?.json::<u64>()?;
  Ok(last_bounty_id)
}

impl Env {
  pub async fn init() -> anyhow::Result<Self> {
    let worker = workspaces::sandbox().await?;
    let root = worker.root_account().unwrap();

    let bounties_contract_admin = root
      .create_subaccount("admin")
      .initial_balance(parse_near!("30 N"))
      .transact()
      .await?
      .into_result()?;

    let project_owner = root
      .create_subaccount("owner")
      .initial_balance(parse_near!("30 N"))
      .transact()
      .await?
      .into_result()?;

    let dao_council_member = root
      .create_subaccount("expert")
      .initial_balance(parse_near!("30 N"))
      .transact()
      .await?
      .into_result()?;

    let freelancer = root
      .create_subaccount("freelancer")
      .initial_balance(parse_near!("30 N"))
      .transact()
      .await?
      .into_result()?;

    let test_token = worker.dev_deploy(&std::fs::read(TEST_TOKEN_WASM)?).await?;
    let mut res = test_token
      .call("new")
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;

    res = test_token
      .call("mint")
      .args_json((project_owner.id(), "100000000000000000000".to_string()))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Self::register_user(&test_token, freelancer.id()).await?;

    let bounties = worker.dev_deploy(&std::fs::read(BOUNTIES_WASM)?).await?;
    res = bounties
      .call("new")
      .args_json(json!({
        "admins_whitelist": vec![bounties_contract_admin.id()],
      }))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Self::set_live_status(&bounties).await?;
    Self::register_user(&test_token, bounties.id()).await?;
    Self::add_token(&test_token, &bounties, &bounties_contract_admin).await?;

    let validators_dao = worker.dev_deploy(&std::fs::read(SPUTNIK_DAO2_WASM)?).await?;
    res = validators_dao
      .call("new")
      .args_json(json!({
        "config": {
          "name": "genesis2",
          "purpose": "test",
          "metadata": "",
        },
        "policy": vec![dao_council_member.id()],
      }))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;

    let arbitrator = root
      .create_subaccount("arbitrator")
      .initial_balance(parse_near!("30 N"))
      .transact()
      .await?
      .into_result()?;

    let dispute_dao = worker.dev_deploy(&std::fs::read(SPUTNIK_DAO2_WASM)?).await?;
    let mut res = dispute_dao
      .call("new")
      .args_json(json!({
        "config": {
          "name": "genesis2",
          "purpose": "test",
          "metadata": "",
        },
        "policy": vec![arbitrator.id()],
      }))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;

    let disputed_bounties = worker.dev_deploy(&std::fs::read(BOUNTIES_WASM)?).await?;

    let mut disputes_config = disputes::Config::default();
    disputes_config.argument_period = U64(1_000_000_000 * 60 * 5); // 5 min
    disputes_config.decision_period = U64(1_000_000_000 * 60 * 5); // 5 min

    let dispute_contract = worker.dev_deploy(&std::fs::read(DISPUTES_WASM)?).await?;
    res = dispute_contract
      .call("new")
      .args_json(json!({
        "bounties_contract": disputed_bounties.id(),
        "dispute_dao": dispute_dao.id(),
        "admin_whitelist": vec![bounties_contract_admin.id()],
        "config": disputes_config,
      }))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Self::set_live_status(&dispute_contract).await?;

    let reputation_contract = worker.dev_deploy(&std::fs::read(REPUTATION_WASM)?).await?;
    res = reputation_contract
      .call("new")
      .args_json(json!({
        "bounties_contract": disputed_bounties.id(),
        "admin_whitelist": vec![bounties_contract_admin.id()],
      }))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;

    let kyc_service_account = root
      .create_subaccount("kyc_service")
      .initial_balance(parse_near!("30 N"))
      .transact()
      .await?
      .into_result()?;

    let kyc_whitelist_contract = worker.dev_deploy(&std::fs::read(KYC_WHITELIST_WASM)?).await?;
    res = kyc_whitelist_contract
      .call("new")
      .args_json(json!({
        "admin_account": bounties_contract_admin.id(),
        "config": Option::<Config>::None
      }))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;

    res = bounties_contract_admin
      .call(kyc_whitelist_contract.id(), "create_provider")
      .args_json(json!({
        "provider": json!({
          "name": "fractal",
          "enabled": ActivationType::Enabled
        })
      }))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;

    res = bounties_contract_admin
      .call(kyc_whitelist_contract.id(), "create_service_profile")
      .args_json(json!({
        "service_profile": json!({
          "service_name": "heroes",
          "service_account": kyc_service_account.id(),
          "verification_types": vec![json!({
            "provider": "fractal",
            "verification_type": VerificationType::KYC,
            "verification_level": "basic+liveness+uniq",
            "enabled": ActivationType::Enabled
          })]
        })
      }))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;

    res = bounties_contract_admin
      .call(kyc_whitelist_contract.id(), "set_default_profile")
      .args_json(("heroes",))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;

    let mut bounties_config = bounties::Config::default();
    bounties_config.period_for_opening_dispute = U64(1_000_000_000 * 60 * 10); // 10 min
    bounties_config.bounty_forgiveness_period = U64(1_000_000_000 * 60 * 10); // 10 min

    res = disputed_bounties
      .call("new")
      .args_json(json!({
        "admins_whitelist": vec![bounties_contract_admin.id()],
        "config": bounties_config,
        "reputation_contract": reputation_contract.id(),
        "dispute_contract": dispute_contract.id(),
        "kyc_whitelist_contract": kyc_whitelist_contract.id(),
        "recipient_of_platform_fee": bounties_contract_admin.id(),
      }))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Self::set_live_status(&disputed_bounties).await?;
    Self::register_user(&test_token, disputed_bounties.id()).await?;
    Self::register_user(&test_token, bounties_contract_admin.id()).await?;
    Self::register_user(&test_token, validators_dao.id()).await?;
    Self::add_token(&test_token, &disputed_bounties, &bounties_contract_admin).await?;
    Self::update_configuration_dictionary_entries(
      &disputed_bounties,
      &bounties_contract_admin,
      ReferenceType::Currencies,
      "USD".to_string()
    ).await?;

    Ok(
      Self {
        worker,
        root,
        bounties_contract_admin,
        project_owner,
        dao_council_member,
        freelancer,
        test_token,
        bounties,
        validators_dao,
        arbitrator,
        dispute_dao,
        disputed_bounties,
        dispute_contract,
        reputation_contract,
        kyc_whitelist_contract,
        kyc_service_account,
      }
    )
  }

  pub async fn register_user(test_token: &Contract, user: &AccountId) -> anyhow::Result<()> {
    let res = test_token
      .call("storage_deposit")
      .args_json((user, Option::<bool>::None))
      .deposit(parse_near!("0.1 N"))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  async fn add_token(
    test_token: &Contract,
    bounties: &Contract,
    administrator: &Account,
  ) -> anyhow::Result<()> {
    let res = administrator
      .call(bounties.id(), "add_token_id")
      .args_json(json!({
        "token_id": test_token.id(),
        "min_amount_for_kyc": MIN_AMOUNT_FOR_KYC,
      }))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn update_token(
    &self,
    bounties: &Contract,
    token_details: &TokenDetails,
  ) -> anyhow::Result<()> {
    let res = self.bounties_contract_admin
      .call(bounties.id(), "update_token")
      .args_json(json!({
        "token_id": self.test_token.id(),
        "token_details": token_details,
      }))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn add_account(
    &self,
    account_id: &str,
  ) -> anyhow::Result<Account> {
    let account = self.root
      .create_subaccount(account_id)
      .initial_balance(parse_near!("30 N"))
      .transact()
      .await?
      .into_result()?;
    Ok(account)
  }

  async fn assert_contract_call_result(
    result: ExecutionFinalResult,
    expected_msg: Option<&str>,
  ) -> anyhow::Result<()> {
    if expected_msg.is_none() {
      if result.is_failure() {
        println!("{:#?}", result.clone().into_result().err());
      }
      assert!(result.is_success());
    } else {
      assert!(result.is_failure());
      assert!(
        result
          .clone()
          .into_result()
          .err()
          .unwrap()
          .to_string()
          .contains(&expected_msg.unwrap())
      );
    }
    Ok(())
  }

  pub async fn assert_exists_failed_receipts(
    result: ExecutionFinalResult,
    expected_msg: &str,
  ) -> anyhow::Result<()> {
    assert!(
      result
        .into_result()
        .unwrap()
        .receipt_outcomes()
        .into_iter()
        .find(|&e| e.is_failure())
        .expect("No failed receipts")
        .clone()
        .into_result()
        .err()
        .unwrap()
        .to_string()
        .contains(expected_msg)
    );
    Ok(())
  }

  pub async fn assert_statuses(
    &self,
    bounties: &Contract,
    bounty_id: u64,
    claimer_id: Option<&AccountId>,
    claim_number: Option<u8>,
    claim_status: ClaimStatus,
    bounty_status: BountyStatus,
  ) -> anyhow::Result<(BountyClaim, Bounty)> {
    let bounty_claims = Self::get_bounty_claims_by_id(bounties, bounty_id).await?;
    let freelancer = claimer_id.unwrap_or(self.freelancer.id());
    let claim_idx = bounty_claims
      .iter()
      .position(
        |c|
          c.0.to_string() == freelancer.to_string() && c.1.claim_number == claim_number
      );
    let bounty_claim = bounty_claims[claim_idx.expect("No claim found")].clone().1;
    assert_eq!(bounty_claim.status, claim_status);
    let bounty = get_bounty(bounties, bounty_id).await?;
    assert_eq!(bounty.status, bounty_status);
    Ok((bounty_claim, bounty))
  }

  async fn get_proposal_bond(&self) -> anyhow::Result<U128> {
    let policy: DaoPolicy = self.validators_dao
      .call("get_policy")
      .view()
      .await?
      .json()?;
    Ok(policy.proposal_bond)
  }

  async fn get_claim_proposal_id(
    bounties: &Contract,
    bounty_id: u64,
    claim_index: usize,
    for_payout_proposal: bool,
  ) -> anyhow::Result<U64> {
    let bounty_claims = Self::get_bounty_claims_by_id(bounties, bounty_id).await?;
    assert!(bounty_claims.len() > claim_index);
    let bounty_claim = bounty_claims[claim_index].clone().1;
    let proposal_id = if for_payout_proposal {
      bounty_claim.bounty_payout_proposal_id.expect("No payout proposal found")
    } else {
      bounty_claim.approve_claimer_proposal_id.expect("No claimer approve proposal found")
    };
    Ok(proposal_id)
  }

  pub async fn get_bounty_claims_by_id(
    bounties: &Contract,
    bounty_id: u64,
  ) -> anyhow::Result<Vec<(near_sdk::AccountId, BountyClaim)>> {
    let bounty_claims = bounties
      .call("get_bounty_claims_by_id")
      .args_json((bounty_id,))
      .view()
      .await?
      .json()?;
    Ok(bounty_claims)
  }

  pub async fn get_token_balance(
    &self,
    account_id: &AccountId,
  ) -> anyhow::Result<u128> {
    let token_balance: U128 = self.test_token
      .call("ft_balance_of")
      .args_json((account_id,))
      .view()
      .await?
      .json()?;
    Ok(token_balance.0)
  }

  pub async fn get_bounty_claims(
    &self,
    bounties: &Contract,
  ) -> anyhow::Result<Vec<BountyClaim>> {
    let bounty_claims = bounties
      .call("get_bounty_claims")
      .args_json((self.freelancer.id(),))
      .view()
      .await?
      .json()?;
    Ok(bounty_claims)
  }

  pub async fn get_account_bounties(
    &self,
    bounties: &Contract,
  ) -> anyhow::Result<Vec<(u64, Bounty)>> {
    let bounty_indexes: Vec<(u64, Bounty)> = bounties
      .call("get_account_bounties")
      .args_json((self.project_owner.id(),))
      .view()
      .await?
      .json()?;
    Ok(bounty_indexes)
  }

  async fn get_bounty_amount(
    total_amount: Option<U128>,
    reviewers: Option<Reviewers>,
    postpaid: Option<Postpaid>,
  ) -> anyhow::Result<u128> {
    let mut amount;
    if total_amount.is_some() {
      amount = total_amount.unwrap().0;
    } else {
      amount = BOUNTY_AMOUNT.0 + if postpaid.is_none() { PLATFORM_FEE.0 } else { 0 };
      if reviewers.is_some() && postpaid.is_none() {
        amount += match reviewers.clone().unwrap() {
          Reviewers::ValidatorsDao { .. } => DAO_FEE.0,
          _ => 0,
        };
      }
    }
    Ok(amount)
  }

  pub async fn add_bounty(
    &self,
    bounties: &Contract,
    deadline: Value,
    claimer_approval: Value,
    reviewers_params: Option<ReviewersParams>,
    total_amount: Option<U128>,
    kyc_required: Option<KycConfig>,
    postpaid: Option<Postpaid>,
    multitasking: Option<Multitasking>,
    allow_deadline_stretch: Option<bool>,
    bounty_flow: Option<BountyFlow>,
    allow_creating_many_claims: Option<bool>,
    expected_msg: Option<&str>,
  ) -> anyhow::Result<ExecutionFinalResult> {
    let metadata = json!({
      "title": "Test bounty title",
      "description": "Test bounty description",
      "category": "Marketing",
      "attachments": vec!["http://ipfs-url/1", "http://ipfs-url/2"],
      "experience": "Intermediate",
      "tags": vec!["Community", "NFT", "CustomTag"],
      "acceptance_criteria": "test acceptance criteria",
      "contact_details": json!({
        "contact": "example@domain.net",
        "contact_type": "Email"
      }),
    });

    let bounty_create = json!({
      "metadata": metadata,
      "deadline": deadline,
      "claimer_approval": claimer_approval,
      "reviewers": match reviewers_params.clone() {
        Some(r) => json!(r),
        _ => json!(null),
      },
      "kyc_config": if kyc_required.is_some() {
        kyc_required
      } else {
        Some(KycConfig::KycNotRequired)
      },
      "postpaid": match postpaid.clone() {
        Some(r) => json!(r),
        _ => json!(null),
      },
      "multitasking": match multitasking.clone() {
        Some(r) => json!(r),
        _ => json!(null),
      },
      "allow_deadline_stretch": json!(allow_deadline_stretch.unwrap_or_default()),
      "bounty_flow": bounty_flow,
      "allow_creating_many_claims": json!(allow_creating_many_claims.unwrap_or_default()),
    });

    let reviewers = match reviewers_params {
      Some(p) => Some(p.to_reviewers()),
      _ => None
    };
    let amount = Self::get_bounty_amount(total_amount, reviewers, postpaid.clone()).await?;
    let res;

    if postpaid.is_none() {
      res = self.project_owner
        .call(self.test_token.id(), "ft_transfer_call")
        .args_json(json!({
          "receiver_id": bounties.id(),
          "amount": U128(amount),
          "msg": bounty_create.to_string(),
        }))
        .max_gas()
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    } else {
      let token_id = if postpaid.is_some() &&
        matches!(postpaid.clone().unwrap(), Postpaid::PaymentOutsideContract { .. })
      {
        json!(null)
      } else {
        json!(self.test_token.id())
      };
      res = self.project_owner
        .call(bounties.id(), "bounty_create")
        .args_json(json!({
          "bounty_create": bounty_create,
          "token_id": token_id,
          "amount": if amount == 0 { json!(null) } else { json!(U128(amount)) },
        }))
        .max_gas()
        .deposit(ONE_YOCTO)
        .transact()
        .await?;
    }
    Self::assert_contract_call_result(res.clone(), expected_msg).await?;
    Ok(res)
  }

  pub async fn mark_as_paid(
    &self,
    bounties: &Contract,
    bounty_id: u64,
    claimer: Option<&AccountId>,
    claim_number: Option<u8>,
  ) -> anyhow::Result<()> {
    let res = self.project_owner
      .call(bounties.id(), "mark_as_paid")
      .args_json((bounty_id, claimer, claim_number))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn confirm_payment(
    &self,
    bounties: &Contract,
    bounty_id: u64,
    user: Option<&Account>,
    claim_number: Option<u8>,
  ) -> anyhow::Result<()> {
    let res = if user.is_some() { user.unwrap() } else { &self.freelancer }
      .call(bounties.id(), "confirm_payment")
      .args_json((bounty_id, claim_number))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn bounty_claim(
    &self,
    bounties: &Contract,
    bounty_id: u64,
    deadline: Option<U64>,
    description: String,
    slot: Option<usize>,
    user: Option<&Account>,
    expected_msg: Option<&str>,
  ) -> anyhow::Result<()> {
    let config: bounties::Config = bounties.call("get_config").view().await?.json()?;
    let res = if user.is_some() { user.unwrap() } else { &self.freelancer }
      .call(bounties.id(), "bounty_claim")
      .args_json((bounty_id, deadline, description, slot))
      .max_gas()
      .deposit(config.bounty_claim_bond.0)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, expected_msg).await?;
    Ok(())
  }

  pub async fn bounty_done(
    &self,
    bounties: &Contract,
    bounty_id: u64,
    description: String,
    freelancer: Option<&Account>,
    claim_number: Option<u8>,
    expected_msg: Option<&str>,
  ) -> anyhow::Result<()> {
    let res = if freelancer.is_some() { freelancer.unwrap() } else { &self.freelancer }
      .call(bounties.id(), "bounty_done")
      .args_json((bounty_id, description, claim_number))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, expected_msg).await?;
    Ok(())
  }

  pub async fn bounty_action_by_validators_dao(
    &self,
    bounties: &Contract,
    bounty_id: u64,
    proposal_action: String,
    claim_idx: usize,
    for_payout_proposal: bool,
  ) -> anyhow::Result<()> {
    let proposal_id = Self::get_claim_proposal_id(
      bounties,
      bounty_id,
      claim_idx,
      for_payout_proposal
    ).await?;
    let res = self.dao_council_member
      .call(self.validators_dao.id(), "act_proposal")
      .args_json((proposal_id.0, proposal_action, Option::<String>::None))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn bounty_action_by_user(
    bounties: &Contract,
    bounty_id: u64,
    user: &Account,
    action: &BountyAction,
    claim_number: Option<u8>,
    expected_msg: Option<&str>,
  ) -> anyhow::Result<()> {
    let res = user
      .call(bounties.id(), "bounty_action")
      .args_json((bounty_id, action, claim_number))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, expected_msg).await?;
    Ok(())
  }

  pub async fn decision_on_claim(
    &self,
    bounties: &Contract,
    bounty_id: u64,
    user: &Account,
    approve: bool,
    freelancer: Option<&Account>,
    kyc_postponed: Option<DefermentOfKYC>,
    claim_number: Option<u8>,
    expected_msg: Option<&str>,
  ) -> anyhow::Result<()> {
    let freelancer = if freelancer.is_some() { freelancer.unwrap() } else { &self.freelancer };
    let res = user
      .call(bounties.id(), "decision_on_claim")
      .args_json((
        bounty_id,
        freelancer.id(),
        approve,
        kyc_postponed,
        claim_number,
      ))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, expected_msg).await?;
    Ok(())
  }

  pub async fn bounty_give_up(
    &self,
    bounties: &Contract,
    bounty_id: u64,
    freelancer: Option<&Account>,
    claim_number: Option<u8>,
  ) -> anyhow::Result<()> {
    let res = if freelancer.is_some() { freelancer.unwrap() } else { &self.freelancer }
      .call(bounties.id(), "bounty_give_up")
      .args_json((bounty_id, claim_number))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn bounty_cancel(
    &self,
    bounties: &Contract,
    bounty_id: u64,
  ) -> anyhow::Result<()> {
    let res = self.project_owner
      .call(bounties.id(), "bounty_cancel")
      .args_json((bounty_id,))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn bounty_update(
    &self,
    bounties: &Contract,
    bounty_id: u64,
    bounty_update: BountyUpdate,
  ) -> anyhow::Result<()> {
    let res = self.project_owner
      .call(bounties.id(), "bounty_update")
      .args_json((bounty_id, bounty_update))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn open_dispute(
    &self,
    bounties: &Contract,
    bounty_id: u64,
    freelancer: Option<&Account>,
    claim_number: Option<u8>,
  ) -> anyhow::Result<()> {
    let res = if freelancer.is_some() { freelancer.unwrap() } else { &self.freelancer }
      .call(bounties.id(), "open_dispute")
      .args_json((bounty_id, "Dispute description", claim_number))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn provide_arguments(
    &self,
    dispute_id: u64,
    user: &Account,
    description: String,
  ) -> anyhow::Result<()> {
    let res = user
      .call(self.dispute_contract.id(), "provide_arguments")
      .args_json((dispute_id, description))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn escalation(
    &self,
    dispute_id: u64,
    user: &Account,
  ) -> anyhow::Result<()> {
    let res = user
      .call(self.dispute_contract.id(), "escalation")
      .args_json((dispute_id,))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn dispute_dao_action(
    &self,
    proposal_id: u64,
    proposal_action: String,
  ) -> anyhow::Result<()> {
    let res = self.arbitrator
      .call(self.dispute_dao.id(), "act_proposal")
      .args_json((proposal_id, proposal_action, Option::<String>::None))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn finalize_dispute(
    &self,
    dispute_id: u64,
    user: &Account,
  ) -> anyhow::Result<()> {
    let res = user
      .call(self.dispute_contract.id(), "finalize_dispute")
      .args_json((dispute_id,))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn cancel_dispute(
    &self,
    dispute_id: u64,
    user: &Account,
  ) -> anyhow::Result<()> {
    let res = user
      .call(self.dispute_contract.id(), "cancel_dispute")
      .args_json((dispute_id,))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn reviewer_is_dao(&self) -> anyhow::Result<ReviewersParams> {
    let reviewers_params = ReviewersParams::ValidatorsDao {
      validators_dao: ValidatorsDaoParams {
        account_id: self.validators_dao.id().to_string().parse().unwrap(),
        add_proposal_bond: self.get_proposal_bond().await?,
        gas_for_add_proposal: None,
        gas_for_claim_approval: None,
        gas_for_claimer_approval: None,
      }
    };
    Ok(reviewers_params)
  }

  pub async fn more_reviewers(&self, reviewer: &Account) -> anyhow::Result<ReviewersParams> {
    let reviewers_params = ReviewersParams::MoreReviewers {
      more_reviewers: vec![reviewer.id().to_string().parse().unwrap()],
    };
    Ok(reviewers_params)
  }

  async fn get_reputation_stats(
    &self,
    claimer: Option<&AccountId>,
  ) -> anyhow::Result<(Option<ClaimerMetrics>, Option<BountyOwnerMetrics>)> {
    let freelancer = if claimer.is_some() { claimer.unwrap() } else { self.freelancer.id() };
    let bounty_owner_stats: (Option<ClaimerMetrics>, Option<BountyOwnerMetrics>) =
      self.reputation_contract
        .call("get_statistics")
        .args_json((self.project_owner.id(),))
        .view()
        .await?
        .json()?;
    let claimer_stats: (Option<ClaimerMetrics>, Option<BountyOwnerMetrics>) =
      self.reputation_contract
        .call("get_statistics")
        .args_json((freelancer,))
        .view()
        .await?
        .json()?;
    Ok((claimer_stats.0, bounty_owner_stats.1))
  }

  pub async fn claimer_metrics_value_of(values: [u64; 8]) -> anyhow::Result<ClaimerMetrics> {
    let stats = ClaimerMetrics {
      number_of_claims: values[0],
      number_of_accepted_claims: values[1],
      number_of_successful_claims: values[2],
      number_of_unsuccessful_claims: values[3],
      number_of_overdue_claims: values[4],
      number_of_canceled_claims: values[5],
      number_of_open_disputes: values[6],
      number_of_disputes_won: values[7],
    };
    Ok(stats)
  }

  pub async fn bounty_owner_metrics_value_of(values: [u64; 9]) -> anyhow::Result<BountyOwnerMetrics> {
    let stats = BountyOwnerMetrics {
      number_of_bounties: values[0],
      number_of_successful_bounties: values[1],
      number_of_canceled_bounties: values[2],
      number_of_claims: values[3],
      number_of_approved_claimers: values[4],
      number_of_approved_claims: values[5],
      number_of_rejected_claims: values[6],
      number_of_open_disputes: values[7],
      number_of_disputes_won: values[8],
    };
    Ok(stats)
  }

  pub async fn assert_reputation_stat_values_eq(
    &self,
    claimer_stats: Option<[u64; 8]>,
    bounty_owner_stats: Option<[u64; 9]>,
    claimer: Option<&AccountId>,
  ) -> anyhow::Result<()> {
    let stats = self.get_reputation_stats(claimer).await?;
    assert_eq!(stats.0.is_some(), claimer_stats.is_some());
    if claimer_stats.is_some() {
      assert_eq!(
        stats.0.unwrap(),
        Self::claimer_metrics_value_of(claimer_stats.unwrap()).await?
      );
    }
    assert_eq!(stats.1.is_some(), bounty_owner_stats.is_some());
    if bounty_owner_stats.is_some() {
      assert_eq!(
        stats.1.unwrap(),
        Self::bounty_owner_metrics_value_of(bounty_owner_stats.unwrap()).await?
      );
    }
    Ok(())
  }

  pub async fn get_dispute(
    &self,
    dispute_id: u64,
  ) -> anyhow::Result<Dispute> {
    let dispute = self.dispute_contract
      .call("get_dispute")
      .args_json((dispute_id,))
      .view()
      .await?
      .json::<Dispute>()?;
    Ok(dispute)
  }

  pub async fn get_last_dispute_id(
    &self,
  ) -> anyhow::Result<u64> {
    let last_dispute_id = self.dispute_contract
      .call("get_last_dispute_id")
      .view().await?
      .json::<u64>()?;
    Ok(last_dispute_id)
  }

  pub async fn get_proposal(dao: &Contract, proposal_id: u64) -> anyhow::Result<Proposal> {
    let proposal = dao
      .call("get_proposal")
      .args_json((proposal_id,))
      .view()
      .await?
      .json::<Proposal>()?;
    Ok(proposal)
  }

  pub async fn add_to_some_whitelist(
    &self,
    account: &AccountId,
    whitelist_type: WhitelistType,
  ) -> anyhow::Result<()> {
    let res = self.bounties_contract_admin
      .call(self.disputed_bounties.id(), "add_to_some_whitelist")
      .args_json((
        account,
        Option::<Vec<AccountId>>::None,
        whitelist_type
      ))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn remove_from_some_whitelist(
    &self,
    account: &AccountId,
    whitelist_type: WhitelistType,
  ) -> anyhow::Result<()> {
    let res = self.bounties_contract_admin
      .call(self.disputed_bounties.id(), "remove_from_some_whitelist")
      .args_json((
        account,
        Option::<Vec<AccountId>>::None,
        whitelist_type
      ))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn add_account_to_kyc_whitelist_by_service(
    &self,
    freelancer: &Account,
  ) -> anyhow::Result<()> {
    let res = self.kyc_service_account
      .call(self.kyc_whitelist_contract.id(), "add_account")
      .args_json((
        freelancer.id(),
        "fractal",
        VerificationType::KYC,
        "basic+liveness+uniq",
        Option::<U64>::None
      ))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn get_platform_fees(&self) -> anyhow::Result<FeeStats> {
    let stats = self.disputed_bounties
      .call("get_total_fees")
      .args_json((self.test_token.id(),))
      .view()
      .await?
      .json::<FeeStats>()?;
    Ok(stats)
  }

  pub async fn get_dao_fees(&self) -> anyhow::Result<FeeStats> {
    let result = self.disputed_bounties
      .call("get_total_validators_dao_fees")
      .args_json((self.validators_dao.id(),))
      .view()
      .await?
      .json::<Vec<DaoFeeStats>>()?;
    if result.len() == 1 {
      assert_eq!(result[0].token_id.to_string(), self.test_token.id().to_string());
      Ok(result[0].fee_stats.clone())
    } else {
      assert_eq!(result.len(), 0);
      Ok(FeeStats::new())
    }
  }

  pub async fn withdraw_platform_fee(&self) -> anyhow::Result<()> {
    let res = self.bounties_contract_admin
      .call(self.disputed_bounties.id(), "withdraw_platform_fee")
      .args_json((self.test_token.id(),))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn withdraw_validators_dao_fee(&self) -> anyhow::Result<()> {
    let res = self.validators_dao.as_account()
      .call(self.disputed_bounties.id(), "withdraw_validators_dao_fee")
      .args_json((self.test_token.id(),))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn update_configuration_dictionary_entries(
    bounties: &Contract,
    administrator: &Account,
    dict: ReferenceType,
    currency: String
  ) -> anyhow::Result<()> {
    let res = administrator
      .call(bounties.id(), "update_configuration_dictionary_entries")
      .args_json((dict, currency, Option::<Vec<String>>::None))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn start_competition(
    &self,
    bounty_id: u64,
  ) -> anyhow::Result<()> {
    let res = self.project_owner
      .call(self.disputed_bounties.id(), "start_competition")
      .args_json((bounty_id,))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn withdraw(
    &self,
    bounty_id: u64,
    freelancer: Option<&Account>,
    claim_number: Option<u8>,
    expected_msg: Option<&str>,
  ) -> anyhow::Result<()> {
    let res = if freelancer.is_some() { freelancer.unwrap() } else { &self.freelancer }
      .call(self.disputed_bounties.id(), "withdraw")
      .args_json((bounty_id, claim_number))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, expected_msg).await?;
    Ok(())
  }

  pub async fn set_live_status(contarct: &Contract) -> anyhow::Result<()> {
    let res = contarct
      .call("set_status")
      .args_json(json!({ "status": "Live" }))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub fn assert_almost_eq_with_max_delta(left: Balance, right: Balance, max_delta: Balance) {
    assert!(
      std::cmp::max(left, right) - std::cmp::min(left, right) <= max_delta,
      "{}",
      format!(
        "Left {} is not even close to Right {} within delta {}",
        left, right, max_delta
      )
    );
  }

  pub fn assert_eq_with_gas(left: Balance, right: Balance) {
    Self::assert_almost_eq_with_max_delta(left, right, MAX_GAS);
  }

  pub async fn get_non_refunded_bonds_amount(bounties: &Contract) -> anyhow::Result<U128> {
    let unlocked_amount = bounties
      .call("get_non_refunded_bonds_amount")
      .view()
      .await?
      .json()?;
    Ok(unlocked_amount)
  }

  pub async fn withdraw_non_refunded_bonds(&self, bounties: &Contract) -> anyhow::Result<()> {
    let res = self.bounties_contract_admin
      .call(bounties.id(), "withdraw_non_refunded_bonds")
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn set_bounties_config(
    &self,
    bounties: &Contract,
    config_create: &ConfigCreate
  ) -> anyhow::Result<()> {
    let res = self.bounties_contract_admin
      .call(bounties.id(), "change_config")
      .args_json((config_create,))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res, None).await?;
    Ok(())
  }

  pub async fn get_bounties_config(bounties: &Contract) -> anyhow::Result<bounties::Config> {
    let config = bounties
      .call("get_config")
      .view()
      .await?
      .json()?;
    Ok(config)
  }

  async fn set_using_owner_whitelist(
    &self,
    bounties: &Contract,
    use_owners_whitelist: bool,
  ) -> anyhow::Result<()> {
    let mut current_config = Self::get_bounties_config(bounties).await?;
    current_config.use_owners_whitelist = true;
    self.set_bounties_config(
      bounties,
      &ConfigCreate {
        bounty_claim_bond: current_config.bounty_claim_bond,
        bounty_forgiveness_period: current_config.bounty_forgiveness_period,
        period_for_opening_dispute: current_config.period_for_opening_dispute,
        platform_fee_percentage: current_config.platform_fee_percentage,
        validators_dao_fee_percentage: current_config.validators_dao_fee_percentage,
        penalty_platform_fee_percentage: current_config.penalty_platform_fee_percentage,
        penalty_validators_dao_fee_percentage: current_config.penalty_validators_dao_fee_percentage,
        use_owners_whitelist,
      }
    ).await?;
    Ok(())
  }

  pub async fn start_using_owner_whitelist(&self, bounties: &Contract) -> anyhow::Result<()> {
    self.set_using_owner_whitelist(bounties, true).await?;
    Ok(())
  }

  pub async fn stop_using_owner_whitelist(&self, bounties: &Contract) -> anyhow::Result<()> {
    self.set_using_owner_whitelist(bounties, false).await?;
    Ok(())
  }
}
