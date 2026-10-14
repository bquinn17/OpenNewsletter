# lambda-image-process

S3 `ObjectCreated` trigger. Reads originals, generates display + thumbnail WebP/GIF, writes to the processed bucket, marks the DynamoDB row `ready`. See [`docs/design/media.md`](../../../docs/design/media.md).

## Environment

`TABLE_NAME`, `MEDIA_ORIGINALS_BUCKET`, `AVATARS_ORIGINALS_BUCKET`, `PROCESSED_BUCKET`, `AVATARS_PROCESSED_BUCKET`.

The two *originals* bucket names are needed because the handler branches on the triggering bucket's **name**: both originals buckets use the same `uploads/` key prefix ([`media.md` §11.2](../../../docs/design/media.md#112-upload--processing), [`infrastructure.md` §5.3](../../../docs/design/infrastructure.md#53-lambda-image-process-rust-mediapipelinestack)).
