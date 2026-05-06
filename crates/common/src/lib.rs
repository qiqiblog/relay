pub mod logging;
pub mod types;

pub use types::*;

/// Wire protocol version between master and node.
/// - 0/unset = legacy (M8 and earlier, Rule-based)
/// - 2 = M9 (Forward/ForwardConfig/ForwardStats)
pub const PROTOCOL_VERSION: u32 = 2;
