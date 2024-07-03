
# epb-ecaas-api

## Setting up Terraform locally

The Terraform for deployed ECaaS environments is in the `api` folder - navigate to that folder in your local repo. 

Initialise using the right backend-config, to avoid clash of state file names.

Terraform to your heart's content!

```bash
$ cd api
$ aws-vault exec ecaas-integration -- terraform init -backend-config=backend_ecaas_api_integration.hcl
$ aws-vault exec ecaas-integration -- terraform plan
```

## Using just commands to manage .tfvars files
1. To switch your AWS profile to the relevant environment run

    ```bash
    just set-profile {aws-environment-name}
    ```

  for example

    ```bash
    just set-profile ecaas-integration
    ```

2. To pull the tfvars file from the remote state storage bucket and populate local `.auto.tfvars` run

  ```bash
  just tfvars-get api {aws-environment-name}
  ```
    
  for example 

  ```bash
  just tfvars-get api ecaas-integration
  ```

3. To push your local version of `{aws-environment-name}-ecaas-api.tfvars` to the remote state storage bucket run

  ```bash
  just tfvars-put api {aws-environment-name}
  ```

  for example

  ```bash
  just tfvars-put api ecaas-staging
  ```

4. To delete all local .tfvars files run

  ```bash
  just tfvars-delete
  ```