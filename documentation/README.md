# ECaaS API Documentation

This directory contains documentation of the ECaaS API.

The `openapi.yaml` file contains the [OpenAPI specification v3.1.0](https://spec.openapis.org/oas/v3.1.0.html) of the API. It references a `schema.json` file which defines a valid request body for the FHS endpoint.

Note, the current schema was generated from the Rust HEM codebase with some small manual tweaks to ensure validity. It follows the [2020-12 JSON Schema dialect](https://json-schema.org/draft/2020-12/).

## How to generate static Redoc documentation

The static HTML file was generated using Redocly. It can be regenerated using the below command:

```
npx @redocly/cli build-docs openapi.yaml
```

Source: [Redocly](https://github.com/Redocly/redoc?tab=readme-ov-file#usage)
