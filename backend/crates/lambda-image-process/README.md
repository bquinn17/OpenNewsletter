# lambda-image-process

S3 `ObjectCreated` trigger. Reads originals, generates display + thumbnail WebP/GIF, writes to the processed bucket, marks the DynamoDB row `ready`. See [`../../../plans/08-media-uploads.md`](../../../plans/08-media-uploads.md).

## Environment

`TABLE_NAME`, `MEDIA_ORIGINALS_BUCKET`, `AVATARS_ORIGINALS_BUCKET`, `PROCESSED_BUCKET`, `AVATARS_PROCESSED_BUCKET`.

The two *originals* bucket names are needed because the handler branches on the triggering bucket's **name**: both originals buckets use the same `uploads/` key prefix (`plans/08-media-uploads.md` §11.2, `plans/01-infrastructure-cdk.md` §5.3).
