#!/usr/bin/env python3
"""CDK app entry point.

Usage:
    cdk synth --context env=dev
    cdk deploy --context env=dev DataStack-dev AuthStack-dev ...
"""

import aws_cdk as cdk

from opennewsletter.api_stack import ApiStack
from opennewsletter.auth_stack import AuthStack
from opennewsletter.config import load_config
from opennewsletter.data_stack import DataStack
from opennewsletter.frontend_stack import FrontendStack
from opennewsletter.media_persistent_stack import MediaPersistentStack
from opennewsletter.media_pipeline_stack import MediaPipelineStack
from opennewsletter.monitoring_stack import MonitoringStack

app = cdk.App()

env_name: str = app.node.try_get_context("env") or "dev"
config = load_config(env_name)

# All stacks target us-east-1 (ACM certs for CloudFront + API must be in us-east-1).
aws_env = cdk.Environment(region="us-east-1")

suffix = f"-{env_name}"

data_stack = DataStack(app, f"DataStack{suffix}", config=config, env=aws_env)

auth_stack = AuthStack(app, f"AuthStack{suffix}", config=config, env=aws_env)

# FrontendStack creates the ACM cert consumed by MediaPersistentStack and (later) ApiStack.
frontend_stack = FrontendStack(
    app, f"FrontendStack{suffix}", config=config, env=aws_env
)

media_persistent_stack = MediaPersistentStack(
    app,
    f"MediaPersistentStack{suffix}",
    config=config,
    table=data_stack.table,
    certificate=frontend_stack.certificate,
    env=aws_env,
)

media_pipeline_stack = MediaPipelineStack(
    app,
    f"MediaPipelineStack{suffix}",
    config=config,
    table=data_stack.table,
    originals_bucket=media_persistent_stack.originals_bucket,
    processed_bucket=media_persistent_stack.processed_bucket,
    env=aws_env,
)

api_stack = ApiStack(
    app,
    f"ApiStack{suffix}",
    config=config,
    table=data_stack.table,
    user_pool=auth_stack.user_pool,
    user_pool_client=auth_stack.frontend_client,
    certificate=frontend_stack.certificate,
    env=aws_env,
)

monitoring_stack = MonitoringStack(
    app, f"MonitoringStack{suffix}", config=config, env=aws_env
)

app.synth()
