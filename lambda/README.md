# HEM Lambda

The code for the HEM lambda.

Locally this depends on the home-energy-model repository being in the location that the simlink inside `lambda/hem` points to (`../../epb-home-energy-model/`).

## Building the lambda

`make build-lambda-binary`

`make build-lambda-zip`

## Interacting with the lambda locally

The watch subcommand emulates the AWS Lambda control plane API. Run this command at the root of a Rust workspace and cargo-lambda will use cargo-watch to hot compile changes in your Lambda functions.

`make watch-lambda`

Then you can invoke the lambda locally, e.g. by using Postman. The lambda will be available at: `http://localhost:9000/lambda-url/ecaas-lambda` and the payload can be any valid future home standard input JSON.

The lambda can also be invoked via the terminal with `make invoke-lambda`. This will call the lambda with the
`demo_fhs_bottomup.json` example file.

## Resources

[Cargo lambda commands](https://www.cargo-lambda.info/commands/)
