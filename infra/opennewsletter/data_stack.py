from __future__ import annotations

from typing import Any

import aws_cdk as cdk
from aws_cdk import aws_dynamodb as dynamodb
from constructs import Construct

from .config import EnvConfig


class DataStack(cdk.Stack):
    def __init__(self, scope: Construct, id: str, *, config: EnvConfig, **kwargs: Any) -> None:
        super().__init__(scope, id, **kwargs)

        removal_policy = (
            cdk.RemovalPolicy.RETAIN if config.env == "prod" else cdk.RemovalPolicy.DESTROY
        )

        self.table = dynamodb.Table(
            self,
            "Table",
            table_name=f"OpenNewsletter-{config.env}",
            partition_key=dynamodb.Attribute(name="pk", type=dynamodb.AttributeType.STRING),
            sort_key=dynamodb.Attribute(name="sk", type=dynamodb.AttributeType.STRING),
            billing_mode=dynamodb.BillingMode.PAY_PER_REQUEST,
            time_to_live_attribute="ttl",
            point_in_time_recovery=config.env == "prod",
            removal_policy=removal_policy,
            encryption=dynamodb.TableEncryption.AWS_MANAGED,
        )

        self.table.add_global_secondary_index(
            index_name="gsi1",
            partition_key=dynamodb.Attribute(name="gsi1pk", type=dynamodb.AttributeType.STRING),
            sort_key=dynamodb.Attribute(name="gsi1sk", type=dynamodb.AttributeType.STRING),
            projection_type=dynamodb.ProjectionType.ALL,
        )

        self.table.add_global_secondary_index(
            index_name="gsi2",
            partition_key=dynamodb.Attribute(name="gsi2pk", type=dynamodb.AttributeType.STRING),
            sort_key=dynamodb.Attribute(name="gsi2sk", type=dynamodb.AttributeType.STRING),
            projection_type=dynamodb.ProjectionType.ALL,
        )

        cdk.CfnOutput(self, "TableName", value=self.table.table_name)
        cdk.CfnOutput(self, "TableArn", value=self.table.table_arn)
