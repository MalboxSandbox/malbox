//! Plugin type definitions and their associated transports.

use crate::transport::plugin::{GrpcEmitter, GrpcReceiver};
use crate::transport::ipc::{EventEmitter, EventReceiver};
use crate::transport::traits::{TransportEmitter, TransportReceiver};

/// Marker trait linking plugin type to its transport emitter and receiver.
///
/// Each plugin type specifies which transport backend it uses for sending
/// and receiving events. Host plugins use IPC (iceoryx2), guest plugins
/// will use gRPC.
pub trait PluginType {
    /// The emitter type used to send events.
    type Emitter: TransportEmitter;

    /// The receiver type used to receive events.
    type Receiver: TransportReceiver;
}

/// Host plugin type — uses iceoryx2 IPC.
///
/// Host plugins run on the same machine as the daemon and communicate
/// via zero-copy shared memory.
pub enum Host {}

impl PluginType for Host {
    type Emitter = EventEmitter;
    type Receiver = EventReceiver;
}

/// Guest plugin type — will use gRPC.
///
/// Guest plugins run inside virtual machines and communicate
/// via gRPC over the network.
pub enum Guest {}

impl PluginType for Guest {
    type Emitter = GrpcEmitter;
    type Receiver = GrpcReceiver;
}
