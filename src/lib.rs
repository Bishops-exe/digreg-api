#![forbid(unsafe_code)]

pub mod school;
pub mod routes;
pub mod session;
mod utils;

pub use school::{School, SchoolCreationError, SchoolLoginError};
pub use session::Session;
