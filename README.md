
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
