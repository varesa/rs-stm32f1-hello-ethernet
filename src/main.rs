
#![deny(unsafe_code)]   //  Don't allow unsafe code in this file.
//#![deny(warnings)]      //  If the Rust compiler generates a warning, stop the compilation with an error.
#![no_main]             //  Don't use the Rust standard bootstrap. We will provide our own.
#![no_std]              //  Don't use the Rust standard library. We are building a binary that can run on its own.

mod net;

use cortex_m::{iprint, iprintln};
use cortex_m_rt::{entry, exception, ExceptionFrame};    //  Stack frame for exception handling.
use cortex_m_semihosting::hprintln;                     //  For displaying messages on the debug console.
use panic_semihosting as _;

use embedded_hal::digital::v2::OutputPin;
use stm32f1xx_hal::{delay::Delay, pac, prelude::*, spi::Spi};
use enc28j60::{Enc28j60};
use jnet::{ether, mac, Buffer};
use heapless::consts::*;
use heapless::{FnvIndexMap};

/* Configuration */
//const MAC: mac::Addr = mac::Addr([0x20, 0x18, 0x72, 0x75, 0x73, 0x74]);
const MAC: mac::Addr = mac::Addr([0x48, 0x69, 0x52, 0x75, 0x73, 0x74]);
const LOG_ALL: bool = false;

/* Constants */
const KB: u16 = 1024; // bytes
const MTU: usize = 1518;

#[entry]
fn main() -> ! {
    hprintln!("Started (semihosting)!").unwrap();

    // Core peripherals
    let mut cp = cortex_m::Peripherals::take().unwrap();
    let _stim = &mut cp.ITM.stim[0];
    cp.DWT.enable_cycle_counter();
    
    iprintln!(_stim, "Started (ITM)!");

    let dp = pac::Peripherals::take().unwrap();
    let mut rcc = dp.RCC.constrain();
    let mut afio = dp.AFIO.constrain(&mut rcc.apb2);
    let mut flash = dp.FLASH.constrain();
    let mut gpioa = dp.GPIOA.split(&mut rcc.apb2);
    let mut gpioc = dp.GPIOC.split(&mut rcc.apb2);
    let clocks = rcc.cfgr.freeze(&mut flash.acr);

    let mut led = gpioa.pa0.into_push_pull_output(&mut gpioa.crl);

    // turn the LED off during initialization
    led.set_high().unwrap();

    iprintln!(_stim, "Configuring SPI");
    // SPI
    let mut ncs = gpioc.pc14.into_push_pull_output(&mut gpioc.crh);
    ncs.set_high().unwrap();
    let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
    let miso = gpioa.pa6;
    let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);
    let spi = Spi::spi1(
        dp.SPI1,
        (sck, miso, mosi),
        &mut afio.mapr,
        enc28j60::MODE,
        1.mhz(),
        clocks,
        &mut rcc.apb2,
    );

    iprintln!(_stim, "Configuring ENC28J60");
    // ENC28J60
    let mut reset = gpioc.pc13.into_push_pull_output(&mut gpioc.crh);
    let int = gpioc.pc15.into_floating_input(&mut gpioc.crh);
    reset.set_high().unwrap();
    let mut delay = Delay::new(cp.SYST, clocks);
    let mut enc28j60 = Enc28j60::new(
        spi,
        ncs,
        int,
        reset,
        &mut delay,
        7 * KB,
        MAC.0,
    )
    .ok()
    .unwrap();

    // LED on after initialization
    led.set_low().unwrap();

    // FIXME some frames are lost when sent right after initialization
    delay.delay_ms(100_u8);

    // ARP cache
    let mut cache = FnvIndexMap::<_, _, U8>::new();

    let mut buf = [0; MTU];
    loop {
        //iprintln!(_stim, "Receiving");
        let mut buf = Buffer::new(&mut buf);
        let len = enc28j60.receive(buf.as_mut()).expect("Error receiving from ENC28J60");
        buf.truncate(len);

        if let Ok(mut eth) = ether::Frame::parse(buf) {
            if LOG_ALL {
                iprint!(_stim, "\nRx({})", eth.as_bytes().len());
                iprintln!(_stim, " * {:?}", eth);
            }

            match eth.get_type() {
                ether::Type::Arp => {
                    if !LOG_ALL {
                        iprintln!(_stim, "* {:?}", eth);
                    }
                    net::handle_arp(_stim, &mut enc28j60, &mut eth, &mut cache)
                }
                ether::Type::Ipv4 => {
                    net::handle_ipv4(_stim, &mut enc28j60, &mut eth, &mut cache)
                }
                _ => {}
            }
        } else {
            // malformed Ethernet frame
            iprintln!(_stim, "Err(E)");
        }
    }
}



#[exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("Hard fault: {:#?}", ef);
}

#[exception]
fn DefaultHandler(irqn: i16) {
    panic!("Unhandled exception (IRQn = {})", irqn);
}

