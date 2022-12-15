#!/bin/bash
set -e
./build.sh

# Change these to your account ids
export BASE_ACCOUNT=beskyd_dev.testnet
export FREELANCER_ACCOUNT=beskyd_test.testnet
export BOUNTY_CONTRACT_ID=bounty.$BASE_ACCOUNT
printenv BOUNTY_CONTRACT_ID
export DAO_CONTRACT_ID=bounty_dao.$BASE_ACCOUNT
printenv DAO_CONTRACT_ID

# Redo account (if contract already exists)
#near call usdn.testnet ft_transfer '{"receiver_id":"'$BASE_ACCOUNT'","amount":"1000000000000000000"}' --accountId $BOUNTY_CONTRACT_ID --depositYocto 1 --gas 300000000000000
#near call usdn.testnet ft_transfer '{"receiver_id":"'$BASE_ACCOUNT'","amount":"1000000000000000000"}' --accountId $FREELANCER_ACCOUNT --depositYocto 1 --gas 300000000000000
set +e
near delete $BOUNTY_CONTRACT_ID $BASE_ACCOUNT 2> /dev/null # Ignore errors
near delete $DAO_CONTRACT_ID $BASE_ACCOUNT 2> /dev/null # Ignore errors
set -e
near create-account $BOUNTY_CONTRACT_ID --masterAccount $BASE_ACCOUNT
near create-account $DAO_CONTRACT_ID --masterAccount $BASE_ACCOUNT

# Set up bounty contract
near deploy $BOUNTY_CONTRACT_ID --wasmFile res/bounties.wasm
export ADMIN_ACCOUNT='["'$BASE_ACCOUNT'"]'
near call $BOUNTY_CONTRACT_ID new '{"token_account_ids":["usdn.testnet"],"admin_whitelist":'$ADMIN_ACCOUNT'}' --accountId $BOUNTY_CONTRACT_ID
# Set up validators DAO contract
near deploy $DAO_CONTRACT_ID --wasmFile ../tests/res/sputnikdao2.wasm
export COUNCIL='["'$BASE_ACCOUNT'"]'
near call $DAO_CONTRACT_ID new '{"config": {"name": "genesis2", "purpose": "test", "metadata": ""}, "policy": '$COUNCIL'}' --accountId $DAO_CONTRACT_ID

# Add a bounty under the control of the validators DAO
near call usdn.testnet ft_transfer_call '{"receiver_id":"'$BOUNTY_CONTRACT_ID'","amount":"1000000000000000000","msg":"{\"description\":\"Test bounty description\",\"bounty_type\":\"MarketingServices\",\"max_deadline\":\"604800000000000\",\"validators_dao\":{\"account_id\":\"'$DAO_CONTRACT_ID'\",\"add_proposal_bond\":\"1000000000000000000000000\"}}"}' --accountId $BASE_ACCOUNT --depositYocto 1 --gas 300000000000000
# Add a bounty under the control of the project owner
#near call usdn.testnet ft_transfer_call '{"receiver_id":"'$BOUNTY_CONTRACT_ID'","amount":"1000000000000000000","msg":"{\"description\":\"Test bounty description\",\"bounty_type\":\"MarketingServices\",\"max_deadline\":\"604800000000000\"}"}' --accountId $BASE_ACCOUNT --depositYocto 1 --gas 300000000000000

# Verify
near view usdn.testnet ft_balance_of '{"account_id":"'$BOUNTY_CONTRACT_ID'"}'
near view $BOUNTY_CONTRACT_ID get_bounties '{}'
near view $BOUNTY_CONTRACT_ID get_bounty '{"id":0}'
near view $BOUNTY_CONTRACT_ID get_account_bounties '{"account_id":"'$BASE_ACCOUNT'"}'
near view $BOUNTY_CONTRACT_ID get_last_bounty_id '{}'
near view $BOUNTY_CONTRACT_ID get_bounties_by_ids '{"ids":[0]}'
near view $BOUNTY_CONTRACT_ID get_bounty_claims_by_id '{"id":0}'
near view $BOUNTY_CONTRACT_ID get_bounty_claims '{"account_id":"'$FREELANCER_ACCOUNT'"}'
near view $BOUNTY_CONTRACT_ID get_token_account_ids '{}'
near view $BOUNTY_CONTRACT_ID get_admin_whitelist '{}'
near view $BOUNTY_CONTRACT_ID get_reputation_contract_account_id '{}'
near view $BOUNTY_CONTRACT_ID get_dispute_contract_account_id '{}'
near view $BOUNTY_CONTRACT_ID get_locked_storage_amount '{}'
near view $BOUNTY_CONTRACT_ID get_available_amount '{}'

# Someone claims bounty
near call $BOUNTY_CONTRACT_ID bounty_claim '{"id":0,"deadline":"86400000000000"}' --accountId $FREELANCER_ACCOUNT --deposit 1 --gas 25000000000000

# Show bounty claims
near view $BOUNTY_CONTRACT_ID get_bounty_claims_by_id '{"id":0}'
near view $BOUNTY_CONTRACT_ID get_bounty_claims '{"account_id":"'$FREELANCER_ACCOUNT'"}'
# Verify bounty
near view $BOUNTY_CONTRACT_ID get_bounty '{"id":0}'

# Call bounty_done
near call $BOUNTY_CONTRACT_ID bounty_done '{"id":0,"description":"Bounty done by '$FREELANCER_ACCOUNT'"}' --accountId $FREELANCER_ACCOUNT --depositYocto 1 --gas 75000000000000
#near call $BOUNTY_CONTRACT_ID bounty_give_up '{"id":0}' --accountId $FREELANCER_ACCOUNT --depositYocto 1 --gas 25000000000000

# Show bounty and bounty claims
near view $BOUNTY_CONTRACT_ID get_bounty_claims_by_id '{"id":0}'
near view $BOUNTY_CONTRACT_ID get_bounty '{"id":0}'

# Claim is approved by validators DAO
near call $DAO_CONTRACT_ID act_proposal '{"id": 0, "action": "VoteApprove"}' --accountId $BASE_ACCOUNT --gas 85000000000000
#near call $DAO_CONTRACT_ID act_proposal '{"id": 0, "action": "VoteReject"}' --accountId $BASE_ACCOUNT --gas 25000000000000
# Claim is approved by project owner
#near call $BOUNTY_CONTRACT_ID bounty_action '{"id":0,"action":{"ClaimApproved":{"receiver_id": "'$FREELANCER_ACCOUNT'"}}}' --accountId $BASE_ACCOUNT --depositYocto 1 --gas 60000000000000
# Claim was rejected by project owner
#near call $BOUNTY_CONTRACT_ID bounty_action '{"id":0,"action":{"ClaimRejected":{"receiver_id": "'$FREELANCER_ACCOUNT'"}}}' --accountId $BASE_ACCOUNT --depositYocto 1 --gas 25000000000000

# Show DAO proposal
near view $DAO_CONTRACT_ID get_proposal '{"id": 0}'

# Show bounty and bounty claims
near view $BOUNTY_CONTRACT_ID get_bounty_claims_by_id '{"id":0}'
near view $BOUNTY_CONTRACT_ID get_bounty '{"id":0}'

# Show balances
near view usdn.testnet ft_balance_of '{"account_id":"'$BOUNTY_CONTRACT_ID'"}'
near view usdn.testnet ft_balance_of '{"account_id":"'$FREELANCER_ACCOUNT'"}'
