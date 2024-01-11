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
ARGS=$(printf "$BOUNTY_TEMPLATE" "$BOUNTY_CONTRACT_ID" "$BOUNTY_AMOUNT" "$FREELANCER_ACCOUNT" "$(expr $BOUNTY_AMOUNT / 4)")
near call "$TOKEN_ID" ft_transfer_call "$ARGS" --accountId "$OWNER_ACCOUNT" --depositYocto 1 --gas 300000000000000
# TODO: asserting the results

# Claim a bounty
printf "\nClaim a bounty:\n"
near call "$BOUNTY_CONTRACT_ID" bounty_claim '{"id":0,"deadline":"86400000000000","description":"test claim"}' --accountId "$FREELANCER_ACCOUNT" --deposit 1 --gas 25000000000000

# TODO: asserting the results

# Approve Worker
printf "\nApprove Worker:\n"
near call "$BOUNTY_CONTRACT_ID" decision_on_claim "{\"id\":0,\"claimer\":\"$FREELANCER_ACCOUNT\",\"approve\":true}" --accountId "$OWNER_ACCOUNT" --depositYocto 1 --gas 25000000000000

# TODO: asserting the results

# Submit Work
printf "\nSubmit Work:\n"
near call "$BOUNTY_CONTRACT_ID" bounty_done "{\"id\":0,\"description\":\"It's done\"}" --accountId "$FREELANCER_ACCOUNT" --depositYocto 1 --gas 25000000000000

# TODO: asserting the results

# Approve Work
printf "\nApprove Work:\n"
near call "$TOKEN_ID" storage_deposit "{\"registration_only\":true,\"account_id\":\"$FREELANCER_ACCOUNT\"}" --accountId "$OWNER_ACCOUNT" --deposit 0.1 > /dev/null
near call "$BOUNTY_CONTRACT_ID" bounty_action "{\"id\":0,\"action\":{\"ClaimApproved\":{\"receiver_id\": \"$FREELANCER_ACCOUNT\"}}}" --accountId "$OWNER_ACCOUNT" --depositYocto 1 --gas 150000000000000

# TODO: asserting the results

printf "\nDeleting test accounts\n"
echo "y" | near delete "$BOUNTY_CONTRACT_ID" "$BASE_ACCOUNT" > /dev/null 2> /dev/null
echo "y" | near delete "$FREELANCER_ACCOUNT" "$BASE_ACCOUNT" > /dev/null 2> /dev/null
echo "y" | near delete "$OWNER_ACCOUNT" "$BASE_ACCOUNT" > /dev/null 2> /dev/null
echo "y" | near delete "$TOKEN_ACCOUNT" "$BASE_ACCOUNT" > /dev/null 2> /dev/null
