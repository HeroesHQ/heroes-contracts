use near_sdk::json_types::{U128, U64};
use near_sdk::serde::Deserialize;
use near_sdk::ONE_YOCTO;
use near_units::parse_near;
use serde_json::{json, Value};
use workspaces::{Account, AccountId, Contract, Worker};
use workspaces::network::Sandbox;
use workspaces::result::ExecutionFinalResult;
use bounties::{Bounty, BountyAction, BountyClaim, BountyStatus, ClaimStatus, FungibleTokenMetadata,
               ReviewersParams, ValidatorsDaoParams};
use disputes::{Dispute, Proposal};
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

pub const BOUNTY_AMOUNT: U128 = U128(10u128.pow(18));
pub const PLATFORM_FEE: U128 = U128(10u128.pow(17));
pub const DAO_FEE: U128 = U128(10u128.pow(17));
pub const MAX_DEADLINE: U64 = U64(604_800_000_000_000);

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
    Self::assert_contract_call_result(res).await?;

    res = test_token
      .call("mint")
      .args_json((project_owner.id(), "100000000000000000000".to_string()))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res).await?;
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
    Self::assert_contract_call_result(res).await?;
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
    Self::assert_contract_call_result(res).await?;

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
    Self::assert_contract_call_result(res).await?;

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
    Self::assert_contract_call_result(res).await?;

    let reputation_contract = worker.dev_deploy(&std::fs::read(REPUTATION_WASM)?).await?;
    res = reputation_contract
      .call("new")
      .args_json(json!({
      "bounties_contract": disputed_bounties.id(),
    }))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res).await?;

    let mut bounties_config = bounties::Config::default();
    bounties_config.period_for_opening_dispute = U64(1_000_000_000 * 60 * 10); // 5 min

    res = disputed_bounties
      .call("new")
      .args_json(json!({
      "admins_whitelist": vec![bounties_contract_admin.id()],
      "config": bounties_config,
      "reputation_contract": reputation_contract.id(),
      "dispute_contract": dispute_contract.id(),
    }))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res).await?;
    Self::register_user(&test_token, disputed_bounties.id()).await?;
    Self::add_token(&test_token, &disputed_bounties, &bounties_contract_admin).await?;

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
      }
    )
  }

  async fn register_user(test_token: &Contract, user: &AccountId) -> anyhow::Result<()> {
    let res = test_token
      .call("storage_deposit")
      .args_json((user, Option::<bool>::None))
      .deposit(parse_near!("90 N"))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res).await?;
    Ok(())
  }

  async fn add_token(
    test_token: &Contract,
    bounties: &Contract,
    administrator: &Account,
  ) -> anyhow::Result<()> {
    let metadata: FungibleTokenMetadata = test_token.call("ft_metadata").view().await?.json()?;
    let res = administrator
      .call(bounties.id(), "add_token_id")
      .args_json(json!({
      "token_id": test_token.id(),
      "min_amount_for_kyc": U128(150 * metadata.decimals as u128),
    }))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res).await?;
    Ok(())
  }

  async fn assert_contract_call_result(result: ExecutionFinalResult) -> anyhow::Result<()> {
    if result.is_failure() {
      println!("{:#?}", result.clone().into_result().err());
    }
    assert!(result.is_success());
    Ok(())
  }

  pub async fn assert_statuses(
    &self,
    bounties: &Contract,
    bounty_id: u64,
    claim_status: ClaimStatus,
    bounty_status: BountyStatus,
  ) -> anyhow::Result<(BountyClaim, Bounty)> {
    let bounty_claims = Self::get_bounty_claims_by_id(bounties, bounty_id).await?;
    assert_eq!(bounty_claims.len(), 1);
    assert_eq!(bounty_claims[0].0.to_string(), self.freelancer.id().to_string());
    let bounty_claim = bounty_claims[0].clone().1;
    assert_eq!(bounty_claim.bounty_id, bounty_id);
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
  ) -> anyhow::Result<U64> {
    let bounty_claims = Self::get_bounty_claims_by_id(bounties, bounty_id).await?;
    assert!(bounty_claims.len() > claim_index);
    let bounty_claim = bounty_claims[claim_index].clone().1;
    Ok(bounty_claim.bounty_payout_proposal_id.unwrap())
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

  pub async fn add_bounty(
    &self,
    bounties: &Contract,
    deadline: Value,
    claimer_approval: String,
    reviewers: Option<ReviewersParams>,
  ) -> anyhow::Result<()> {
    let metadata = json!({
      "title": "Test bounty title",
      "description": "Test bounty description",
      "category": "Marketing",
      "attachments": vec!["http://ipfs-url/1", "http://ipfs-url/2"],
      "experience": "Intermediate",
      "tags": vec!["Community", "NFT"],
      "acceptance_criteria": "test acceptance criteria",
      "contact_details": json!({
        "contact": "example@domain.net",
        "contact_type": "Email"
      }),
    });

    let mut amount = BOUNTY_AMOUNT.0 + PLATFORM_FEE.0;
    if reviewers.is_some() {
      amount += match reviewers.clone().unwrap() {
        ReviewersParams::ValidatorsDao { validators_dao: _validators_dao } => DAO_FEE.0,
        _ => 0,
      };
    }

    let res = self.project_owner
      .call(self.test_token.id(), "ft_transfer_call")
      .args_json(json!({
        "receiver_id": bounties.id(),
        "amount": U128(amount),
        "msg": json!({
          "metadata": metadata,
          "deadline": deadline,
          "claimer_approval": claimer_approval,
          "reviewers": match reviewers {
            Some(r) => json!(r),
            _ => json!(null),
          },
        })
          .to_string(),
      }))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res).await?;
    Ok(())
  }

  pub async fn bounty_claim(
    &self,
    bounties: &Contract,
    bounty_id: u64,
    deadline: U64,
    description: String,
  ) -> anyhow::Result<()> {
    let config: bounties::Config = bounties.call("get_config").view().await?.json()?;
    let res = self.freelancer
      .call(bounties.id(), "bounty_claim")
      .args_json((bounty_id, deadline, description))
      .max_gas()
      .deposit(config.bounty_claim_bond.0)
      .transact()
      .await?;
    Self::assert_contract_call_result(res).await?;
    Ok(())
  }

  pub async fn bounty_done(
    &self,
    bounties: &Contract,
    bounty_id: u64,
    description: String,
  ) -> anyhow::Result<()> {
    let res = self.freelancer
      .call(bounties.id(), "bounty_done")
      .args_json((bounty_id, description))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res).await?;
    Ok(())
  }

  pub async fn bounty_action_by_validators_dao(
    &self,
    bounties: &Contract,
    bounty_id: u64,
    proposal_action: String,
  ) -> anyhow::Result<()> {
    let proposal_id = Self::get_claim_proposal_id(bounties, bounty_id, 0).await?;
    let res = self.dao_council_member
      .call(self.validators_dao.id(), "act_proposal")
      .args_json((proposal_id.0, proposal_action, Option::<bool>::None))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res).await?;
    Ok(())
  }

  pub async fn bounty_action_by_user(
    bounties: &Contract,
    bounty_id: u64,
    user: &Account,
    action: &BountyAction,
  ) -> anyhow::Result<()> {
    let res = user
      .call(bounties.id(), "bounty_action")
      .args_json((bounty_id, action))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res).await?;
    Ok(())
  }

  pub async fn bounty_give_up(
    &self,
    bounties: &Contract,
    bounty_id: u64,
  ) -> anyhow::Result<()> {
    let res = self.freelancer
      .call(bounties.id(), "bounty_give_up")
      .args_json((bounty_id,))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res).await?;
    Ok(())
  }

  pub async fn open_dispute(
    &self,
    bounties: &Contract,
    bounty_id: u64,
  ) -> anyhow::Result<()> {
    let res = self.freelancer
      .call(bounties.id(), "open_dispute")
      .args_json((bounty_id, "Dispute description"))
      .max_gas()
      .deposit(ONE_YOCTO)
      .transact()
      .await?;
    Self::assert_contract_call_result(res).await?;
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
    Self::assert_contract_call_result(res).await?;
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
    Self::assert_contract_call_result(res).await?;
    Ok(())
  }

  pub async fn dispute_dao_action(
    &self,
    proposal_id: u64,
    proposal_action: String,
  ) -> anyhow::Result<()> {
    let res = self.arbitrator
      .call(self.dispute_dao.id(), "act_proposal")
      .args_json((proposal_id, proposal_action, Option::<bool>::None))
      .max_gas()
      .transact()
      .await?;
    Self::assert_contract_call_result(res).await?;
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
    Self::assert_contract_call_result(res).await?;
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
    Self::assert_contract_call_result(res).await?;
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

  async fn get_reputation_stats(
    &self,
  ) -> anyhow::Result<(Option<ClaimerMetrics>, Option<BountyOwnerMetrics>)> {
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
        .args_json((self.freelancer.id(),))
        .view()
        .await?
        .json()?;
    Ok((claimer_stats.0, bounty_owner_stats.1))
  }

  pub async fn claimer_metrics_value_of(values: [u64; 7]) -> anyhow::Result<ClaimerMetrics> {
    let stats = ClaimerMetrics {
      number_of_claims: values[0],
      number_of_successful_claims: values[1],
      number_of_unsuccessful_claims: values[2],
      number_of_overdue_claims: values[3],
      number_of_canceled_claims: values[4],
      number_of_open_disputes: values[5],
      number_of_disputes_won: values[6],
    };
    Ok(stats)
  }

  pub async fn bounty_owner_metrics_value_of(values: [u64; 8]) -> anyhow::Result<BountyOwnerMetrics> {
    let stats = BountyOwnerMetrics {
      number_of_bounties: values[0],
      number_of_successful_bounties: values[1],
      number_of_canceled_bounties: values[2],
      number_of_claims: values[3],
      number_of_approved_claims: values[4],
      number_of_rejected_claims: values[5],
      number_of_open_disputes: values[6],
      number_of_disputes_won: values[7],
    };
    Ok(stats)
  }

  pub async fn assert_reputation_stat_values_eq(
    &self,
    claimer_stats: Option<[u64; 7]>,
    bounty_owner_stats: Option<[u64; 8]>,
  ) -> anyhow::Result<()> {
    let stats = self.get_reputation_stats().await?;
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

  pub async fn get_proposal(dao: &Contract, proposal_id: u64) -> anyhow::Result<Proposal> {
    let proposal = dao
      .call("get_proposal")
      .args_json((proposal_id,))
      .view()
      .await?
      .json::<Proposal>()?;
    Ok(proposal)
  }
}
