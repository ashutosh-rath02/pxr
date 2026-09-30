//! Reference profile only: units are mm/s and mrad/s; platforms may register their own table.
use crate::*;
pub const ESTOP: u32 = 1;
pub const OBSTACLE: u32 = 2;
pub const BATTERY_CRITICAL: u32 = 4;
pub const MOTOR_FAULT: u32 = 8;
pub const ARM_MOVING: u32 = 16;
pub const GRIPPER_OPEN: u32 = 1;
pub const DRIVE: u16 = 1;
pub const STOP: u16 = 2;
pub const OPEN: u16 = 3;
pub const CLOSE: u16 = 4;
pub const CAPABILITIES: [Capability; 4] = [
    Capability {
        id: DRIVE,
        resource: 0,
        class: ExecutionClass::Stream,
        parameter_count: 2,
        bounds: [
            Bounds {
                min: -1000,
                max: 1000,
            },
            Bounds {
                min: -2000,
                max: 2000,
            },
        ],
        require_set: 0,
        require_clear: ESTOP | OBSTACLE | BATTERY_CRITICAL | MOTOR_FAULT,
        verification: Verification::Parameters,
    },
    Capability {
        id: STOP,
        resource: 0,
        class: ExecutionClass::Control,
        parameter_count: 0,
        bounds: [Bounds::ZERO; 2],
        require_set: 0,
        require_clear: 0,
        verification: Verification::Parameters,
    },
    Capability {
        id: OPEN,
        resource: 1,
        class: ExecutionClass::Discrete,
        parameter_count: 0,
        bounds: [Bounds::ZERO; 2],
        require_set: 0,
        require_clear: ESTOP | ARM_MOVING,
        verification: Verification::State {
            mask: GRIPPER_OPEN,
            value: GRIPPER_OPEN,
        },
    },
    Capability {
        id: CLOSE,
        resource: 1,
        class: ExecutionClass::Discrete,
        parameter_count: 0,
        bounds: [Bounds::ZERO; 2],
        require_set: 0,
        require_clear: ESTOP | ARM_MOVING,
        verification: Verification::State {
            mask: GRIPPER_OPEN,
            value: 0,
        },
    },
];
