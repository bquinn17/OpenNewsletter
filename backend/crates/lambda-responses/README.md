# lambda-responses

Draft save, publish, and poll-vote routes for member answers
(`plans/03-api-contract.md` §7). Last-write-wins on content — no version
tokens, no conflict errors (`plans/02-data-model-dynamodb.md` §5). Publishing
is sticky: once a response is published, a later autosave with `publish=false`
still saves the new content but cannot unpublish it. Validates image-attachment
ownership, `ready` status, and `purpose=response` before allowing a save.
