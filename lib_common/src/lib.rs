// #![feature(exit_status_error)]

// Re-export the `beep` function from the `actually_beep` crate
pub use actually_beep::beep_with_hz_and_millis;

// Declare the modules to re-export
pub mod config_cloud;
pub mod config_sys;
pub mod loggers; // New parent module for logrecord and loggerlocal
pub mod utils;   // New parent module for sys_info and utils
pub mod retrieve; // New parent module for ky_http
pub mod markets;  // New parent module for nasdaq::apicall

// Re-export everything
pub use config_cloud::*;
pub use config_sys::*;
pub use loggers::logrecord::*;
pub use loggers::loggerlocal::*;
pub use utils::misc::sys_info::*;
pub use utils::misc::utils::*;
pub use retrieve::ky_http::*;
pub use markets::nasdaq::apicall::*;
pub use markets::cnn::fearandgreed::*;

// pub fn add(left: u64, right: u64) -> u64 {
//     left + right
// }

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn it_works() {
//         let result = add(2, 2);
//         assert_eq!(result, 4);
//     }
// }

pub mod core;
pub mod ingestors;

// This allows lib_common::VERSION access
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
