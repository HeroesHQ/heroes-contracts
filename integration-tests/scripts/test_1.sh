#!/bin/bash
set -e

export BASE_ACCOUNT=$1

BOUNTY_AMOUNT=100000000000000000000 # 100 TFT
BOUNTY_TEMPLATE='{"receiver_id":"%s","amount":"%s","msg":"{\\"metadata\\":{\\"title\\":\\"Test bounty title\\",\\"description\\":\\"Test bounty description\\",\\"category\\":\\"Design\\"},\\"deadline\\":{\\"MaxDeadline\\":{\\"max_deadline\\":\\"604800000000000\\"}},\\"claimer_approval\\":{\\"WhitelistWithApprovals\\":{\\"claimers_whitelist\\":[\\"%s\\"]}},\\"kyc_config\\":\\"KycNotRequired\\",\\"multitasking\\":{\\"OneForAll\\":{\\"number_of_slots\\":4,\\"amount_per_slot\\":\\"%s\\"}}}"}'

create_account() {
  set +e
  ACCOUNT_ID="${1}"
  echo "y" | near delete "$ACCOUNT_ID" "$BASE_ACCOUNT" > /dev/null 2> /dev/null # Ignore errors
  set -e
  printf "Creating %s\n" "$ACCOUNT_ID"
  near create-account "$ACCOUNT_ID" --masterAccount "$BASE_ACCOUNT" --initialBalance="${2}" > /dev/null
}

# Set up accounts
printf "Set up accounts:\n"
export OWNER_ACCOUNT=owner.$BASE_ACCOUNT
create_account "$OWNER_ACCOUNT" 10

export FREELANCER_ACCOUNT=freelancer.$BASE_ACCOUNT
create_account "$FREELANCER_ACCOUNT" 10

export BOUNTY_CONTRACT_ID=bounties.$BASE_ACCOUNT
create_account "$BOUNTY_CONTRACT_ID" 10

export TOKEN_ID=token.$BASE_ACCOUNT
create_account "$TOKEN_ID" 10

# shellcheck disable=SC2003
PLATFORM_FEE=$(expr "$BOUNTY_AMOUNT" / 10) # 10 TFT

# Set up FT tokens contract
printf "\nSet up ft tokens contract\n"
near deploy "$TOKEN_ID" --wasmFile ./test-token/res/test_token.wasm > /dev/null
near call "$TOKEN_ID" new "{}" --accountId "$TOKEN_ID" > /dev/null
near call "$TOKEN_ID" mint "{\"account_id\":\"$OWNER_ACCOUNT\",\"amount\":\"$BOUNTY_AMOUNT\"}" --accountId "$OWNER_ACCOUNT" > /dev/null
near call "$TOKEN_ID" storage_deposit "{\"registration_only\":true,\"account_id\":\"$BOUNTY_CONTRACT_ID\"}" --accountId "$BOUNTY_CONTRACT_ID" --deposit 0.1 > /dev/null

# Set up bounty contract
printf "\nSet up bounty contract:\n"
printf "Deploying contract to %s\n" "$BOUNTY_CONTRACT_ID"
near deploy "$BOUNTY_CONTRACT_ID" --wasmFile ./bounties/res/bounties.wasm > /dev/null
echo "Initializing contract"
near call "$BOUNTY_CONTRACT_ID" new "{\"token_account_ids\":[\"$TOKEN_ID\"],\"admins_whitelist\":[\"$BASE_ACCOUNT\"]}" --accountId "$BOUNTY_CONTRACT_ID"
near call "$BOUNTY_CONTRACT_ID" set_status '{"status":"Live"}' --accountId "$BOUNTY_CONTRACT_ID" > /dev/null
near call "$BOUNTY_CONTRACT_ID" add_token_id "{\"token_id\":\"$TOKEN_ID\"}" --accountId "$BASE_ACCOUNT" --depositYocto 1 --gas 50000000000000 > /dev/null

# Create a bounty
printf "\nCreating a bounty:\n"
# shellcheck disable=SC2003
# shellcheck disable=SC2059
ARGS=$(printf "$BOUNTY_TEMPLATE" "$BOUNTY_CONTRACT_ID" "$BOUNTY_AMOUNT" "$FREELANCER_ACCOUNT" "$(expr "$BOUNTY_AMOUNT" / 4)")
near call "$TOKEN_ID" ft_transfer_call "$ARGS" --accountId "$OWNER_ACCOUNT" --depositYocto 1 --gas 300000000000000

if [[ $(near view "$TOKEN_ID" ft_balance_of "{\"account_id\":\"$BOUNTY_CONTRACT_ID\"}" | grep -Po "'\d+'") == "'$BOUNTY_AMOUNT'" ]]; then
  printf "\n\033[0;32m%s\033[0m\n" "Bounty contract token balance is correct after bounty creation"
else
  printf "\n\033[0;31m%s\033[0m\n" "Bounty contract token balance is incorrect after bounty creation"
  exit 1
fi

if [[ $(near view "$TOKEN_ID" ft_balance_of "{\"account_id\":\"$OWNER_ACCOUNT\"}" | grep -Po "'\d+'") == "'0'" ]]; then
  printf "\n\033[0;32m%s\033[0m\n" "The token balance of the owner's account is zero after bounty creation"
else
  printf "\n\033[0;31m%s\033[0m\n" "Owner's account token balance is incorrect after bounty creation"
  exit 1
fi

if [[ $(near view "$BOUNTY_CONTRACT_ID" get_bounty '{"id":0}' | tr '\n' '\r'| sed -r "s/^.*status: '([A-Za-z]+)',.*$/\1/") == "New" ]]; then
  printf "\n\033[0;32m%s\033[0m\n" "Bounty successfully created"
else
  printf "\n\033[0;31m%s\033[0m\n" "Bounty has not been created"
  exit 1
fi

# Claim a bounty
printf "\nClaim a bounty:\n"
near call "$BOUNTY_CONTRACT_ID" bounty_claim '{"id":0,"deadline":"86400000000000","description":"I can do it"}' --accountId "$FREELANCER_ACCOUNT" --deposit 1 --gas 25000000000000

if [[ $(near view "$BOUNTY_CONTRACT_ID" get_bounty_claims_by_id '{"id":0}' | tr '\n' '\r'| sed -r "s/^.*bounty_id: ([[:digit:]]+),.*$/\1/") == "0" ]]; then
  printf "\n\033[0;32m%s\033[0m\n" "Claim created successfully"
else
  printf "\n\033[0;31m%s\033[0m\n" "No claim has been created"
  exit 1
fi

# Approve Worker
printf "\nApprove Worker:\n"
near call "$BOUNTY_CONTRACT_ID" decision_on_claim "{\"id\":0,\"claimer\":\"$FREELANCER_ACCOUNT\",\"approve\":true}" --accountId "$OWNER_ACCOUNT" --depositYocto 1 --gas 25000000000000

if [[ $(near view "$BOUNTY_CONTRACT_ID" get_bounty_claims_by_id '{"id":0}' | tr '\n' '\r'| sed -r "s/^.*status: '([A-Za-z]+)',.*$/\1/") == "InProgress" ]]; then
  printf "\n\033[0;32m%s\033[0m\n" "The worker is approved"
else
  printf "\n\033[0;31m%s\033[0m\n" "The worker is not approved"
  exit 1
fi

# Submit Work
printf "\nSubmit Work:\n"
near call "$BOUNTY_CONTRACT_ID" bounty_done "{\"id\":0,\"description\":\"It's done\"}" --accountId "$FREELANCER_ACCOUNT" --depositYocto 1 --gas 25000000000000

if [[ $(near view "$BOUNTY_CONTRACT_ID" get_bounty_claims_by_id '{"id":0}' | tr '\n' '\r'| sed -r "s/^.*status: '([A-Za-z]+)',.*$/\1/") == "Completed" ]]; then
  printf "\n\033[0;32m%s\033[0m\n" "The work has been submitted"
else
  printf "\n\033[0;31m%s\033[0m\n" "The work has not been submitted"
  exit 1
fi

# Approve Work
printf "\nApprove Work:\n"
near call "$TOKEN_ID" storage_deposit "{\"registration_only\":true,\"account_id\":\"$FREELANCER_ACCOUNT\"}" --accountId "$OWNER_ACCOUNT" --deposit 0.1 > /dev/null
near call "$BOUNTY_CONTRACT_ID" bounty_action "{\"id\":0,\"action\":{\"ClaimApproved\":{\"receiver_id\": \"$FREELANCER_ACCOUNT\"}}}" --accountId "$OWNER_ACCOUNT" --depositYocto 1 --gas 150000000000000

if [[ $(near view "$BOUNTY_CONTRACT_ID" get_bounty_claims_by_id '{"id":0}' | tr '\n' '\r'| sed -r "s/^.*status: '([A-Za-z]+)',.*$/\1/") == "Approved" ]]; then
  printf "\n\033[0;32m%s\033[0m\n" "The work is approved"
else
  printf "\n\033[0;31m%s\033[0m\n" "The work is not approved"
  exit 1
fi

if [[ $(near view "$BOUNTY_CONTRACT_ID" get_bounty '{"id":0}' | tr '\n' '\r'| sed -r "s/^.*status: '([A-Za-z]+)',.*$/\1/") == "AwaitingClaims" ]]; then
  printf "\n\033[0;32m%s\033[0m\n" "Bounty is waiting for the next claimants"
else
  printf "\n\033[0;31m%s\033[0m\n" "Bounty has an incorrect state"
  exit 1
fi

# shellcheck disable=SC2003
if [[ $(near view "$TOKEN_ID" ft_balance_of "{\"account_id\":\"$BOUNTY_CONTRACT_ID\"}" | grep -Po "'\d+'") == "'$(expr "$BOUNTY_AMOUNT" \* 3 / 4 + "$PLATFORM_FEE" / 4)'" ]]; then
  printf "\n\033[0;32m%s\033[0m\n" "Bounty contract token balance is correct after bounty payout"
else
  printf "\n\033[0;31m%s\033[0m\n" "Bounty contract token balance is incorrect after bounty payout"
  exit 1
fi

# shellcheck disable=SC2003
if [[ $(near view "$TOKEN_ID" ft_balance_of "{\"account_id\":\"$FREELANCER_ACCOUNT\"}" | grep -Po "'\d+'") == "'$(expr "$BOUNTY_AMOUNT" / 4 - "$PLATFORM_FEE" / 4)'" ]]; then
  printf "\n\033[0;32m%s\033[0m\n" "Freelancer's account token balance is correct after bounty payout"
else
  printf "\n\033[0;31m%s\033[0m\n" "Freelancer's account token balance is incorrect after bounty payout"
  exit 1
fi

printf "\nDeleting test accounts\n"
echo "y" | near delete "$BOUNTY_CONTRACT_ID" "$BASE_ACCOUNT" > /dev/null 2> /dev/null
echo "y" | near delete "$FREELANCER_ACCOUNT" "$BASE_ACCOUNT" > /dev/null 2> /dev/null
echo "y" | near delete "$OWNER_ACCOUNT" "$BASE_ACCOUNT" > /dev/null 2> /dev/null
echo "y" | near delete "$TOKEN_ACCOUNT" "$BASE_ACCOUNT" > /dev/null 2> /dev/null
