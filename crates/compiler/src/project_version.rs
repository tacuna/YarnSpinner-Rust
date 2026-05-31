//! Version constants for the Yarn Spinner project file format.
//!
//! These are kept in their own module so that they are always available,
//! regardless of whether the `serde` feature is enabled (the main
//! [`crate::project`] module requires `serde`).

/// A version number representing Yarn Spinner 2
pub const YARNSPINNER_PROJECT_VERSION_2: u32 = 2;

/// A version number representing Yarn Spinner 3
pub const YARNSPINNER_PROJECT_VERSION_3: u32 = 3;

/// A version number representing project version 4 (released with Yarn Spinner 3.2.0)
pub const YARNSPINNER_PROJECT_VERSION_4: u32 = 4;

/// Current version of the .yarnproject
pub const CURRENT_PROJECT_FILE_VERSION: u32 = YARNSPINNER_PROJECT_VERSION_4;
