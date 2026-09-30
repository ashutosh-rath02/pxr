//! C ABI: caller owns storage and serialization; see include/pxr.h for the contract.
#![cfg_attr(not(feature = "std"), no_std)]
use core::{
    ffi::c_void,
    mem::{align_of, size_of},
    ptr, slice,
};
use pxr_runtime_core::{profile::CAPABILITIES, *};

#[cfg(not(feature = "std"))]
#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CObservation {
    pub parameters: [i32; 2],
    pub state: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Callbacks {
    pub user: *mut c_void,
    pub observe: Option<unsafe extern "C" fn(*mut c_void, u8, *mut CObservation) -> i32>,
    pub execute: Option<unsafe extern "C" fn(*mut c_void, u16, i32, i32) -> i32>,
    pub fallback: Option<unsafe extern "C" fn(*mut c_void, u8, u16) -> i32>,
}
impl Driver for Callbacks {
    fn observe(&mut self, resource: u8) -> Result<Observation, DriverError> {
        let mut out = CObservation::default();
        let callback = self.observe.ok_or(DriverError)?;
        // SAFETY: callback lifetime and user pointer are the embedding application's contract.
        if unsafe { callback(self.user, resource, &mut out) } != 0 {
            return Err(DriverError);
        }
        Ok(Observation {
            parameters: out.parameters,
            state: out.state,
        })
    }
    fn execute(&mut self, capability: u16, parameters: [i32; 2]) -> Result<(), DriverError> {
        let callback = self.execute.ok_or(DriverError)?;
        // SAFETY: see Callbacks contract in include/pxr.h.
        if unsafe { callback(self.user, capability, parameters[0], parameters[1]) } == 0 {
            Ok(())
        } else {
            Err(DriverError)
        }
    }
    fn fallback(&mut self, resource: u8, reason: Reason) -> Result<(), DriverError> {
        let callback = self.fallback.ok_or(DriverError)?;
        // SAFETY: see Callbacks contract in include/pxr.h.
        if unsafe { callback(self.user, resource, reason as u16) } == 0 {
            Ok(())
        } else {
            Err(DriverError)
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CCapability {
    pub id: u16,
    pub resource: u8,
    pub class: u8,
    pub parameter_count: u8,
    pub verification: u8,
    pub reserved: u16,
    pub min: [i32; 2],
    pub max: [i32; 2],
    pub require_set: u32,
    pub require_clear: u32,
    pub verify_mask: u32,
    pub verify_value: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CLease {
    pub id: u64,
    pub expires_at: u64,
    pub epoch: u64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CReceipt {
    pub receipt_id: u64,
    pub action_id: u64,
    pub sequence: u64,
    pub lease_id: u64,
    pub epoch: u64,
    pub received_at: u64,
    pub admitted_at: u64,
    pub executed_at: u64,
    pub original_receipt: u64,
    pub requested: [i32; 2],
    pub observed: [i32; 2],
    pub pre_state: u32,
    pub post_state: u32,
    pub capability: u16,
    pub reason: u16,
    pub decision: u8,
    pub resource: u8,
    pub dispatched: u8,
    pub observed_valid: u8,
}
impl From<Receipt> for CReceipt {
    fn from(r: Receipt) -> Self {
        Self {
            receipt_id: r.receipt_id,
            action_id: r.action_id,
            sequence: r.sequence,
            lease_id: r.lease_id,
            epoch: r.epoch,
            received_at: r.received_at,
            admitted_at: r.admitted_at,
            executed_at: r.executed_at,
            original_receipt: r.original_receipt,
            requested: r.requested,
            observed: r.observed,
            pre_state: r.pre_state,
            post_state: r.post_state,
            capability: r.capability,
            reason: r.reason as u16,
            decision: r.decision as u8,
            resource: r.resource,
            dispatched: r.dispatched as u8,
            observed_valid: r.observed_valid as u8,
        }
    }
}
struct Context {
    runtime: Runtime,
    driver: Callbacks,
}
fn valid_pointer<T>(p: *const T) -> bool {
    !p.is_null() && (p as usize) % align_of::<T>() == 0
}

#[no_mangle]
pub extern "C" fn pxr_abi_version() -> u32 {
    1
}
#[no_mangle]
pub extern "C" fn pxr_context_size() -> usize {
    size_of::<Context>()
}
#[no_mangle]
pub extern "C" fn pxr_context_align() -> usize {
    align_of::<Context>()
}

/// Initialize the motor/gripper reference profile.
/// # Safety
/// `storage` must be aligned writable memory of `length` bytes, and callbacks must remain valid.
#[no_mangle]
pub unsafe extern "C" fn pxr_init(
    storage: *mut c_void,
    length: usize,
    boot_id: u64,
    now: u64,
    initial_flags: u32,
    callbacks: Callbacks,
) -> i32 {
    // SAFETY: requirements are forwarded to initialize.
    unsafe {
        initialize(
            storage,
            length,
            boot_id,
            now,
            initial_flags,
            callbacks,
            &CAPABILITIES,
        )
    }
}
unsafe fn initialize(
    storage: *mut c_void,
    length: usize,
    boot_id: u64,
    now: u64,
    initial_flags: u32,
    mut callbacks: Callbacks,
    caps: &[Capability],
) -> i32 {
    if !valid_pointer(storage.cast::<Context>())
        || length < size_of::<Context>()
        || callbacks.observe.is_none()
        || callbacks.execute.is_none()
        || callbacks.fallback.is_none()
    {
        return -1;
    }
    let runtime = match Runtime::new(
        Config::default(),
        caps,
        boot_id,
        now,
        initial_flags,
        &mut callbacks,
    ) {
        Ok(r) => r,
        Err(_) => return -3,
    };
    // SAFETY: caller supplies writable storage; alignment and size checked above. Context has no Drop.
    unsafe {
        ptr::write(
            storage.cast::<Context>(),
            Context {
                runtime,
                driver: callbacks,
            },
        );
    }
    0
}

/// Initialize a custom static capability profile. Specs are copied into fixed runtime storage.
/// # Safety
/// Same as pxr_init; specs must reference count readable, aligned entries, without aliasing storage.
#[no_mangle]
pub unsafe extern "C" fn pxr_init_custom(
    storage: *mut c_void,
    length: usize,
    boot_id: u64,
    now: u64,
    initial_flags: u32,
    callbacks: Callbacks,
    specs: *const CCapability,
    count: usize,
) -> i32 {
    if !valid_pointer(specs) || count == 0 || count > MAX_CAPABILITIES {
        return -1;
    }
    // SAFETY: caller guarantees readable entries; length has a fixed upper bound.
    let input = unsafe { slice::from_raw_parts(specs, count) };
    let mut caps = [CAPABILITIES[0]; MAX_CAPABILITIES];
    for (out, spec) in caps.iter_mut().zip(input) {
        if spec.reserved != 0 {
            return -3;
        }
        let class = match spec.class {
            0 => ExecutionClass::Stream,
            1 => ExecutionClass::Discrete,
            2 => ExecutionClass::Control,
            _ => return -3,
        };
        let verification = match spec.verification {
            0 => Verification::Parameters,
            1 => Verification::State {
                mask: spec.verify_mask,
                value: spec.verify_value,
            },
            _ => return -3,
        };
        *out = Capability {
            id: spec.id,
            resource: spec.resource,
            class,
            parameter_count: spec.parameter_count,
            bounds: [
                Bounds {
                    min: spec.min[0],
                    max: spec.max[0],
                },
                Bounds {
                    min: spec.min[1],
                    max: spec.max[1],
                },
            ],
            require_set: spec.require_set,
            require_clear: spec.require_clear,
            verification,
        };
    }
    // SAFETY: forwarded caller storage requirements; capabilities are copied by Runtime::new.
    unsafe {
        initialize(
            storage,
            length,
            boot_id,
            now,
            initial_flags,
            callbacks,
            &caps[..count],
        )
    }
}

/// Grant one resource lease. Bounds further narrow the registered capability limits.
/// # Safety
/// ctx is initialized exclusive storage; out is a separate writable aligned CLease.
#[no_mangle]
pub unsafe extern "C" fn pxr_acquire(
    ctx: *mut c_void,
    owner: u64,
    resource: u8,
    capabilities: u64,
    duration_ms: u32,
    min0: i32,
    max0: i32,
    min1: i32,
    max1: i32,
    now: u64,
    out: *mut CLease,
) -> i32 {
    if !valid_pointer(ctx.cast::<Context>()) || !valid_pointer(out) {
        return -1;
    }
    // SAFETY: caller guarantees initialized exclusive context and separate output.
    let ctx = unsafe { &mut *ctx.cast::<Context>() };
    match ctx.runtime.acquire(
        LeaseRequest {
            owner,
            resource,
            capabilities,
            duration_ms,
            limits: [
                Bounds {
                    min: min0,
                    max: max0,
                },
                Bounds {
                    min: min1,
                    max: max1,
                },
            ],
        },
        now,
        &mut ctx.driver,
    ) {
        Ok(l) => {
            unsafe {
                ptr::write(
                    out,
                    CLease {
                        id: l.id,
                        expires_at: l.expires_at,
                        epoch: ctx.runtime.epoch(),
                    },
                );
            }
            0
        }
        Err(e) => e as i32,
    }
}

/// Decode and execute a single fixed-size action. Zero means receipt produced, not action executed.
/// # Safety
/// ctx is initialized exclusive storage; frame is readable length bytes, out writable and separate.
#[no_mangle]
pub unsafe extern "C" fn pxr_submit(
    ctx: *mut c_void,
    frame: *const u8,
    length: usize,
    principal: u64,
    received_at: u64,
    now: u64,
    out: *mut CReceipt,
) -> i32 {
    if !valid_pointer(ctx.cast::<Context>()) || frame.is_null() || !valid_pointer(out) {
        return -1;
    }
    if length != pxr_runtime_codec::FRAME_SIZE {
        return -2;
    }
    // SAFETY: frame has the validated fixed length and readable storage per caller contract.
    let action = match pxr_runtime_codec::decode(unsafe { slice::from_raw_parts(frame, length) }) {
        Ok(a) => a,
        Err(_) => return -2,
    };
    // SAFETY: caller guarantees initialized exclusive context and non-aliasing output.
    let ctx = unsafe { &mut *ctx.cast::<Context>() };
    let receipt = ctx
        .runtime
        .submit(action, principal, received_at, now, &mut ctx.driver);
    unsafe {
        ptr::write(out, receipt.into());
    }
    0
}

/// # Safety
/// ctx is initialized exclusive storage. Call independently of ingress, at least every 50 ms.
#[no_mangle]
pub unsafe extern "C" fn pxr_tick(ctx: *mut c_void, now: u64) -> i32 {
    if !valid_pointer(ctx.cast::<Context>()) {
        return -1;
    }
    let c = unsafe { &mut *ctx.cast::<Context>() };
    c.runtime.tick(now, &mut c.driver);
    c.runtime.state() as i32
}
/// # Safety
/// ctx is initialized exclusive storage; flags are from trusted local sensors.
#[no_mangle]
pub unsafe extern "C" fn pxr_update_state(ctx: *mut c_void, flags: u32, now: u64) -> i32 {
    if !valid_pointer(ctx.cast::<Context>()) {
        return -1;
    }
    let c = unsafe { &mut *ctx.cast::<Context>() };
    c.runtime.update_state(flags, now, &mut c.driver);
    c.runtime.state() as i32
}
/// # Safety
/// ctx is initialized exclusive storage. This local API must bypass remote action queues.
#[no_mangle]
pub unsafe extern "C" fn pxr_estop(ctx: *mut c_void, now: u64) -> i32 {
    if !valid_pointer(ctx.cast::<Context>()) {
        return -1;
    }
    let c = unsafe { &mut *ctx.cast::<Context>() };
    c.runtime.emergency_stop(now, &mut c.driver);
    0
}
/// # Safety
/// ctx is initialized exclusive storage. Only trusted local recovery may call this function.
#[no_mangle]
pub unsafe extern "C" fn pxr_recover_local(ctx: *mut c_void, now: u64) -> i32 {
    if !valid_pointer(ctx.cast::<Context>()) {
        return -1;
    }
    let c = unsafe { &mut *ctx.cast::<Context>() };
    c.runtime
        .recover_local(now, &mut c.driver)
        .map_or_else(|e| e as i32, |_| 0)
}
/// # Safety
/// ctx is initialized exclusive storage; out is a separate aligned writable CLease.
#[no_mangle]
pub unsafe extern "C" fn pxr_renew(
    ctx: *mut c_void,
    owner: u64,
    lease: u64,
    renewal: u64,
    duration: u32,
    now: u64,
    out: *mut CLease,
) -> i32 {
    if !valid_pointer(ctx.cast::<Context>()) || !valid_pointer(out) {
        return -1;
    }
    let c = unsafe { &mut *ctx.cast::<Context>() };
    match c
        .runtime
        .renew(owner, lease, renewal, duration, now, &mut c.driver)
    {
        Ok(l) => {
            unsafe {
                ptr::write(
                    out,
                    CLease {
                        id: l.id,
                        expires_at: l.expires_at,
                        epoch: c.runtime.epoch(),
                    },
                );
            }
            0
        }
        Err(e) => e as i32,
    }
}
/// # Safety
/// ctx is initialized exclusive storage; owner must be established by the authority adapter.
#[no_mangle]
pub unsafe extern "C" fn pxr_cancel(ctx: *mut c_void, owner: u64, lease: u64, now: u64) -> i32 {
    if !valid_pointer(ctx.cast::<Context>()) {
        return -1;
    }
    let c = unsafe { &mut *ctx.cast::<Context>() };
    c.runtime
        .cancel(owner, lease, now, &mut c.driver)
        .map_or_else(|e| e as i32, |_| 0)
}
/// # Safety
/// ctx is initialized readable storage; it must not be concurrently mutated.
#[no_mangle]
pub unsafe extern "C" fn pxr_epoch(ctx: *const c_void) -> u64 {
    if !valid_pointer(ctx.cast::<Context>()) {
        return 0;
    }
    unsafe { (&*ctx.cast::<Context>()).runtime.epoch() }
}
/// # Safety
/// ctx is initialized readable storage; out is aligned writable and does not alias it.
#[no_mangle]
pub unsafe extern "C" fn pxr_receipt(ctx: *const c_void, index: usize, out: *mut CReceipt) -> i32 {
    if !valid_pointer(ctx.cast::<Context>()) || !valid_pointer(out) {
        return -1;
    }
    let c = unsafe { &*ctx.cast::<Context>() };
    match c.runtime.receipt(index) {
        Some(r) => {
            unsafe {
                ptr::write(out, r.into());
            }
            0
        }
        None => -4,
    }
}
