//! Wheel speed control: turns a body velocity target (v, ω) into motor duty cycles.

use crate::encoder::Encoders;
use crate::motor::MotorController;

const WHEEL_RADIUS: f32 = 0.016; // m
const WHEEL_BASE: f32 = 0.0842; // m

fn wheel_targets(v: f32, omega: f32) -> (f32, f32) {
    let w_left = (v - omega * WHEEL_BASE / 2.0) / WHEEL_RADIUS;
    let w_right = (v + omega * WHEEL_BASE / 2.0) / WHEEL_RADIUS;

    (w_left, w_right) // rad/s
}

pub const DT_MS: u64 = 1;
const DT_S: f32 = DT_MS as f32 / 1000.0;

/// Encoder counts per wheel revolution: 12 per motor revolution × 15.25 gearbox.
/// Negative: the encoders count down when the wheel turns forward.
const ENCODER_CPR: f32 = -183.0;

/// Cutoff frequency of the speed low-pass filter in Hz
const FILTER_HZ: f32 = 3.0;
/// Weight of each new measurement: `dt / (tau + dt)` with `tau = 1 / (2π f)`
const FILTER_ALPHA: f32 = DT_S / (1.0 / (2.0 * core::f32::consts::PI * FILTER_HZ) + DT_S);

const KP: f32 = 0.01;
const KI: f32 = 0.02;

/// Below this target speed in rad/s on both wheels, the motors are switched off
const STOP_THRESHOLD: f32 = 0.1;

/// Speed measurement and PI controller of one wheel
struct Wheel {
    previous_count: i32,
    speed: f32,    // rad/s, low-pass filtered
    integral: f32, // accumulated `KI * error * dt`, i.e. already in duty
}

impl Wheel {
    fn new(count: i32) -> Self {
        Self {
            previous_count: count,
            speed: 0.0,
            integral: 0.0,
        }
    }

    /// Measures the speed from the encoder `count` and returns the duty cycle in `[-1, 1]`
    /// that drives it towards `target` (rad/s).
    fn update(&mut self, count: i32, target: f32) -> f32 {
        let delta = count.wrapping_sub(self.previous_count);
        self.previous_count = count;
        let measured = delta as f32 * 2.0 * core::f32::consts::PI / ENCODER_CPR / DT_S;
        self.speed += FILTER_ALPHA * (measured - self.speed);

        let error = target - self.speed;
        // Anti-windup clamp
        self.integral = (self.integral + KI * error * DT_S).clamp(-1.0, 1.0);
        (KP * error + self.integral).clamp(-1.0, 1.0)
    }
}

/// Drives the robot at a body velocity. Owns the motors and encoders.
pub struct Control {
    motors: MotorController,
    encoders: Encoders,
    left: Wheel,
    right: Wheel,
}

impl Control {
    pub fn new(motors: MotorController, encoders: Encoders) -> Self {
        let (count_left, count_right) = encoders.counts();
        Self {
            motors,
            encoders,
            left: Wheel::new(count_left),
            right: Wheel::new(count_right),
        }
    }

    /// Call every `DT_MS` with forward speed `v` in m/s and turn rate `omega` in rad/s.
    pub fn update(&mut self, v: f32, omega: f32) {
        let (target_left, target_right) = wheel_targets(v, omega);
        let (count_left, count_right) = self.encoders.counts();

        let duty_left = self.left.update(count_left, target_left);
        let duty_right = self.right.update(count_right, target_right);

        if target_left.abs() < STOP_THRESHOLD && target_right.abs() < STOP_THRESHOLD {
            self.left.integral = 0.0;
            self.right.integral = 0.0;
            self.motors.stop();
        } else {
            self.motors.set_speed(duty_left, duty_right);
        }
    }
}
