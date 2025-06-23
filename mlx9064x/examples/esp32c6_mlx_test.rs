#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::i2c::master::{Config, I2c};
use esp_hal::timer::systimer::SystemTimer;
use esp_println::{print, println};
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

    // Create I2C on GPIO21/GPIO22
    let i2c = I2c::new(peripherals.I2C0, Config::default())
        .unwrap()
        .with_sda(peripherals.GPIO21)
        .with_scl(peripherals.GPIO22);

    println!("ESP32-C6 MLX90640 Test");
    println!("I2C pins: SDA=GPIO21, SCL=GPIO22");
    
    // Try to create MLX90640 driver at 0x33
    match Mlx90640Driver::new(i2c, 0x33) {
        Ok(mut camera) => {
            println!("MLX90640 initialized successfully!");
            println!("Resolution: 24x32 pixels");
            
            let mut temps = alloc::vec![0f32; 24 * 32];
            let mut frame_count = 0;
            
            loop {
                match camera.generate_image_if_ready(&mut temps) {
                    Ok(true) => {
                        frame_count += 1;
                        
                        // Calculate statistics
                        let sum: f32 = temps.iter().sum();
                        let avg = sum / temps.len() as f32;
                        
                        let min = temps.iter().fold(f32::INFINITY, |a, &b| a.min(b));
                        let max = temps.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
                        
                        println!("\nFrame {}: ", frame_count);
                        println!("  Average: {:.1}°C", avg);
                        println!("  Min: {:.1}°C, Max: {:.1}°C", min, max);
                        
                        // Show center pixel
                        let center = temps[12 * 24 + 16]; // Approximate center
                        println!("  Center pixel: {:.1}°C", center);
                        
                        // Display thermal image
                        print_thermal_image(&temps, 32, 24, min, max);
                    }
                    Ok(false) => {
                        // No new frame ready yet
                    }
                    Err(e) => {
                        println!("Error reading frame: {:?}", e);
                    }
                }
                
                Timer::after(Duration::from_millis(100)).await;
            }
        }
        Err(_) => {
            println!("Failed to initialize MLX90640 at address 0x33");
            println!("The device was detected in the scan, so this might be:");
            println!("- I2C speed issue (try slower speeds)");
            println!("- Power issue (sensor needs stable 3.3V)");
            println!("- Timing issue (sensor needs time after power-on)");
        }
    }

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}

fn print_thermal_image(temps: &[f32], width: usize, height: usize, min: f32, max: f32) {
    let range = max - min;
    
    // ASCII characters from cold to hot
    let chars = [' ', '.', ':', '-', '=', '+', '*', '#', '@'];
    
    println!("\n╔{}╗", "═".repeat(width));
    
    for y in 0..height {
        print!("║");
        for x in 0..width {
            let idx = y * width + x;
            let temp = temps[idx];
            
            // Normalize temperature to 0-1 range
            let normalized = if range > 0.0 {
                ((temp - min) / range).clamp(0.0, 1.0)
            } else {
                0.5
            };
            
            // Map to character index
            let char_idx = (normalized * (chars.len() - 1) as f32) as usize;
            print!("{}", chars[char_idx]);
        }
        println!("║");
    }
    
    println!("╚{}╝", "═".repeat(width));
    
    // Print legend
    println!("Scale: {} = {:.1}°C ... {} = {:.1}°C", 
        chars[0], min, chars[chars.len()-1], max);
}