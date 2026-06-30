use aws_sdk_dynamodb::Client;

/// Thin handle bundling the DynamoDB client and table name. Cheap to clone.
#[derive(Debug, Clone)]
pub struct Repo {
    pub client: Client,
    pub table: String,
}

impl Repo {
    pub fn new(client: Client, table: impl Into<String>) -> Self {
        Self { client, table: table.into() }
    }
}
