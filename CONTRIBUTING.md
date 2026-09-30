# Contributing

Start with ARCHITECTURE.md, ACTION_ABI.md and SAFETY_MODEL.md. Keep dependencies,
allocation and transport code out of the core. Changes to timing, authority,
replay or failure behavior need a contract update and a meaningful conformance test.

Run formatting, Clippy with warnings denied, workspace tests, the demo, and both
embedded cross-builds in V0_TEST_PLAN.md. If changing the ABI, also compile and run
the C example and update the header. Preserve the original PRD as product intent;
record engineering decisions in the contracts.

Discuss new platform or ecosystem integrations only after the execution semantics
are tested. Include target/toolchain details with benchmark reports. Never label
simulated results as physical safety evidence or a maximum observed sample as WCET.
