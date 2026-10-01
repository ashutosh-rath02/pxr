//! Exercises the C entry points from Rust so Miri can check every unsafe path they take.
use super::*;
use core::ffi::c_void;
use pxr_runtime_core::profile::{DRIVE, STOP};
use std::boxed::Box;

#[repr(C, align(16))]
struct Storage([u8; 16384]);

#[derive(Default)]
struct Device {
    velocity: [i32; 2],
    executions: u32,
    fallbacks: u32,
}

unsafe extern "C" fn observe(user: *mut c_void, resource: u8, out: *mut CObservation) -> i32 {
    // SAFETY: tests pass a live Device and a writable observation.
    let device = unsafe { &*user.cast::<Device>() };
    unsafe {
        *out = CObservation {
            parameters: if resource == 0 {
                device.velocity
            } else {
                [0; 2]
            },
            state: 0,
        };
    }
    0
}
unsafe extern "C" fn execute(user: *mut c_void, capability: u16, p0: i32, p1: i32) -> i32 {
    // SAFETY: tests pass a live Device.
    let device = unsafe { &mut *user.cast::<Device>() };
    device.executions += 1;
    match capability {
        DRIVE => device.velocity = [p0, p1],
        STOP => device.velocity = [0; 2],
        _ => return -1,
    }
    0
}
unsafe extern "C" fn fallback(user: *mut c_void, resource: u8, _: u16) -> i32 {
    // SAFETY: tests pass a live Device.
    let device = unsafe { &mut *user.cast::<Device>() };
    device.fallbacks += 1;
    if resource == 0 {
        device.velocity = [0; 2];
    }
    0
}

fn callbacks(device: &mut Device) -> Callbacks {
    Callbacks {
        user: (device as *mut Device).cast(),
        observe: Some(observe),
        execute: Some(execute),
        fallback: Some(fallback),
    }
}

fn context(device: &mut Device) -> Box<Storage> {
    let mut storage = Box::new(Storage([0; 16384]));
    assert!(pxr_context_size() <= storage.0.len());
    let p = storage.0.as_mut_ptr().cast();
    // SAFETY: storage is aligned, large enough and outlives every call in the test.
    assert_eq!(
        unsafe { pxr_init(p, storage.0.len(), 42, 0, 0, callbacks(device)) },
        0
    );
    storage
}

fn drive(lease: &CLease, id: u64, parameters: [i32; 2]) -> [u8; 92] {
    let action = CAction {
        boot_id: 42,
        requester: 7,
        lease_id: lease.id,
        action_id: id,
        sequence: id,
        based_on_epoch: lease.epoch,
        execute_after: 0,
        deadline: 20,
        valid_for_ms: 100,
        parameters,
        capability: DRIVE,
        reserved: 0,
    };
    let mut frame = [0u8; 92];
    // SAFETY: both pointers reference live, correctly sized locals.
    assert_eq!(
        unsafe { pxr_encode_action(&action, frame.as_mut_ptr(), 92) },
        0
    );
    frame
}

#[test]
fn full_session_through_the_c_abi() {
    let mut device = Device::default();
    let mut storage = context(&mut device);
    let ctx: *mut c_void = storage.0.as_mut_ptr().cast();
    let mut lease = CLease::default();
    let mut receipt = CReceipt::default();
    // SAFETY: ctx was initialized above; all outputs are live locals.
    unsafe {
        let mask = (1 << DRIVE) | (1 << STOP);
        assert_eq!(
            pxr_acquire(ctx, 7, 0, mask, 500, -500, 500, -1000, 1000, 0, &mut lease),
            0
        );
        let frame = drive(&lease, 1, [400, -200]);
        assert_eq!(
            pxr_submit(ctx, frame.as_ptr(), 92, 7, 0, 0, &mut receipt),
            0
        );
        assert_eq!(receipt.decision, Decision::Executed as u8);
        assert_eq!(
            pxr_submit(ctx, frame.as_ptr(), 92, 7, 0, 0, &mut receipt),
            0
        );
        assert_eq!(receipt.reason, Reason::Duplicate as u16);

        let mut corrupt = drive(&lease, 2, [1, 1]);
        corrupt[80] ^= 1;
        assert_eq!(
            pxr_submit(ctx, corrupt.as_ptr(), 92, 7, 0, 0, &mut receipt),
            -2
        );
        assert_eq!(
            pxr_submit(ctx, corrupt.as_ptr(), 91, 7, 0, 0, &mut receipt),
            -2
        );

        let mut decoded = CAction::default();
        let frame = drive(&lease, 3, [5, 6]);
        assert_eq!(pxr_decode_action(frame.as_ptr(), 92, &mut decoded), 0);
        assert_eq!(decoded.parameters, [5, 6]);

        assert_eq!(pxr_renew(ctx, 7, lease.id, 1, 400, 10, &mut lease), 0);
        assert_eq!(pxr_tick(ctx, 20), 1);
        assert_eq!(pxr_update_state(ctx, 0, 30), 1);

        let mut snapshot = CSnapshot::default();
        assert_eq!(pxr_snapshot(ctx, &mut snapshot), 0);
        assert_eq!((snapshot.boot_id, snapshot.active_leases), (42, 1));

        let mut cap = CCapability::from(CAPABILITIES[0]);
        assert_eq!(pxr_get_capability(ctx, 0, &mut cap), 0);
        assert_eq!(cap.id, DRIVE);
        assert_eq!(pxr_get_capability(ctx, MAX_CAPABILITIES, &mut cap), -4);

        let mut frame = [0u8; 140];
        assert_eq!(pxr_receipt_frame(ctx, 0, frame.as_mut_ptr(), 140), 0);
        assert!(pxr_runtime_codec::decode_receipt(&frame).is_ok());
        assert_eq!(pxr_receipt_frame(ctx, 0, frame.as_mut_ptr(), 139), -2);
        assert_eq!(pxr_receipt(ctx, 0, &mut receipt), 0);
        assert_eq!(pxr_receipt(ctx, RECEIPT_CAPACITY, &mut receipt), -4);

        assert_eq!(pxr_estop(ctx, 40), 0);
        assert_eq!(pxr_recover_local(ctx, 40), Reason::Estop as i32);
        assert_eq!(pxr_update_state(ctx, 0, 41), 3);
        assert_eq!(pxr_recover_local(ctx, 41), 0);
        assert_eq!(pxr_cancel(ctx, 7, lease.id, 42), Reason::Authority as i32);
        assert!(pxr_epoch(ctx) > 1);
    }
    assert_eq!(device.executions, 1);
    assert_eq!(device.velocity, [0; 2]);
}

#[test]
fn init_rejects_bad_storage_and_configuration() {
    let mut device = Device::default();
    let mut storage = Box::new(Storage([0; 16384]));
    let base = storage.0.as_mut_ptr();
    let size = storage.0.len();
    // SAFETY: every call either rejects its arguments or writes within `storage`.
    unsafe {
        let cb = callbacks(&mut device);
        assert_eq!(pxr_init(ptr::null_mut(), size, 42, 0, 0, cb), -1);
        assert_eq!(pxr_init(base.add(1).cast(), size - 1, 42, 0, 0, cb), -1);
        assert_eq!(
            pxr_init(base.cast(), pxr_context_size() - 1, 42, 0, 0, cb),
            -1
        );
        let mut missing = cb;
        missing.execute = None;
        assert_eq!(pxr_init(base.cast(), size, 42, 0, 0, missing), -1);
        assert_eq!(pxr_init(base.cast(), size, 0, 0, 0, cb), -3);

        let mut spec = CCapability::from(CAPABILITIES[0]);
        spec.class = 9;
        assert_eq!(
            pxr_init_custom(base.cast(), size, 42, 0, 0, cb, &spec, 1),
            -3
        );
        spec = CCapability::from(CAPABILITIES[0]);
        spec.reserved = 1;
        assert_eq!(
            pxr_init_custom(base.cast(), size, 42, 0, 0, cb, &spec, 1),
            -3
        );
        spec.reserved = 0;
        assert_eq!(
            pxr_init_custom(base.cast(), size, 42, 0, 0, cb, &spec, 0),
            -1
        );
        assert_eq!(
            pxr_init_custom(base.cast(), size, 42, 0, 0, cb, &spec, 1),
            0
        );

        let mut config = CConfig::default();
        assert_eq!(pxr_default_config(&mut config), 0);
        config.reserved = 1;
        let null = ptr::null();
        assert_eq!(
            pxr_init_config(base.cast(), size, 42, 0, 0, cb, &config, null, 0),
            -3
        );
        config.reserved = 0;
        config.watchdog_ms = 0;
        assert_eq!(
            pxr_init_config(base.cast(), size, 42, 0, 0, cb, &config, null, 0),
            -3
        );
        config.watchdog_ms = 50;
        assert_eq!(
            pxr_init_config(base.cast(), size, 42, 0, 0, cb, &config, null, 1),
            -1
        );
        assert_eq!(
            pxr_init_config(base.cast(), size, 42, 0, 0, cb, &config, null, 0),
            0
        );
    }
}

#[test]
fn null_and_misaligned_outputs_are_rejected_without_writes() {
    let mut device = Device::default();
    let mut storage = context(&mut device);
    let ctx: *mut c_void = storage.0.as_mut_ptr().cast();
    let mut bytes = [0u64; 32];
    let misaligned = bytes.as_mut_ptr().cast::<u8>().wrapping_add(1);
    // SAFETY: invalid pointers must be rejected before any dereference.
    unsafe {
        assert_eq!(
            pxr_acquire(ctx, 7, 0, 2, 100, 0, 0, 0, 0, 0, ptr::null_mut()),
            -1
        );
        assert_eq!(
            pxr_acquire(ctx, 7, 0, 2, 100, 0, 0, 0, 0, 0, misaligned.cast()),
            -1
        );
        assert_eq!(
            pxr_submit(ctx, ptr::null(), 92, 7, 0, 0, misaligned.cast()),
            -1
        );
        assert_eq!(pxr_tick(misaligned.cast(), 0), -1);
        assert_eq!(pxr_receipt(ctx, 0, misaligned.cast()), -1);
        assert_eq!(pxr_snapshot(ptr::null(), misaligned.cast()), -1);
        assert_eq!(pxr_decode_action(ptr::null(), 92, misaligned.cast()), -1);
        assert_eq!(pxr_epoch(ptr::null()), 0);
    }
    assert_eq!(bytes, [0u64; 32]);
}

unsafe extern "C" fn sink(user: *mut c_void, frame: *const u8) {
    // SAFETY: tests pass a live Vec and a 140-byte frame.
    let log = unsafe { &mut *user.cast::<std::vec::Vec<[u8; 140]>>() };
    let mut copy = [0u8; 140];
    unsafe { ptr::copy_nonoverlapping(frame, copy.as_mut_ptr(), 140) };
    log.push(copy);
}

#[test]
fn uninitialized_or_failed_storage_is_rejected() {
    let mut device = Device::default();
    let mut storage = Box::new(Storage([0; 16384]));
    let ctx: *mut c_void = storage.0.as_mut_ptr().cast();
    let mut lease = CLease::default();
    // SAFETY: storage is aligned and large enough; the calls must refuse it.
    unsafe {
        assert_eq!(pxr_tick(ctx, 0), -1);
        assert_eq!(
            pxr_acquire(ctx, 7, 0, 2, 100, 0, 0, 0, 0, 0, &mut lease),
            -1
        );
        assert_eq!(pxr_init(ctx, 16384, 42, 0, 0, callbacks(&mut device)), 0);
        assert_eq!(pxr_tick(ctx, 0), 0);
        assert_eq!(pxr_init(ctx, 16384, 0, 0, 0, callbacks(&mut device)), -3);
        assert_eq!(
            pxr_tick(ctx, 0),
            -1,
            "a failed re-init invalidates the old context"
        );
    }
}

#[test]
fn estop_signal_raised_from_an_interrupt_latches_on_the_next_call() {
    let mut device = Device::default();
    let mut storage = context(&mut device);
    let ctx: *mut c_void = storage.0.as_mut_ptr().cast();
    let signal = CEstopSignal(EstopSignal::new());
    let mut lease = CLease::default();
    let mut receipt = CReceipt::default();
    // SAFETY: ctx is initialized; signal outlives the context use.
    unsafe {
        assert_eq!(pxr_attach_estop_signal(ctx, &signal), 0);
        let mask = (1 << DRIVE) | (1 << STOP);
        assert_eq!(
            pxr_acquire(ctx, 7, 0, mask, 500, -500, 500, -1000, 1000, 0, &mut lease),
            0
        );
        let frame = drive(&lease, 1, [100, 0]);
        assert_eq!(
            pxr_submit(ctx, frame.as_ptr(), 92, 7, 0, 0, &mut receipt),
            0
        );
        assert_eq!(receipt.decision, Decision::Executed as u8);

        pxr_estop_signal_raise(&signal);
        assert_eq!(pxr_tick(ctx, 1), RuntimeState::Estopped as i32);
        let frame = drive(&lease, 2, [100, 0]);
        assert_eq!(
            pxr_submit(ctx, frame.as_ptr(), 92, 7, 1, 1, &mut receipt),
            0
        );
        assert_eq!(receipt.reason, Reason::Estop as u16);
        assert_eq!(pxr_update_state(ctx, 0, 2), RuntimeState::Estopped as i32);
        assert_eq!(
            pxr_recover_local(ctx, 2),
            0,
            "a handled raise does not re-trigger"
        );
        pxr_estop_signal_raise(&signal);
        assert_eq!(pxr_recover_local(ctx, 3), Reason::Estop as i32);
        pxr_estop_signal_raise(ptr::null());
        assert_eq!(pxr_attach_estop_signal(ctx, ptr::null()), 0);
    }
    assert_eq!(device.executions, 1);
    assert_eq!(device.velocity, [0; 2]);
}

#[test]
fn receipt_sink_sees_every_receipt_in_order() {
    let mut device = Device::default();
    let mut storage = context(&mut device);
    let ctx: *mut c_void = storage.0.as_mut_ptr().cast();
    let mut log: std::vec::Vec<[u8; 140]> = std::vec::Vec::new();
    let mut lease = CLease::default();
    let mut receipt = CReceipt::default();
    // SAFETY: ctx is initialized; log outlives every call that can reach the sink.
    unsafe {
        let user = (&mut log as *mut std::vec::Vec<[u8; 140]>).cast();
        assert_eq!(pxr_set_receipt_sink(ctx, Some(sink), user), 0);
        let mask = (1 << DRIVE) | (1 << STOP);
        assert_eq!(
            pxr_acquire(ctx, 7, 0, mask, 500, -500, 500, -1000, 1000, 0, &mut lease),
            0
        );
        for id in 1..=3 {
            let frame = drive(&lease, id, [10, 0]);
            assert_eq!(
                pxr_submit(ctx, frame.as_ptr(), 92, 7, 0, 0, &mut receipt),
                0
            );
        }
        assert_eq!(pxr_set_receipt_sink(ctx, None, ptr::null_mut()), 0);
        assert_eq!(pxr_tick(ctx, 600), RuntimeState::Faulted as i32);
    }
    assert_eq!(
        log.len(),
        4,
        "grant plus three executions; nothing after detaching"
    );
    let ids: std::vec::Vec<u64> = log
        .iter()
        .map(|f| pxr_runtime_codec::decode_receipt(f).unwrap().receipt_id)
        .collect();
    assert_eq!(ids, [1, 2, 3, 4]);
}
