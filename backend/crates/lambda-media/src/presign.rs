//! Presigned S3 `PUT` URLs for direct browser uploads
//! (`plans/08-media-uploads.md` §3.2).

use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::Client as S3Client;
use domain::ApiError;
use std::collections::BTreeMap;
use std::time::Duration;

/// How long a presigned upload URL stays valid (`03-api-contract.md` §9.1).
pub const EXPIRES_IN_SECONDS: u64 = 600;

pub struct Presigned {
    pub upload_url: String,
    /// Every header the request was signed with, lower-cased. The client
    /// must send all of them verbatim on the `PUT`.
    pub headers: BTreeMap<String, String>,
}

/// Builds a presigned `PUT` for `key` in `bucket`. When `sha256_base64` is
/// given, it's signed in as `x-amz-checksum-sha256` so S3 rejects a body
/// that doesn't match (`08-media-uploads.md` §3.2). `Content-Length` is
/// deliberately not signed — S3 presigned `PUT`s can't enforce a size range
/// (only presigned POST policies can); the byte-size cap is enforced in the
/// request-validation layer instead.
pub async fn presign_put(
    s3: &S3Client,
    bucket: &str,
    key: &str,
    content_type: &str,
    sha256_base64: Option<&str>,
) -> Result<Presigned, ApiError> {
    let presigning_config = PresigningConfig::expires_in(Duration::from_secs(EXPIRES_IN_SECONDS))
        .map_err(|e| {
        tracing::error!(error = ?e, "failed to build presigning config");
        ApiError::internal("failed to build presign config")
    })?;

    let mut request = s3
        .put_object()
        .bucket(bucket)
        .key(key)
        .content_type(content_type);
    if let Some(sha256) = sha256_base64 {
        request = request.checksum_sha256(sha256);
    }

    let presigned = request.presigned(presigning_config).await.map_err(|e| {
        tracing::error!(error = ?e, bucket, key, "failed to presign upload");
        ApiError::internal("failed to presign upload")
    })?;

    let headers = presigned
        .headers()
        .map(|(name, value)| (name.to_ascii_lowercase(), value.to_owned()))
        .collect();

    Ok(Presigned {
        upload_url: presigned.uri().to_owned(),
        headers,
    })
}
