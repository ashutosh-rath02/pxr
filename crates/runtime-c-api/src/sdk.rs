use super::*;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CAction {
    pub boot_id: u64,
    pub requester: u64,
    pub lease_id: u64,
    pub action_id: u64,
    pub sequence: u64,
    pub based_on_epoch: u64,
    pub execute_after: u64,
    pub deadline: u64,
    pub valid_for_ms: u32,
    pub parameters: [i32; 2],
    pub capability: u16,
    pub reserved: u16,
}
impl From<CAction> for Action {
    fn from(a: CAction) -> Self {
        Self {
            boot_id: a.boot_id,
            requester: a.requester,
            lease_id: a.lease_id,
            action_id: a.action_id,
            sequence: a.sequence,
            based_on_epoch: a.based_on_epoch,
            execute_after: a.execute_after,
            deadline: a.deadline,
            valid_for_ms: a.valid_for_ms,
            parameters: a.parameters,
            capability: a.capability,
        }
    }
}
impl From<Action> for CAction {
    fn from(a: Action) -> Self {
        Self {
            boot_id: a.boot_id,
            requester: a.requester,
            lease_id: a.lease_id,
            action_id: a.action_id,
            sequence: a.sequence,
            based_on_epoch: a.based_on_epoch,
            execute_after: a.execute_after,
            deadline: a.deadline,
            valid_for_ms: a.valid_for_ms,
            parameters: a.parameters,
            capability: a.capability,
            reserved: 0,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CConfig {
    pub watchdog_ms: u32,
    pub state_ttl_ms: u32,
    pub max_lease_ms: u32,
    pub max_valid_for_ms: u32,
    pub estop_mask: u32,
    pub invalid_limit: u16,
    pub reserved: u16,
}
impl From<Config> for CConfig {
    fn from(c: Config) -> Self {
        Self {
            watchdog_ms: c.watchdog_ms,
            state_ttl_ms: c.state_ttl_ms,
            max_lease_ms: c.max_lease_ms,
            max_valid_for_ms: c.max_valid_for_ms,
            estop_mask: c.estop_mask,
            invalid_limit: c.invalid_limit,
            reserved: 0,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CSnapshot {
    pub boot_id: u64,
    pub epoch: u64,
    pub now: u64,
    pub sensor_tick: u64,
    pub next_receipt_id: u64,
    pub flags: u32,
    pub retained_receipts: u16,
    pub state: u8,
    pub active_leases: u8,
}
impl From<Capability> for CCapability {
    fn from(c: Capability) -> Self {
        let (verification, verify_mask, verify_value) = match c.verification {
            Verification::Parameters => (0, 0, 0),
            Verification::State { mask, value } => (1, mask, value),
        };
        Self {
            id: c.id,
            resource: c.resource,
            class: match c.class {
                ExecutionClass::Stream => 0,
                ExecutionClass::Discrete => 1,
                ExecutionClass::Control => 2,
            },
            parameter_count: c.parameter_count,
            verification,
            reserved: 0,
            min: [c.bounds[0].min, c.bounds[1].min],
            max: [c.bounds[0].max, c.bounds[1].max],
            require_set: c.require_set,
            require_clear: c.require_clear,
            verify_mask,
            verify_value,
        }
    }
}

/// Returns defaults that callers can customize before initialization.
/// # Safety
/// out points to aligned writable CConfig storage.
#[no_mangle]
pub unsafe extern "C" fn pxr_default_config(out: *mut CConfig) -> i32 {
    if !valid_pointer(out) {
        return -1;
    }
    unsafe {
        ptr::write(out, Config::default().into());
    }
    0
}

/// Initialize with explicit configuration and optionally a custom profile (count=0 uses reference).
/// # Safety
/// Storage, callback and aliasing requirements match pxr_init. config and specs are readable.
#[no_mangle]
pub unsafe extern "C" fn pxr_init_config(
    storage: *mut c_void,
    length: usize,
    boot_id: u64,
    now: u64,
    initial_flags: u32,
    callbacks: Callbacks,
    config: *const CConfig,
    specs: *const CCapability,
    count: usize,
) -> i32 {
    if !valid_pointer(config) || count > MAX_CAPABILITIES {
        return -1;
    }
    let c = unsafe { *config };
    if c.reserved != 0 {
        return -3;
    }
    let config = Config {
        watchdog_ms: c.watchdog_ms,
        state_ttl_ms: c.state_ttl_ms,
        max_lease_ms: c.max_lease_ms,
        max_valid_for_ms: c.max_valid_for_ms,
        estop_mask: c.estop_mask,
        invalid_limit: c.invalid_limit,
    };
    if count == 0 {
        return unsafe {
            initialize(
                storage,
                length,
                boot_id,
                now,
                initial_flags,
                callbacks,
                &CAPABILITIES,
                config,
            )
        };
    }
    if !valid_pointer(specs) {
        return -1;
    }
    let specs = unsafe { slice::from_raw_parts(specs, count) };
    let mut caps = [CAPABILITIES[0]; MAX_CAPABILITIES];
    for (out, c) in caps.iter_mut().zip(specs) {
        if c.reserved != 0 {
            return -3;
        }
        let class = match c.class {
            0 => ExecutionClass::Stream,
            1 => ExecutionClass::Discrete,
            2 => ExecutionClass::Control,
            _ => return -3,
        };
        let verification = match c.verification {
            0 => Verification::Parameters,
            1 => Verification::State {
                mask: c.verify_mask,
                value: c.verify_value,
            },
            _ => return -3,
        };
        *out = Capability {
            id: c.id,
            resource: c.resource,
            class,
            parameter_count: c.parameter_count,
            bounds: [
                Bounds {
                    min: c.min[0],
                    max: c.max[0],
                },
                Bounds {
                    min: c.min[1],
                    max: c.max[1],
                },
            ],
            require_set: c.require_set,
            require_clear: c.require_clear,
            verification,
        };
    }
    unsafe {
        initialize(
            storage,
            length,
            boot_id,
            now,
            initial_flags,
            callbacks,
            &caps[..count],
            config,
        )
    }
}

/// # Safety
/// action is aligned readable storage, out has exactly 92 writable bytes and does not overlap action.
#[no_mangle]
pub unsafe extern "C" fn pxr_encode_action(
    action: *const CAction,
    out: *mut u8,
    length: usize,
) -> i32 {
    if !valid_pointer(action) || out.is_null() {
        return -1;
    }
    if length != pxr_runtime_codec::FRAME_SIZE {
        return -2;
    }
    let action = unsafe { *action };
    if action.reserved != 0 {
        return -3;
    }
    let bytes = pxr_runtime_codec::encode(&action.into());
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), out, length);
    }
    0
}
/// # Safety
/// frame is readable length bytes; out is aligned writable non-overlapping storage.
#[no_mangle]
pub unsafe extern "C" fn pxr_decode_action(
    frame: *const u8,
    length: usize,
    out: *mut CAction,
) -> i32 {
    if frame.is_null() || !valid_pointer(out) {
        return -1;
    }
    if length != pxr_runtime_codec::FRAME_SIZE {
        return -2;
    }
    let action = match pxr_runtime_codec::decode(unsafe { slice::from_raw_parts(frame, length) }) {
        Ok(a) => a,
        Err(_) => return -2,
    };
    unsafe {
        ptr::write(out, action.into());
    }
    0
}
/// Snapshot is a read-only view; it does not service the supervisor or refresh sensors.
/// # Safety
/// ctx is initialized readable storage; out is separate aligned writable storage. No concurrent mutation.
#[no_mangle]
pub unsafe extern "C" fn pxr_snapshot(ctx: *const c_void, out: *mut CSnapshot) -> i32 {
    if !valid_pointer(ctx.cast::<Context>()) || !valid_pointer(out) {
        return -1;
    }
    let s = unsafe { (&*ctx.cast::<Context>()).runtime.snapshot() };
    unsafe {
        ptr::write(
            out,
            CSnapshot {
                boot_id: s.boot_id,
                epoch: s.epoch,
                now: s.now,
                sensor_tick: s.sensor_tick,
                next_receipt_id: s.next_receipt_id,
                flags: s.flags,
                retained_receipts: s.retained_receipts as u16,
                state: s.state as u8,
                active_leases: s.active_leases,
            },
        );
    }
    0
}
/// # Safety
/// ctx is initialized readable storage; out is separate aligned writable storage. No concurrent mutation.
#[no_mangle]
pub unsafe extern "C" fn pxr_get_capability(
    ctx: *const c_void,
    index: usize,
    out: *mut CCapability,
) -> i32 {
    if !valid_pointer(ctx.cast::<Context>()) || !valid_pointer(out) {
        return -1;
    }
    let c = unsafe { &*ctx.cast::<Context>() };
    match c.runtime.capabilities().nth(index) {
        Some(cap) => {
            unsafe {
                ptr::write(out, (*cap).into());
            }
            0
        }
        None => -4,
    }
}
/// Export one retained receipt as a canonical 140-byte frame including authority context.
/// # Safety
/// ctx is initialized readable storage; out is separate writable length bytes. No concurrent mutation.
#[no_mangle]
pub unsafe extern "C" fn pxr_receipt_frame(
    ctx: *const c_void,
    index: usize,
    out: *mut u8,
    length: usize,
) -> i32 {
    if !valid_pointer(ctx.cast::<Context>()) || out.is_null() {
        return -1;
    }
    if length != pxr_runtime_codec::RECEIPT_FRAME_SIZE {
        return -2;
    }
    let c = unsafe { &*ctx.cast::<Context>() };
    match c.runtime.receipt(index) {
        Some(r) => {
            let frame = pxr_runtime_codec::encode_receipt(&r);
            unsafe {
                ptr::copy_nonoverlapping(frame.as_ptr(), out, length);
            }
            0
        }
        None => -4,
    }
}
