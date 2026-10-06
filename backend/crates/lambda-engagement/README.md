# lambda-engagement

Post-publication comments and emoji reactions
(`plans/03-api-contract.md` §8, `plans/09-engagement.md`). Every route is
nested under `/groups/{groupId}/newsletters/{cycleId}/questions/{questionId}/responses/{responseId}/...`
and requires the cycle to be `published`; the answer must be a published
`text` response (poll answers are never engagement targets).

Comments: `GET`/`POST .../comments`, `PATCH`/`DELETE .../comments/{commentId}`.
Reactions: `GET .../reactions`, `PUT`/`DELETE .../reactions/{emoji}`.

Binary: `engagement-api`.
