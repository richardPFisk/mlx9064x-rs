#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::i2c::master::{Config, I2c};
use esp_hal::timer::systimer::SystemTimer;
use esp_println::println;
use mlx9064x::{Mlx90640Driver, Mlx90641Driver};

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

    // Create I2C
    let mut i2c = I2c::new(peripherals.I2C0, Config::default())
        .unwrap()
        .with_sda(peripherals.GPIO4)
        .with_scl(peripherals.GPIO5);

    println!("ESP32-C6 MLX9064X I2C Scanner");
    println!("I2C pins: SDA=GPIO4, SCL=GPIO5");
    println!("Make sure you have pull-up resistors (4.7k) on both lines!");

    loop {
        println!("\n--- I2C Bus Scan ---");
        let mut found_count = 0;
        
        for addr in 0x08..=0x77 {
            // Try a simple read
            let mut buffer = [0u8; 1];
            match i2c.read(addr, &mut buffer) {
                Ok(_) => {
                    println!("Found device at address 0x{:02X}", addr);
                    found_count += 1;
                    
                    // If it's a potential MLX address, try to read more info
                    if addr == 0x33 || addr == 0x34 {
                        println!("  Potential MLX9064X device!");
                        
                        // Try to read ID register
                        let mut id_buffer = [0u8; 3];
                        match i2c.write_read(addr, &[0x24, 0x0F], &mut id_buffer) {
                            Ok(_) => {
                                println!("  ID register: 0x{:02X} 0x{:02X} 0x{:02X}", 
                                    id_buffer[0], id_buffer[1], id_buffer[2]);
                            }
                            Err(_) => {
                                println!("  Failed to read ID register");
                            }
                        }
                    }
                }
                Err(_) => {
                    // No device at this address
                }
            }
        }
        
        if found_count == 0 {
            println!("No I2C devices found!");
            println!("Check:");
            println!("- Power (3.3V) to sensor");
            println!("- Ground connection");
            println!("- SDA wire to GPIO21");
            println!("- SCL wire to GPIO22");
            println!("- Pull-up resistors (4.7k) from SDA and SCL to 3.3V");
        } else {
            println!("\nFound {} device(s)", found_count);
        }
        
        Timer::after(Duration::from_secs(3)).await;
    }
}