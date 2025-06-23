#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::i2c::master::{Config, I2c};
use esp_hal::timer::systimer::SystemTimer;
use esp_println::println;
use mlx9064x::Mlx90640Driver;

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

    // Create I2C with simple configuration
    let mut i2c = I2c::new(peripherals.I2C0, Config::default())
        .unwrap()
        .with_sda(peripherals.GPIO21)
        .with_scl(peripherals.GPIO22);

    println!("ESP32-C6 MLX90640 Test");
    println!("I2C pins: SDA=GPIO21, SCL=GPIO22");

    // Scan for I2C devices
    println!("Scanning I2C bus...");
    for addr in 0x03..=0x77 {
        let mut buffer = [0u8; 1];
        match i2c.write_read(addr, &[0x00], &mut buffer) {
            Ok(_) => {
                println!("Found device at address 0x{:02X}", addr);
            }
            Err(_) => {
                // No device at this address
            }
        }
    }

    // Try both common addresses for MLX90640
    let addresses = [0x33, 0x34];
    let mut found = false;
    
    for &addr in &addresses {
        println!("\nTrying MLX90640 at address 0x{:02X}...", addr);
        
        // Try to read ID register
        let mut buffer = [0u8; 2];
        match i2c.write_read(addr, &[0x24, 0x0F], &mut buffer) {
            Ok(_) => {
                println!("Device responded! ID bytes: 0x{:02X} 0x{:02X}", buffer[0], buffer[1]);
                
                // Try to create driver
                match Mlx90640Driver::new(i2c, addr) {
        Ok(mut camera) => {
            println!("MLX90640 initialized!");
            
            let mut temps = alloc::vec![0f32; 24 * 32];
            
            loop {
                match camera.generate_image_if_ready(&mut temps) {
                    Ok(true) => {
                        println!("Got new frame!");
                        
                        // Find average temperature
                        let sum: f32 = temps.iter().sum();
                        let avg = sum / temps.len() as f32;
                        println!("Average temperature: {:.1}°C", avg);
                    }
                    Ok(false) => {
                        // No new frame
                    }
                    Err(e) => {
                        println!("Error: {:?}", e);
                    }
                }
                
                Timer::after(Duration::from_millis(200)).await;
            }
        }
        Err(_) => {
            println!("Failed to initialize MLX90640");
        }
    }

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}