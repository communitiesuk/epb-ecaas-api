# Set up API Gateway
resource "aws_api_gateway_rest_api" "ECaaSAPI" {
  name        = "ECaas API"
  description = "API for ECaaS (Energy Calculation as a Service)"

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
        "title" : "Energy Calculation as a Service",
        "version" : var.api_version,
        "links" : {
          "describedBy" : "https://ecaas-api-docs.epcregisters.net"
        }
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

# create lambda
resource "aws_lambda_function" "lambda" {
  filename      = "lambda.zip"
  function_name = "hem-lambda"
  role          = aws_iam_role.lambda_role.arn
  handler       = "bootstrap"
  runtime       = "provided.al2023"
  architectures = ["arm64"]
  timeout = 30
  source_code_hash = filebase64sha256("lambda.zip")
}

resource "aws_iam_role" "lambda_role" {
  name               = "lambda-role"
  assume_role_policy = data.aws_iam_policy_document.assume_role_lambda.json
}

data "aws_iam_policy_document" "assume_role_lambda" {
  statement {
    effect = "Allow"

    principals {
      type        = "Service"
      identifiers = ["lambda.amazonaws.com"]
    }

    actions = ["sts:AssumeRole"]
  }
}

data "aws_iam_policy_document" "lambda_logging" {
  statement {
    effect = "Allow"

    actions = [
      "logs:CreateLogGroup",
      "logs:CreateLogStream",
      "logs:PutLogEvents",
    ]

    resources = ["arn:aws:logs:*:*:*"]
  }
}

resource "aws_iam_policy" "lambda_logging" {
  name        = "lambda_logging"
  path        = "/"
  description = "IAM policy for logging from a lambda"
  policy      = data.aws_iam_policy_document.lambda_logging.json
}

resource "aws_iam_role_policy_attachment" "lambda_logs" {
  role       = aws_iam_role.lambda_role.name
  policy_arn = aws_iam_policy.lambda_logging.arn
}

# hook up lambda
resource "aws_api_gateway_resource" "proxy" {
  rest_api_id = "${aws_api_gateway_rest_api.ECaaSAPI.id}"
  parent_id   = "${aws_api_gateway_rest_api.ECaaSAPI.root_resource_id}"
  path_part   = "home-energy-model"
}

resource "aws_api_gateway_method" "proxy" {
  rest_api_id   = "${aws_api_gateway_rest_api.ECaaSAPI.id}"
  resource_id   = "${aws_api_gateway_resource.proxy.id}"
  http_method   = "POST"
  authorization = "NONE"
}

resource "aws_api_gateway_integration" "lambda" {
  rest_api_id = "${aws_api_gateway_rest_api.ECaaSAPI.id}"
  resource_id = "${aws_api_gateway_method.proxy.resource_id}"
  http_method = "${aws_api_gateway_method.proxy.http_method}"

  integration_http_method = "POST"
  type                    = "AWS_PROXY"
  uri                     = aws_lambda_function.lambda.invoke_arn
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