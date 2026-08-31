pub mod control;
pub mod jitter;
pub mod render;
pub mod task_query;
pub mod update_cli;
pub mod updater;
mod updater_archive;
#[cfg(target_os = "windows")]
pub mod updater_native;

#[cfg(test)]
#[path = "updater_test_support.rs"]
mod updater_test_support;

#[cfg(test)]
#[path = "updater_tests.rs"]
mod updater_tests;
