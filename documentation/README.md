# ECaaS API Documentation

This directory contains documentation of the ECaaS API.

The `YAML` file contains the [OpenAPI specification v3.1.0](https://spec.openapis.org/oas/v3.1.0.html) for our API. It references the `schema.json` file which defines a valid request body for the FHS endpoint.

Note, the current schema was generated from the Rust codebase with some small manual tweaks to ensure validity.

## How to generate static Redoc documentation

The static html file was generated using redocly. It can be regenerated using the below command:

```
npx @redocly/cli build-docs openapi.yaml
```
Source: [Redocly](https://github.com/Redocly/redoc?tab=readme-ov-file#usage)
