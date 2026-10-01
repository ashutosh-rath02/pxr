//! In-crate invariant tests. Random operation sequences are checked against properties
//! that must hold regardless of input, rather than against expected outputs.
extern crate std;
use super::Runtime;
use crate::{profile::*, *};
use std::vec::Vec;

#[derive(Default)]
struct Rec {
    motor: [i32; 2],
    gripper_open: bool,
    executions: u64,
    fallbacks: [u64; MAX_RESOURCES],
    fail_execute: bool,
    fail_observe: bool,
    fail_fallback: bool,
    wrong_feedback: bool,
    stuck_gripper: bool,
    log: Vec<Receipt>,
    authority_ids: Vec<u64>,
}
impl Driver for Rec {
    fn observe(&mut self, resource: u8) -> Result<Observation, DriverError> {
        if self.fail_observe {
            return Err(DriverError);
        }
        match resource {
            0 => Ok(Observation {
                parameters: if self.wrong_feedback {
                    [7, 7]
                } else {
                    self.motor
                },
                state: 0,
            }),
            1 => Ok(Observation {
                parameters: [0; 2],
                state: if self.gripper_open { GRIPPER_OPEN } else { 0 },
            }),
            _ => Err(DriverError),
        }
    }
    fn execute(&mut self, capability: u16, parameters: [i32; 2]) -> Result<(), DriverError> {
        self.executions += 1;
        if self.fail_execute {
            return Err(DriverError);
        }
        match capability {
            DRIVE => self.motor = parameters,
            STOP => self.motor = [0; 2],
            OPEN | CLOSE if self.stuck_gripper => {}
            OPEN => self.gripper_open = true,
            CLOSE => self.gripper_open = false,
            _ => return Err(DriverError),
        }
        Ok(())
    }
    fn record_receipt(&mut self, receipt: &Receipt) {
        self.log.push(*receipt);
        if receipt.decision != Decision::Rejected {
            self.authority_ids.push(receipt.receipt_id);
        }
    }
    fn fallback(&mut self, resource: u8, _: Reason) -> Result<(), DriverError> {
        self.fallbacks[resource as usize] += 1;
        if resource == 0 {
            self.motor = [0; 2];
        }
        if self.fail_fallback {
            Err(DriverError)
        } else {
            Ok(())
        }
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }
    fn param(&mut self) -> i32 {
        if self.chance(40) {
            return self.below(400) as i32 - 200;
        }
        const EDGES: [i32; 13] = [
            i32::MIN,
            -2001,
            -2000,
            -1001,
            -1000,
            -1,
            0,
            1,
            999,
            1000,
            1001,
            2000,
            2001,
        ];
        if self.chance(70) {
            EDGES[self.below(EDGES.len() as u64) as usize]
        } else {
            self.next() as i32
        }
    }
}

const OWNER: u64 = 7;
const BOOT: u64 = 42;
const SAFETY_MASK: u32 = ESTOP | OBSTACLE | BATTERY_CRITICAL | MOTOR_FAULT;

fn capability(id: u16) -> Option<Capability> {
    CAPABILITIES.iter().copied().find(|c| c.id == id)
}

struct Harness {
    rt: Runtime,
    d: Rec,
    rng: Rng,
    now: u64,
    estop_latched: bool,
    signal: EstopSignal,
    history: Vec<Action>,
    executed: Vec<(u64, u64, u64)>,
    next_action: u64,
    stream_deadline: u64,
    last_receipt_id: u64,
    last_epoch: u64,
    reasons: [u64; 32],
}

impl Harness {
    fn new(seed: u64) -> Self {
        let mut d = Rec::default();
        let rt = Runtime::new(Config::default(), &CAPABILITIES, BOOT, 0, 0, &mut d).unwrap();
        Self {
            rt,
            d,
            rng: Rng(seed),
            now: 0,
            estop_latched: false,
            signal: EstopSignal::new(),
            history: Vec::new(),
            executed: Vec::new(),
            next_action: 1,
            stream_deadline: 0,
            last_receipt_id: 0,
            last_epoch: 1,
            reasons: [0; 32],
        }
    }

    fn step(&mut self) {
        // Driver faults are one-shot so the run spends most of its time operational.
        self.d.fail_execute = false;
        self.d.fail_observe = false;
        self.d.fail_fallback = false;
        self.d.wrong_feedback = false;
        if self.rng.chance(60) {
            self.now += self.rng.below(15);
        }
        let mut at = self.now;
        if self.rng.chance(1) {
            at = at.saturating_sub(1 + self.rng.below(20));
        }
        let down = matches!(
            self.rt.state(),
            RuntimeState::Faulted | RuntimeState::Estopped
        );
        let missing = (0..2u8).find(|r| self.rt.lease(*r).is_none());
        let op = if down && self.rng.chance(30) {
            90
        } else if let Some(resource) = missing.filter(|_| self.rng.chance(50)) {
            let _ = self.rt.acquire(
                LeaseRequest {
                    owner: OWNER,
                    resource,
                    capabilities: if resource == 0 {
                        (1 << DRIVE) | (1 << STOP)
                    } else {
                        (1 << OPEN) | (1 << CLOSE)
                    },
                    duration_ms: 300 + self.rng.below(1700) as u32,
                    limits: [
                        Bounds {
                            min: -(self.rng.below(1500) as i32),
                            max: self.rng.below(1500) as i32,
                        },
                        Bounds::ANY,
                    ],
                },
                at,
                &mut self.d,
            );
            self.check_global(at);
            return;
        } else {
            self.rng.below(100)
        };
        match op {
            0..=39 => self.submit(at),
            40..=54 => self.rt.tick(at, &mut self.d),
            55..=74 => {
                let flags = match self.rng.below(100) {
                    0..=84 => 0,
                    85..=89 => OBSTACLE,
                    90..=92 => ARM_MOVING,
                    93..=94 => BATTERY_CRITICAL,
                    95..=98 => MOTOR_FAULT,
                    _ => ESTOP,
                };
                self.rt.update_state(flags, at, &mut self.d);
            }
            75..=82 => {
                let resource = match (self.rt.lease(0), self.rt.lease(1)) {
                    (None, _) if self.rng.chance(70) => 0,
                    (_, None) if self.rng.chance(70) => 1,
                    _ => [0, 1, 2][self.rng.below(3) as usize],
                };
                let _ = self.rt.acquire(
                    LeaseRequest {
                        owner: if self.rng.chance(90) {
                            OWNER
                        } else {
                            self.rng.below(3)
                        },
                        resource,
                        capabilities: match self.rng.below(8) {
                            0 => self.rng.next(),
                            _ if resource == 0 => (1 << DRIVE) | (1 << STOP),
                            _ => (1 << OPEN) | (1 << CLOSE),
                        },
                        duration_ms: [0, 1, 50, 300, 2000, 2001][self.rng.below(6) as usize],
                        limits: [
                            Bounds {
                                min: -(self.rng.below(1500) as i32),
                                max: self.rng.below(1500) as i32,
                            },
                            Bounds::ANY,
                        ],
                    },
                    at,
                    &mut self.d,
                );
            }
            83..=86 => {
                if let Some(l) = self.rt.lease(self.rng.below(2) as u8) {
                    let renewal = l.last_renewal + self.rng.below(3);
                    let _ = self.rt.renew(
                        l.owner,
                        l.id,
                        renewal,
                        self.rng.below(2100) as u32,
                        at,
                        &mut self.d,
                    );
                }
            }
            87..=88 => {
                if let Some(l) = self.rt.lease(self.rng.below(2) as u8) {
                    let _ = self.rt.cancel(l.owner, l.id, at, &mut self.d);
                }
            }
            89 if self.rng.chance(30) => {
                if self.rng.chance(50) {
                    self.rt.emergency_stop(at, &mut self.d);
                } else {
                    // Interrupt path: raise, then the next call polls it.
                    self.signal.raise();
                    assert!(self.rt.poll_estop(&self.signal, at, &mut self.d));
                }
                self.estop_latched = true;
            }
            90..=95 => {
                if self.rng.chance(50) {
                    self.rt.update_state(0, at, &mut self.d);
                }
                match self.rt.recover_local(at, &mut self.d) {
                    Ok(()) => self.estop_latched = false,
                    // E-stop was cleared but a fallback failed: latch is replaced by a fault.
                    Err(Reason::Driver) => {
                        assert_eq!(self.rt.state(), RuntimeState::Faulted);
                        self.estop_latched = false;
                    }
                    Err(_) => {}
                }
            }
            96..=99 => {
                match self.rng.below(4) {
                    0 => self.d.fail_execute = true,
                    1 => self.d.fail_observe = true,
                    2 => self.d.fail_fallback = true,
                    _ => self.d.wrong_feedback = true,
                }
                if self.rng.chance(50) {
                    self.submit(at)
                } else {
                    self.rt.update_state(OBSTACLE, at, &mut self.d)
                }
            }
            _ => self.rt.tick(at, &mut self.d),
        }
        self.check_global(at);
    }

    fn action(&mut self, at: u64) -> (Action, u64, u64) {
        let principal = if self.rng.chance(95) {
            OWNER
        } else {
            self.rng.below(3)
        };
        let received = if self.rng.chance(95) {
            at.saturating_sub(self.rng.below(60))
        } else {
            at + 1
        };
        if !self.history.is_empty() && self.rng.chance(15) {
            let i = self.rng.below(self.history.len() as u64) as usize;
            return (self.history[i], principal, received);
        }
        let cap = if self.rng.chance(85) {
            [DRIVE, DRIVE, DRIVE, STOP, OPEN, CLOSE][self.rng.below(6) as usize]
        } else {
            self.rng.below(6) as u16
        };
        let lease = capability(cap).and_then(|c| self.rt.lease(c.resource));
        let id = if !self.history.is_empty() && self.rng.chance(10) {
            self.history[self.rng.below(self.history.len() as u64) as usize].action_id
        } else {
            self.next_action += 1;
            self.next_action
        };
        let execute_after = if self.rng.chance(80) {
            at.saturating_sub(self.rng.below(5))
        } else {
            at + self.rng.below(5)
        };
        let action = Action {
            boot_id: if self.rng.chance(98) { BOOT } else { BOOT + 1 },
            requester: if self.rng.chance(97) {
                principal
            } else {
                OWNER + 1
            },
            lease_id: lease.map_or(self.rng.below(5), |l| l.id),
            action_id: id,
            sequence: lease.map_or(1, |l| l.last_sequence) + self.rng.below(3),
            capability: cap,
            based_on_epoch: self
                .rt
                .epoch()
                .wrapping_add([0, 0, 0, 0, 0, 0, 1, u64::MAX][self.rng.below(8) as usize]),
            execute_after,
            deadline: execute_after.saturating_add(self.rng.below(200)),
            valid_for_ms: if self.rng.chance(85) {
                [20, 100, 1000][self.rng.below(3) as usize]
            } else {
                [0, 1, 1001][self.rng.below(3) as usize]
            },
            parameters: [self.rng.param(), self.rng.param()],
        };
        (action, principal, received)
    }

    fn submit(&mut self, at: u64) {
        let (action, principal, received) = self.action(at);
        let before_flags = self.rt.flags();
        let before_state = self.rt.state();
        let lease_before = capability(action.capability).and_then(|c| self.rt.lease(c.resource));
        let executions = self.d.executions;
        let r = self.rt.submit(action, principal, received, at, &mut self.d);
        self.reasons[r.reason as usize] += 1;
        self.history.push(action);
        let dispatched = self.d.executions - executions;
        assert!(dispatched <= 1);
        assert_eq!(r.dispatched, dispatched == 1, "{r:?}");
        if dispatched == 0 {
            assert_ne!(r.decision, Decision::Executed);
            return;
        }
        // Everything below is an admission obligation for a dispatched command.
        let cap = capability(action.capability).expect("dispatched unknown capability");
        let lease = lease_before.expect("dispatched without a lease");
        assert!(!self.estop_latched);
        assert!(matches!(before_state, RuntimeState::Armed));
        assert_eq!(before_flags & ESTOP, 0);
        assert!(cap.permits(before_flags));
        assert_eq!(action.boot_id, BOOT);
        assert_eq!(action.based_on_epoch, r.epoch);
        assert_eq!(principal, lease.owner);
        assert_eq!(action.requester, principal);
        assert_eq!(action.lease_id, lease.id);
        assert!(lease.capabilities & (1 << cap.id) != 0);
        assert!(action.sequence > lease.last_sequence);
        assert!(at < action.deadline && at >= action.execute_after);
        assert!(received <= at && at < received + action.valid_for_ms as u64);
        assert!(action.valid_for_ms <= Config::default().max_valid_for_ms);
        assert!(at < lease.expires_at);
        for n in 0..2 {
            assert!(cap.bounds[n].contains(action.parameters[n]));
            assert!(lease.limits[n].contains(action.parameters[n]));
        }
        // Exact frames never re-dispatch. A reused action ID is only deduplicated while it is
        // inside the replay window; outside it, lease ID and sequence binding are the guard.
        let recent = self.executed.len().saturating_sub(REPLAY_CAPACITY);
        for (n, e) in self.executed.iter().enumerate() {
            if e.0 == principal && e.1 == action.action_id {
                assert_ne!(
                    e.2, action.lease_id,
                    "frame for action {} dispatched twice",
                    e.1
                );
                assert!(
                    n < recent,
                    "action {} re-dispatched inside the replay window",
                    e.1
                );
            }
        }
        self.executed
            .push((principal, action.action_id, action.lease_id));
        if r.decision == Decision::Executed && cap.id == DRIVE {
            self.stream_deadline = (received + action.valid_for_ms as u64).min(lease.expires_at);
        }
    }

    fn check_global(&mut self, at: u64) {
        let state = self.rt.state();
        let snap = self.rt.snapshot();
        let leases = (0..MAX_RESOURCES as u8).filter_map(|r| self.rt.lease(r));
        let any_lease = leases.clone().count() > 0;

        if self.rt.flags() & ESTOP != 0 || self.estop_latched {
            assert_eq!(state, RuntimeState::Estopped);
        }
        if matches!(state, RuntimeState::Faulted | RuntimeState::Estopped) {
            assert!(!any_lease, "{state:?} retained a lease");
        }
        assert_eq!(state == RuntimeState::Armed, any_lease, "{state:?}");
        for l in leases {
            assert!(l.expires_at > at.min(snap.now));
        }
        if self.d.motor != [0; 2] {
            assert_eq!(state, RuntimeState::Armed);
            assert_eq!(self.rt.flags() & SAFETY_MASK, 0);
            assert!(self.rt.lease(0).is_some_and(|l| l.expires_at > snap.now));
            assert!(
                snap.now < self.stream_deadline,
                "motor running past its TTL"
            );
        }
        assert!(self.d.motor[0].abs() <= 1000 && self.d.motor[1].abs() <= 2000);

        assert!(snap.epoch >= self.last_epoch);
        self.last_epoch = snap.epoch;
        if let Some(last) = snap
            .retained_receipts
            .checked_sub(1)
            .and_then(|i| self.rt.receipt(i))
        {
            assert_eq!(last.receipt_id + 1, snap.next_receipt_id);
            assert!(last.receipt_id >= self.last_receipt_id);
            self.last_receipt_id = last.receipt_id;
        }
        let retained: Vec<Receipt> = (0..snap.retained_receipts)
            .map(|i| self.rt.receipt(i).unwrap())
            .collect();
        assert!(retained
            .windows(2)
            .all(|w| w[0].receipt_id < w[1].receipt_id));
        for r in &retained {
            assert_eq!(
                self.d.log[(r.receipt_id - 1) as usize],
                *r,
                "hook and ring disagree"
            );
        }
        // Rejections are evicted first: the newest authority receipts are always retained.
        let kept: Vec<u64> = retained
            .iter()
            .filter(|r| r.decision != Decision::Rejected)
            .map(|r| r.receipt_id)
            .collect();
        let all = &self.d.authority_ids;
        assert!(kept.len() >= all.len().min(RECEIPT_CAPACITY - 1));
        assert_eq!(kept[..], all[all.len() - kept.len()..]);
    }
}

#[test]
fn random_operation_sequences_preserve_admission_invariants() {
    for seed in 1..=24u64 {
        let mut h = Harness::new(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        for _ in 0..20_000 {
            h.step();
        }
        let executed = h.reasons[Reason::Ok as usize];
        assert!(
            executed >= 100,
            "seed {seed}: {executed} executions, harness lost coverage"
        );
        for reason in [
            Reason::Bound,
            Reason::Stale,
            Reason::Duplicate,
            Reason::Epoch,
            Reason::Estop,
        ] {
            assert!(
                h.reasons[reason as usize] > 0,
                "seed {seed} never produced {reason:?}"
            );
        }
    }
}

fn armed() -> (Runtime, Rec, Lease) {
    let mut d = Rec::default();
    let mut rt = Runtime::new(Config::default(), &CAPABILITIES, BOOT, 0, 0, &mut d).unwrap();
    let lease = rt
        .acquire(
            LeaseRequest {
                owner: OWNER,
                resource: 0,
                capabilities: (1 << DRIVE) | (1 << STOP),
                duration_ms: 500,
                limits: [Bounds::ANY; 2],
            },
            0,
            &mut d,
        )
        .unwrap();
    (rt, d, lease)
}

fn drive(rt: &Runtime, lease: Lease, id: u64, parameters: [i32; 2]) -> Action {
    Action {
        boot_id: BOOT,
        requester: OWNER,
        lease_id: lease.id,
        action_id: id,
        sequence: id,
        capability: DRIVE,
        based_on_epoch: rt.epoch(),
        execute_after: 0,
        deadline: 20,
        valid_for_ms: 100,
        parameters,
    }
}

#[test]
fn bounds_are_inclusive_at_both_edges() {
    let (mut rt, mut d, lease) = armed();
    for (id, p) in [(1, [1000, 2000]), (2, [-1000, -2000])] {
        let a = drive(&rt, lease, id, p);
        assert_eq!(
            rt.submit(a, OWNER, 0, 0, &mut d).decision,
            Decision::Executed
        );
    }
    for (id, p) in [(3, [1001, 0]), (4, [0, -2001])] {
        let a = drive(&rt, lease, id, p);
        assert_eq!(rt.submit(a, OWNER, 0, 0, &mut d).reason, Reason::Bound);
    }
}

#[test]
fn deadline_ttl_and_execute_after_are_exact() {
    let (mut rt, mut d, lease) = armed();
    let mut a = drive(&rt, lease, 1, [1, 1]);
    a.deadline = 5;
    rt.update_state(0, 5, &mut d);
    a.based_on_epoch = rt.epoch();
    assert_eq!(rt.submit(a, OWNER, 5, 5, &mut d).reason, Reason::Stale);
    a.action_id = 2;
    a.sequence = 2;
    a.deadline = 6;
    assert_eq!(
        rt.submit(a, OWNER, 5, 5, &mut d).decision,
        Decision::Executed
    );

    a.action_id = 3;
    a.sequence = 3;
    a.valid_for_ms = 5;
    a.deadline = 100;
    assert_eq!(rt.submit(a, OWNER, 0, 5, &mut d).reason, Reason::Stale);
    a.action_id = 4;
    a.sequence = 4;
    a.valid_for_ms = 6;
    assert_eq!(
        rt.submit(a, OWNER, 0, 5, &mut d).decision,
        Decision::Executed
    );

    a.action_id = 5;
    a.sequence = 5;
    a.execute_after = 6;
    assert_eq!(rt.submit(a, OWNER, 5, 5, &mut d).reason, Reason::TooEarly);
    a.execute_after = 5;
    assert_eq!(
        rt.submit(a, OWNER, 5, 5, &mut d).decision,
        Decision::Executed
    );
}

#[test]
fn valid_for_and_lease_duration_limits_are_inclusive() {
    let (mut rt, mut d, lease) = armed();
    let mut a = drive(&rt, lease, 1, [0, 0]);
    a.valid_for_ms = 1001;
    assert_eq!(rt.submit(a, OWNER, 0, 0, &mut d).reason, Reason::Invalid);
    a.valid_for_ms = 1000;
    a.action_id = 2;
    a.sequence = 2;
    assert_eq!(
        rt.submit(a, OWNER, 0, 0, &mut d).decision,
        Decision::Executed
    );
    assert_eq!(
        rt.renew(OWNER, lease.id, 1, 2001, 0, &mut d),
        Err(Reason::Invalid)
    );
    assert!(rt.renew(OWNER, lease.id, 1, 2000, 0, &mut d).is_ok());
}

#[test]
fn lease_and_stream_expire_exactly_at_their_deadline() {
    let (mut rt, mut d, lease) = armed();
    let mut a = drive(&rt, lease, 1, [10, 0]);
    a.valid_for_ms = 30;
    assert_eq!(
        rt.submit(a, OWNER, 0, 0, &mut d).decision,
        Decision::Executed
    );
    rt.update_state(0, 29, &mut d);
    assert_eq!(d.motor, [10, 0]);
    rt.update_state(0, 30, &mut d);
    assert_eq!(d.motor, [0, 0]);
    assert!(rt.lease(0).is_none(), "stream expiry revokes the lease");

    let (mut rt, mut d, _) = armed();
    for t in (10..500).step_by(10) {
        rt.update_state(0, t, &mut d);
    }
    rt.update_state(0, 499, &mut d);
    assert!(rt.lease(0).is_some());
    rt.update_state(0, 500, &mut d);
    assert!(rt.lease(0).is_none());
}

#[test]
fn watchdog_and_state_ttl_boundaries() {
    let (mut rt, mut d, _) = armed();
    rt.tick(50, &mut d);
    assert_eq!(rt.state(), RuntimeState::Armed);
    rt.tick(101, &mut d);
    assert_eq!(rt.state(), RuntimeState::Faulted);

    let (mut rt, mut d, _) = armed();
    for t in (0..250).step_by(50) {
        rt.tick(t, &mut d);
    }
    rt.tick(249, &mut d);
    assert_eq!(rt.state(), RuntimeState::Armed);
    rt.tick(250, &mut d);
    assert_eq!(rt.state(), RuntimeState::Faulted);
}

#[test]
fn invalid_storm_trips_exactly_at_limit() {
    let (mut rt, mut d, lease) = armed();
    let limit = Config::default().invalid_limit as u64;
    for id in 1..limit {
        let a = drive(&rt, lease, id, [5000, 0]);
        rt.submit(a, OWNER, 0, 0, &mut d);
    }
    assert_eq!(rt.state(), RuntimeState::Armed);
    let a = drive(&rt, lease, limit, [5000, 0]);
    rt.submit(a, OWNER, 0, 0, &mut d);
    assert_eq!(rt.state(), RuntimeState::Faulted);
}

#[test]
fn successful_execution_resets_invalid_counter() {
    let (mut rt, mut d, lease) = armed();
    let limit = Config::default().invalid_limit as u64;
    let mut id = 0;
    for _ in 0..3 {
        for _ in 1..limit {
            id += 1;
            let a = drive(&rt, lease, id, [5000, 0]);
            rt.submit(a, OWNER, 0, 0, &mut d);
        }
        id += 1;
        let a = drive(&rt, lease, id, [1, 0]);
        assert_eq!(
            rt.submit(a, OWNER, 0, 0, &mut d).decision,
            Decision::Executed
        );
    }
    assert_eq!(rt.state(), RuntimeState::Armed);
}

#[test]
fn config_and_capability_validation_rejects_each_bad_field() {
    fn new(c: Config, caps: &[Capability], boot: u64, d: &mut Rec) -> Option<ConfigError> {
        Runtime::new(c, caps, boot, 0, 0, d).err()
    }
    let mut d = Rec::default();
    assert_eq!(
        new(Config::default(), &CAPABILITIES, 0, &mut d),
        Some(ConfigError::InvalidBootId)
    );
    assert_eq!(
        new(Config::default(), &[], 1, &mut d),
        Some(ConfigError::InvalidConfig)
    );
    let zeroed: [fn(&mut Config); 6] = [
        |c| c.watchdog_ms = 0,
        |c| c.state_ttl_ms = 0,
        |c| c.max_lease_ms = 0,
        |c| c.max_valid_for_ms = 0,
        |c| c.invalid_limit = 0,
        |c| c.estop_mask = 0,
    ];
    for f in zeroed {
        let mut c = Config::default();
        f(&mut c);
        assert_eq!(
            new(c, &CAPABILITIES, 1, &mut d),
            Some(ConfigError::InvalidConfig)
        );
    }
    let bad: [fn(&mut Capability); 8] = [
        |c| c.id = 0,
        |c| c.id = 64,
        |c| c.resource = MAX_RESOURCES as u8,
        |c| c.parameter_count = 3,
        |c| {
            c.require_set = 2;
            c.require_clear = 2;
        },
        |c| c.bounds[0] = Bounds { min: 1, max: 0 },
        |c| {
            c.parameter_count = 1;
            c.bounds[1] = Bounds { min: 0, max: 1 };
        },
        |c| c.verification = Verification::State { mask: 1, value: 2 },
    ];
    for f in bad {
        let mut c = CAPABILITIES[0];
        f(&mut c);
        assert_eq!(
            new(Config::default(), &[c], 1, &mut d),
            Some(ConfigError::InvalidCapability)
        );
    }
    let mut c = CAPABILITIES[0];
    c.id = 63;
    assert!(new(Config::default(), &[c], 1, &mut d).is_none());
    assert_eq!(
        new(
            Config::default(),
            &[CAPABILITIES[0], CAPABILITIES[0]],
            1,
            &mut d
        ),
        Some(ConfigError::DuplicateCapability)
    );
    let mut d = Rec {
        fail_fallback: true,
        ..Rec::default()
    };
    assert_eq!(
        new(Config::default(), &CAPABILITIES, 1, &mut d),
        Some(ConfigError::Driver)
    );
    assert_eq!(
        d.fallbacks[0] + d.fallbacks[1],
        2,
        "every fallback is attempted"
    );
}

#[test]
fn initial_estop_flag_boots_estopped() {
    let mut d = Rec::default();
    let rt = Runtime::new(Config::default(), &CAPABILITIES, 1, 0, ESTOP, &mut d).unwrap();
    assert_eq!(rt.state(), RuntimeState::Estopped);
}

#[test]
fn counter_exhaustion_faults_instead_of_wrapping() {
    let (mut rt, mut d, _) = armed();
    rt.epoch = u64::MAX - 1;
    rt.update_state(OBSTACLE, 1, &mut d);
    rt.tick(2, &mut d);
    assert_eq!(rt.state(), RuntimeState::Faulted);
    assert_eq!(rt.recover_local(3, &mut d), Err(Reason::Precondition));
}

#[test]
fn only_the_owner_can_cancel_or_renew_its_own_lease() {
    let (mut rt, mut d, lease) = armed();
    assert_eq!(
        rt.cancel(OWNER + 1, lease.id, 0, &mut d),
        Err(Reason::Authority)
    );
    assert_eq!(
        rt.cancel(OWNER, lease.id + 1, 0, &mut d),
        Err(Reason::Authority)
    );
    assert_eq!(
        rt.renew(OWNER + 1, lease.id, 1, 100, 0, &mut d),
        Err(Reason::Authority)
    );
    assert!(rt.lease(0).is_some());
    assert_eq!(rt.cancel(OWNER, lease.id, 0, &mut d), Ok(()));
    assert!(rt.lease(0).is_none());
}

#[test]
fn submit_enforces_the_capabilities_granted_by_the_lease() {
    let mut d = Rec::default();
    let mut rt = Runtime::new(Config::default(), &CAPABILITIES, BOOT, 0, 0, &mut d).unwrap();
    let lease = rt
        .acquire(
            LeaseRequest {
                owner: OWNER,
                resource: 0,
                capabilities: 1 << STOP,
                duration_ms: 500,
                limits: [Bounds::ANY; 2],
            },
            0,
            &mut d,
        )
        .unwrap();
    let a = drive(&rt, lease, 1, [1, 1]);
    assert_eq!(rt.submit(a, OWNER, 0, 0, &mut d).reason, Reason::Authority);
    assert_eq!(d.executions, 0);
}

#[test]
fn acquire_rejects_each_invalid_request_field() {
    let (mut d, good) = (
        Rec::default(),
        LeaseRequest {
            owner: OWNER,
            resource: 0,
            capabilities: 1 << DRIVE,
            duration_ms: 100,
            limits: [Bounds::ANY; 2],
        },
    );
    let mut rt = Runtime::new(Config::default(), &CAPABILITIES[..2], BOOT, 0, 0, &mut d).unwrap();
    let bad: [fn(&mut LeaseRequest); 7] = [
        |r| r.resource = 1,
        |r| r.resource = MAX_RESOURCES as u8,
        |r| r.owner = 0,
        |r| r.capabilities = 0,
        |r| r.duration_ms = 0,
        |r| r.duration_ms = Config::default().max_lease_ms + 1,
        |r| r.limits[1] = Bounds { min: 1, max: 0 },
    ];
    for f in bad {
        let mut r = good;
        f(&mut r);
        assert_eq!(rt.acquire(r, 0, &mut d), Err(Reason::Invalid), "{r:?}");
    }
    let mut r = good;
    r.capabilities = 1 << OPEN;
    assert_eq!(rt.acquire(r, 0, &mut d), Err(Reason::Authority));
    r.duration_ms = Config::default().max_lease_ms;
    r.capabilities = 1 << DRIVE;
    assert!(rt.acquire(r, 0, &mut d).is_ok());
    assert_eq!(rt.acquire(good, 0, &mut d), Err(Reason::Busy));
}

#[test]
fn zero_action_id_or_sequence_is_invalid() {
    let (mut rt, mut d, lease) = armed();
    let mut a = drive(&rt, lease, 1, [0, 0]);
    a.sequence = 0;
    assert_eq!(rt.submit(a, OWNER, 0, 0, &mut d).reason, Reason::Invalid);
    let mut a = drive(&rt, lease, 1, [0, 0]);
    a.action_id = 0;
    assert_eq!(rt.submit(a, OWNER, 0, 0, &mut d).reason, Reason::Invalid);
    assert_eq!(d.executions, 0);
}

#[test]
fn fallback_failure_on_state_change_latches_a_fault() {
    let (mut rt, mut d, lease) = armed();
    let a = drive(&rt, lease, 1, [5, 5]);
    rt.submit(a, OWNER, 0, 0, &mut d);
    d.fail_fallback = true;
    rt.update_state(OBSTACLE, 1, &mut d);
    assert_eq!(rt.state(), RuntimeState::Faulted);

    let (mut rt, mut d, lease) = armed();
    let a = drive(&rt, lease, 1, [5, 5]);
    rt.submit(a, OWNER, 0, 0, &mut d);
    rt.update_state(OBSTACLE, 1, &mut d);
    assert_eq!(
        rt.state(),
        RuntimeState::SafeIdle,
        "a successful fallback is not a fault"
    );
}

#[test]
fn receipt_and_lease_counter_exhaustion_also_fault() {
    for exhaust in [
        (|rt: &mut Runtime| rt.next_receipt = u64::MAX) as fn(&mut Runtime),
        |rt| rt.next_lease = u64::MAX,
    ] {
        let (mut rt, mut d, _) = armed();
        exhaust(&mut rt);
        rt.tick(1, &mut d);
        assert_eq!(rt.state(), RuntimeState::Faulted);
        rt.update_state(0, 2, &mut d);
        assert_eq!(rt.recover_local(3, &mut d), Err(Reason::Precondition));
    }
}

#[test]
fn receipts_carry_accurate_bookkeeping_fields() {
    let (mut rt, mut d, lease) = armed();
    let granted = rt.receipt(rt.receipt_count() - 1).unwrap();
    assert_eq!(
        (granted.decision, granted.dispatched),
        (Decision::Control, false)
    );

    let mut a = drive(&rt, lease, 1, [1, 1]);
    a.capability = 99;
    let unknown = rt.submit(a, OWNER, 0, 0, &mut d);
    assert_eq!(
        (unknown.reason, unknown.resource),
        (Reason::UnknownCapability, u8::MAX)
    );

    let earlier = drive(&rt, lease, 2, [1, 1]);
    rt.submit(earlier, OWNER, 0, 0, &mut d);
    let a = drive(&rt, lease, 3, [1, 1]);
    let first = rt.submit(a, OWNER, 0, 0, &mut d);
    let dup = rt.submit(a, OWNER, 0, 0, &mut d);
    assert_eq!(
        (dup.reason, dup.original_receipt),
        (Reason::Duplicate, first.receipt_id)
    );
    let mut other = a;
    other.requester = OWNER + 1;
    let foreign = rt.submit(other, OWNER + 1, 0, 0, &mut d);
    assert_eq!(
        (foreign.reason, foreign.original_receipt),
        (Reason::Authority, 0)
    );

    rt.cancel(OWNER, lease.id, 1, &mut d).unwrap();
    let fallback = rt.receipt(rt.receipt_count() - 1).unwrap();
    assert_eq!(
        (fallback.decision, fallback.dispatched),
        (Decision::Fallback, true)
    );
    assert_eq!((fallback.lease_id, fallback.principal), (lease.id, OWNER));
}

#[test]
fn config_accessor_returns_the_configured_values() {
    let mut d = Rec::default();
    let config = Config {
        watchdog_ms: 7,
        ..Config::default()
    };
    let rt = Runtime::new(config, &CAPABILITIES, BOOT, 0, 0, &mut d).unwrap();
    assert_eq!(rt.config().watchdog_ms, 7);
}

fn gripper() -> (Runtime, Rec, Lease) {
    let mut d = Rec::default();
    let mut rt = Runtime::new(Config::default(), &CAPABILITIES, BOOT, 0, 0, &mut d).unwrap();
    let lease = rt
        .acquire(
            LeaseRequest {
                owner: OWNER,
                resource: 1,
                capabilities: (1 << OPEN) | (1 << CLOSE),
                duration_ms: 500,
                limits: [Bounds::ZERO; 2],
            },
            0,
            &mut d,
        )
        .unwrap();
    (rt, d, lease)
}

fn grip(rt: &Runtime, lease: Lease, id: u64, capability: u16) -> Action {
    Action {
        capability,
        parameters: [0; 2],
        ..drive(rt, lease, id, [0; 2])
    }
}

#[test]
fn masked_state_verification_detects_a_stuck_gripper() {
    for cap in [OPEN, CLOSE] {
        let (mut rt, mut d, lease) = gripper();
        d.gripper_open = cap == CLOSE;
        d.stuck_gripper = true;
        let r = rt.submit(grip(&rt, lease, 1, cap), OWNER, 0, 0, &mut d);
        assert_eq!(
            (r.decision, r.reason),
            (Decision::Failed, Reason::Verification)
        );
        assert_eq!(rt.state(), RuntimeState::Faulted);
    }
    let (mut rt, mut d, lease) = gripper();
    for (id, cap) in [(1, OPEN), (2, CLOSE)] {
        let r = rt.submit(grip(&rt, lease, id, cap), OWNER, 0, 0, &mut d);
        assert_eq!(r.decision, Decision::Executed);
    }
}

#[test]
fn gripper_interlocks_block_both_directions() {
    for cap in [OPEN, CLOSE] {
        for flag in [ARM_MOVING, ESTOP] {
            let (mut rt, mut d, lease) = gripper();
            rt.update_state(flag, 0, &mut d);
            if flag == ARM_MOVING {
                let r = rt.submit(grip(&rt, lease, 1, cap), OWNER, 0, 0, &mut d);
                assert_eq!(r.reason, Reason::Precondition, "{cap} with {flag}");
            }
            assert_eq!(d.executions, 0);
        }
    }
}

#[test]
fn invalid_frame_flood_cannot_evict_authority_receipts() {
    let (mut rt, mut d, lease) = armed();
    for id in 1..=40 {
        let a = drive(&rt, lease, id, [1, 1]);
        assert_eq!(
            rt.submit(a, OWNER, 0, 0, &mut d).decision,
            Decision::Executed
        );
    }
    for id in 41..1000 {
        let mut a = drive(&rt, lease, id, [1, 1]);
        a.boot_id = 0;
        rt.submit(a, OWNER, 0, 0, &mut d);
    }
    let retained: Vec<_> = (0..rt.receipt_count())
        .map(|i| rt.receipt(i).unwrap())
        .collect();
    assert_eq!(retained.len(), RECEIPT_CAPACITY);
    let executed = retained
        .iter()
        .filter(|r| r.decision == Decision::Executed)
        .count();
    assert_eq!(executed, 40, "all executions survive the flood");
    assert_eq!(
        retained[0].decision,
        Decision::Control,
        "the lease grant survives too"
    );
    assert_eq!(
        retained.last().unwrap().receipt_id,
        d.log.last().unwrap().receipt_id
    );
}

#[test]
fn estop_signal_latches_once_per_raise() {
    let (mut rt, mut d, lease) = armed();
    let signal = EstopSignal::new();
    let a = drive(&rt, lease, 1, [9, 9]);
    rt.submit(a, OWNER, 0, 0, &mut d);
    assert!(!rt.poll_estop(&signal, 0, &mut d));
    assert_eq!(d.motor, [9, 9]);
    signal.raise();
    assert!(rt.poll_estop(&signal, 1, &mut d));
    assert_eq!((rt.state(), d.motor), (RuntimeState::Estopped, [0, 0]));
    assert!(
        !rt.poll_estop(&signal, 1, &mut d),
        "a handled raise does not re-trigger"
    );
    rt.update_state(0, 2, &mut d);
    assert_eq!(rt.recover_local(2, &mut d), Ok(()));
    signal.raise();
    signal.raise();
    assert!(rt.poll_estop(&signal, 3, &mut d));
    assert_eq!(rt.state(), RuntimeState::Estopped);
}

#[test]
fn unrelated_flags_do_not_block_and_required_flags_do() {
    let (mut rt, mut d, lease) = armed();
    rt.update_state(ARM_MOVING, 0, &mut d);
    let a = drive(&rt, lease, 1, [1, 1]);
    assert_eq!(
        rt.submit(a, OWNER, 0, 0, &mut d).decision,
        Decision::Executed
    );

    let mut cap = CAPABILITIES[0];
    cap.require_set = ARM_MOVING;
    assert!(cap.permits(ARM_MOVING) && cap.permits(ARM_MOVING | GRIPPER_OPEN << 8));
    assert!(!cap.permits(0) && !cap.permits(ARM_MOVING | OBSTACLE));
}
