use pxr_runtime_core::Config;
use pxr_runtime_sim::plant::*;

fn check(config: Config, model: MotorModel, timing: Loop) {
    for fault in FAULTS {
        let o = run(fault, model, config, timing);
        let detected = o.detected_after_ms.expect("fault never detected");
        let stopped = o
            .stopped_after_ms
            .expect("motor never moved after the fault");
        assert!(detected <= o.detection_bound_ms, "{o:?}");
        assert!(stopped <= o.stop_bound_ms, "{o:?}");
        let cruise = timing.cruise_mm_s as f64;
        let worst_coast = cruise * (o.detection_bound_ms + model.command_delay_ms) as f64 / 1000.0
            + cruise * model.time_constant_ms / 1000.0;
        assert!(o.coast_mm <= worst_coast + 1.0, "{o:?}");
    }
}

#[test]
fn every_fault_stops_the_motor_within_its_configured_bound() {
    check(Config::default(), MotorModel::default(), Loop::default());
}

#[test]
fn bounds_track_configuration_and_plant_changes() {
    let config = Config {
        state_ttl_ms: 120,
        watchdog_ms: 30,
        ..Config::default()
    };
    let model = MotorModel {
        time_constant_ms: 120.0,
        command_delay_ms: 15,
    };
    let timing = Loop {
        supervisor_period_ms: 5,
        sensor_period_ms: 5,
        command_valid_for_ms: 40,
        ..Loop::default()
    };
    check(config, model, timing);
}

#[test]
fn estop_is_detected_immediately_and_stall_needs_resumption() {
    let o = run(
        Fault::EstopPressed,
        MotorModel::default(),
        Config::default(),
        Loop::default(),
    );
    assert_eq!(o.detected_after_ms, Some(0));
    let o = run(
        Fault::SupervisorStall(200),
        MotorModel::default(),
        Config::default(),
        Loop::default(),
    );
    // Without a hardware watchdog, nothing inside PXR can act while its own thread is stalled.
    assert!(o.detected_after_ms.unwrap() >= 200);
}
