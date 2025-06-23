#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::i2c::master::{Config, I2c};
use esp_hal::timer::systimer::SystemTimer;
use esp_hal::gpio::{GpioPin, Output, PushPull};
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

    println!("ESP32-C6 Multi-Pin I2C Scanner");
    println!("Testing different pin combinations...\n");

    // Test pin combination 1: GPIO4/GPIO5
    println!("Testing GPIO4 (SDA) / GPIO5 (SCL)...");
    {
        let i2c = I2c::new(peripherals.I2C0, Config::default())
            .unwrap()
            .with_sda(peripherals.GPIO4)
            .with_scl(peripherals.GPIO5);
        
        scan_i2c_bus(i2c, "GPIO4/GPIO5").await;
    }
    
    Timer::after(Duration::from_millis(500)).await;

    // Test pin combination 2: GPIO2/GPIO3
    println!("\nTesting GPIO2 (SDA) / GPIO3 (SCL)...");
    {
        let i2c = I2c::new(peripherals.I2C0, Config::default())
            .unwrap()
            .with_sda(peripherals.GPIO2)
            .with_scl(peripherals.GPIO3);
        
        scan_i2c_bus(i2c, "GPIO2/GPIO3").await;
    }
    
    Timer::after(Duration::from_millis(500)).await;

    // Test pin combination 3: GPIO18/GPIO19
    println!("\nTesting GPIO18 (SDA) / GPIO19 (SCL)...");
    {
        let i2c = I2c::new(peripherals.I2C0, Config::default())
            .unwrap()
            .with_sda(peripherals.GPIO18)
            .with_scl(peripherals.GPIO19);
        
        scan_i2c_bus(i2c, "GPIO18/GPIO19").await;
    }
    
    Timer::after(Duration::from_millis(500)).await;

    // Test pin combination 4: GPIO14/GPIO15
    println!("\nTesting GPIO14 (SDA) / GPIO15 (SCL)...");
    {
        let i2c = I2c::new(peripherals.I2C0, Config::default())
            .unwrap()
            .with_sda(peripherals.GPIO14)
            .with_scl(peripherals.GPIO15);
        
        scan_i2c_bus(i2c, "GPIO14/GPIO15").await;
    }

    println!("\n=== Scan Complete ===");
    println!("If no devices were found on any pins:");
    println!("1. Check power connections (3.3V and GND)");
    println!("2. Verify pull-up resistors (4.7k) on SDA and SCL");
    println!("3. Try different pull-up values (2.2k or 10k)");
    println!("4. Check if module needs 5V instead of 3.3V");

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}

async fn scan_i2c_bus<I2C>(mut i2c: I2C, pin_name: &str) 
where
    I2C: embedded_hal::i2c::I2c,
{
    let mut found_count = 0;
    
    for addr in 0x08..=0x77 {
        let mut buffer = [0u8; 1];
        match i2c.read(addr, &mut buffer) {
            Ok(_) => {
                println!("  [{}] Found device at 0x{:02X}", pin_name, addr);
                found_count += 1;
                
                // Check if it might be MLX9064X
                if addr == 0x33 || addr == 0x34 {
                    println!("    -> Potential MLX9064X device!");
                }
            }
            Err(_) => {
                // No device at this address
            }
        }
    }
    
    if found_count == 0 {
        println!("  [{}] No devices found", pin_name);
    } else {
        println!("  [{}] Total: {} device(s)", pin_name, found_count);
    }
}