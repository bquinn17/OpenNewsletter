//! Upload, avatar, and CloudFront signed-cookie routes (`plans/03-api-contract.md`
//! §9). Split into a library so integration tests can call the handlers directly.

pub mod cloudfront;
pub mod handlers;
pub mod presign;
pub mod router;
pub mod state;
pub mod validation;
