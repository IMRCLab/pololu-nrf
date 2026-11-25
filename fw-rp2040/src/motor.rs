use embassy_rp::{
    gpio::Output,
    pwm::{PwmOutput, SetDutyCycle},
};

pub struct Motor<'a> {
    pwm: PwmOutput<'a>,
    direction: Output<'a>,
}

impl<'a> Motor<'a> {
    pub fn new(pwm: PwmOutput<'a>, direction: Output<'a>) -> Self {
        Self { pwm, direction }
    }

    pub fn set_speed(&mut self, speed: f32) {
        let speed = speed.clamp(-1.0, 1.0);

        if speed >= 0.0 {
            self.direction.set_high();
        } else {
            self.direction.set_low();
        }

        let duty = (speed.abs() * self.pwm.max_duty_cycle() as f32) as u16;

        self.pwm.set_duty_cycle(duty).unwrap();
    }

    pub fn stop(&mut self) {
        self.pwm.set_duty_cycle(0).unwrap();
    }
}

pub struct MotorController {
    left: Motor<'static>,
    right: Motor<'static>,
}

impl MotorController {
    pub fn new(left: Motor<'static>, right: Motor<'static>) -> Self {
        Self { left, right }
    }

    pub fn set_speed(&mut self, left_speed: f32, right_speed: f32) {
        self.left.set_speed(left_speed);
        self.right.set_speed(right_speed);
    }

    pub fn stop(&mut self) {
        self.left.stop();
        self.right.stop();
    }
}

pub fn init_motors(
    pwm_left: PwmOutput<'static>,
    pwm_right: PwmOutput<'static>,
    direction_left: Output<'static>,
    direction_right: Output<'static>,
) -> MotorController {
    let left = Motor::new(pwm_left, direction_left);
    let right = Motor::new(pwm_right, direction_right);

    MotorController::new(left, right)
}
