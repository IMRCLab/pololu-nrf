#![no_std]
#![no_main]

use embassy_rp::gpio::{Level, Output};
use embassy_time::{Duration, Ticker, Timer};

use embassy_rp::pwm::{Config as PwmConfig, Pwm};
use panic_halt as _;

mod board {
    pub mod encoder;
    pub mod motor;
}
use crate::board::{encoder, motor::init_motors};

#[embassy_executor::task]
async fn led_blink_task(mut led: Output<'static>) {
    loop {
        led.toggle();
        Timer::after(Duration::from_millis(500)).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let p = embassy_rp::init(Default::default());
    let led = Output::new(p.PIN_25, Level::High);

    spawner.spawn(led_blink_task(led).unwrap());

    let _encoders = encoder::Encoders::new(p.PIO0, p.PIN_12, p.PIN_13, p.PIN_8, p.PIN_9);
    let _ = _encoders.counts();

    let direction_left = Output::new(p.PIN_11, Level::Low);
    let direction_right = Output::new(p.PIN_10, Level::Low);
    let pwm_left = p.PIN_15;
    let pwm_right = p.PIN_14;

    let mut pwm_config = PwmConfig::default();
    pwm_config.top = 10_000;

    let pwm = Pwm::new_output_ab(p.PWM_SLICE7, pwm_right, pwm_left, pwm_config.clone());

    let (pwm_right, pwm_left) = pwm.split();

    let mut motors = init_motors(
        pwm_left.expect("left motor PWM channel missing"),
        pwm_right.expect("right motor PWM channel missing"),
        direction_left,
        direction_right,
    );

    motors.stop();

    // test spin (no encoder readings used yet)
    let mut ticker = Ticker::every(Duration::from_millis(1));
    let mut speed: f32 = 0.0;
    let mut delta: f32 = 0.0001;
    let max_duty = 0.3;
    loop {
        ticker.next().await;
        speed += delta;
        if speed.abs() >= max_duty {
            delta *= -1.0;
        }
        motors.set_speed(speed, -speed);
    }
}
