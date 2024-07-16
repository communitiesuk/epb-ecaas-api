
# epb-ecaas-api 

## Setting up Terraform locally

The Terraform for deployed ECaaS environments is in the `api` folder - navigate to that folder in your local repo. 

Initialise using the right backend-config, to avoid clash of state file names.

Terraform to your heart's content.

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

## Contributing

### Using the commit template

If you've done work in a pair or ensemble why not add your co-author(s) to the commit? This way everyone involved is
given credit and people know who they can approach for questions about specific commits. To make this easy there is a
commit template with a list of regular contributors to this code base. You will find it at the root of this
project: `commit_template.txt`. Each row represents a possible co-author, however everyone is commented out by default (
using `#`), and any row that is commented out will not show up in the commit.

#### Editing the template

If your name is not in the `commit_template.txt` yet, edit the file and add a new row with your details, following the
format `#Co-Authored-By: Name <email>`, e.g. `#Co-Authored-By: Maja <maja@gmail.com>`. The email must match the email
you use for your GitHub account. To protect your privacy, you can activate and use your noreply GitHub addresses (find
it in GitHub under Settings > Emails > Keep my email addresses private).

#### Getting set up

To apply the commit template navigate to the root of the project in a terminal and
use: `git config commit.template commit_template.txt`. This will edit your local git config for the project and apply
the template to every future commit.

#### Using the template (committing with co-authors)

When creating a new commit, edit your commit (e.g. using vim, or a code editor) and delete the `#` in front of any
co-author(s) you want to credit. This means that it's probably easier and quicker to use `git commit` (instead
of `git commit -m ""` followed by a `git commit --amend`), as it will show you the commit template content for you to
edit.