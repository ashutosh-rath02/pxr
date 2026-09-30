use crate::*;

#[derive(Clone, Copy)]
struct Active {
    capability: Capability,
    expires_at: u64,
}
#[derive(Clone, Copy)]
struct Replay {
    owner: u64,
    action_id: u64,
    receipt: u64,
}

/// Single-owner runtime. Fixed storage; no queues, threads, allocator or clock access.
pub struct Runtime {
    config: Config,
    boot_id: u64,
    capabilities: [Option<Capability>; MAX_CAPABILITIES],
    resources: [bool; MAX_RESOURCES],
    leases: [Option<Lease>; MAX_RESOURCES],
    active: [Option<Active>; MAX_RESOURCES],
    replay: [Option<Replay>; REPLAY_CAPACITY],
    replay_cursor: usize,
    receipts: [Option<Receipt>; RECEIPT_CAPACITY],
    receipt_cursor: usize,
    receipt_count: usize,
    next_receipt: u64,
    next_lease: u64,
    epoch: u64,
    flags: u32,
    state_at: u64,
    last_tick: u64,
    state: RuntimeState,
    invalid_count: u16,
}

impl Runtime {
    pub fn new(
        config: Config,
        capabilities: &[Capability],
        boot_id: u64,
        now: u64,
        initial_flags: u32,
        driver: &mut impl Driver,
    ) -> Result<Self, ConfigError> {
        if boot_id == 0 {
            return Err(ConfigError::InvalidBootId);
        }
        if capabilities.is_empty()
            || capabilities.len() > MAX_CAPABILITIES
            || config.watchdog_ms == 0
            || config.state_ttl_ms == 0
            || config.max_lease_ms == 0
            || config.max_valid_for_ms == 0
            || config.invalid_limit == 0
            || config.estop_mask == 0
        {
            return Err(ConfigError::InvalidConfig);
        }
        let mut table = [None; MAX_CAPABILITIES];
        let mut resources = [false; MAX_RESOURCES];
        for (i, cap) in capabilities.iter().enumerate() {
            if cap.id == 0
                || cap.id >= 64
                || cap.resource as usize >= MAX_RESOURCES
                || cap.parameter_count > 2
                || cap.require_set & cap.require_clear != 0
                || cap.bounds.iter().any(|b| b.min > b.max)
                || cap.bounds[cap.parameter_count as usize..]
                    .iter()
                    .any(|b| *b != Bounds::ZERO)
                || matches!(cap.verification, Verification::State { mask, value } if mask == 0 || value & !mask != 0)
            {
                return Err(ConfigError::InvalidCapability);
            }
            if table[..i]
                .iter()
                .flatten()
                .any(|c: &Capability| c.id == cap.id)
            {
                return Err(ConfigError::DuplicateCapability);
            }
            table[i] = Some(*cap);
            resources[cap.resource as usize] = true;
        }
        // Attempt every configured fallback, even when an earlier one fails.
        let mut failed = false;
        for (i, used) in resources.iter().enumerate() {
            if *used && driver.fallback(i as u8, Reason::Startup).is_err() {
                failed = true;
            }
        }
        if failed {
            return Err(ConfigError::Driver);
        }
        Ok(Self {
            config,
            boot_id,
            capabilities: table,
            resources,
            leases: [None; MAX_RESOURCES],
            active: [None; MAX_RESOURCES],
            replay: [None; REPLAY_CAPACITY],
            replay_cursor: 0,
            receipts: [None; RECEIPT_CAPACITY],
            receipt_cursor: 0,
            receipt_count: 0,
            next_receipt: 1,
            next_lease: 1,
            epoch: 1,
            flags: initial_flags,
            state_at: now,
            last_tick: now,
            state: if initial_flags & config.estop_mask != 0 {
                RuntimeState::Estopped
            } else {
                RuntimeState::SafeIdle
            },
            invalid_count: 0,
        })
    }

    pub fn state(&self) -> RuntimeState {
        self.state
    }
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    pub fn boot_id(&self) -> u64 {
        self.boot_id
    }
    pub fn flags(&self) -> u32 {
        self.flags
    }
    pub fn lease(&self, resource: u8) -> Option<Lease> {
        self.leases.get(resource as usize).copied().flatten()
    }
    pub fn capabilities(&self) -> impl Iterator<Item = &Capability> {
        self.capabilities.iter().flatten()
    }
    pub fn receipt_count(&self) -> usize {
        self.receipt_count
    }
    pub fn config(&self) -> Config {
        self.config
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            boot_id: self.boot_id,
            epoch: self.epoch,
            now: self.last_tick,
            sensor_tick: self.state_at,
            flags: self.flags,
            state: self.state,
            active_leases: self.leases.iter().flatten().count() as u8,
            retained_receipts: self.receipt_count,
            next_receipt_id: self.next_receipt,
        }
    }
    /// Oldest retained receipt first. Overwrite is bounded and observable via receipt IDs.
    pub fn receipt(&self, index: usize) -> Option<Receipt> {
        if index >= self.receipt_count {
            return None;
        }
        self.receipts[(self.receipt_cursor + RECEIPT_CAPACITY - self.receipt_count + index)
            % RECEIPT_CAPACITY]
    }

    fn bump_epoch(&mut self) {
        self.epoch = self.epoch.saturating_add(1);
    }
    fn record(&mut self, mut receipt: Receipt) -> Receipt {
        receipt.receipt_id = self.next_receipt;
        self.next_receipt = self.next_receipt.saturating_add(1);
        self.receipts[self.receipt_cursor] = Some(receipt);
        self.receipt_cursor = (self.receipt_cursor + 1) % RECEIPT_CAPACITY;
        self.receipt_count = (self.receipt_count + 1).min(RECEIPT_CAPACITY);
        receipt
    }
    fn event(
        &mut self,
        resource: u8,
        lease_id: u64,
        principal: u64,
        decision: Decision,
        reason: Reason,
        now: u64,
    ) {
        self.record(Receipt {
            boot_id: self.boot_id,
            principal,
            safety_flags: self.flags,
            receipt_id: 0,
            action_id: 0,
            sequence: 0,
            capability: 0,
            resource,
            lease_id,
            decision,
            reason,
            epoch: self.epoch,
            received_at: now,
            admitted_at: now,
            executed_at: now,
            requested: [0; 2],
            observed: [0; 2],
            pre_state: self.flags,
            post_state: self.flags,
            original_receipt: 0,
            dispatched: decision == Decision::Fallback,
            observed_valid: false,
        });
    }
    fn refresh_state(&mut self) {
        if matches!(self.state, RuntimeState::SafeIdle | RuntimeState::Armed) {
            self.state = if self.leases.iter().any(Option::is_some) {
                RuntimeState::Armed
            } else {
                RuntimeState::SafeIdle
            };
        }
    }
    fn fallback(
        &mut self,
        resource: usize,
        reason: Reason,
        now: u64,
        driver: &mut impl Driver,
    ) -> bool {
        let lease = self.leases[resource].map_or(0, |l| l.id);
        let principal = self.leases[resource].map_or(0, |l| l.owner);
        self.active[resource] = None;
        self.leases[resource] = None;
        self.bump_epoch();
        let ok = driver.fallback(resource as u8, reason).is_ok();
        self.event(
            resource as u8,
            lease,
            principal,
            Decision::Fallback,
            if ok { reason } else { Reason::Driver },
            now,
        );
        ok
    }
    fn trip(&mut self, reason: Reason, now: u64, driver: &mut impl Driver) {
        self.state = if reason == Reason::Estop || self.state == RuntimeState::Estopped {
            RuntimeState::Estopped
        } else {
            RuntimeState::Faulted
        };
        for i in 0..MAX_RESOURCES {
            if self.resources[i] {
                self.fallback(i, reason, now, driver);
            }
        }
    }

    /// Service at least once per configured watchdog period, independently of ingress traffic.
    pub fn tick(&mut self, now: u64, driver: &mut impl Driver) {
        if now < self.last_tick {
            self.trip(Reason::Clock, now, driver);
            return;
        }
        if self.epoch == u64::MAX || self.next_receipt == u64::MAX || self.next_lease == u64::MAX {
            self.trip(Reason::Faulted, now, driver);
            return;
        }
        if now - self.last_tick > self.config.watchdog_ms as u64
            && self.state == RuntimeState::Armed
        {
            self.trip(Reason::Watchdog, now, driver);
        }
        self.last_tick = now;
        if now - self.state_at >= self.config.state_ttl_ms as u64
            && self.state == RuntimeState::Armed
        {
            self.trip(Reason::StateStale, now, driver);
        }
        for i in 0..MAX_RESOURCES {
            let reason = if self.leases[i].is_some_and(|l| now >= l.expires_at) {
                Some(Reason::LeaseExpired)
            } else if self.active[i].is_some_and(|a| now >= a.expires_at) {
                Some(Reason::StreamExpired)
            } else {
                None
            };
            if let Some(reason) = reason {
                if !self.fallback(i, reason, now, driver) {
                    self.trip(Reason::Driver, now, driver);
                }
            }
        }
        self.refresh_state();
    }

    /// Trusted local sensor update. Changing flags invalidates the decision epoch.
    pub fn update_state(&mut self, flags: u32, now: u64, driver: &mut impl Driver) {
        self.tick(now, driver);
        if now < self.last_tick {
            return;
        }
        self.state_at = now;
        if flags == self.flags {
            return;
        }
        self.flags = flags;
        self.bump_epoch();
        if flags & self.config.estop_mask != 0 {
            self.trip(Reason::Estop, now, driver);
            return;
        }
        for i in 0..MAX_RESOURCES {
            if self.active[i].is_some_and(|a| !a.capability.permits(flags))
                && !self.fallback(i, Reason::Precondition, now, driver)
            {
                self.trip(Reason::Driver, now, driver);
            }
        }
        self.refresh_state();
    }

    pub fn emergency_stop(&mut self, now: u64, driver: &mut impl Driver) {
        // E-stop always wins, even with a regressed timestamp. It also fences subsequent
        // state updates/recovery against an older clock value.
        self.last_tick = self.last_tick.max(now);
        self.flags |= self.config.estop_mask;
        self.trip(Reason::Estop, self.last_tick, driver);
    }

    /// This API must only be exposed to a trusted local operator/interlock.
    /// A sensor update must clear the physical e-stop before recovery.
    pub fn recover_local(&mut self, now: u64, driver: &mut impl Driver) -> Result<(), Reason> {
        self.tick(now, driver);
        if self.flags & self.config.estop_mask != 0 {
            return Err(Reason::Estop);
        }
        if now < self.last_tick
            || now - self.state_at >= self.config.state_ttl_ms as u64
            || self.epoch == u64::MAX
            || self.next_receipt == u64::MAX
            || self.next_lease == u64::MAX
        {
            return Err(Reason::Precondition);
        }
        let mut ok = true;
        for i in 0..MAX_RESOURCES {
            if self.resources[i] {
                ok &= self.fallback(i, Reason::Recovered, now, driver);
            }
        }
        if !ok {
            self.state = RuntimeState::Faulted;
            return Err(Reason::Driver);
        }
        self.state = RuntimeState::SafeIdle;
        self.invalid_count = 0;
        Ok(())
    }

    fn ready(&self, now: u64) -> Result<(), Reason> {
        if self.state == RuntimeState::Estopped {
            return Err(Reason::Estop);
        }
        if self.state == RuntimeState::Faulted {
            return Err(Reason::Faulted);
        }
        if now < self.last_tick {
            return Err(Reason::Clock);
        }
        if now - self.state_at >= self.config.state_ttl_ms as u64 {
            return Err(Reason::StateStale);
        }
        Ok(())
    }

    /// Called by a trusted authority adapter after authenticating and authorizing the principal.
    pub fn acquire(
        &mut self,
        request: LeaseRequest,
        now: u64,
        driver: &mut impl Driver,
    ) -> Result<Lease, Reason> {
        self.tick(now, driver);
        self.ready(now)?;
        let i = request.resource as usize;
        if i >= MAX_RESOURCES
            || !self.resources[i]
            || request.owner == 0
            || request.capabilities == 0
            || request.duration_ms == 0
            || request.duration_ms > self.config.max_lease_ms
            || request.limits.iter().any(|b| b.min > b.max)
        {
            return Err(Reason::Invalid);
        }
        let allowed = self
            .capabilities()
            .filter(|c| c.resource == request.resource)
            .fold(0, |m, c| m | (1u64 << c.id));
        if request.capabilities & !allowed != 0 {
            return Err(Reason::Authority);
        }
        if self.leases[i].is_some() {
            return Err(Reason::Busy);
        }
        let expires_at = now
            .checked_add(request.duration_ms as u64)
            .ok_or(Reason::Invalid)?;
        let lease = Lease {
            id: self.next_lease,
            owner: request.owner,
            resource: request.resource,
            capabilities: request.capabilities,
            expires_at,
            limits: request.limits,
            last_sequence: 0,
            last_renewal: 0,
        };
        self.next_lease += 1;
        self.leases[i] = Some(lease);
        self.bump_epoch();
        self.refresh_state();
        self.event(
            request.resource,
            lease.id,
            lease.owner,
            Decision::Control,
            Reason::LeaseGranted,
            now,
        );
        Ok(lease)
    }

    /// Renewal counters are independent of action sequences; replay cannot extend authority.
    pub fn renew(
        &mut self,
        owner: u64,
        lease_id: u64,
        renewal: u64,
        duration_ms: u32,
        now: u64,
        driver: &mut impl Driver,
    ) -> Result<Lease, Reason> {
        self.tick(now, driver);
        self.ready(now)?;
        if duration_ms == 0 || duration_ms > self.config.max_lease_ms {
            return Err(Reason::Invalid);
        }
        let i = self
            .leases
            .iter()
            .position(|l| l.is_some_and(|l| l.id == lease_id && l.owner == owner))
            .ok_or(Reason::Authority)?;
        let mut lease = self.leases[i].ok_or(Reason::Authority)?;
        if renewal <= lease.last_renewal {
            return Err(Reason::OldSequence);
        }
        lease.expires_at = now.checked_add(duration_ms as u64).ok_or(Reason::Invalid)?;
        lease.last_renewal = renewal;
        self.leases[i] = Some(lease);
        self.event(
            i as u8,
            lease_id,
            lease.owner,
            Decision::Control,
            Reason::LeaseRenewed,
            now,
        );
        Ok(lease)
    }

    pub fn cancel(
        &mut self,
        owner: u64,
        lease_id: u64,
        now: u64,
        driver: &mut impl Driver,
    ) -> Result<(), Reason> {
        self.tick(now, driver);
        let i = self
            .leases
            .iter()
            .position(|l| l.is_some_and(|l| l.id == lease_id && l.owner == owner))
            .ok_or(Reason::Authority)?;
        if !self.fallback(i, Reason::Canceled, now, driver) {
            self.trip(Reason::Driver, now, driver);
            return Err(Reason::Driver);
        }
        self.refresh_state();
        Ok(())
    }

    fn validate(
        &self,
        action: &Action,
        principal: u64,
        received: u64,
        now: u64,
    ) -> Result<(Capability, u64), Reason> {
        self.ready(now)?;
        if action.boot_id != self.boot_id {
            return Err(Reason::BootMismatch);
        }
        if principal == 0 || principal != action.requester {
            return Err(Reason::Authority);
        }
        let cap = *self
            .capabilities()
            .find(|c| c.id == action.capability)
            .ok_or(Reason::UnknownCapability)?;
        let lease = self.leases[cap.resource as usize].ok_or(Reason::Authority)?;
        if lease.id != action.lease_id
            || lease.owner != principal
            || lease.capabilities & (1u64 << cap.id) == 0
        {
            return Err(Reason::Authority);
        }
        if action.action_id == 0
            || action.sequence == 0
            || received > now
            || action.valid_for_ms == 0
            || action.valid_for_ms > self.config.max_valid_for_ms
            || action.execute_after >= action.deadline
        {
            return Err(Reason::Invalid);
        }
        if self
            .replay
            .iter()
            .flatten()
            .any(|r| r.owner == principal && r.action_id == action.action_id)
        {
            return Err(Reason::Duplicate);
        }
        if action.sequence <= lease.last_sequence {
            return Err(Reason::OldSequence);
        }
        let expires = received
            .checked_add(action.valid_for_ms as u64)
            .ok_or(Reason::Invalid)?;
        if now >= action.deadline || now >= expires {
            return Err(Reason::Stale);
        }
        if now < action.execute_after {
            return Err(Reason::TooEarly);
        }
        if action.based_on_epoch != self.epoch {
            return Err(Reason::Epoch);
        }
        for n in 0..2 {
            if !cap.bounds[n].contains(action.parameters[n])
                || !lease.limits[n].contains(action.parameters[n])
            {
                return Err(Reason::Bound);
            }
        }
        if !cap.permits(self.flags) {
            return Err(Reason::Precondition);
        }
        Ok((cap, expires.min(lease.expires_at)))
    }

    /// Executes synchronously. `principal` and `received_at` come from the trusted adapter,
    /// never from untrusted frame fields. Callbacks must complete within the platform budget.
    pub fn submit(
        &mut self,
        action: Action,
        principal: u64,
        received_at: u64,
        now: u64,
        driver: &mut impl Driver,
    ) -> Receipt {
        self.tick(now, driver);
        let mut receipt = Receipt {
            boot_id: self.boot_id,
            principal,
            safety_flags: self.flags,
            receipt_id: 0,
            action_id: action.action_id,
            sequence: action.sequence,
            capability: action.capability,
            resource: self
                .capabilities()
                .find(|c| c.id == action.capability)
                .map_or(u8::MAX, |c| c.resource),
            lease_id: action.lease_id,
            decision: Decision::Rejected,
            reason: Reason::Ok,
            epoch: self.epoch,
            received_at,
            admitted_at: now,
            executed_at: 0,
            requested: action.parameters,
            observed: [0; 2],
            pre_state: 0,
            post_state: 0,
            original_receipt: 0,
            dispatched: false,
            observed_valid: false,
        };
        let (cap, expires_at) = match self.validate(&action, principal, received_at, now) {
            Ok(v) => v,
            Err(reason) => {
                receipt.reason = reason;
                if reason == Reason::Duplicate {
                    receipt.original_receipt = self
                        .replay
                        .iter()
                        .flatten()
                        .find(|r| r.owner == principal && r.action_id == action.action_id)
                        .map_or(0, |r| r.receipt);
                }
                self.invalid_count = self.invalid_count.saturating_add(1);
                let result = self.record(receipt);
                if self.invalid_count >= self.config.invalid_limit
                    && self.state == RuntimeState::Armed
                {
                    self.trip(Reason::InvalidStorm, now, driver);
                }
                return result;
            }
        };
        let i = cap.resource as usize;
        receipt.resource = cap.resource;
        // Consume the sequence and ID BEFORE dispatch: an uncertain driver outcome is never retried.
        if let Some(l) = self.leases[i].as_mut() {
            l.last_sequence = action.sequence;
        }
        self.replay[self.replay_cursor] = Some(Replay {
            owner: principal,
            action_id: action.action_id,
            receipt: self.next_receipt,
        });
        self.replay_cursor = (self.replay_cursor + 1) % REPLAY_CAPACITY;
        let result = (|| {
            let before = driver.observe(cap.resource).map_err(|_| Reason::Driver)?;
            receipt.pre_state = before.state;
            receipt.dispatched = true;
            driver
                .execute(cap.id, action.parameters)
                .map_err(|_| Reason::Driver)?;
            receipt.executed_at = now;
            let after = driver.observe(cap.resource).map_err(|_| Reason::Driver)?;
            receipt.observed = after.parameters;
            receipt.post_state = after.state;
            receipt.observed_valid = true;
            let verified = match cap.verification {
                Verification::Parameters => after.parameters == action.parameters,
                Verification::State { mask, value } => after.state & mask == value,
            };
            if !verified {
                return Err(Reason::Verification);
            }
            Ok(())
        })();
        match result {
            Ok(()) => {
                receipt.decision = Decision::Executed;
                self.invalid_count = 0;
                // Discrete actions remain supervised for state and lease loss. Only streams expire with TTL.
                self.active[i] = if cap.class == ExecutionClass::Control {
                    None
                } else {
                    Some(Active {
                        capability: cap,
                        expires_at: if cap.class == ExecutionClass::Stream {
                            expires_at
                        } else {
                            u64::MAX
                        },
                    })
                };
                self.record(receipt)
            }
            Err(reason) => {
                receipt.decision = Decision::Failed;
                receipt.reason = reason;
                let receipt = self.record(receipt);
                self.trip(reason, now, driver);
                receipt
            }
        }
    }
}
