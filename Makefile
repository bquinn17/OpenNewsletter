SHELL = /bin/bash

.PHONY: help build-lambdas deploy-dev seed redeploy-volatile reset-all deploy-lambda fe

# Volatile stacks can be destroyed and redeployed freely; persistent stacks are left alone.
VOLATILE_STACKS = DataStack-dev MediaPipelineStack-dev MonitoringStack-dev

help: ## Show available targets
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+:.*## / {printf "  %-24s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

build-lambdas: ## Cross-compile every Lambda binary for arm64
	cd backend && cargo lambda build --release --arm64

# CDK falls back to a shell stub for any binary it can't find in
# backend/target/lambda, so skipping the build here would deploy stubs silently.
deploy-dev: build-lambdas ## Build Lambdas, deploy all stacks to dev, write frontend/.env.dev
	cd infra && . .venv/bin/activate && \
	    cdk deploy --context env=dev --all \
	        --require-approval never \
	        --outputs-file cdk.out/dev-outputs.json
	python3 scripts/write_frontend_env.py --env dev

seed: ## Reset DynamoDB + S3 and recreate fixture data (keeps Cognito users)
	python3 scripts/seed_dev_data.py --env dev

redeploy-volatile: ## Destroy + redeploy volatile stacks (Data, Pipeline, Monitoring)
	cd infra && . .venv/bin/activate && \
	    cdk destroy --context env=dev $(VOLATILE_STACKS) --force
	cd infra && . .venv/bin/activate && \
	    cdk deploy --context env=dev $(VOLATILE_STACKS) \
	        --require-approval never \
	        --outputs-file cdk.out/dev-outputs.json

reset-all: ## Tear down ALL dev stacks — destroys Cognito user pool (confirm manually)
	@echo "WARNING: This destroys ALL dev stacks, including the Cognito user pool."
	@echo "         All Cognito users will be permanently deleted."
	@read -r -p "         Type 'yes' to proceed: " confirm && \
	    [ "$$confirm" = "yes" ] || { echo "Aborted."; exit 1; }
	cd infra && . .venv/bin/activate && \
	    cdk destroy --context env=dev --all --force

# Cargo binary name -> deployed CloudFormation function name. They differ because
# lambda-invites ships two entry points, so its binaries can't both be `bootstrap`.
FUNCTION_groups-api        = Groups
FUNCTION_invites-api       = Invites
FUNCTION_invites-presignup = PreSignUp

deploy-lambda: ## Fast-update one Lambda binary: make deploy-lambda LAMBDA=<binary-name>
	@test -n "$(LAMBDA)" || { \
	    echo "Usage: make deploy-lambda LAMBDA=<binary-name>"; \
	    echo "Known binaries: groups-api invites-api invites-presignup"; exit 1; }
	@test -n "$(FUNCTION_$(LAMBDA))" || { \
	    echo "Unknown binary '$(LAMBDA)'. Add it to the FUNCTION_* map in the Makefile."; exit 1; }
	cd backend && cargo lambda build --release --arm64 --bin $(LAMBDA)
	aws lambda update-function-code \
	    --function-name OpenNewsletter-$(FUNCTION_$(LAMBDA))-dev \
	    --zip-file fileb://backend/target/lambda/$(LAMBDA)/bootstrap.zip \
	    --no-cli-pager

fe: ## Start frontend dev server pointing at the dev API
	cd frontend && npm run dev -- --mode dev
