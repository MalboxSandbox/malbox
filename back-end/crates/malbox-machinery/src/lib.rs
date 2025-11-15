//! Malbox crate that contains interfaces related to machinery (providers, provisioners, etc.)
//!
//! This crate contains machinery functionalities and provider interfaces. Actual implementations
//! are found in the different `malbox-provider-*` and `malbox-provisioner-*` crates.
//!
//! # Type Erasure Pattern
//!
//! The core `Provider` and `Machine` traits use associated types to maintain type safety and
//! zero-cost abstractions at compile time. However, associated types prevent these traits from
//! being object-safe, which means they cannot be used with dynamic dispatch (`dyn Trait`).
//!
//! To enable dynamic dispatch while preserving the strongly-typed interface, this crate provides
//! parallel object-safe traits `DynProvider` and `DynMachine`. These traits use concrete error
//! types (`Box<dyn Error>`) instead of associated types, making them object-safe.
//!
//! The connection between the two trait hierarchies is established through blanket implementations.
//! Every type implementing `Machine` automatically implements `DynMachine`, and every type
//! implementing `Provider` can be wrapped in `ProviderWrapper` to implement `DynProvider`.
//!
//! This pattern allows provider implementations to use the strongly-typed `Provider` and `Machine`
//! traits directly, while higher-level orchestration code can work with `dyn DynProvider` and
//! `dyn DynMachine` without knowing concrete types at compile time. The `ProviderWrapper` handles
//! downcasting when necessary, particularly for operations like deallocation that require passing
//! the original concrete machine type back to the provider.

pub mod machine;
pub mod provider;
pub mod provisioner;

// Re-export commonly used types for convenience
pub use machine::{DynMachine, Machine, MachineEndpoint, MachineId, MachineSpec, MachineState};
pub use provider::{DynProvider, Provider, ProviderWrapper};
