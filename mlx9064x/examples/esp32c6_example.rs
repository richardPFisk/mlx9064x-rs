// SPDX-License-Identifier: Apache-2.0
// ESP32-C6 MLX90640 Thermal Camera Example
// 
// This example demonstrates how to use the mlx9064x library with ESP32-C6
// to read thermal data from an MLX90640 sensor and display it as ASCII art.
//
// Hardware Requirements:
// - ESP32-C6 development board
// - MLX90640 thermal camera breakout (e.g., Adafruit #4407)
// - 4.7kΩ pull-up resistors for I2C lines
//
// Wiring:
// MLX90640 → ESP32-C6
// VIN     → 3.3V
// GND     → GND  
// SDA     → GPIO01 (with 4.7kΩ pull-up to 3.3V)
// SCL     → GPIO00 (with 4.7kΩ pull-up to 3.3V)

#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::i2c::master::{Config, I2c};
use esp_hal::timer::systimer::SystemTimer;
use esp_println::{print, println};
use mlx9064x::Mlx90640DriverAsync;

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

    // Initialize I2C on GPIO01 (SDA) and GPIO00 (SCL) in async mode
    let i2c = I2c::new(peripherals.I2C0, Config::default())
        .unwrap()
        .with_sda(peripherals.GPIO1)
        .with_scl(peripherals.GPIO0)
        .into_async();

    println!("ESP32-C6 MLX90640 Thermal Camera Example");
    println!("I2C: SDA=GPIO01, SCL=GPIO00");
    
    // Initialize MLX90640 thermal camera
    match Mlx90640DriverAsync::new(i2c, 0x33).await {
        Ok(mut camera) => {
            println!("MLX90640 initialized successfully!");
            println!("Resolution: 24x32 pixels");
            
            // Allocate buffer for temperature data (24 rows × 32 columns)
            let mut temperatures = alloc::vec![0f32; 24 * 32];
            let mut frame_count = 0;
            
            // Main sensor reading loop
            loop {
                match camera.generate_image_if_ready(&mut temperatures).await {
                    Ok(true) => {
                        frame_count += 1;
                        
                        // Calculate temperature statistics
                        let sum: f32 = temperatures.iter().sum();
                        let average = sum / temperatures.len() as f32;
                        let min_temp = temperatures.iter().fold(f32::INFINITY, |a, &b| a.min(b));
                        let max_temp = temperatures.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
                        
                        // Display frame information
                        println!("\n=== Frame {} ===", frame_count);
                        println!("Average: {:.1}°C", average);
                        println!("Range: {:.1}°C to {:.1}°C", min_temp, max_temp);
                        
                        // Show center pixel temperature
                        let center_pixel = temperatures[12 * 32 + 16]; // Approximate center
                        println!("Center: {:.1}°C", center_pixel);
                        
                        // Display thermal image as ASCII art
                        print_thermal_image(&temperatures, 32, 24, min_temp, max_temp);
                    }
                    Ok(false) => {
                        // No new frame available yet, continue polling
                    }
                    Err(e) => {
                        println!("Error reading thermal data: {:?}", e);
                    }
                }
                
                // Wait 100ms between readings
                Timer::after(Duration::from_millis(100)).await;
            }
        }
        Err(_) => {
            println!("Failed to initialize MLX90640!");
            println!("Check:");
            println!("- Wiring connections");
            println!("- Pull-up resistors (4.7kΩ on SDA and SCL)");
            println!("- Power supply (3.3V to VIN)");
            println!("- I2C address (should be 0x33)");
        }
    }

    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}

/// Display thermal image as ASCII art with border and legend
fn print_thermal_image(temps: &[f32], width: usize, height: usize, min_temp: f32, max_temp: f32) {
    let temp_range = max_temp - min_temp;
    
    // ASCII characters representing temperature levels (cold to hot)
    let thermal_chars = [' ', '.', ':', '-', '=', '+', '*', '#', '@'];
    
    // Print top border
    println!("\n╔{}╗", "═".repeat(width));
    
    // Print thermal data row by row
    for y in 0..height {
        print!("║");
        for x in 0..width {
            let pixel_index = y * width + x;
            let temperature = temps[pixel_index];
            
            // Normalize temperature to 0.0-1.0 range
            let normalized = if temp_range > 0.0 {
                ((temperature - min_temp) / temp_range).clamp(0.0, 1.0)
            } else {
                0.5 // Use middle character if no temperature variation
            };
            
            // Map normalized value to character index
            let char_index = (normalized * (thermal_chars.len() - 1) as f32) as usize;
            print!("{}", thermal_chars[char_index]);
        }
        println!("║");
    }
    
    // Print bottom border
    println!("╚{}╝", "═".repeat(width));
    
    // Print temperature scale legend
    println!("Scale: '{}' = {:.1}°C ... '{}' = {:.1}°C", 
        thermal_chars[0], min_temp, 
        thermal_chars[thermal_chars.len()-1], max_temp);
}