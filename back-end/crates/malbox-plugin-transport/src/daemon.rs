//! Daemon-side transport types.
//!
//! Contains the gRPC client used by the daemon to communicate with guest
//! plugins running inside virtual machines.

mod grpc;

pub use grpc::GrpcClient;
