// #![feature(exit_status_error)]

// Re-export the `beep` function from the `actually_beep` crate
pub use actually_beep::beep_with_hz_and_millis;

// Declare the modules to re-export
pub mod config_cloud;
pub mod config_sys;
pub mod logrecord;
pub mod sys_info;

// Re-export everything
pub use config_cloud::*;
pub use config_sys::*;
pub use logrecord::*;
pub use sys_info::*;

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
