#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::prelude::*;
use esp_hal::i2c::I2c;
use esp_hal::timer::systimer::SystemTimer;
use esp_println::{print, println};
use mlx9064x::{Mlx90640Driver, Mlx90641Driver};

extern crate alloc;
use alloc::vec;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("Panic: {:?}", info);
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    let peripherals = esp_hal::init({
        let mut config = esp_hal::Config::default();
        config.cpu_clock = esp_hal::clock::CpuClock::max();
        config
    });

    esp_alloc::heap_allocator!(64 * 1024);

    let timer0 = SystemTimer::new(peripherals.SYSTIMER);
    esp_hal_embassy::init(timer0.alarm0);

    // Initialize I2C on pins 6 (SDA) and 7 (SCL)
    let i2c = I2c::new(peripherals.I2C0, 100.kHz())
        .with_sda(peripherals.GPI21)
        .with_scl(peripherals.GPI22);

    println!("ESP32-C6 MLX9064X Test Starting...");
    
    // Try common I2C addresses
    let addresses = [0x33, 0x34];
    
    for &addr in &addresses {
        println!("Checking I2C address 0x{:02X}...", addr);
        
        // Try to create MLX90640 driver
        match Mlx90640Driver::new(i2c, addr) {
            Ok(mut camera) => {
                println!("Found MLX90640 at 0x{:02X}!", addr);
                run_camera_loop(&mut camera).await;
                return;
            }
            Err(returned_i2c) => {
                // Try MLX90641
                match Mlx90641Driver::new(returned_i2c, addr) {
                    Ok(mut camera) => {
                        println!("Found MLX90641 at 0x{:02X}!", addr);
                        run_camera_loop(&mut camera).await;
                        return;
                    }
                    Err(i2c_back) => {
                        i2c = i2c_back;
                    }
                }
            }
        }
    }
    
    println!("No MLX9064X sensor found!");
    println!("Check wiring: SDA=GPIO6, SCL=GPIO7, 3.3V power");
    
    loop {
        Timer::after(Duration::from_secs(1)).await;
    }
}

async fn run_camera_loop<I2C, Camera>(camera: &mut Camera) 
where
    I2C: embedded_hal::i2c::I2c,
    Camera: mlx9064x::MelexisCamera<I2C>,
{
    println!("Camera initialized!");
    println!("Resolution: {}x{}", camera.width(), camera.height());
    
    let mut temperatures = vec![0f32; camera.height() * camera.width()];
    let mut frame_count = 0;
    
    loop {
        // Check for new data and process it
        match camera.data_available() {
            Ok(Some(_subpage)) => {
                // Try to generate temperature image
                match camera.generate_image_if_ready(&mut temperatures) {
                    Ok(true) => {
                        frame_count += 1;
                        
                        // Display results
                        if let Some(ambient) = camera.ambient_temperature() {
                            println!("\nFrame {}: Ambient: {:.1}°C", frame_count, ambient);
                        }
                        
                        // Find min/max
                        let (min, max) = find_temp_range(&temperatures);
                        println!("Range: {:.1}°C to {:.1}°C", min, max);
                        
                        // Center pixel
                        let center = temperatures[temperatures.len() / 2];
                        println!("Center: {:.1}°C", center);
                        
                        // Simple visualization
                        print_simple_thermal(&temperatures, camera.width(), camera.height());
                    }
                    Ok(false) => {
                        // Frame not ready yet
                    }
                    Err(e) => {
                        println!("Error generating image: {:?}", e);
                    }
                }
            }
            Ok(None) => {
                // No new data
            }
            Err(e) => {
                println!("Error checking data: {:?}", e);
            }
        }
        
        Timer::after(Duration::from_millis(100)).await;
    }
}

fn find_temp_range(temps: &[f32]) -> (f32, f32) {
    let mut min = f32::MAX;
    let mut max = f32::MIN;
    
    for &t in temps {
        if t.is_finite() {
            min = min.min(t);
            max = max.max(t);
        }
    }
    
    (min, max)
}

fn print_simple_thermal(temps: &[f32], width: usize, height: usize) {
    let (min, max) = find_temp_range(temps);
    let range = max - min;
    
    if range == 0.0 {
        return;
    }
    
    println!("\nThermal Image:");
    
    // Sample every other pixel for compact display
    for y in (0..height).step_by(2) {
        for x in (0..width).step_by(2) {
            let temp = temps[y * width + x];
            let normalized = ((temp - min) / range).clamp(0.0, 1.0);
            
            // Simple character mapping
            let c = match (normalized * 9.0) as u8 {
                0 => ' ',
                1 => '.',
                2 => ':',
                3 => '-',
                4 => '=',
                5 => '+',
                6 => '*',
                7 => '#',
                _ => '@',
            };
            
            print!("{}", c);
        }
        println!();
    }
}