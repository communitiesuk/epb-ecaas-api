# HEM Lambda

The code for the lambda HEM.

Locally this depends on the home-energy-model repository being located where the simlink inside `lambda/hem` points to (`../../epb-home-energy-model/`).

## Building the lambda

`make build-lambda-binary`

`make build-lambda-zip`


## Cargo watch

The watch subcommand emulates the AWS Lambda control plane API. Run this command at the root of a Rust workspace and cargo-lambda will use cargo-watch to hot compile changes in your Lambda functions.


`aws-vault exec ecaas-integration -- cargo lambda watch`

## Invoke the lambda locally

`make invoke-lambda`

