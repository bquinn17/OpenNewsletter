# lambda-image-process

S3 `ObjectCreated` trigger. Reads originals, generates display + thumbnail WebP/GIF, writes to the processed bucket, marks the DynamoDB row `ready`. See [`../../../plans/08-media-uploads.md`](../../../plans/08-media-uploads.md).
