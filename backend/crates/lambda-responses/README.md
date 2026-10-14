# lambda-responses

Draft save, publish, and poll-vote routes for member answers
([`api.md` §7](../../../docs/design/api.md#7-response-routes-drafts--publish)). Last-write-wins on content — no version
tokens, no conflict errors ([`data-model.md` §5](../../../docs/design/data-model.md#5-concurrency-for-drafts)). Publishing
is sticky: once a response is published, a later autosave with `publish=false`
still saves the new content but cannot unpublish it. Validates image-attachment
ownership, `ready` status, and `purpose=response` before allowing a save.
