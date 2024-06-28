# HEROES Contracts
Smart Contracts for HEROES

## Bounty Smart Contract

### Smart Contract Initialization

After deployment, the contract needs to be initialized.

Initialization method:

```rust
#[init]
pub fn new(
  admins_whitelist: Vec<AccountId>,
  config: Option<Config>,
  reputation_contract: Option<AccountId>,
  dispute_contract: Option<AccountId>,
  kyc_whitelist_contract: Option<AccountId>,
  recipient_of_platform_fee: Option<AccountId>
) -> Self
```

Parameters:

- admin\_whitelist: List of admin accounts for the smart contract. The list must contain at least one account. This list can be changed later by one of the admins.
- config: Contract configuration. If not specified, the default configuration is used. The configuration can be changed later by an admin.
- reputation\_contract: Account of the reputation smart contract used for accumulating statistics about bounty owners and freelancers. If not specified, statistics will not be collected, and the reputation contract cannot be added later. If specified during initialization, it can be changed later by an admin to another one.
- dispute\_contract: Account of the dispute smart contract used for creating and resolving disputes between bounty owners and freelancers. If not specified, this function will not be available, and the dispute contract cannot be added later. If specified during initialization, it can be changed later by an admin to another one.
- kyc\_whitelist\_contract: Account of the smart contract used for KYC/KYB status verification. If not specified, the contract will not perform verification. Regardless of whether this parameter is specified during contract initialization, it can be changed later by an admin.
- recipient\_of\_platform\_fee: Account authorized to collect the platform fee. This parameter can be changed later by an admin.

### Bounty Smart Contract Configuration

The configuration of the smart contract is stored in the Config structure, which can be specified during the contract initialization. Configuration parameters that can be changed using the change\_config method:

- bounty\_claim\_bond: The bond amount in yoctoNEAR used when creating a claim for a bounty task.
- bounty\_forgiveness\_period: The period from the start of the task during which a freelancer can cancel the claim (bounty\_give\_up method) without losing the bond (in nanoseconds).
- period\_for\_opening\_dispute: The period from the rejection of the task result during which a freelancer can open a dispute (open\_dispute method) for reviewing the results and decision (in nanoseconds).
- platform\_fee\_percentage: Platform fee percentage (in thousandths of a percent).
- validators\_dao\_fee\_percentage: Service DAO usage fee percentage (in thousandths of a percent).
- penalty\_platform\_fee\_percentage: Platform fee percentage deducted if the bounty is canceled by the owner (in thousandths of a percent, default value is 0).
- penalty\_validators\_dao\_fee\_percentage: Service DAO usage fee percentage deducted if the bounty is canceled by the owner (in thousandths of a percent, default value is 0).
- use\_owners\_whitelist: If true, only accounts listed in the 'OwnersWhitelist' of the smart contract can create bounties; if false, all accounts can create bounties.
- max\_due\_date: If specified, it limits the maximum duration for completing the bounty that the owner can set when creating the bounty.

### Creating a Bounty

To create a new bounty, its owner must transfer tokens to the bounty contract using the ft_transfer_call method.

The msg parameter must contain the BountyCreate structure in JSON format.

The transferred amount includes the reward for completing the bounty and any applicable fees.

Fields of the BountyCreate structure:

- metadata: Structure BountyMetadata containing information about the bounty. Fields of the BountyMetadata structure:
  - title: Title of the bounty.
  - description: Description of the bounty.
  - category: Bounty category according to the internal reference of the smart contract.
  - attachments: Array of links to additional materials needed to complete the task (optional parameter).
  - experience: Required experience level for this task (optional parameter). Possible values: 'Beginner', 'Intermediate', 'Advanced', 'AnyExperience'.
  - tags: Array of tags relevant to the topic or technologies used in the bounty (optional parameter).
  - acceptance\_criteria: Description of the requirements for the task result (optional parameter).
  - contact\_details: Contact details of the bounty owner (optional parameter).


- deadline: Desired task completion deadline. Possible values:
  - DueDate: The final completion date (in nanoseconds since January 1, 1970).
  - MaxDeadline: The maximum time period for task completion (in nanoseconds from the start of the task by the freelancer).
  - WithoutDeadline: No deadline.


- claimant\_approval: Method of approving the bounty performer. Possible values:
  - MultipleClaims: Any number of candidates can claim the task; the bounty owner approves or rejects candidate claims; approved claims are performed by freelancers.
  - WithoutApproval: No approval needed. As soon as a claim is created, it is automatically approved.
  - ApprovalByWhitelist: The claim is automatically approved if the performer's account is in the whitelist for this bounty.
  - WhitelistWithApprovals: Only accounts in the whitelist for this bounty can claim; claims are approved by the bounty owner (similar to 'MultipleClaims').


- reviewers: This parameter allows the owner to delegate decision-making rights for the created bounty to someone else (optional parameter). If not specified, the owner makes all decisions independently. Possible values:
  - ValidatorsDao: The bounty owner can use a DAO (Service DAO) for performer approval or task result evaluation. This parameter must contain the ValidatorsDaoParams structure with DAO parameters.
  - MoreReviewers: Should contain an array of accounts. A reviewer account can perform the same actions as the Service DAO.


- kyc\_config: Need for KYC/KYB verification (optional parameter). Possible values:
  - KycNotRequired: No verification required.
  - KycRequired: Verification required. This value contains an additional parameter 'kyc\_verification\_method' which can have the values: 'WhenCreatingClaim' – the freelancer must pass verification before submitting a claim; 'DuringClaimApproval' – verification result must be available at the time of claim approval.


- postpaid: Bounty with post-payment reward outside the contract (optional parameter). If not specified, this is a regular bounty with a deposit upon creation. Can have the value 'PaymentOutsideContract' which must also contain the sub-parameter 'currency' – the currency in which the reward will be paid according to the internal reference of the smart contract.


- bounty\_flow: Type of bounty lifecycle (optional parameter). Possible values:
  - SimpleBounty: Simple mode. In simple mode, the freelancer submits the task result immediately when creating a claim for the reward. The start and end of the task are not separated in time. This mode also lacks the function of approving the claim by the bounty owner before execution. After creating the claim, the owner either approves or rejects the task result.
  - AdvancedFlow: Advanced mode (default value). The freelancer creates a task claim. The claim is approved by the bounty owner or automatically depending on the value of the 'claimant\_approval' parameter. After this, the task execution period begins. The freelancer reports task completion, changing the claim status. After this, the owner either approves or rejects the task result.


- multitasking: Ability for multiple performers to work on the task simultaneously. If not specified, the bounty can have only one performer at a time. Possible values:
  - OneForAll: The bounty is designed for a certain number of performers who can start and finish work, receiving rewards independently of each other. All performers receive the same reward for completing the task. This mode has sub-parameters:
    - number\_of\_slots: Number of performers (slots) who can work on the task simultaneously.
    - amount\_per\_slot: Reward amount per slot.
    - min\_slots\_to\_start: Minimum number of slots needed to start the task.
  - ContestOrHackathon: Bounty contest or hackathon. Many participants can join the contest or hackathon, but only one or several winners receive the reward. Sub-parameters defining contest rules:
    - prize\_places: Array with information about prize places defining the description of each prize place and reward amount. If not specified, the contest can have only one winner.
    - allowed\_create\_claim\_to: Defines the date or period after the start of the contest during which new participants can submit claims. If not specified, new participants can submit claims until the contest ends.
    - successful\_claims\_for\_result: If specified, it defines the minimum number of participants who must finish the task for the winner to be determined. If not specified, at least one finished participant is enough for each prize place.
    - start\_conditions: Contest start condition. If not specified, the contest starts with the first claim. If specified, it can contain: minimum number of participants required to start the contest or the value 'ManuallyStart' – the contest starts after the start\_competition method is called.
  - DifferentTasks: Bounty project. To receive the reward, several different tasks with different payment amounts must be completed. The subtasks sub-parameter must be specified with information about the sub-tasks, containing the description of the sub-task and the reward amount as a percentage of the total bounty amount (in thousandths of a percent). All participants must submit separate claims specifying the sub-task number. Work on all tasks starts simultaneously. After each participant reports task completion, the overall work result can be accepted by the owner only for all participants simultaneously. The owner can reject individual task results, and the unfinished task must be completed again (by the same, any other, or a new participant) for the bounty to be successfully completed.


- allow\_creating\_many\_claims: If true, an account can create more than one active claim for one bounty simultaneously. If false, only one active claim can be created simultaneously (default value). This parameter can be specified only if the multitasking parameter is set.


- allow\_deadline\_stretch: If true, allows creating a claim without specifying an individual deadline. If not specified, the general bounty deadline is used as the task completion deadline. If false (default value), the deadline must be specified when creating a claim. This parameter is used only for the AdvancedFlow mode.

### Smart Contract Methods for Working with Bounties

```rust
pub fn bounty_claim(
  &mut self,
  id: BountyIndex,
  deadline: Option<U64>,
  description: String,
  slot: Option<usize>
) -> PromiseOrValue<()>
```

<p>Creates a freelancer's claim for a bounty. This method requires a deposit equal to the bond amount specified in the smart contract configuration. The bond is returned to the freelancer when the claim is closed. The bond is not returned if the task deadline is exceeded or if the freelancer cancels the claim after it has been in progress for a certain period specified in the contract configuration. Bounty owners or reviewers cannot create claims.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li><li>deadline: Task completion deadline acceptable to the performer (cannot exceed the general bounty deadline).</li><li>description: Comment provided by the performer with the claim.</li><li>slot: Sub-task number (starting from 0) if the bounty includes multiple tasks with different reward amounts (bounty type 'DifferentTasks').</li></ul>

```rust
pub fn accept_claimant(
  &mut self,
  id: BountyIndex,
  receiver_id: AccountId,
  claim_number: Option<u8>,
  kyc_postponed: Option<DefermentOfKYC>
) -> PromiseOrValue<()>
```

<p>Approve the claim. Available to the bounty owner or one of the reviewers. Used in 'AdvancedFlow' mode. After this method is executed, the claim transitions to the execution stage by the freelancer.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li><li>receiver_id: Account of the claim owner (freelancer).</li><li>claim_number: Serial number (starting from 0) of the claim for this account (specified only if multiple claims by one account for one bounty are allowed).</li><li>kyc_postponed: If specified, it can defer KYC/KYB verification until the freelancer completes the task (bounty_done method) or cancel verification for this claim. This parameter can be used if the bounty was created with the 'DuringClaimApproval' KYC verification method.</li></ul>

```rust
pub fn decline_claimant(
  &mut self,
  id: BountyIndex,
  receiver_id: AccountId,
  claim_number: Option<u8>
) -> PromiseOrValue<()>
```

<p>Reject the claim. Available to the bounty owner or one of the reviewers. Used in 'AdvancedFlow' mode. After this method is executed, the claim is no longer considered for task completion.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li><li>receiver_id: Account of the claim owner (freelancer).</li><li>claim_number: Serial number (starting from 0) of the claim for this account (specified only if multiple claims by one account for one bounty are allowed).</li></ul>

```rust
pub fn bounty_done(
  &mut self,
  id: BountyIndex,
  description: String,
  claim_number: Option<u8>
) -> PromiseOrValue<()>
```

<p>Mark the task as completed and ready for review by the bounty owner. This method is executed by the freelancer. Used in 'AdvancedFlow' mode.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li><li>description: Comment provided by the performer about task completion.</li><li>claim_number: Serial number (starting from 0) of the claim for this account (specified only if multiple claims by one account for one bounty are allowed).</li></ul>

```rust
pub fn bounty_give_up(
  &mut self,
  id: BountyIndex,
  claim_number: Option<u8>
) -> PromiseOrValue<()>
```

<p>Cancel the claim. Available to the freelancer. The claim can be canceled both before and after approval by the bounty owner but before the bounty\_done method is executed.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li><li>claim_number: Serial number (starting from 0) of the claim for this account (specified only if multiple claims by one account for one bounty are allowed).</li></ul>

```rust
pub fn bounty_cancel(
  &mut self,
  id: BountyIndex
) -> PromiseOrValue<()>
```

<p>Cancel the bounty. Available to the bounty owner. Unused deposit is returned to the owner (if the bounty is partially completed, the owner receives the difference). The bounty can be canceled only if there are no active claims at the time of cancellation.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li></ul>

```rust
pub fn bounty_approve(
  &mut self,
  id: BountyIndex,
  receiver_id: AccountId,
  claim_number: Option<u8>,
  prize_place: Option<usize>
) -> PromiseOrValue<()>
```

<p>Approve the task result (freelancer's work) and pay the reward. Available to the bounty owner or one of the reviewers. For 'postpaid' bounties, the reward is not paid at the moment of method execution. This method is not used for 'DifferentTasks' bounties.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li><li>receiver_id: Account of the performer.</li><li>claim_number: Serial number (starting from 0) of the claim for this account (specified only if multiple claims by one account for one bounty are allowed).</li><li>prize_place: Prize place number (starting from 0). Used for 'ContestOrHackathon' type bounties if more than one prize place is specified when creating the bounty.</li></ul>

```rust
pub fn bounty_reject(
  &mut self,
  id: BountyIndex,
  receiver_id: AccountId,
  claim_number: Option<u8>
) -> PromiseOrValue<()>
```

<p>Reject the task result. Available to the bounty owner or one of the reviewers. If the bounty smart contract includes a dispute contract, the freelancer can open a dispute within the period specified in the contract configuration. The dispute is not used for 'ContestOrHackathon' or 'postpaid' bounties. If the dispute is not used, the claim is finally rejected and no longer considered.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li><li>receiver_id: Account of the performer.</li><li>claim_number: Serial number (starting from 0) of the claim for this account (specified only if multiple claims by one account for one bounty are allowed).</li></ul>

```rust
pub fn bounty_approve_of_several(
  &mut self,
  id: BountyIndex
) -> PromiseOrValue<()>
```

<p>Approve the result of all tasks for 'DifferentTasks' type bounties. Available to the bounty owner or one of the reviewers. Freelancer's work can be accepted only simultaneously for all sub-tasks. The reward is not paid at the moment of method execution. Each performer can receive the reward separately using the withdraw method.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li></ul>

```rust
pub fn bounty_finalize(
  &mut self,
  id: BountyIndex,
  claimant: Option<(AccountId, Option<u8>)>
) -> PromiseOrValue<()>
```

<p>Finalize the claim state if the decision period has passed. Used if, for example, the freelancer did not report task completion (bounty\_done method) and the task period has expired, or if the dispute opening period has passed. Available to any account interested in continuing work with the bounty.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li><li>claimant: Pair of values: performer's account and serial number (starting from 0) of the claim for this account (specified only if multiple claims by one account for one bounty are allowed). The claimant parameter is specified only for bounty types where multiple claims by freelancers can be simultaneously in the execution stage.</li></ul>

```rust
pub fn bounty_update(
  &mut self,
  id: BountyIndex,
  bounty_update: BountyUpdate
)
```

<p>Change bounty parameters after creation. Available to the bounty owner.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li><li>bounty_update: Structure BountyUpdate containing the parameters to change.</li></ul>

```rust
pub fn extend_claim_deadline(
  &mut self,
  id: BountyIndex,
  receiver_id: AccountId,
  claim_number: Option<u8>,
  deadline: U64
)
```

<p>Extend the task deadline. This method allows the bounty owner to extend the individual deadline specified by the freelancer when creating the claim. The deadline cannot be shortened. An individual deadline cannot be set if the freelancer did not specify one. The general bounty deadline can be changed using the bounty\_update method.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li><li>receiver_id: Account of the performer.</li><li>claim_number: Serial number (starting from 0) of the claim for this account (specified only if multiple claims by one account for one bounty are allowed).</li><li>deadline: New individual task completion deadline.</li></ul>

```rust
pub fn bounty_create(
  &mut self,
  bounty_create: BountyCreate,
  token_id: Option<AccountId>,
  amount: U128
)
```

<p>Create a bounty with post-payment reward outside the contract (for 'postpaid' type bounties). This method allows creating a bounty without a deposit for reward payment.</p><p></p><p>Parameters:</p><ul><li>bounty_create: Structure BountyCreate containing the bounty parameters.</li><li>token_id: Not used when creating postpaid bounties.</li><li>amount: Bounty amount. The bounty currency is determined by the 'currency' parameter in the 'postpaid' field of the BountyCreate structure.</li></ul>

```rust
pub fn mark_as_paid(
  &mut self,
  id: BountyIndex,
  receiver_id: Option<AccountId>,
  claim_number: Option<u8>,
  prize_place: Option<usize>
)
```

<p>Mark the payment as made outside the contract. This allows the bounty owner to mark that the reward was paid to the freelancer. Used only for 'postpaid' bounties. This method must be executed before the bounty\_approve method.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li><li>receiver_id: Account of the performer.</li><li>claim_number: Serial number (starting from 0) of the claim for this account (specified only if multiple claims by one account for one bounty are allowed).</li><li>prize_place: Prize place number (starting from 0). Used for 'ContestOrHackathon' type bounties if more than one prize place is specified when creating the bounty.</li></ul>

```rust
pub fn confirm_payment(
  &mut self,
  id: BountyIndex,
  claim_number: Option<u8>
)
```

<p>Mark the reward as received. This allows the freelancer to mark that they received the reward from the bounty owner. Used only for 'postpaid' bounties. This method must be executed before the bounty\_approve method.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li><li>claim_number: Serial number (starting from 0) of the claim for this account (specified only if multiple claims by one account for one bounty are allowed).</li></ul>

```rust
pub fn start_competition(
  &mut self,
  id: BountyIndex
)
```

<p>Start the contest or hackathon. Used for 'ContestOrHackathon' type bounties. Applied if the 'start\_conditions' parameter is specified as 'ManuallyStart'. Available only to the bounty owner.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li></ul>

```rust
pub fn withdraw(
  &mut self,
  id: BountyIndex,
  claim_number: Option<u8>
) -> PromiseOrValue<()>
```

<p>Withdraw the reward for a claim. This allows the freelancer to receive the reward for the completed work. This method is used only for 'DifferentTasks' bounties after the bounty\_approve\_of\_several method is executed by the bounty owner.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li><li>claim_number: Serial number (starting from 0) of the claim for this account (specified only if multiple claims by one account for one bounty are allowed).</li></ul>

```rust
pub fn open_dispute(
  &mut self,
  id: BountyIndex,
  description: String,
  claim_number: Option<u8>
) -> PromiseOrValue<()>
```

<p>Open a dispute. This allows the freelancer to open a dispute if they disagree with the bounty owner's decision to reject the claim result. The dispute can be opened if less time has passed since the claim rejection than the dispute opening period specified in the contract configuration. This method can be used only if the dispute contract is specified in the smart contract configuration. This method is not used for 'ContestOrHackathon' or 'postpaid' bounties.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li><li>description: Comment from the freelancer about the reasons for opening the dispute.</li><li>claim_number: Serial number (starting from 0) of the claim for this account (specified only if multiple claims by one account for one bounty are allowed).</li></ul>

### Smart Contract Methods for Admins or Accounts with Special Permissions

```rust
pub fn add_to_some_whitelist(
  &mut self,
  account_id: Option<AccountId>,
  account_ids: Option<Vec<AccountId>>,
  whitelist_type: WhitelistType
)
```

<p>Add one or more accounts to the special permissions list. This method allows the smart contract admin to add new accounts to the admin list or another target list. The admin of the smart contract is not the full access key owner but an account listed in the admin list stored in the contract.</p><p></p><p>Parameters:</p><ul><li>account_id: Account to be added to the list.</li><li>- account_ids: List of accounts to be added to the list. Either account_id or account_ids can be used simultaneously.</li><li>whitelist_type: List type. Possible values: 'AdministratorsWhitelist' – admin list; 'OwnersWhitelist' – list of accounts allowed to create bounties (used if this ability is restricted by the configuration parameter); 'PostpaidSubscribersWhitelist' – list of accounts allowed to create postpaid bounties.</li></ul>

```rust
pub fn remove_from_some_whitelist(
  &mut self,
  account_id: Option<AccountId>,
  account_ids: Option<Vec<AccountId>>,
  whitelist_type: WhitelistType
)
```

<p>Remove one or more accounts from the special permissions list. This method allows the smart contract admin to remove accounts from the admin list or another target list.</p><p></p><p>Parameters:</p><ul><li>account_id: Account to be removed from the list.</li><li>account_ids: List of accounts to be removed from the list.</li><li>whitelist_type: List type.</li></ul>

```rust
pub fn add_token_id(
  &mut self,
  token_id: AccountId,
  min_amount_for_kyc: Option<U128>
)
```

<p>Add a new ft-token to the list allowed for creating bounties. Available only to the smart contract admin.</p><p></p><p>Parameters:</p><ul><li>token_id: Account of the ft-token contract.</li><li>min_amount_for_kyc: Deprecated parameter, no longer used.</li></ul>

```rust
pub fn update_token(
  &mut self,
  token_id: AccountId,
  token_details: TokenDetails
)
```

<p>Enable or disable the use of an ft-token. Available only to the smart contract admin.</p><p></p><p>Parameters:</p><ul><li>token_id: Account of the ft-token contract.</li><li>token_details: Structure TokenDetails containing the information to change. To enable or disable the token, set the enabled parameter of the structure to true or false.</li></ul>

```rust
pub fn update_kyc_whitelist_contract(
  &mut self,
  kyc_whitelist_contract: Option<AccountId>
)
```

<p>Set the account of the smart contract used for KYC/KYB status verification. If the contract account is not specified, the KYC/KYB status check is not performed by the bounty smart contract. Available only to the smart contract admin.</p><p></p><p>Parameters:</p><ul><li>kyc_whitelist_contract: Account of the smart contract used for KYC/KYB status verification. If None is specified, verification will not be performed for bounties created from this point onward.</li></ul>

```rust
pub fn update_reputation_contract(
  &mut self,
  reputation_contract: AccountId
)
```

<p>Change the account of the smart contract used for accumulating statistics about bounty owners and freelancers. Available only to the smart contract admin. The account can be changed only if the reputation contract account was specified during the initial contract initialization.</p><p></p><p>Parameters:</p><ul><li>reputation_contract: Account of the reputation contract.</li></ul>

```rust
pub fn update_dispute_contract(
  &mut self,
  dispute_contract: AccountId
)
```

<p>Change the account of the smart contract used for creating and resolving disputes between bounty owners and freelancers. Available only to the smart contract admin. The account can be changed only if the dispute contract account was specified during the initial contract initialization.</p><p></p><p>Parameters:</p><ul><li>dispute_contract: Account of the dispute contract.</li></ul>

```rust
pub fn change_config(
  &mut self,
  config_create: ConfigCreate
)
```

<p>Change the configuration of the smart contract. Available only to the smart contract admin.</p><p></p><p>Parameters:</p><ul><li>config_create: Structure ConfigCreate containing the smart contract configuration.</li></ul>

```rust
pub fn update_configuration_dictionary_entries(
  &mut self,
  dict: ReferenceType,
  entry: Option<String>,
  entries: Option<Vec<String>>
)
```

<p>Add one or more values to the internal reference of the smart contract. Available only to the smart contract admin.</p><p></p><p>Parameters:</p><ul><li>dict: Reference type. Possible values: 'Categories' – bounty categories; 'Tags' – bounty tags; 'Currencies' – currencies for postpaid bounties.</li><li>entry: Value to be added to the reference.</li><li>entries: Multiple values to be added to the reference. Either entry or entries can be used simultaneously.</li></ul>

```rust
pub fn remove_configuration_dictionary_entries(
  &mut self,
  dict: ReferenceType,
  entry: Option<String>,
  entries: Option<Vec<String>>
)
```

<p>Remove one or more values from the internal reference of the smart contract. Available only to the smart contract admin.</p><p></p><p>Parameters:</p><ul><li>dict: Reference type.</li><li>entry: Value to be removed from the reference.</li><li>entries: Multiple values to be removed from the reference. Either entry or entries can be used simultaneously.</li></ul>

```rust
pub fn change_recipient_of_platform_fee(
  &mut self,
  recipient_of_platform_fee: AccountId
)
```

<p>Change the account authorized to collect the platform fee. Available only to the smart contract admin.</p><p></p><p>Parameters:</p><ul><li>recipient_of_platform_fee: Account of the platform fee recipient. To collect the platform fee, use the withdraw_platform_fee method.</li></ul>

```rust
pub fn withdraw_platform_fee(
  &mut self,
  token_id: AccountId
) -> PromiseOrValue<()>
```

<p>Collect the platform fee. The platform fee can be accrued for each bounty created by the contract. The fee is accrued in the bounty currency. Available only to the account specified during the contract initialization or using the change\_recipient\_of\_platform\_fee method.</p><p></p><p>Parameters:</p><ul><li>token_id: Account of the ft-token for which the platform fee is collected.</li></ul>

```rust
pub fn withdraw_validators_dao_fee(
  &mut self,
  token_id: AccountId
) -> PromiseOrValue<()>
```

<p>Collect the Service DAO fee. If the bounty is created using the Service DAO, a fee is accrued for the DAO in the bounty currency. Available only to the Service DAO account.</p><p></p><p>Parameters:</p><ul><li>token_id: Account of the ft-token for which the Service DAO fee is collected.</li></ul>

```rust
pub fn update_validators_dao_params(
  &mut self,
  id: BountyIndex,
  dao_params: ValidatorsDaoParams
)
```

<p>Change the Service DAO parameters. This allows changing the Service DAO parameters specified when creating the bounty (except the DAO account). Available only to the bounty owner.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li><li>dao_params: Structure ValidatorsDaoParams containing the Service DAO parameters.</li></ul>

```rust
pub fn dispute_result(
  &mut self,
  id: BountyIndex,
  receiver_id: AccountId,
  claim_number: Option<u8>,
  success: bool
) -> PromiseOrValue<()>
```

<p>Report the dispute decision results. This applies the dispute decision in favor of one of the parties. Available only to the dispute smart contract.</p><p></p><p>Parameters:</p><ul><li>id: Bounty number.</li><li>receiver_id: Account of the freelancer.</li><li>claim_number: Serial number (starting from 0) of the claim for this account (specified only if multiple claims by one account for one bounty are allowed).</li><li>success: Dispute decision result (true – in favor of the freelancer; false – in favor of the bounty owner).</li></ul>

```rust
pub fn withdraw_non_refunded_bonds(
  &mut self
) -> Promise
```

Collect the non-refunded bond amounts not returned to the freelancer due to task deadline expiration or late claim cancellation. Available only to the account authorized to receive the platform fee.

```rust
pub fn set_status(
  &mut self,
  status: ContractStatus
)
```

<p>Set the global contract status. Available only to the smart contract admin.</p><p></p><p>Parameters:</p><ul><li>status: Contract status to be set. Possible values: 'Live' – main mode where all contract operations are available; 'ReadOnly' – read-only mode where only information can be read and any state changes are prohibited.</li></ul>

### Bounty Statuses

|**Status**|**Description**|
| :- | :- |
|New|Initial status after creating a bounty and before any claims are in the execution stage. In this state, the bounty can be canceled.|
|Claimed|An active claim exists. Used for bounties without the multitasking parameter.|
|ManyClaimed|Active claims exist. Used for bounties with the multitasking parameter.|
|Completed|The bounty is completed, and the reward is fully paid.|
|Canceled|The bounty is canceled, and the reward is not paid.|
|AwaitingClaims|The bounty has slots with paid rewards but still unused slots, and there are no active claims. Used for bounties in the 'OneForAll' mode. In this state, the bounty can be canceled.|
|PartiallyCompleted|The bounty is completed, and the reward is partially paid. Used for bounties in the 'OneForAll' mode. The reward is paid for some slots. The bounty was canceled in the 'AwaitingClaims' status.|

### Freelancer Claim Statuses

|**Status**|**Description**|
| :- | :- |
|New|New claim, the performer's candidature is subject to approval by the bounty owner.|
|InProgress|Claim in the execution stage by the freelancer.|
|Completed|The task is completed by the freelancer, and the result is under review by the bounty owner.|
|Approved|The task result is approved by the bounty owner, and the reward is paid. Final status.|
|Rejected|The task result is rejected. In this status, the freelancer can open a dispute if they disagree with the decision. Intermediate status.|
|Canceled|The claim is canceled. Final status.|
|Expired|Task completion is overdue. Final status.|
|Disputed|A dispute is opened, and actions on the claim are frozen until the dispute is resolved.|
|NotCompleted|The task result was rejected or the claim lost the competition. Final status.|
|NotHired|The performer's candidature was rejected by the bounty owner.|
|Competes|The claim is participating in the competition.|
|ReadyToStart|The performer's candidature was approved, or the claim is ready to start execution. Used if the performer's candidature was approved, but the execution stage has not yet started because the bounty is waiting for other conditions to be met.|
|CompletedWithDispute|The task result was rejected by the bounty owner, but the freelancer opened and won a dispute. The task result can no longer be rejected. The claim is waiting for task acceptance for all sub-tasks. Can be used only for 'DifferentTasks' bounties.|
