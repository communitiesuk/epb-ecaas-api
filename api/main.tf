terraform {
  required_version = "~>1.8"

  required_providers {
    aws = {
      version = "~>5.0"
      source  = "hashicorp/aws"
    }
  }
  backend "s3" {}
}

provider "aws" {
  region                   = var.region
  shared_config_files      = ["~/.aws/config"]
  shared_credentials_files = ["~/.aws/credentials"]
}

# Set up API Gateway
resource "aws_api_gateway_rest_api" "ECaaSAPI" {
  name = "ECaas API"
  description = "API for ECaaS (Energy Calculation as a Service)."

  endpoint_configuration {
    types = ["REGIONAL"]
  }
}

resource "aws_api_gateway_integration" "GatewayIntegration" {
  rest_api_id = aws_api_gateway_rest_api.ECaaSAPI.id
  resource_id = aws_api_gateway_resource.ApiResource.id
  http_method = aws_api_gateway_method.GetApiMethod.http_method
  type        = "MOCK"
  request_templates = {
    "application/json" = jsonencode({
      statusCode = 200,
    })
  }
}

# Set up /api method
resource "aws_api_gateway_resource" "ApiResource" {
  rest_api_id = aws_api_gateway_rest_api.ECaaSAPI.id
  parent_id   = aws_api_gateway_rest_api.ECaaSAPI.root_resource_id
  path_part   = "api"
}

resource "aws_api_gateway_method" "GetApiMethod" {
  rest_api_id   = aws_api_gateway_rest_api.ECaaSAPI.id
  resource_id   = aws_api_gateway_resource.ApiResource.id
  http_method   = "GET"
  authorization = "NONE"
}

resource "aws_api_gateway_integration_response" "GetApiIntegrationResponse" {
  rest_api_id = aws_api_gateway_rest_api.ECaaSAPI.id
  resource_id = aws_api_gateway_resource.ApiResource.id
  http_method = aws_api_gateway_method.GetApiMethod.http_method
  status_code = aws_api_gateway_method_response.GetApiMethodResponse.status_code
  response_templates = {
    "application/json" = jsonencode(
      {
        "title": "Energy Calculation as a Service",
        "version": var.api_version
      }
    )  
  }
}

resource "aws_api_gateway_method_response" "GetApiMethodResponse" {
  rest_api_id = aws_api_gateway_rest_api.ECaaSAPI.id
  resource_id = aws_api_gateway_resource.ApiResource.id
  http_method = aws_api_gateway_method.GetApiMethod.http_method
  status_code = "200"
}

# Set up deployment
resource "aws_api_gateway_deployment" "Deployment" {
  rest_api_id = aws_api_gateway_rest_api.ECaaSAPI.id

  triggers = {
    redeployment = sha1(jsonencode([
      aws_api_gateway_rest_api.ECaaSAPI.id,
      aws_api_gateway_rest_api.ECaaSAPI.description,
      aws_api_gateway_resource.ApiResource.id,
      aws_api_gateway_method.GetApiMethod.id,
      aws_api_gateway_integration.GatewayIntegration.id,
      aws_api_gateway_integration_response.GetApiIntegrationResponse.response_templates
    ]))
  }

  lifecycle {
    create_before_destroy = true
  }
}

resource "aws_api_gateway_stage" "DeploymentStage" {
  deployment_id = aws_api_gateway_deployment.Deployment.id
  rest_api_id   = aws_api_gateway_rest_api.ECaaSAPI.id
  stage_name    = "Deployment"  
}