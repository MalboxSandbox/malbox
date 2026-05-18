//! Provisioner imports for inventory registration.
//!
//! This module ensures that enabled provisioner crates are linked into the binary,
//! which allows their `inventory::submit!` calls to register provisioners in the
//! global registry.
//!
//! Provisioners are enabled via Cargo features:
//! - `provisioner-ansible` - Ansible provisioner

#![allow(unused_imports)]

#[cfg(feature = "provisioner-ansible")]
use malbox_provisioner_ansible as _;
