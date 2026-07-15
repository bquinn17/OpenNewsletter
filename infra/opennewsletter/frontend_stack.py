"""FrontendStack — ACM certificate and (optionally) Route53 DNS records.

The frontend itself is hosted on GitHub Pages; CDK only manages the domain
plumbing. The ACM cert is created here and passed to MediaPersistentStack
(CloudFront alias) and ApiStack (HTTP API custom domain) as a constructor arg.
"""

from __future__ import annotations

from typing import Any

import aws_cdk as cdk
from aws_cdk import aws_certificatemanager as acm
from aws_cdk import aws_route53 as route53
from constructs import Construct

from .config import EnvConfig


class FrontendStack(cdk.Stack):
    def __init__(self, scope: Construct, id: str, *, config: EnvConfig, **kwargs: Any) -> None:
        super().__init__(scope, id, **kwargs)

        if config.hosted_zone_id and config.hosted_zone_name:
            hosted_zone: route53.IHostedZone | None = route53.HostedZone.from_hosted_zone_attributes(
                self,
                "HostedZone",
                hosted_zone_id=config.hosted_zone_id,
                zone_name=config.hosted_zone_name,
            )
        else:
            hosted_zone = None

        # Single cert covering the app domain, CDN alias, and API domain.
        # Must be in us-east-1 because CloudFront requires it.
        self.certificate = acm.Certificate(
            self,
            "Certificate",
            domain_name=config.domain,
            subject_alternative_names=[
                f"cdn.{config.domain}",
                config.api_domain,
            ],
            validation=(
                acm.CertificateValidation.from_dns(hosted_zone)
                if hosted_zone
                else acm.CertificateValidation.from_dns()
            ),
        )

        if hosted_zone:
            # GitHub Pages CNAME for the app domain
            route53.CnameRecord(
                self,
                "AppCname",
                zone=hosted_zone,
                record_name=config.domain,
                domain_name="bquinn.github.io",
                ttl=cdk.Duration.minutes(5),
            )

        if not hosted_zone:
            # Emit the cert validation CNAMEs so the operator can add them manually.
            cdk.CfnOutput(
                self,
                "CertificateArn",
                value=self.certificate.certificate_arn,
                description="Add the DNS validation CNAMEs shown in the ACM console for this cert.",
            )

        cdk.CfnOutput(self, "CertArn", value=self.certificate.certificate_arn)
