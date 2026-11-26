//! 128x64 SH1106 OLED display of the Pololu 3pi+ 2040.
//!
//! Wiring (see pololu-3pi-2040-robot/c/pololu_3pi_2040_robot/sh1106.c):
//!   GP0 = D/C, GP1 = RESET, GP2 = SCK (SPI0), GP3 = MOSI (SPI0); CS is not connected.
//!
//! [`Display`] dereferences to an `oled_async` [`GraphicsMode`], so it can be used
//! directly as an `embedded-graphics` draw target. Call [`GraphicsMode::flush`] to
//! send the frame buffer to the display.

use core::convert::Infallible;
use core::ops::{Deref, DerefMut};

use display_interface::DisplayError;
use display_interface_spi::SPIInterface;
use embassy_rp::Peri;
use embassy_rp::dma::{self, ChannelInstance};
use embassy_rp::gpio::{Level, Output};
use embassy_rp::interrupt::typelevel::Binding;
use embassy_rp::peripherals::{DMA_CH0, PIN_0, PIN_1, PIN_2, PIN_3, SPI0};
use embassy_rp::spi::{self, Async, Spi};
use embassy_time::{Delay, Duration, Timer};
use embedded_hal_bus::spi::ExclusiveDevice;
use oled_async::{Builder, displays::sh1106::Sh1106_128_64, prelude::*};

const WIDTH: usize = 128;
const HEIGHT: usize = 64;

type Interface =
    SPIInterface<ExclusiveDevice<Spi<'static, SPI0, Async>, NoCs, Delay>, Output<'static>>;

/// The `oled_async` driver type for the 3pi+ 2040 display.
pub type Oled = GraphicsMode<Sh1106_128_64, Interface, { WIDTH * HEIGHT / 8 }>;

/// The display driver together with the pins it needs to keep driven.
pub struct Display {
    oled: Oled,
    // RESET must stay driven high: dropping this `Output` releases the pin and
    // resets the display. Keep the `Display` alive as long as it is used.
    _rst: Output<'static>,
}

impl Display {
    /// Reset and initialize the display. The frame buffer starts out cleared.
    pub async fn new(
        spi: Peri<'static, SPI0>,
        sck: Peri<'static, PIN_2>,
        mosi: Peri<'static, PIN_3>,
        dc: Peri<'static, PIN_0>,
        rst: Peri<'static, PIN_1>,
        tx_dma: Peri<'static, DMA_CH0>,
        irq: impl Binding<<DMA_CH0 as ChannelInstance>::Interrupt, dma::InterruptHandler<DMA_CH0>>
        + 'static,
    ) -> Result<Self, DisplayError> {
        let mut config = spi::Config::default();
        config.frequency = 20_000_000;
        let spi = Spi::new_txonly(spi, sck, mosi, tx_dma, irq, config);
        let spi = ExclusiveDevice::new(spi, NoCs, Delay).unwrap();
        let dc = Output::new(dc, Level::Low);
        let mut rst = Output::new(rst, Level::High);

        // Default rotation (Rotate0) uses segment remap + reversed COM scan, matching
        // sh1106_configure_default() in the Pololu C library. Sh1106_128_64 applies
        // the 2-column offset of the SH1106's 132-column RAM.
        let mut oled: Oled = Builder::new(Sh1106_128_64 {})
            .connect(SPIInterface::new(spi, dc))
            .into();

        // Give the OLED time to power up, then pulse reset (like sh1106_init() in the C library).
        Timer::after(Duration::from_millis(20)).await;
        rst.set_low();
        Timer::after(Duration::from_micros(10)).await;
        rst.set_high();
        Timer::after(Duration::from_micros(10)).await;

        oled.init().await?;
        oled.set_contrast(0xFF).await?;
        oled.clear();
        oled.flush().await?;

        Ok(Self { oled, _rst: rst })
    }
}

impl Deref for Display {
    type Target = Oled;

    fn deref(&self) -> &Oled {
        &self.oled
    }
}

impl DerefMut for Display {
    fn deref_mut(&mut self) -> &mut Oled {
        &mut self.oled
    }
}

/// The SH1106's chip select is not connected (always selected), but
/// `ExclusiveDevice` needs a CS pin, so use a no-op one.
pub struct NoCs;

impl embedded_hal::digital::ErrorType for NoCs {
    type Error = Infallible;
}

impl embedded_hal::digital::OutputPin for NoCs {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
