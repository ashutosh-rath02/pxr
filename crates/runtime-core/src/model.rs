pub const MAX_CAPABILITIES: usize = 16;
pub const MAX_RESOURCES: usize = 8;
pub const REPLAY_CAPACITY: usize = 32;
pub const RECEIPT_CAPACITY: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub min: i32,
    pub max: i32,
}
impl Bounds {
    pub const ANY: Self = Self {
        min: i32::MIN,
        max: i32::MAX,
    };
    pub const ZERO: Self = Self { min: 0, max: 0 };
    pub const fn contains(self, value: i32) -> bool {
        value >= self.min && value <= self.max
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionClass {
    Stream,
    Discrete,
    Control,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verification {
    /// Driver observation must equal the requested parameters.
    Parameters,
    /// Driver observation must satisfy this exact masked state.
    State { mask: u32, value: u32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capability {
    pub id: u16,
    pub resource: u8,
    pub class: ExecutionClass,
    pub parameter_count: u8,
    pub bounds: [Bounds; 2],
    pub require_set: u32,
    pub require_clear: u32,
    pub verification: Verification,
}
impl Capability {
    pub fn permits(self, flags: u32) -> bool {
        flags & self.require_set == self.require_set && flags & self.require_clear == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Action {
    pub boot_id: u64,
    pub requester: u64,
    pub lease_id: u64,
    pub action_id: u64,
    pub sequence: u64,
    pub capability: u16,
    pub based_on_epoch: u64,
    pub execute_after: u64,
    pub deadline: u64,
    pub valid_for_ms: u32,
    pub parameters: [i32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeaseRequest {
    pub owner: u64,
    pub resource: u8,
    /// Bit N authorizes capability ID N. IDs are in 1..64.
    pub capabilities: u64,
    pub duration_ms: u32,
    pub limits: [Bounds; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lease {
    pub id: u64,
    pub owner: u64,
    pub resource: u8,
    pub capabilities: u64,
    pub expires_at: u64,
    pub limits: [Bounds; 2],
    pub last_sequence: u64,
    pub last_renewal: u64,
}

#[repr(u16)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    Ok = 0,
    Bound = 1,
    Stale = 2,
    Authority = 3,
    Duplicate = 4,
    Precondition = 5,
    OldSequence = 6,
    Epoch = 7,
    UnknownCapability = 8,
    TooEarly = 9,
    Invalid = 10,
    Estop = 11,
    Faulted = 12,
    Watchdog = 13,
    LeaseExpired = 14,
    StreamExpired = 15,
    Driver = 16,
    Verification = 17,
    Clock = 18,
    Busy = 19,
    Canceled = 20,
    InvalidStorm = 21,
    StateStale = 22,
    BootMismatch = 23,
    LeaseGranted = 24,
    LeaseRenewed = 25,
    Recovered = 26,
    Startup = 27,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Executed = 0,
    Rejected = 1,
    Fallback = 2,
    Control = 3,
    Failed = 4,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeState {
    SafeIdle = 0,
    Armed = 1,
    Faulted = 2,
    Estopped = 3,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Observation {
    pub parameters: [i32; 2],
    pub state: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DriverError;

/// Every operation must be synchronous, bounded, non-blocking and non-reentrant.
/// The platform supplies a hardware watchdog if this thread or driver stops running.
pub trait Driver {
    fn observe(&mut self, resource: u8) -> Result<Observation, DriverError>;
    fn execute(&mut self, capability: u16, parameters: [i32; 2]) -> Result<(), DriverError>;
    fn fallback(&mut self, resource: u8, reason: Reason) -> Result<(), DriverError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Receipt {
    pub receipt_id: u64,
    pub action_id: u64,
    pub sequence: u64,
    pub capability: u16,
    pub resource: u8,
    pub lease_id: u64,
    pub decision: Decision,
    pub reason: Reason,
    pub epoch: u64,
    pub received_at: u64,
    pub admitted_at: u64,
    pub executed_at: u64,
    pub requested: [i32; 2],
    pub observed: [i32; 2],
    pub pre_state: u32,
    pub post_state: u32,
    /// Original executed receipt for a duplicate; zero otherwise.
    pub original_receipt: u64,
    pub dispatched: bool,
    pub observed_valid: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub watchdog_ms: u32,
    pub state_ttl_ms: u32,
    pub max_lease_ms: u32,
    pub max_valid_for_ms: u32,
    pub invalid_limit: u16,
    pub estop_mask: u32,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            watchdog_ms: 50,
            state_ttl_ms: 250,
            max_lease_ms: 2000,
            max_valid_for_ms: 1000,
            invalid_limit: 32,
            estop_mask: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    InvalidConfig,
    InvalidCapability,
    DuplicateCapability,
    InvalidBootId,
    Driver,
}
