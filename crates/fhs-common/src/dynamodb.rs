use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client;
use tokio::sync::OnceCell;

static DYNAMO_DB_CLIENT: OnceCell<Client> = OnceCell::const_new();

pub async fn get_global_dynamodb_client() -> &'static Client {
    DYNAMO_DB_CLIENT
        .get_or_init(|| async {
            // if we are running in a cargo-lambda watch environment, use a local DynamoDB instance
            // otherwise we expect to be within real AWS
            if std::env::var("CARGO_HOME").is_ok() {
                let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
                    // DynamoDB run locally uses port 8000 by default.
                    .endpoint_url("http://localhost:8000")
                    .load()
                    .await;
                let dynamodb_local_config =
                    aws_sdk_dynamodb::config::Builder::from(&config).build();

                aws_sdk_dynamodb::Client::from_conf(dynamodb_local_config)
            } else {
                let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
                Client::new(&config)
            }
        })
        .await
}
