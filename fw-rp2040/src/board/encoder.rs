//! Quadrature encoders, read by PIO0 state machines 0 (left) and 1 (right).
//! On every change of the A/B pins, a state machine pushes `current << 2 | previous`.

use core::sync::atomic::{AtomicI32, Ordering};

use embassy_rp::interrupt::typelevel::{Handler, PIO0_IRQ_0};
use embassy_rp::peripherals::{PIN_8, PIN_9, PIN_12, PIN_13, PIO0};
use embassy_rp::pio::{
    Common, Config, Direction, FifoJoin, InterruptHandler, LoadedProgram, Pio, PioPin,
    ShiftDirection, StateMachine,
};
use embassy_rp::pio_programs::clock_divider::calculate_pio_clock_divider;
use embassy_rp::{Peri, bind_interrupts, pac};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>, IrqHandler;
});

/// Owns the running state machines. Counting stops when this is dropped.
pub struct Encoders {
    _left: StateMachine<'static, PIO0, 0>,
    _right: StateMachine<'static, PIO0, 1>,
}

impl Encoders {
    pub fn new(
        pio: Peri<'static, PIO0>,
        left_a: Peri<'static, PIN_12>,
        left_b: Peri<'static, PIN_13>,
        right_a: Peri<'static, PIN_8>,
        right_b: Peri<'static, PIN_9>,
    ) -> Self {
        let Pio {
            mut common,
            sm0,
            sm1,
            ..
        } = Pio::new(pio, Irqs);
        let program = load_program(&mut common);
        Self {
            _left: init_sm(&mut common, sm0, &program, left_a, left_b),
            _right: init_sm(&mut common, sm1, &program, right_a, right_b),
        }
    }

    /// Encoder counts since boot: `(left, right)`
    pub fn counts(&self) -> (i32, i32) {
        (
            LEFT_COUNT.load(Ordering::Relaxed),
            RIGHT_COUNT.load(Ordering::Relaxed),
        )
    }
}

/// Loads the encoder program into PIO0's instruction memory (shared by both state machines).
fn load_program<'d>(common: &mut Common<'d, PIO0>) -> LoadedProgram<'d, PIO0> {
    let prg = embassy_rp::pio::program::pio_asm!(
        "in pins, 2"        // ISR = A/B
        "mov x, isr"        // X = previous A/B
        ".wrap_target"
        "sample:"
        "mov isr, null"
        "in pins, 2"        // ISR = current A/B
        "mov y, isr"        // Y = current A/B
        "jmp x!=y changed"
        ".wrap"             // unchanged: sample again
        "changed:"
        "in x, 2"           // ISR = current << 2 | previous
        "push"
        "mov x, y"          // previous = current
        "jmp sample"
    );
    common.load_program(&prg.program)
}

/// Starts one state machine on the encoder pins `a` and `b`.
fn init_sm<'d, const SM: usize>(
    pio: &mut Common<'d, PIO0>,
    mut sm: StateMachine<'d, PIO0, SM>,
    program: &LoadedProgram<'d, PIO0>,
    a: Peri<'d, impl PioPin>,
    b: Peri<'d, impl PioPin>,
) -> StateMachine<'d, PIO0, SM> {
    let a = pio.make_pio_pin(a);
    let b = pio.make_pio_pin(b);
    sm.set_pin_dirs(Direction::In, &[&a, &b]);

    let mut cfg = Config::default();
    cfg.set_in_pins(&[&a, &b]);
    cfg.shift_in.direction = ShiftDirection::Left;
    cfg.fifo_join = FifoJoin::RxOnly; // 8-entry RX FIFO
    cfg.clock_divider = calculate_pio_clock_divider(12_000_000); // TODO try 125MHz

    cfg.use_program(program, &[]);
    sm.set_config(&cfg);
    sm.set_enable(true);
    enable_irq::<SM>();
    sm
}

/// Counts since boot, written only by the interrupt handler
static LEFT_COUNT: AtomicI32 = AtomicI32::new(0);
static RIGHT_COUNT: AtomicI32 = AtomicI32::new(0);

/// Count change per pushed value `current << 2 | previous` (A/B in bits 0/1).
/// 0 = no change or invalid jump (both pins changed: a step was missed).
const STEP: [i8; 16] = [
    // previous: 00, 01, 10, 11
    0, -1, 1, 0, // current 00
    1, 0, 0, -1, // current 01
    -1, 0, 0, 1, // current 10
    0, 1, -1, 0, // current 11
];

/// Fires when a state machine pushed a value (RX FIFO not empty)
pub struct IrqHandler;

impl Handler<PIO0_IRQ_0> for IrqHandler {
    unsafe fn on_interrupt() {
        drain::<0>(&LEFT_COUNT);
        drain::<1>(&RIGHT_COUNT);
    }
}

/// Applies every value waiting in state machine `SM`'s RX FIFO to `count`.
fn drain<const SM: usize>(count: &AtomicI32) {
    let mut delta = 0i32;
    while pac::PIO0.fstat().read().rxempty() & (1 << SM) == 0 {
        let pushed = pac::PIO0.rxf(SM).read() & 0b1111;
        delta += STEP[pushed as usize] as i32;
    }
    count.store(
        count.load(Ordering::Relaxed).wrapping_add(delta),
        Ordering::Relaxed,
    );
    // embassy's PIO handler on the same interrupt disables the sources that fired
    enable_irq::<SM>();
}

/// Interrupt when state machine `SM` has data (`SMn_RXNEMPTY` is bit n)
fn enable_irq<const SM: usize>() {
    pac::PIO0.irqs(0).inte().modify(|w| w.0 |= 1 << SM);
}
