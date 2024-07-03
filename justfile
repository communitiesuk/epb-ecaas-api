set dotenv-load

# List available commands
default:
    @just --list

# Add AWS config and aws-vault profile required to run many commands. Note: this will update .env file in current directory
add-profile profile:
    #!/usr/bin/env bash

    PROFILE=$(cat ~/.aws/config | grep "\[profile {{profile}}\]")
    
    if [ -z "$PROFILE" ]; then
        mfa_serial=$(cat ~/.aws/config | grep -m 1 mfa_serial | cut -d'=' -f2)
        account_id=$(read -p "Enter account id: " account_id; echo $account_id)
        role=$(read -p "Enter account role: " role; echo $role)

        echo "adding profile: {{profile}}"
        echo "[profile {{profile}}]" >> ~/.aws/config
        echo "role_arn=arn:aws:iam::$account_id:role/$role" >> ~/.aws/config
        echo "mfa_serial=$mfa_serial" >> ~/.aws/config
        echo "region=eu-west-2" >> ~/.aws/config
        echo "output=json" >> ~/.aws/config

        echo "profile {{profile}} added to ~/.aws/config"

        aws-vault add {{profile}}
        
    else
        echo "profile {{profile}} already exists"
    fi

    just _set-profile {{profile}}


# Set previously added aws-vault profile. Note: this will update .env file in current directory
set-profile profile:
    #!/usr/bin/env bash

    PROFILE=$(aws-vault list --profiles | grep {{profile}})

    if [ -z "$PROFILE" ]; then
        echo "profile {{profile}} does not exist. Run 'just add-profile {{profile}}' to add it."
    else
        just _set-profile {{profile}}
    fi


_set-profile profile:
    #!/usr/bin/env bash

    echo "setting current profile to {{profile}}"

    if [ -f .env ]; then
        sed -i -e 's/export AWS_PROFILE=.*$/export AWS_PROFILE={{profile}}/g' .env
    else
        echo export AWS_PROFILE={{profile}} > .env
    fi

    echo $(cat ~/.aws/config | grep -A 4 "\[profile {{profile}}\]")

[no-exit-message]
_ensure_aws_profile:
    #!/usr/bin/env bash

    if [[ -z "${AWS_PROFILE}" ]]; then
      echo "Please define your AWS_PROFILE environment variable, e.g. 'export AWS_PROFILE=ecaas-integration'"
      exit 1
    fi

[no-exit-message]
_ensure_jq:
    #!/usr/bin/env bash

    if ! command -v jq &> /dev/null; then
      if [ "$(uname)" == "Darwin" ]; then
        if ! command -v brew &> /dev/null; then
          echo "You need `jq` to run this just recipe. It can be installed using e.g. Homebrew."
          exit 1
        else
          echo "As this just recipe needs jq, installing it using Homebrew..."
          echo ""
          brew install jq
          echo ""
          echo "...jq installed using Homebrew - ready to run the just recipe!"
          echo ""
        fi
      else
        echo "You need `jq` available in your shell environment in order to run this just recipe. For installation, see https://stedolan.github.io/jq/"
        exit 1
      fi
    fi

tf-init path="." backend="": _ensure_aws_profile
    #!/usr/bin/env bash

    if [ "{{backend}}" != "" ]; then
        echo "initialising terraform with backend {{backend}}"
        cd {{path}} && aws-vault exec $AWS_PROFILE -- terraform init -backend-config={{backend}} -reconfigure
    else
        echo "initialising terraform"
        cd {{path}} && aws-vault exec $AWS_PROFILE -- terraform init
    fi

# Updates tfvars file in S3 with values from local file. environment should be one of 'ecaas-integration', 'ecaas-staging' or 'ecaas-production'
tfvars-put path="." environment="ecaas-integration": _ensure_aws_profile
    #!/usr/bin/env bash

    cd {{path}} && aws-vault exec $AWS_PROFILE -- aws s3api put-object --bucket epbr-{{environment}}-terraform-state --key ecaas-api.tfvars --body {{environment}}-ecaas-api.tfvars

    bg_red='\033[0;41m'
    green='\033[0;32m'
    cyan='\033[0;36m'
    clear='\033[0m'
    printf "${bg_red}LOOSE LIPS SINK SHIPS!${clear}\n${green}Always run '${cyan}rm -f {*.tfvars,.*.tfvars}${green}' once you've applied your changes!\n\n"


# Updates local tfvars file with values stored in S3 bucket. environment should be one of 'ecaas-integration', 'ecaas-staging' or 'ecaas-production'
tfvars-get path="." environment="ecaas-integration": _ensure_aws_profile
    #!/usr/bin/env bash

    cd {{path}}
    aws-vault exec $AWS_PROFILE -- aws s3api get-object --bucket epbr-{{environment}}-terraform-state --key ecaas-api.tfvars {{environment}}-ecaas-api.tfvars
    cp {{environment}}-ecaas-api.tfvars .auto.tfvars

    bg_red='\033[0;41m'
    green='\033[0;32m'
    cyan='\033[0;36m'
    clear='\033[0m'
    printf "${bg_red}CARELESS TALK COSTS LIVES!${clear}\n${green}Always run '${cyan}just tfvars-delete${green}' or '${cyan}rm -f {*.tfvars,.*.tfvars}${green}' once you've applied your changes!\n\n"


# Deletes all tvars from local
tfvars-delete path="api":
    #!/usr/bin/env bash
    cd {{path}}
    rm -f {*.tfvars,.*.tfvars}
