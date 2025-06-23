#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::i2c::master::{Config, I2c};
use esp_hal::timer::systimer::SystemTimer;
use esp_println::println;

extern crate alloc;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("Panic: {:?}", info);
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    esp_alloc::heap_allocator!(size: 64 * 1024);

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);

    // Try GPIO21/GPIO22 (your actual wiring)
    let mut i2c = I2c::new(peripherals.I2C0, Config::default())
        .unwrap()
        .with_sda(peripherals.GPIO21)
        .with_scl(peripherals.GPIO22);

    println!("ESP32-C6 I2C Scanner - GPIO21/GPIO22");
    println!("Scanning for devices...");

    loop {
        let mut found_any = false;
        
        for addr in 0x08..=0x77 {
            let mut buffer = [0u8; 1];
            match i2c.read(addr, &mut buffer) {
                Ok(_) => {
                    println!("Found device at 0x{:02X}", addr);
                    found_any = true;
                    
                    if addr == 0x33 || addr == 0x34 {
                        println!("  -> This could be MLX9064X!");
                        
                        // Try to read more info
                        let mut id_buf = [0u8; 3];
                        match i2c.write_read(addr, &[0x24, 0x0F], &mut id_buf) {
                            Ok(_) => {
                                println!("  -> ID: 0x{:02X} 0x{:02X} 0x{:02X}", 
                                    id_buf[0], id_buf[1], id_buf[2]);
                            }
                            Err(_) => {
                                println!("  -> Failed to read ID register");
                            }
                        }
                    }
                }
                Err(_) => {
                    // No device
                }
            }
        }
        
        if !found_any {
            println!("No devices found on GPIO21/GPIO22");
            println!("Check:");
            println!("- Pull-up resistors (4.7k) from SDA and SCL to 3.3V");
            println!("- Power: VIN connected to 3.3V");
            println!("- Ground connection");
            println!("- Try different GPIO pins if needed");
        }
        
        Timer::after(Duration::from_secs(3)).await;
    }
}