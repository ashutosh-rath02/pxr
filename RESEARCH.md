# Feasibility and direction review

Reviewed 30 September 2026 against the revised PRD. This is a targeted comparison
of primary sources, not proof of novelty or an exhaustive market survey.

## Verdict

Proceed with the revised **generic, embedded-first v0 simulation scope**. Rust
`no_std`, bounded arrays, static policies and synchronous driver interfaces are
technically suitable. The useful hypothesis is a reusable controller-side action
contract across devices. Broad claims to have invented deterministic AI gating,
leases, watchdogs, runtime assurance or physical safety are unsupported.

The project earns further integration work if engineers can reuse its core and
failure semantics more easily than maintaining a small device-specific wrapper.
Simulation can validate the software contract; a second physical port and user
feedback are needed to establish that product value.

## Relevant overlap

| Primary source | What it establishes | Consequence for PXR |
|---|---|---|
| [EdgeEmbed Runtime](https://edgeembed.com/runtime/) and [getting started](https://edgeembed.com/docs/getting-started/) | Deterministic policy decisions, replay, C engine; public SDK with a separate closed engine evaluated on Linux | Strong conceptual overlap. PXR's open source, fixed memory, embedded core must be the useful distinction |
| [SINT Protocol](https://github.com/sint-ai/sint-protocol) | TypeScript governance stack with capabilities, physical constraints, revocation and evidence | Possible upstream authority provider; avoid cloning its control plane |
| [ROSClaw](https://github.com/ros-claw/rosclaw) | Governed physical-agent actions and verification above robot middleware | Keep PXR usable below an agent runtime |
| [AEROS paper](https://arxiv.org/abs/2604.07039) | Embodied-agent operating architecture with capability modules | Broader agent lifecycle than this execution core |
| [Invariant](https://github.com/clay-good/invariant) | Rust command-validation system with signed policies and evidence, robotics and biosynthesis surfaces | Additional material overlap beyond the earlier PRD. “Rust physical action firewall” is not a sufficient differentiator |
| [Ori](https://github.com/ori-platform/ori-runtime) | Agentic IoT runtime with graduated actuation authority and reasoning on a Pi | Shows growing demand and competition at the host layer |
| [Sedaro SAFE](https://github.com/sedaro/safe) | Mission autonomy framework and simulation-based gatekeeping | Related assurance architecture, different scope and workload |
| [PX4 Offboard](https://docs.px4.io/main/en/flight_modes/offboard) | External control proof-of-life and local failsafe on loss | Reuse that established authority/fallback pattern |
| [NASA runtime assurance framework](https://ntrs.nasa.gov/citations/20240007986) | Formal framework for monitoring and trusted reversionary behavior | PXR can be a component of assurance; it does not itself establish a complete assurance case |
| [HACP draft](https://datatracker.ietf.org/doc/draft-sunyi-hacp-protocol/) and [Model Hardware Standard preview](https://www.anthropic.com/news/model-hardware-standard-research-preview) | Higher-level hardware capability contracts and interoperability efforts | Keep these above PXR; adapters can follow stable core semantics |
| [Espressif esp-hal](https://docs.espressif.com/projects/rust/esp-hal/1.2.0/esp32/esp_hal/index.html) | Existing `no_std` ecosystem for embedded targets | Supports language feasibility, not this project's board qualification |

## Necessary corrections to the vision

1. **Exactly once:** impossible to promise globally around crashes and uncertain
   actuator effects without a much stronger durable transaction contract. V0
   implements volatile bounded duplicate suppression and sequence fencing.
2. **Safe:** static bounds and predicates enforce a configured policy. Correct
   physical constraints, trustworthy sensing and independent protection still
   belong to a complete device design.
3. **Time:** receive TTL cannot reveal time spent in transit. V0 also uses deadlines
   in the controller's clock domain, with a conservative snapshot-based host contract.
4. **Authority:** an owner ID and lease are not authentication. Trusted control-plane
   calls establish authority and the ingress adapter supplies the verified principal.
5. **Watchdog:** stalled software cannot invoke its own fallback. An independent
   hardware watchdog is mandatory for physical integrations.
6. **Verification:** a driver's immediate readback supports only the observation
   contract that driver implements. It is not independent hardware attestation.
7. **Real time:** deterministic decisions and fixed storage do not prove WCET.
   Measure on the actual MCU/RTOS with the actual callbacks and interference.

These corrections are reflected in the implementation and contract documents.
The user's PRD remains unchanged as the product-intent source.

## Next decision gate

Compare this library with a minimal direct-driver wrapper using the same fault
matrix and physical device. Proceed if reusable semantics and evidence justify
the added code and latency. Narrow or stop if another maintained embedded runtime
already covers the required contract with less integration burden, or a second
device requires changing the core rather than implementing its driver/profile.
