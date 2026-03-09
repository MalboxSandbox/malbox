//! Individual handler attribute macros.
//!
//! These are now pass-through — the actual trait generation happens in
//! `#[malbox::handlers]` (handlers_impl.rs) which scans the impl block.
//! These macros exist so `#[malbox::on_task]` etc. can be used standalone
//! if needed (they just pass through the method unchanged).
